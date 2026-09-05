use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn direct_generation_validates_and_protects_existing_output() {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("sep-rs-cli-test-{}-{suffix}", std::process::id()));
    std::fs::create_dir(&directory).expect("temporary output directory");

    let binary = env!("CARGO_BIN_EXE_sep-rs");
    let generate = || {
        Command::new(binary)
            .args([
                "generate",
                "device",
                "--mac",
                "00:08:2f:b6:b4:aa",
                "--model",
                "7965",
                "--protocol",
                "sccp",
                "--host",
                "pbx.example.test",
                "--firmware",
                "SCCP45.9-4-2SR1-1S",
                "--output",
            ])
            .arg(&directory)
            .output()
            .expect("run generator")
    };

    let first = generate();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let artifact = directory.join("SEP00082FB6B4AA.cnf.xml");
    assert!(artifact.is_file());

    let validation = Command::new(binary)
        .arg("validate")
        .arg(&artifact)
        .output()
        .expect("run validator");
    assert!(validation.status.success());
    assert_eq!(String::from_utf8_lossy(&validation.stdout).trim(), "valid");

    let unknown_model = Command::new(binary)
        .args([
            "generate",
            "device",
            "--mac",
            "00:11:22:33:44:66",
            "--model",
            "CP-9999",
            "--protocol",
            "sccp",
            "--host",
            "pbx.example.test",
            "--stdout",
        ])
        .output()
        .expect("generate unlisted model");
    assert!(unknown_model.status.success());
    assert!(String::from_utf8_lossy(&unknown_model.stdout).contains("<device>"));
    assert!(String::from_utf8_lossy(&unknown_model.stderr).contains("unknown_model"));

    let second = generate();
    assert_eq!(second.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&second.stderr).contains("refusing to overwrite"));

    std::fs::remove_dir_all(&directory).expect("remove controlled temporary directory");
}

#[test]
fn explore_resolves_known_profiles_and_keeps_unknown_models_explorable() {
    let binary = env!("CARGO_BIN_EXE_sep-rs");
    let known = Command::new(binary)
        .args(["explore", "7945", "--format", "json"])
        .output()
        .expect("explore known model");
    assert!(known.status.success());
    let known: serde_json::Value =
        serde_json::from_slice(&known.stdout).expect("known exploration JSON");
    assert_eq!(known["resolved_model"], "CP-7945G");
    assert_eq!(known["options"].as_array().map(Vec::len), Some(2));

    let unknown = Command::new(binary)
        .args(["explore", "7609", "--protocol", "sccp", "--format", "json"])
        .output()
        .expect("explore unknown model");
    assert!(unknown.status.success());
    let unknown: serde_json::Value =
        serde_json::from_slice(&unknown.stdout).expect("unknown exploration JSON");
    assert!(unknown["resolved_model"].is_null());
    assert_eq!(unknown["options"][0]["protocol"], "sccp");
    assert!(
        unknown["options"][0]["settings"]
            .as_array()
            .is_some_and(|settings| !settings.is_empty())
    );
}

#[test]
fn export_writes_complete_and_model_specific_catalogs() {
    let binary = env!("CARGO_BIN_EXE_sep-rs");

    let all_json = Command::new(binary)
        .args(["export", "all"])
        .output()
        .expect("export complete catalog as JSON");
    assert!(
        all_json.status.success(),
        "{}",
        String::from_utf8_lossy(&all_json.stderr)
    );
    let all_json: serde_json::Value =
        serde_json::from_slice(&all_json.stdout).expect("complete catalog JSON");
    assert_eq!(all_json["settings"].as_array().map(Vec::len), Some(481));

    let all_xml = Command::new(binary)
        .args(["export", "all", "--format", "xml"])
        .output()
        .expect("export complete catalog as XML");
    assert!(
        all_xml.status.success(),
        "{}",
        String::from_utf8_lossy(&all_xml.stderr)
    );
    assert_well_formed_xml(&all_xml.stdout, "sepSettingsCatalog");

    let all_toml = Command::new(binary)
        .args(["export", "all", "--format", "toml"])
        .output()
        .expect("export complete catalog as TOML");
    assert!(
        all_toml.status.success(),
        "{}",
        String::from_utf8_lossy(&all_toml.stderr)
    );
    let all_toml: toml::Value =
        toml::from_str(std::str::from_utf8(&all_toml.stdout).expect("complete catalog UTF-8"))
            .expect("complete catalog TOML");
    assert_eq!(all_toml["settings"].as_array().map(Vec::len), Some(481));

    let phone = Command::new(binary)
        .args(["export", "7945"])
        .output()
        .expect("export model-specific catalog");
    assert!(
        phone.status.success(),
        "{}",
        String::from_utf8_lossy(&phone.stderr)
    );
    let phone: serde_json::Value =
        serde_json::from_slice(&phone.stdout).expect("model-specific JSON");
    assert_eq!(phone["resolved_model"], "CP-7945G");
    assert_eq!(phone["options"].as_array().map(Vec::len), Some(2));

    let phone_xml = Command::new(binary)
        .args(["export", "7945", "--format", "xml"])
        .output()
        .expect("export model-specific catalog as XML");
    assert!(
        phone_xml.status.success(),
        "{}",
        String::from_utf8_lossy(&phone_xml.stderr)
    );
    assert_well_formed_xml(&phone_xml.stdout, "phoneOptionsCatalog");

    let phone_toml = Command::new(binary)
        .args(["export", "7945", "--format", "toml"])
        .output()
        .expect("export model-specific catalog as TOML");
    assert!(
        phone_toml.status.success(),
        "{}",
        String::from_utf8_lossy(&phone_toml.stderr)
    );
    let phone_toml: toml::Value = toml::from_str(
        std::str::from_utf8(&phone_toml.stdout).expect("model-specific catalog UTF-8"),
    )
    .expect("model-specific catalog TOML");
    assert_eq!(phone_toml["resolved_model"].as_str(), Some("CP-7945G"));
}

fn assert_well_formed_xml(document: &[u8], expected_root: &str) {
    let mut reader = quick_xml::Reader::from_reader(document);
    let mut found_root = false;
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(element)) => {
                found_root = element.name().as_ref() == expected_root;
                break;
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Ok(_) => {}
            Err(error) => panic!("invalid export XML: {error}"),
        }
    }
    assert!(found_root, "missing expected export root element");

    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Eof) => break,
            Ok(_) => {}
            Err(error) => panic!("invalid export XML: {error}"),
        }
    }
}
