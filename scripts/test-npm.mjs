import assert from 'node:assert/strict'

import { options } from '../pkg/sep_rs.js'

const all = options()
assert.equal(all.schema_version, 1)
assert.equal(all.targets.length, 5)
assert.equal(all.choices.model_profiles.length, 23)
assert.ok(all.schema.$defs.DeviceSpec)

const device = options('device')
assert.deepEqual(
  device.targets.map(({ target }) => target),
  ['device'],
)
assert.equal(device.targets[0].schema_ref, '#/$defs/DeviceSpec')
assert.ok(device.schema.$defs.ProtocolSpec)

assert.throws(() => options('not_a_target'), /unknown options target/)

console.log(
  `npm options smoke: ${all.targets.length} targets, ${all.choices.model_profiles.length} model profiles`,
)
