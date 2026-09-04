# sep-tools

WebAssembly bindings for generating and validating Cisco enterprise SEP
bootstrap configurations. The package is an ES module intended for bundlers,
including Wrangler for Cloudflare Workers.

```console
npm install sep-tools
```

```ts
import { modelProfiles, options, validateArtifact } from 'sep-tools'

const models = modelProfiles()
const deviceForm = options('device')
const result = validateArtifact({
  filename: 'SEP001122334455.cnf.xml',
  contents: '<device>...</device>',
})
```

Wrangler bundles the package's `.wasm` module when it is imported by a Worker.
All operations are synchronous and perform no network or filesystem I/O.

`options()` returns every input as a JSON Schema Draft 2020-12 document plus
finite choice catalogs. Pass `device`, `defaults`, `bundle`,
`artifact_validation`, or `bundle_validation` to select one form target.

The source repository and Rust API documentation are at
<https://github.com/coral/sep-rs>.
