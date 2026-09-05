#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
test_dir="$(mktemp -d "${TMPDIR:-/tmp}/sep-rs-python-test.XXXXXX")"
trap 'rm -rf "$test_dir"' EXIT

cd "$repo_root"

if ! command -v python3 >/dev/null 2>&1; then
  echo "python3 is required to test the generated bindings" >&2
  exit 1
fi

cargo build --release -p sep-rs-ffi
cp bindings/python/septools.py "$test_dir/"
cp bindings/python/test_bindings.py "$test_dir/"

case "$(uname -s)" in
  Darwin)
    cp target/release/libsep_rs_ffi.dylib "$test_dir/"
    ;;
  Linux)
    cp target/release/libsep_rs_ffi.so "$test_dir/"
    ;;
  *)
    echo "unsupported Python binding test platform: $(uname -s)" >&2
    exit 1
    ;;
esac

cd "$test_dir"
python3 test_bindings.py
