#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if ! command -v uniffi-bindgen-go >/dev/null 2>&1; then
  echo "uniffi-bindgen-go is required; see bindings/go/README.md" >&2
  exit 1
fi

cargo build --release -p sep-rs-ffi
rm -f \
  bindings/go/septools/sep_tools.go \
  bindings/go/septools/septools.go \
  bindings/go/septools/sep_tools.h
uniffi-bindgen-go \
  --out-dir bindings/go \
  --config bindings/go/uniffi.toml \
  crates/sep-rs-ffi/src/sep_tools.udl
perl -pi -e 's/some hot garbage in the `uniffi` crate/the UniFFI generator/' \
  bindings/go/septools/sep_tools.h
perl -pi -e 's|^// Trust me.*$|// Do not edit it manually.|' \
  bindings/go/septools/sep_tools.h
perl -pi -e 's/[[:blank:]]+$//' bindings/go/septools/sep_tools.h
perl -0pi -e 's/\n+\z/\n/' bindings/go/septools/sep_tools.h

echo "Go bindings generated in bindings/go/septools"
echo "Run ./scripts/test-go-bindings.sh to build and test them"
