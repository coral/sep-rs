#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bindgen_target="$repo_root/target/uniffi-bindgen"
cd "$repo_root"

cargo build --release -p sep-rs-ffi
rm -f bindings/python/septools.py
CARGO_TARGET_DIR="$bindgen_target" cargo run --quiet \
  -p sep-rs-ffi \
  --features bindgen \
  --bin sep-rs-bindgen \
  -- generate \
  --language python \
  --out-dir bindings/python \
  --config bindings/python/uniffi.toml \
  crates/sep-rs-ffi/src/sep_tools.udl
perl -pi -e 's/some hot garbage in the `uniffi` crate/the UniFFI generator/' \
  bindings/python/septools.py
perl -pi -e 's/^# Trust me.*$/# Do not edit it manually./' \
  bindings/python/septools.py
perl -pi -e 's/[[:blank:]]+$//' bindings/python/septools.py
perl -0pi -e 's/\n+\z/\n/' bindings/python/septools.py

echo "Python bindings generated in bindings/python/septools.py"
echo "Run ./scripts/test-python-bindings.sh to load and test them"
