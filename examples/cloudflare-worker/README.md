# Cloudflare Worker example

This Worker imports the `sep-tools/cloudflare` entry point directly. Wrangler
bundles its WebAssembly module with the Worker; no separate WASM initialization
or asset upload is needed.

Install the example and run it locally:

```console
npm install
npm run check
npm run dev
```

Then try the reflection and validation endpoints:

```console
curl 'http://localhost:8787/phone-options?model=8841&protocol=sip'
curl 'http://localhost:8787/options?target=device'
curl --json '{
  "filename": "SEP001122334455.cnf.xml",
  "contents": "<device>...</device>",
  "model": "8841"
}' http://localhost:8787/validate-artifact
```

The checked-in Wrangler configuration contains only the local example entry
point and a generic Worker name. To deploy it to your own Cloudflare account,
authenticate Wrangler and run `npm run deploy`; add any account-specific
routes, bindings, and environment configuration outside this example.

When developing against an unpublished local build, first run `npm run
build:npm` at the repository root, then install that build here with `npm
install --no-save ../../pkg`.
