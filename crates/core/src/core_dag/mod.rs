//! Core DAG infrastructure (PR-DAG-A).
//!
//! Each actuator controller is wrapped as a `Core` impl. The
//! `CoreRegistry` validates the dependency graph at construction and
//! executes cores in a deterministic topological order.
//!
//! PR-DAG-A is intentionally zero-behavior-change: the `depends_on`
//! edges below reproduce the hand-rolled execution order that
//! `run_controllers` had before this refactor. PR-DAG-C replaces them
//! with semantic edges derived from the bookkeeping-write/read audit.

use std::collections::{BTreeMap, HashMap};

use crate::Clock;
use crate::topology::Topology;
use crate::types::Effect;
use crate::world::World;

pub mod cores;

#[cfg(test)]
mod tests;

/// Identity of every core known to the registry. The discriminant
/// order is also the deterministic tie-break in Kahn's algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CoreId {
    // Derivation cores (reserved for PR-DAG-B; not yet used in -A).
    ZappiActive,

    // Actuator cores — one per `run_*` in process.rs.
    Setpoint,
    CurrentLimit,
    Schedules,
    ZappiMode,
    EddiMode,
    WeatherSoc,
}

impl CoreId {
    /// Stable `snake_case` name used in the dashboard wire format and
    /// the description registry. PR-tass-dag-view.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ZappiActive => "zappi_active",
            Self::Setpoint => "setpoint",
            Self::CurrentLimit => "current_limit",
            Self::Schedules => "schedules",
            Self::ZappiMode => "zappi_mode",
            Self::EddiMode => "eddi_mode",
            Self::WeatherSoc => "weather_soc",
        }
    }
}

/// A single unit of orchestrated work. One impl per `run_*` today.
///
/// PR-DAG-B: cross-core derivations live in `World::derived` (see
/// [`crate::world::DerivedState`]); each derivation is written by a
/// dedicated `Core` that runs ahead of its consumers via `depends_on`.
pub trait Core: Send + Sync {
    fn id(&self) -> CoreId;

    /// Cores whose execution must precede this one. `CoreRegistry`
    /// validates that every id here exists in the registry.
    fn depends_on(&self) -> &'static [CoreId];

    fn run(
        &self,
        world: &mut World,
        clock: &dyn Clock,
        topology: &Topology,
        effects: &mut Vec<Effect>,
    );

    /// Optional payload to surface in the dashboard's TASS DAG view
    /// (`world.cores_state`). Default `None` for actuator cores whose
    /// effect is on `Decisions`/`Actuated` rather than a single value.
    /// Derivation cores override this to stringify their derived value
    /// (e.g. `ZappiActiveCore` returns `"true"`/`"false"`).
    /// PR-tass-dag-view.
    fn last_payload(&self, _world: &World) -> Option<String> {
        None
    }
}

/// Errors that can arise while validating a set of cores into a DAG.
#[derive(Debug)]
pub enum CoreGraphError {
    MissingDependency { from: CoreId, missing: CoreId },
    Cycle { involving: Vec<CoreId> },
    DuplicateCore(CoreId),
}

/// Validated set of cores with a precomputed execution order.
pub struct CoreRegistry {
    cores: Vec<Box<dyn Core>>,
    order: Vec<usize>,
}

impl std::fmt::Debug for CoreRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ids: Vec<CoreId> = self.order.iter().map(|&i| self.cores[i].id()).collect();
        f.debug_struct("CoreRegistry").field("order", &ids).finish()
    }
}

impl CoreRegistry {
    /// Validate the supplied cores and compute a deterministic
    /// topological order.
    ///
    /// - Rejects duplicate `CoreId`s.
    /// - Rejects edges pointing at non-existent cores.
    /// - Rejects cycles.
    /// - Tie-breaks by `CoreId` discriminant order for determinism.
    pub fn build(cores: Vec<Box<dyn Core>>) -> Result<Self, CoreGraphError> {
        // 1. Index cores by id, rejecting duplicates.
        let mut id_to_idx: HashMap<CoreId, usize> = HashMap::with_capacity(cores.len());
        for (idx, c) in cores.iter().enumerate() {
            let id = c.id();
            if id_to_idx.insert(id, idx).is_some() {
                return Err(CoreGraphError::DuplicateCore(id));
            }
        }

        // 2. Validate every declared dependency resolves to a known core.
        for c in &cores {
            for &dep in c.depends_on() {
                if !id_to_idx.contains_key(&dep) {
                    return Err(CoreGraphError::MissingDependency {
                        from: c.id(),
                        missing: dep,
                    });
                }
            }
        }

        // 3. Build adjacency + in-degrees. Edge dep -> c.
        let n = cores.len();
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut in_degree: Vec<usize> = vec![0; n];
        for (idx, c) in cores.iter().enumerate() {
            for &dep in c.depends_on() {
                let dep_idx = id_to_idx[&dep];
                adj[dep_idx].push(idx);
                in_degree[idx] += 1;
            }
        }

        // 4. Kahn's with deterministic tie-break via a BTreeMap ordered
        // by CoreId (which is itself Ord by discriminant).
        let mut ready: BTreeMap<CoreId, usize> = BTreeMap::new();
        for (idx, c) in cores.iter().enumerate() {
            if in_degree[idx] == 0 {
                ready.insert(c.id(), idx);
            }
        }

        let mut order: Vec<usize> = Vec::with_capacity(n);
        while let Some((_, idx)) = ready.iter().next().map(|(k, v)| (*k, *v)) {
            ready.remove(&cores[idx].id());
            order.push(idx);
            for &nxt in &adj[idx] {
                in_degree[nxt] -= 1;
                if in_degree[nxt] == 0 {
                    ready.insert(cores[nxt].id(), nxt);
                }
            }
        }

        if order.len() != n {
            // Collect the ids still carrying a non-zero in-degree —
            // these are the cores involved in (or downstream of) the
            // cycle.
            let involving: Vec<CoreId> = (0..n)
                .filter(|i| in_degree[*i] > 0)
                .map(|i| cores[i].id())
                .collect();
            return Err(CoreGraphError::Cycle { involving });
        }

        Ok(Self { cores, order })
    }

    /// Execute every core in topological order.
    ///
    /// PR-DAG-B: derivation cores run ahead of their consumers (enforced
    /// by `depends_on` edges validated in `build`) and write their
    /// outputs into `world.derived`. `classify_zappi_active` is invoked
    /// exactly once per tick by `ZappiActiveCore` — closing the A-05
    /// hazard the previous `DerivedView` plumbing addressed
    /// (PR-DAG-A-D01).
    pub fn run_all(
        &self,
        world: &mut World,
        clock: &dyn Clock,
        topology: &Topology,
        effects: &mut Vec<Effect>,
    ) {
        // Reset the per-tick observability snapshot. We rebuild it
        // entry-by-entry as cores run; topo_order is fixed by the
        // registry so we can lock it in up-front. PR-tass-dag-view.
        world.cores_state.cores.clear();
        world.cores_state.cores.reserve(self.order.len());
        if world.cores_state.topo_order.len() != self.order.len() {
            world.cores_state.topo_order =
                self.order.iter().map(|&i| self.cores[i].id().name().to_string()).collect();
        }
        for &idx in &self.order {
            let core = &self.cores[idx];
            core.run(world, clock, topology, effects);
            // "ran without panicking" == success today; see CoresState
            // doc-comment in `crate::world` for why this placeholder is
            // honest given current core semantics.
            let entry = crate::world::CoreState {
                id: core.id().name().to_string(),
                depends_on: core.depends_on().iter().map(|d| d.name().to_string()).collect(),
                last_run_outcome: "success".to_string(),
                last_payload: core.last_payload(world),
            };
            world.cores_state.cores.push(entry);
        }
    }

    /// Topological order as `CoreId`s, for test introspection.
    #[cfg(test)]
    pub fn order(&self) -> Vec<CoreId> {
        self.order.iter().map(|&i| self.cores[i].id()).collect()
    }
}

