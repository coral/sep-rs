#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
package_target="${TMPDIR:-/tmp}/sep-rs-cargo-package-target"
cd "$repo_root"

cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
./scripts/test-go-bindings.sh
./scripts/test-python-bindings.sh
npm run build:npm
node scripts/test-npm.mjs
npm_config_cache="${TMPDIR:-/tmp}/sep-rs-npm-cache" npm pack ./pkg --dry-run
CARGO_TARGET_DIR="$package_target" cargo package -p sep-rs --allow-dirty
