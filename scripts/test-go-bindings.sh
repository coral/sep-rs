#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
library_dir="$repo_root/target/release"
go_cache="${TMPDIR:-/tmp}/sep-rs-go-cache"

cd "$repo_root"
cargo build --release -p sep-rs-ffi

case "$(uname -s)" in
  Darwin)
    cd bindings/go
    GOCACHE="$go_cache" \
      CGO_LDFLAGS="-L$library_dir -lsep_rs_ffi -framework Security -framework SystemConfiguration" \
      DYLD_LIBRARY_PATH="$library_dir${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}" \
      go test ./...
    ;;
  Linux)
    cd bindings/go
    GOCACHE="$go_cache" \
      CGO_LDFLAGS="-L$library_dir -lsep_rs_ffi -ldl -lm" \
      LD_LIBRARY_PATH="$library_dir${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
      go test ./...
    ;;
  *)
    echo "unsupported Go binding test platform: $(uname -s)" >&2
    exit 1
    ;;
esac
