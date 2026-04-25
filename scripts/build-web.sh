#!/usr/bin/env bash
# Build the TypeScript dashboard frontend into
# crates/shell/static/bundle.js so `cargo build` (and dev-shell
# `cargo test`) can `include_str!` it.
#
# Production builds go through the Nix derivation `.#web-bundle`,
# which produces a byte-identical bundle from the same `tsc` +
# `esbuild` versions pinned by the flake. This script exists for two
# reasons: (1) populating the gitignored `bundle.js` once after
# `git clone` so dev-shell `cargo test` works without invoking Nix,
# and (2) `--watch` mode for live-reloading during dev iteration.
#
# Usage:
#   ./scripts/build-web.sh           # one-shot typecheck + bundle
#   ./scripts/build-web.sh --watch   # rebuild on file change
#
# Requires `nix develop` (provides `tsc` and `esbuild`).
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"

cd "$REPO_ROOT/web"

OUT="$REPO_ROOT/crates/shell/static/bundle.js"

if [[ "${1:-}" == "--watch" ]]; then
  exec esbuild src/index.ts --bundle --watch --outfile="$OUT" --sourcemap
fi

tsc --noEmit
esbuild src/index.ts --bundle --minify --outfile="$OUT"
