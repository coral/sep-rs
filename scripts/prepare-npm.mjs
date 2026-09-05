import { copyFile, readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'

const root = resolve(import.meta.dirname, '..')
const packageDirectory = resolve(root, 'pkg')
const webPackageDirectory = resolve(root, 'target', 'wasm-pack-web')
const manifestPath = resolve(packageDirectory, 'package.json')
const manifest = JSON.parse(await readFile(manifestPath, 'utf8'))

if (manifest.name !== 'sep-rs') {
  throw new Error(`expected wasm-pack to generate sep-rs, received ${manifest.name}`)
}

Object.assign(manifest, {
  name: 'sep-tools',
  description: 'Cisco SEP configuration generation and validation for JavaScript and Cloudflare Workers',
  license: 'MIT',
  repository: {
    type: 'git',
    url: 'git+https://github.com/coral/sep-rs.git',
  },
  homepage: 'https://github.com/coral/sep-rs#readme',
  bugs: {
    url: 'https://github.com/coral/sep-rs/issues',
  },
  keywords: ['cisco', 'sep', 'provisioning', 'sccp', 'sip', 'webassembly'],
  sideEffects: [...new Set([...(manifest.sideEffects ?? []), './cloudflare.js'])],
  types: 'index.d.ts',
  exports: {
    '.': {
      types: './index.d.ts',
      import: './sep_rs.js',
      default: './sep_rs.js',
    },
    './cloudflare': {
      types: './index.d.ts',
      import: './cloudflare.js',
      default: './cloudflare.js',
    },
  },
})

manifest.files = [
  ...new Set([
    ...manifest.files,
    'cloudflare.js',
    'sep_rs_web.js',
    'index.d.ts',
    'README.md',
    'LICENSE',
  ]),
]

await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`)
await copyFile(
  resolve(root, 'bindings/npm/cloudflare.js'),
  resolve(packageDirectory, 'cloudflare.js'),
)
await copyFile(
  resolve(webPackageDirectory, 'sep_rs.js'),
  resolve(packageDirectory, 'sep_rs_web.js'),
)
await copyFile(resolve(root, 'bindings/npm/index.d.ts'), resolve(packageDirectory, 'index.d.ts'))
await copyFile(resolve(root, 'bindings/npm/README.md'), resolve(packageDirectory, 'README.md'))
await copyFile(resolve(root, 'LICENSE'), resolve(packageDirectory, 'LICENSE'))
