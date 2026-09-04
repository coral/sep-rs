import { copyFile, readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'

const root = resolve(import.meta.dirname, '..')
const packageDirectory = resolve(root, 'pkg')
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
  types: 'index.d.ts',
  exports: {
    '.': {
      types: './index.d.ts',
      import: './sep_rs.js',
      default: './sep_rs.js',
    },
  },
})

manifest.files = [...new Set([...manifest.files, 'index.d.ts', 'README.md', 'LICENSE'])]

await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`)
await copyFile(resolve(root, 'bindings/npm/index.d.ts'), resolve(packageDirectory, 'index.d.ts'))
await copyFile(resolve(root, 'bindings/npm/README.md'), resolve(packageDirectory, 'README.md'))
await copyFile(resolve(root, 'LICENSE'), resolve(packageDirectory, 'LICENSE'))
