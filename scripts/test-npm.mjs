import assert from 'node:assert/strict'

import {
  options,
  phoneOptions,
  validateBundle,
  validatePhoneSettings,
} from '../pkg/sep_rs.js'

const all = options()
assert.equal(all.schema_version, 2)
assert.equal(all.targets.length, 5)
assert.equal(all.choices.model_profiles.length, 23)
assert.equal(all.choices.sep_settings.length, 481)
assert.ok(all.schema.$defs.DeviceSpec)

const device = options('device')
assert.deepEqual(
  device.targets.map(({ target }) => target),
  ['device'],
)
assert.equal(device.targets[0].schema_ref, '#/$defs/DeviceSpec')
assert.ok(device.schema.$defs.ProtocolSpec)

const phone = phoneOptions('8841', 'sip')
assert.equal(phone.model, 'CP-8841')
assert.equal(phone.settings.length, 391)
assert.equal(
  phone.settings.find(({ path }) => path.endsWith('/recordingToneLocalVolume')).maximum,
  100,
)
const diagnostics = validatePhoneSettings('8841', 'sip', [
  { path: '/device/vendorConfig/recordingToneLocalVolume', value: 101 },
])
assert.equal(diagnostics[0].code, 'invalid_setting')

const bundle = validateBundle({
  files: [
    {
      filename: 'bootstrap-manifest.json',
      contents: JSON.stringify({ files: [] }),
    },
  ],
})
assert.equal(bundle.valid, true)

assert.throws(() => options('not_a_target'), /unknown options target/)

console.log(
  `npm options smoke: ${all.targets.length} targets, ${all.choices.model_profiles.length} model profiles, ${all.choices.sep_settings.length} SEP settings`,
)
