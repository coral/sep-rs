use std::str::FromStr as _;

use sep_rs::{
    ArtifactDialect, ArtifactKind, BootstrapBundle, BundleFile, BundleSpec,
    BundleValidationRequest, CallControlEndpoint, DefaultSpec, DeviceSpec, Diagnostic, Host,
    InventorySource, MacAddress, ModelLoad, PhoneModelId, Protocol, ProtocolSpec, SccpSpec, Secret,
    SepSetting, SepSettingValue, ServiceUrls, Severity, SipLine, SipSpec, Transport,
    generate_bundle, generate_device, parse_artifact, validate, validate_bundle_input,
};

fn endpoint(port: u16) -> CallControlEndpoint {
    CallControlEndpoint {
        host: Host::from_str("pbx.example.test").expect("host"),
        port,
        priority: 0,
        transport: Transport::Tcp,
    }
}

fn base_device(protocol: ProtocolSpec) -> DeviceSpec {
    DeviceSpec {
        mac: MacAddress::from_str("00:11:22:33:44:55").expect("MAC"),
        model: PhoneModelId::from_str("7965").expect("model"),
        firmware: None,
        protocol,
        endpoints: vec![endpoint(2000)],
        phone_label: Some("Ops & Support".to_owned()),
        time_zone: None,
        date_template: None,
        ntp_server: None,
        locale: None,
        services: ServiceUrls::default(),
        settings: Vec::new(),
        allow_unknown_settings: false,
    }
}

fn default_spec(protocol: Protocol, port: u16, model_loads: Vec<ModelLoad>) -> DefaultSpec {
    DefaultSpec {
        protocol,
        firmware: None,
        endpoints: vec![endpoint(port)],
        model_loads,
        time_zone: None,
        date_template: None,
        ntp_server: None,
        locale: None,
    }
}

fn model_load(model: &str, model_id: Option<u16>, firmware: &str) -> ModelLoad {
    ModelLoad {
        model: PhoneModelId::from_str(model).expect("model"),
        model_id,
        firmware: firmware.to_owned(),
    }
}

fn assert_in_memory_bundle_valid(bundle: &BootstrapBundle) {
    let mut files = bundle
        .artifacts
        .iter()
        .map(|artifact| BundleFile {
            filename: artifact.filename.clone(),
            contents: Some(artifact.contents.clone()),
        })
        .collect::<Vec<_>>();
    files.push(BundleFile {
        filename: "bootstrap-manifest.json".to_owned(),
        contents: Some(serde_json::to_string(&bundle.inventory).expect("inventory JSON")),
    });
    files.extend(
        bundle
            .inventory
            .files
            .iter()
            .filter(|entry| entry.source == InventorySource::External)
            .map(|entry| BundleFile {
                filename: entry.filename.clone(),
                contents: None,
            }),
    );
    let result = validate_bundle_input(&BundleValidationRequest { files })
        .expect("in-memory bundle validation");
    assert!(result.valid);
}

#[test]
fn generated_sccp_and_sip_round_trip_through_validation() {
    let mut sccp_spec = base_device(ProtocolSpec::Sccp(SccpSpec::default()));
    sccp_spec.firmware = Some("SCCP45.9-4-2SR1-1S".to_owned());
    let sccp = generate_device(&sccp_spec).expect("SCCP generation");
    let parsed = parse_artifact(&sccp.contents, Some(&sccp.filename)).expect("SCCP parse");
    assert!(validate(&parsed, None).is_empty());

    let mut sip_spec = base_device(ProtocolSpec::Sip(SipSpec {
        lines: vec![SipLine {
            index: 1,
            directory_number: "1001".to_owned(),
            display_name: Some("Ops <Primary>".to_owned()),
            auth_name: Some("1001".to_owned()),
            auth_secret: Some(Secret::new("p<&secret")),
        }],
        ..SipSpec::default()
    }));
    sip_spec.endpoints = vec![endpoint(5060)];
    sip_spec.firmware = Some("SIP45.9-4-2SR1-1S".to_owned());
    let sip = generate_device(&sip_spec).expect("SIP generation");
    assert!(sip.contains_secrets);
    assert!(sip.contents.contains("Ops &lt;Primary&gt;"));
    assert!(sip.contents.contains("p&lt;&amp;secret"));

    let parsed = parse_artifact(&sip.contents, Some(&sip.filename)).expect("SIP parse");
    let diagnostics = validate(&parsed, Some(&sip_spec.model));
    assert!(!diagnostics.iter().any(Diagnostic::is_error));
    assert!(
        diagnostics
            .iter()
            .any(|item| item.code == "cleartext_secret")
    );
    let debug = format!("{sip_spec:?}");
    assert!(!debug.contains("p<&secret"));
}

#[test]
fn invalid_sip_reports_multiple_semantic_errors_without_echoing_secrets() {
    let xml = r#"<device>
      <deviceProtocol>SIP</deviceProtocol>
      <devicePool><callManagerGroup><members>
        <member priority="0"><callManager><ports><sipPort>0</sipPort></ports><processNodeName>pbx</processNodeName></callManager></member>
        <member priority="0"><callManager><ports><sipPort>5060</sipPort></ports><processNodeName>backup</processNodeName></callManager></member>
      </members></callManagerGroup></devicePool>
      <sipProfile><startMediaPort>32000</startMediaPort><stopMediaPort>16000</stopMediaPort><sipLines>
        <line button="1" lineIndex="1"><featureID>9</featureID><name>1001</name><authPassword>never-print-this</authPassword></line>
        <line button="1" lineIndex="1"><featureID>20</featureID></line>
      </sipLines></sipProfile>
      <deviceSecurityMode>3</deviceSecurityMode><transportLayerProtocol>3</transportLayerProtocol><encrConfig>false</encrConfig>
    </device>"#;
    let parsed = parse_artifact(xml, Some("SEP001122334455.cnf.xml")).expect("parse");
    let diagnostics = validate(&parsed, None);
    assert!(diagnostics.iter().filter(|item| item.is_error()).count() >= 5);
    let rendered = format!("{diagnostics:?}");
    assert!(!rendered.contains("never-print-this"));
}

#[test]
fn advanced_settings_are_validated_merged_and_escaped() {
    let mut spec = base_device(ProtocolSpec::Sccp(SccpSpec::default()));
    spec.settings = vec![
        SepSetting::new(
            "/device/vendorConfig/displayOnTime"
                .parse()
                .expect("valid display setting path"),
            SepSettingValue::String("09:15".to_owned()),
        ),
        SepSetting::new(
            "/device/devicePool/callManagerGroup/members/member[1]/@priority"
                .parse()
                .expect("valid priority setting path"),
            SepSettingValue::Integer(7),
        ),
    ];
    let artifact = generate_device(&spec).expect("advanced generation");
    assert!(
        artifact
            .contents
            .contains("<displayOnTime>09:15</displayOnTime>")
    );
    assert!(artifact.contents.contains("<member priority=\"7\">"));
    let parsed = parse_artifact(&artifact.contents, Some(&artifact.filename)).expect("parse");
    assert!(
        !validate(&parsed, Some(&spec.model))
            .iter()
            .any(Diagnostic::is_error)
    );

    spec.settings[0].value = SepSettingValue::String("25:99".to_owned());
    assert!(generate_device(&spec).is_err());

    spec.settings = vec![SepSetting::new(
        "/device/deviceProtocol"
            .parse()
            .expect("valid protocol setting path"),
        SepSettingValue::String("SIP".to_owned()),
    )];
    assert!(generate_device(&spec).is_err());

    spec.settings = vec![SepSetting::new(
        "/device/vendorConfig/futureFirmwareField"
            .parse()
            .expect("valid future setting path"),
        SepSettingValue::String("<&".to_owned()),
    )];
    spec.allow_unknown_settings = true;
    let artifact = generate_device(&spec).expect("forward-compatible generation");
    assert!(artifact.contents.contains("&lt;&amp;"));
    assert!(
        artifact
            .warnings
            .iter()
            .any(|diagnostic| diagnostic.code == "unknown_field")
    );
}

#[test]
fn legacy_and_bundle_generation_use_distinct_dialects_and_inventory_dependencies() {
    let mut legacy = base_device(ProtocolSpec::Sip(SipSpec {
        lines: vec![SipLine {
            index: 1,
            directory_number: "1001".to_owned(),
            display_name: None,
            auth_name: None,
            auth_secret: None,
        }],
        ..SipSpec::default()
    }));
    legacy.model = PhoneModelId::from_str("7960").expect("model");
    legacy.endpoints = vec![endpoint(5060)];
    legacy.firmware = Some("P0S3-8-12-00".to_owned());
    let generated = generate_device(&legacy).expect("legacy generation");
    assert_eq!(generated.dialect, ArtifactDialect::LegacySipText);
    assert_eq!(generated.filename, "SIP001122334455.cnf");
    let parsed = parse_artifact(&generated.contents, Some(&generated.filename)).expect("parse");
    assert!(!validate(&parsed, None).iter().any(Diagnostic::is_error));

    let mut enterprise = legacy.clone();
    enterprise.model = PhoneModelId::from_str("7965").expect("model");
    enterprise.firmware = Some("SIP45.9-4-2SR1-1S".to_owned());

    let bundle = generate_bundle(&BundleSpec {
        devices: vec![legacy, enterprise],
        defaults: vec![default_spec(
            Protocol::Sip,
            5060,
            vec![
                model_load("8845", None, "sip8845_65.12-8-1-0001-455"),
                model_load("7960", None, "P0S3-8-12-00"),
                model_load("CP-9999", Some(9999), "UNLISTED.1"),
            ],
        )],
        ..BundleSpec::default()
    })
    .expect("bundle generation");
    assert!(bundle.inventory.files.iter().any(|entry| {
        entry.kind == ArtifactKind::LoadDescriptor && entry.filename == "P0S3-8-12-00.loads"
    }));
    assert!(bundle.inventory.files.iter().any(|entry| {
        entry.kind == ArtifactKind::DialPlan && entry.source == sep_rs::InventorySource::External
    }));
    assert!(bundle.artifacts.iter().any(|artifact| {
        artifact.filename == "XMLDefault.cnf.xml"
            && artifact.contents.contains(
                "<loadInformation36224 model=\"Cisco 8845\">sip8845_65.12-8-1-0001-455</loadInformation36224>",
            )
            && artifact.contents.contains(
                "<loadInformation7 model=\"Cisco 7960\">P0S3-8-12-00</loadInformation7>",
            )
            && artifact.contents.contains(
                "<loadInformation9999 model=\"CP-9999\">UNLISTED.1</loadInformation9999>",
            )
            && artifact
                .warnings
                .iter()
                .any(|warning| warning.code == "unknown_model")
    }));

    assert_in_memory_bundle_valid(&bundle);
    assert!(bundle.artifacts.iter().any(|artifact| {
        artifact.filename == "SIPDefault.cnf"
            && artifact
                .contents
                .contains("image_version: \"P0S3-8-12-00\"")
    }));
    assert!(
        !bundle
            .inventory
            .files
            .iter()
            .any(|entry| { entry.kind == ArtifactKind::TrustList && entry.required })
    );
    assert!(
        !validate(
            &parse_artifact(&generated.contents, Some(&generated.filename)).expect("parse"),
            None
        )
        .iter()
        .any(|item| item.severity == Severity::Error)
    );

    let compiled_default_bundle = generate_bundle(&BundleSpec {
        defaults: vec![default_spec(
            Protocol::Sccp,
            2000,
            vec![model_load("7940", None, "P00308010200")],
        )],
        ..BundleSpec::default()
    })
    .expect("compiled-default inventory");
    assert!(compiled_default_bundle.inventory.files.iter().any(|entry| {
        entry.filename == "SEPDefault.cnf"
            && entry.kind == ArtifactKind::DefaultConfiguration
            && entry.source == sep_rs::InventorySource::External
            && entry.required
    }));
}
