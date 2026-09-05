import json

import septools


profiles = json.loads(septools.model_profiles_json())
assert profiles

catalog = json.loads(septools.options_json(None))
assert catalog["schema_version"] == 2
assert len(catalog["choices"]["sep_settings"]) == 481

phone = json.loads(septools.phone_options_json("8841", "sip"))
assert phone["model"] == "CP-8841"
assert len(phone["settings"]) == 391

diagnostics = json.loads(
    septools.validate_phone_settings_json(
        json.dumps(
            {
                "model": "8841",
                "protocol": "sip",
                "settings": [
                    {
                        "path": "/device/vendorConfig/recordingToneLocalVolume",
                        "value": 101,
                    }
                ],
            }
        )
    )
)
assert diagnostics[0]["code"] == "invalid_setting"

print(
    f"Python bindings smoke: {len(profiles)} model profiles, "
    f"{len(phone['settings'])} CP-8841 SIP settings"
)
