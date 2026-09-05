import wasmModule from './sep_rs_bg.wasm'
import { initSync } from './sep_rs_web.js'

initSync({ module: wasmModule })

export {
  generateBundle,
  generateDevice,
  modelProfiles,
  options,
  phoneOptions,
  validateArtifact,
  validateBundle,
  validatePhoneSettings,
} from './sep_rs_web.js'
