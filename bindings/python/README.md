# Python bindings

The Python module is generated from the same Rust engine and UniFFI definition
as the Go bindings. Its API uses JSON request and response strings, keeping the
foreign ABI small while exposing the complete validation, generation, and
reflection surface.

Generate `septools.py` and build the native library from the repository root:

```console
./scripts/build-python-bindings.sh
```

The generated module uses only Python's standard library and loads the
platform-native `sep_rs_ffi` dynamic library from the same directory. For
development, verify that pairing with:

```console
./scripts/test-python-bindings.sh
```

Applications can call the generated functions directly and decode their JSON
results:

```python
import json

import septools

catalog = json.loads(septools.options_json(None))
phone = json.loads(septools.phone_options_json("8841", "sip"))
```

For distribution, place `septools.py` and the target platform's
`libsep_rs_ffi` dynamic library together in your application or wheel. The
native filename is `libsep_rs_ffi.dylib` on macOS,
`libsep_rs_ffi.so` on Linux, and `sep_rs_ffi.dll` on Windows.
