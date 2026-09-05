# sep-rs

`sep-rs` is a platform-neutral toolkit for inspecting, validating, and
generating Cisco enterprise-phone bootstrap configuration. It understands
enterprise SCCP and SIP `SEP<MAC>.cnf.xml`, `XMLDefault.cnf.xml`, and the
legacy 7940/7960 SIP text format.

## Published artifacts

Rust: [sep-rs on crates.io](https://crates.io/crates/sep-rs)
NPM: [sep-tools on npmjs.com](https://www.npmjs.com/package/sep-tools)

## Rust library

Add the library to a Rust project:

```console
cargo add sep-rs
```

```rust
use sep_rs::{ArtifactValidationRequest, OptionsTarget, Protocol, options, phone_options,
    validate_artifact_input};

// A typed reflection document with JSON Schema and runtime choice catalogs.
let all_inputs = options();
let device_input = all_inputs.target(OptionsTarget::Device).unwrap();
assert_eq!(device_input.schema_ref, "#/$defs/DeviceSpec");

// Resolve legal values, defaults, ranges, and formats for one concrete phone.
let phone = phone_options(&"8841".parse().unwrap(), Protocol::Sip);
assert_eq!(phone.model.as_deref(), Some("CP-8841"));

let result = validate_artifact_input(&ArtifactValidationRequest {
    filename: "SEP001122334455.cnf.xml".to_owned(),
    contents: "<device>...</device>".to_owned(),
    model: None,
});
assert!(!result.valid);
```

Build and test the complete Rust workspace with:

```console
cargo test --workspace
```

## CLI

Install the CLI from a local checkout:

```console
cargo install --path crates/sep-rs-cli
sep-rs models
```

The CLI is a repository companion and is not published to crates.io.

Validate one artifact or a directory containing a bootstrap bundle:

```console
sep-rs validate SEP00082FB6B4AA.cnf.xml
sep-rs validate --model 7965 SEP00082FB6B4AA.cnf.xml
sep-rs validate --format json SEP00082FB6B4AA.cnf.xml
sep-rs validate bundle ./tftp-root
```

Explore every known setting for a phone model. Known models use their supported
protocols; unrecognized models show the generic enterprise catalogs:

```console
cargo run -- explore 7609
cargo run -- explore 8841 --protocol sip --format json
```

Generate a device configuration from a TOML or JSON manifest:

```console
sep-rs generate device --manifest phone.toml --output ./tftp-root
sep-rs generate bundle --manifest phones.toml --output ./tftp-root
```

Ready-to-edit manifest shapes are available in [`examples/`](examples/).
Add a `firmware` value only after choosing an approved load and making its
descriptor and payloads available to the phone.

Device generation also accepts direct flags for a simple single-endpoint
configuration:

```console
sep-rs generate device \
  --mac 00:08:2f:b6:b4:aa \
  --model 7965 \
  --protocol sccp \
  --host call-control.example.net \
  --firmware SCCP45.9-4-2SR1-1S \
  --output ./tftp-root
```

`--manifest` and direct device flags are mutually exclusive. Generated files
use canonical Cisco names. Existing files are never replaced unless `--force`
is supplied; use `--stdout` for a single generated device artifact.

The CLI exits with:

- `0` when generation succeeds or validation finds no errors (warnings are
  permitted)
- `1` when validation completes and reports semantic errors
- `2` for command usage, input/output, or parse failures

## npm and Cloudflare Workers

The npm package is named `sep-tools`. Build a publishable package in `pkg/`:

```console
npm run build:npm
npm pack ./pkg
```

The package includes a standard bundler entry point and a Cloudflare-specific
entry point. In a Worker, import from `sep-tools/cloudflare`; Wrangler will
bundle and initialize the `.wasm` module synchronously.

```ts
import {
  modelProfiles,
  options,
  phoneOptions,
  validateArtifact,
  validatePhoneSettings,
} from 'sep-tools'

const models = modelProfiles()
const deviceForm = options('device')
const phone = phoneOptions('8841', 'sip')
const settingErrors = validatePhoneSettings('8841', 'sip', [
  { path: '/device/vendorConfig/recordingToneLocalVolume', value: 101 },
])
const result = validateArtifact({
  filename: 'SEP001122334455.cnf.xml',
  contents: '<device>...</device>',
  model: '8841',
})
```

The same functions are available in a Worker by changing only the import:

```ts
import { options, phoneOptions, validateArtifact } from 'sep-tools/cloudflare'
```

A complete, deployment-neutral Worker is available in
[`examples/cloudflare-worker/`](examples/cloudflare-worker/). It includes local
Wrangler commands and HTTP endpoints for model discovery, option reflection,
phone-specific settings, and artifact validation.

## Go bindings

The Go generator uses UniFFI's third-party Go backend and exposes the same
operations through JSON request and response strings. This keeps the foreign
ABI small and stable while sharing all parsing, validation, and generation
logic with Rust and npm.

Install the pinned generator and run:

```console
./scripts/build-go-bindings.sh
```

See [`bindings/go/README.md`](bindings/go/README.md) for generator installation
and native linking details.

## License

Licensed under the [MIT License](LICENSE).

Cisco, SCCP, and related product names are trademarks of their respective
owners. This project is independent and is not affiliated with or endorsed by
Cisco Systems, Inc.
