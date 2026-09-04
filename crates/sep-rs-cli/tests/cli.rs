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
