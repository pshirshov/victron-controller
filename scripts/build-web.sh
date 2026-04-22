#!/usr/bin/env bash
# Build the TypeScript dashboard frontend into
# crates/shell/static/bundle.js so `cargo build` can include_str! it.
#
# Usage:  ./scripts/build-web.sh           # minified production build
#         ./scripts/build-web.sh --watch   # rebuild on file change
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"

cd "$REPO_ROOT/web"

if [[ ! -d node_modules ]]; then
  echo "[web] installing npm deps…"
  npm install
fi

if [[ "${1:-}" == "--watch" ]]; then
  npm run watch
else
  npm run typecheck
  npm run build
fi
