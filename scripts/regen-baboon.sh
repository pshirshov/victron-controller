#!/usr/bin/env bash
# Regenerate the baboon-derived dashboard-model crate.
#
# Runs the baboon compiler against ./models/ and writes Rust + TypeScript
# outputs. Post-processes the generated Rust to fix two upstream codegen
# issues:
#   1. The generated Cargo.toml's package name ("baboon-generated") is
#      renamed to "victron-controller-dashboard-model" with workspace
#      version/edition/etc.
#   2. Ord derives on data types containing `opt[f64]` emit
#      `self.value.total_cmp(&other.value)` — `Option<f64>` has no
#      `total_cmp`. We rewrite those two sites to compare via a helper.
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

RS_OUT="crates/dashboard-model"
TS_OUT="web/src/model"

rm -rf "$RS_OUT/src"
mkdir -p "$RS_OUT/src" "$TS_OUT"

# --- Rust -----------------------------------------------------------------
baboon \
  --model-dir models \
  :rust \
    --output "$RS_OUT/src" \
    --generate-ueba-codecs=true \
    --generate-ueba-codecs-by-default=true \
    --omit-most-recent-version-suffix-from-paths \
    --omit-most-recent-version-suffix-from-namespaces

# Replace the package block in the generated Cargo.toml with our own.
mv "$RS_OUT/src/Cargo.toml" "$RS_OUT/Cargo.toml.generated"
cat > "$RS_OUT/Cargo.toml" <<'TOML'
[package]
name = "victron-controller-dashboard-model"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "Generated baboon wire-format types for the dashboard API"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = { version = "1", optional = true }
rust_decimal = { version = "1", features = ["serde-with-str"], optional = true }
chrono = { version = "0.4", default-features = false, features = ["std", "serde"], optional = true }
uuid = { version = "1", features = ["v4", "serde"], optional = true }

[features]
default = ["json-helpers"]
decimal = ["dep:rust_decimal"]
json-helpers = ["dep:serde_json"]
timestamps = ["dep:chrono"]
uuids = ["dep:uuid"]
TOML
rm "$RS_OUT/Cargo.toml.generated"

# Fatten the allow-list at the crate root. The generated code uses
# many patterns our workspace-level pedantic clippy dislikes (manual
# Default impls, wildcard imports, derivable_impls, etc.); silence
# them rather than drift the codegen.
LIB_RS="$RS_OUT/src/lib.rs"
{
  echo "#![allow(warnings)]"
  echo "#![allow(clippy::all)]"
  echo "#![allow(clippy::pedantic)]"
  echo "#![allow(clippy::nursery)]"
  cat "$LIB_RS"
} > "$LIB_RS.new"
mv "$LIB_RS.new" "$LIB_RS"

# Inject an Option<f64>::total_cmp-compatible helper into the runtime
# and rewrite the two offending .total_cmp() call sites that baboon
# mis-generates for fields of type Option<f64>.
cat >> "$RS_OUT/src/baboon_runtime.rs" <<'RS'

// --- patched by scripts/regen-baboon.sh -----------------------------------
// baboon's Ord derive on types containing `opt[f64]` emits
// `self.value.total_cmp(&other.value)`, but Option<f64> has no
// total_cmp. Provide a helper used by the rewritten call sites below.
pub fn __opt_f64_total_cmp(a: &Option<f64>, b: &Option<f64>) -> std::cmp::Ordering {
    match (a, b) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (Some(_), None) => std::cmp::Ordering::Greater,
        (Some(x), Some(y)) => x.total_cmp(y),
    }
}

// PR-soc-chart-solar: same upstream codegen bug for `lst[f64]` fields.
// Walk pairwise via `f64::total_cmp`; shorter slice is "less" when all
// shared elements compare equal.
pub fn __vec_f64_total_cmp(a: &[f64], b: &[f64]) -> std::cmp::Ordering {
    let n = a.len().min(b.len());
    for i in 0..n {
        match a[i].total_cmp(&b[i]) {
            std::cmp::Ordering::Equal => {}
            ord => return ord,
        }
    }
    a.len().cmp(&b.len())
}
RS

# Targeted rewrites: known field names that are Option<f64>. Apply to
# every version's generated file (latest at the unsuffixed path; older
# versions live under v0_X_Y/ subdirs).
# If new opt[f64] fields are added, extend this list.
find "$RS_OUT/src" -path '*/dashboard*/actual_f64.rs' -print0 | while IFS= read -r -d '' f; do
  sed -i \
    -e 's|self\.value\.total_cmp(&other\.value)|crate::baboon_runtime::__opt_f64_total_cmp(\&self.value, \&other.value)|' \
    "$f"
done
find "$RS_OUT/src" -path '*/dashboard*/actuated_f64.rs' -print0 | while IFS= read -r -d '' f; do
  sed -i \
    -e 's|self\.target_value\.total_cmp(&other\.target_value)|crate::baboon_runtime::__opt_f64_total_cmp(\&self.target_value, \&other.target_value)|' \
    "$f"
done

# PR-soc-chart-solar: ForecastSnapshot.hourly_kwh is `lst[f64]` →
# Vec<f64>, which also has no `total_cmp`. Patch only the v0.2.0+
# generated file (0.1.0 didn't have the field). Same patch covers the
# 0.3.0 addition `hourly_temperature_c` (also `lst[f64]`).
find "$RS_OUT/src" -path '*/dashboard*/forecast_snapshot.rs' -print0 | while IFS= read -r -d '' f; do
  sed -i \
    -e 's|self\.hourly_kwh\.total_cmp(&other\.hourly_kwh)|crate::baboon_runtime::__vec_f64_total_cmp(\&self.hourly_kwh, \&other.hourly_kwh)|' \
    -e 's|self\.hourly_temperature_c\.total_cmp(&other\.hourly_temperature_c)|crate::baboon_runtime::__vec_f64_total_cmp(\&self.hourly_temperature_c, \&other.hourly_temperature_c)|' \
    "$f"
done

# PR-soc-chart / PR-soc-chart-segments add Option<f64> fields on
# SocProjection and SocChart. Same upstream codegen bug as
# actual_f64/actuated_f64. Older 0.2.0 had slope_pct_per_hour /
# terminus_soc_pct on SocProjection — now replaced by `segments` plus
# charge_rate_w. The list below covers every Option<f64> field present
# in any version still in models/.
for field in slope_pct_per_hour terminus_soc_pct net_power_w capacity_wh charge_rate_w; do
  find "$RS_OUT/src" -path '*/dashboard*/soc_projection.rs' -print0 | while IFS= read -r -d '' f; do
    sed -i \
      -e "s|self\.${field}\.total_cmp(&other\.${field})|crate::baboon_runtime::__opt_f64_total_cmp(\&self.${field}, \&other.${field})|" \
      "$f"
  done
done
for field in now_soc_pct discharge_target_pct charge_target_pct export_threshold_pct; do
  find "$RS_OUT/src" -path '*/dashboard*/soc_chart.rs' -print0 | while IFS= read -r -d '' f; do
    sed -i \
      -e "s|self\.${field}\.total_cmp(&other\.${field})|crate::baboon_runtime::__opt_f64_total_cmp(\&self.${field}, \&other.${field})|" \
      "$f"
  done
done

# --- TypeScript -----------------------------------------------------------
baboon \
  --model-dir models \
  :typescript \
    --output "$TS_OUT" \
    --generate-ueba-codecs=true \
    --generate-ueba-codecs-by-default=true \
    --omit-most-recent-version-suffix-from-paths \
    --omit-most-recent-version-suffix-from-namespaces \
    --ts-maps-as-records \
    --ts-timestamps-as-strings

# Inject @ts-nocheck atop every generated TS file so the strict
# tsconfig doesn't complain about unused locals the codegen emits in
# BaboonSharedRuntime's time-of-day readers.
find "$TS_OUT" -name '*.ts' -print0 | while IFS= read -r -d '' f; do
  if ! head -1 "$f" | grep -q '@ts-nocheck'; then
    tmp=$(mktemp)
    { echo "// @ts-nocheck"; cat "$f"; } > "$tmp"
    mv "$tmp" "$f"
  fi
done

echo "regenerated:"
echo "  $RS_OUT/  (Rust)"
echo "  $TS_OUT/  (TypeScript, prefixed with @ts-nocheck)"
