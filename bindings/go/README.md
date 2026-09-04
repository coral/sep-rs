# Go bindings

The Go package is generated from the same Rust engine through UniFFI. Its API
uses JSON request and response strings so it stays aligned with the npm and
edge APIs while preserving categorized native errors.

Install the pinned generator once:

```console
cargo install uniffi-bindgen-go \
  --git https://github.com/NordSecurity/uniffi-bindgen-go \
  --tag v0.7.1+v0.31.0
```

Then generate the package and build its native library:

```console
./scripts/build-go-bindings.sh
```

Generated Go and C bridge files are written to `bindings/go/septools/`. Link
applications against `target/release/libsep_rs_ffi` for the target platform.
Run `./scripts/test-go-bindings.sh` to build the native library and test the
generated package with the correct cgo and runtime library paths.
