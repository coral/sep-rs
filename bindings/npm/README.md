# sep-tools

WebAssembly bindings for generating and validating Cisco enterprise SEP
bootstrap configurations. The package is an ES module intended for bundlers,
including Wrangler for Cloudflare Workers.

```console
npm install sep-tools
```

```ts
import { modelProfiles, validateArtifact } from 'sep-tools'

const models = modelProfiles()
const result = validateArtifact({
  filename: 'SEP001122334455.cnf.xml',
  contents: '<device>...</device>',
})
```

Wrangler bundles the package's `.wasm` module when it is imported by a Worker.
All operations are synchronous and perform no network or filesystem I/O.

The source repository and Rust API documentation are at
<https://github.com/coral/sep-rs>.
