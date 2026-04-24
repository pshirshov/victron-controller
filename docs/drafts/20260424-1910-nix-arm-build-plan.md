# Nix ARMv7 build plan for victron-controller

Date: 2026-04-24
Status: draft — plan only, no implementation

## Goal

Produce `nix build .#victron-controller-armv7` that yields an ARMv7
(`armv7-unknown-linux-gnueabihf`) release binary for the Victron Venus GX,
dispatchable to fast remote Nix builders via the standard
`builders = ssh://...` mechanism.

## 1. Audit of the current `flake.nix`

Currently the flake provides only `devShells.default`. It already does
the load-bearing work that a build derivation will reuse verbatim:

- Pins a fenix Rust toolchain that includes
  `targets.armv7-unknown-linux-gnueabihf.stable.rust-std`.
- Uses `pkgs.pkgsCross.armv7l-hf-multiplatform.stdenv.cc` as the cross linker.
- Sets the full matrix of env vars cargo needs for cross-linking and
  cc-rs-driven native build scripts (ring in particular):
  `CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER`,
  `CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_RUSTFLAGS`
  (`-C target-feature=+v7,+vfp3,+neon`),
  `CC_armv7_unknown_linux_gnueabihf`,
  `AR_armv7_unknown_linux_gnueabihf`,
  `CXX_armv7_unknown_linux_gnueabihf`.
- Provides `baboon` (via the `7mind/baboon` flake input) and Node /
  esbuild / typescript for the frontend.

Missing: `packages.<system>.*` outputs. Everything is gated behind
`nix develop --command cargo build ...`, which cannot be offloaded to
remote builders because the compile runs outside the Nix sandbox.

## 2. Cross-compilation strategy — `rustPlatform.buildRustPackage` with fenix

Three options considered:

- (a) `pkgsCross.armv7l-hf-multiplatform.rustPlatform.buildRustPackage` —
  clean but forces a second toolchain alongside fenix, breaking the
  "one Rust used both in shell and sandbox" invariant.
- (b) **`rustPlatform.buildRustPackage` with fenix toolchain and
  explicit target triple** — same env matrix as the shell; single
  source of truth for the toolchain. **Recommended.**
- (c) Plain derivation invoking `cargo build` — rejected; reinvents
  dependency tracking.

Equivalent with `crane` is also fine; prefer `buildRustPackage` to
minimise new flake inputs. `crane`'s cargo-artifact caching matters
more for dev loops than remote-builder runs.

## 3. Frontend bundle — committed, skip at build time

Confirmed via `git ls-files`:
- `crates/shell/static/bundle.js` is **committed** (`.gitignore`
  documents: "The built bundle IS checked in so `cargo build` works
  without a prior `npm run build`.").
- `crates/shell/src/` uses `include_str!`/`include_bytes!` against
  `crates/shell/static/`.

**Implication:** the Rust Nix derivation does NOT need Node, esbuild,
or typescript. It consumes the committed bundle.

Optional future enhancement: a separate `packages.<system>.web-bundle`
derivation that regenerates `bundle.js` — deferred; the committed-bundle
path is fine and matches current CI / install-victron.sh.

## 4. Baboon codegen — committed, skip at build time

- `crates/dashboard-model/src/victron_controller/dashboard/*.rs` —
  committed (~30 generated files).
- `web/src/model/victron_controller/dashboard/*.ts` — committed.
- `scripts/regen-baboon.sh` is a dev workflow, not a build step.

**Implication:** the Nix build does NOT need baboon available. The
dev-shell `buildInputs` keeps `baboon` for developers running
`regen-baboon.sh`; package derivations do not depend on it. Avoids
IFD concerns around the `7mind/baboon` flake input.

## 5. Proposed flake `packages.<system>` layout

```nix
packages.<system> = {
  # Native binary for the dev host.
  victron-controller = …;

  # ARMv7 release binary for the Venus, ELF interpreter patched.
  victron-controller-armv7 = …;

  default = self.packages.<system>.victron-controller;
};
```

No musl/static variant — Venus ships glibc, the current deploy path
works against it.

## 6. Remote-builder compatibility

`buildRustPackage` with `cargoLock` makes the derivation pure: no
`$HOME`, no network at build time (deps vendored), no IFD, no
references to `/nix/store` paths outside the closure.

Pitfalls to check:

- **Git deps in Cargo.lock.** If any `source = "git+…"`, capture
  `outputHashes` in `cargoLock`. Quick grep before implementation.
- **`baboon` flake input.** Dev-shell only; must not be pulled into
  package derivations.
- **`crates/shell/static/bundle.js`.** Content-addressable input;
  committed-version is the canonical content. No reproducibility risk.
- **cc-rs `$CC` sniffing.** ring etc. sniff `$CC`. The existing
  `CC_armv7_unknown_linux_gnueabihf` env var covers cross calls;
  ensure `depsBuildBuild = [ pkgs.buildPackages.stdenv.cc ]` so build
  scripts compiled for the builder host get a native `$CC` too.

## 7. ELF interpreter patch

Nix cross-toolchain bakes `/nix/store/.../ld-linux-armhf.so.3` into the
ELF. `scripts/install-victron.sh` currently post-processes with
`patchelf --set-interpreter /lib/ld-linux-armhf.so.3`.

Move into the derivation's `postFixup`:
```nix
postFixup = ''
  patchelf \
    --set-interpreter /lib/ld-linux-armhf.so.3 \
    --set-rpath '' \
    $out/bin/victron-controller
'';
```

After this, `$(nix path-info .#victron-controller-armv7)/bin/victron-controller`
is directly runnable on the Venus. `install-victron.sh` can either keep
its defensive patchelf as a no-op fallback for the pre-Nix path, or gain
a `--nix` flag that skips the local build stage.

UPX compression stays OUT of the derivation (non-reproducible; deploy-
time concern).

## 8. Deployment integration (sketch only, separate PR)

Option A — new script `scripts/deploy-nix.sh`:
```bash
#!/usr/bin/env bash
set -euo pipefail
TARGET="$1"; shift
OUT=$(nix build --no-link --print-out-paths .#victron-controller-armv7)
BIN="$OUT/bin/victron-controller"
tmp=$(mktemp); cp "$BIN" "$tmp"
command -v upx >/dev/null && upx --lzma -q -9 "$tmp" || true
scp "$tmp" "$TARGET:/data/opt/victron-controller/bin/victron-controller.new"
# ...rest of install-victron.sh's rcS.local / daemontools setup
```

Option B — `install-victron.sh --nix` flag that bypasses local cargo
build and substitutes the `nix build` step. Remote-side logic
unchanged.

## 9. Risks and non-goals

### Keep working
- `nix develop` dev shell (cargo, cross linker, baboon, node, patchelf, upx).
- Local `cargo build --target armv7-...` inside `nix develop`.
- `./scripts/install-victron.sh user@host` without Nix.

### Risks
- **`ring` cross-compile.** Hand-written ARM asm; existing dev-shell
  proves it works with `CC_armv7_…` + `+neon` RUSTFLAGS. Derivation
  uses the same env matrix → low risk.
- **rumqttc + reqwest with `rustls-tls`.** Pure-Rust TLS, no openssl
  → low risk.
- **zbus.** Pure Rust → low risk.
- **Workspace release profile.** `panic = "abort"`, `opt-level = "z"`,
  LTO. Nix builds pick these up from `Cargo.toml` automatically.
- **Git deps in Cargo.lock.** Needs verification at implementation
  time.

### Non-goals
- Don't replace the dev shell with a nix-run-only workflow.
- Don't add musl/static.
- Don't regenerate baboon or the web bundle inside Nix (both committed).

## 10. Implementation checklist

1. `grep 'source = "git+' Cargo.lock` — capture any `outputHashes`.
2. Add `packages.victron-controller` (native) using fenix +
   `buildRustPackage`.
3. Add `packages.victron-controller-armv7`:
   - `CARGO_BUILD_TARGET = "armv7-unknown-linux-gnueabihf"`.
   - fenix toolchain including armv7 `rust-std`.
   - All cross env vars copied from the dev shell.
   - `depsBuildBuild = [ buildPackages.stdenv.cc ]`.
   - `nativeBuildInputs = [ pkgs.patchelf crossCC ]`.
   - `postFixup` with the interpreter patch.
   - `doCheck = false` (tests need host-arch; run via native package).
4. Test locally: `nix build .#victron-controller-armv7 && file result/bin/victron-controller`.
5. Test with remote builders: `nix build .#victron-controller-armv7 --builders 'ssh://build@host'`.
6. (Optional) `scripts/deploy-nix.sh` or `install-victron.sh --nix`.
7. Document in README / SPEC.

## Critical files for implementation

- `flake.nix`
- `Cargo.lock`
- `crates/shell/Cargo.toml`
- `scripts/install-victron.sh`
- `Cargo.toml`
