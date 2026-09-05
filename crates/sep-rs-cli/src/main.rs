use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use sep_rs::{
    ArtifactDialect, BootstrapBundle, BundleSpec, CallControlEndpoint, DeviceSpec, Diagnostic,
    GeneratedArtifact, Host, MacAddress, PhoneModelId, PhoneOptionsCatalog, Protocol, ProtocolSpec,
    SccpSpec, Secret, ServiceUrls, SettingVariant, Severity, SipLine, SipSpec, Transport,
    detect_artifact, generate_bundle, generate_device, parse_artifact, phone_options, profiles,
    resolve_profile, validate, validate_bundle,
};
use serde::{Serialize, de::DeserializeOwned};

const EXIT_OK: u8 = 0;
const EXIT_INVALID: u8 = 1;
const EXIT_FAILURE: u8 = 2;

#[derive(Debug, Parser)]
#[command(
    name = "sep-rs",
    version,
    about = "Generate and validate Cisco enterprise-phone bootstrap configuration"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Generate one device artifact or a complete bootstrap bundle.
    Generate(GenerateArgs),
    /// Validate one artifact, or use `validate bundle` for a directory.
    Validate(ValidateArgs),
    /// List built-in phone model profiles.
    Models(ModelsArgs),
    /// Explore every SEP setting available to a phone model.
    Explore(ExploreArgs),
}

#[derive(Debug, Args)]
struct GenerateArgs {
    #[command(subcommand)]
    command: GenerateCommand,
}

#[derive(Debug, Subcommand)]
enum GenerateCommand {
    /// Generate one canonical device configuration.
    Device(DeviceArgs),
    /// Generate all artifacts described by a bundle manifest.
    Bundle(BundleArgs),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum ProtocolArg {
    Sccp,
    Sip,
}

#[derive(Debug, Args)]
struct DeviceArgs {
    /// TOML or JSON file containing a `DeviceSpec`.
    #[arg(long, value_name = "PATH", conflicts_with_all = ["mac", "model", "protocol", "host", "port", "priority", "firmware", "sip_user", "sip_display_name", "sip_auth_user", "sip_password"])]
    manifest: Option<PathBuf>,

    /// Device MAC address; separators are accepted.
    #[arg(long, value_name = "MAC", requires_all = ["model", "protocol", "host"])]
    mac: Option<String>,

    /// Phone model name or numeric enterprise model identifier.
    #[arg(long, value_name = "MODEL", requires = "mac")]
    model: Option<String>,

    /// Signaling protocol used by the installed enterprise firmware.
    #[arg(long, value_enum, requires = "mac")]
    protocol: Option<ProtocolArg>,

    /// Call-control hostname or IP address.
    #[arg(long, value_name = "HOST", requires = "mac")]
    host: Option<String>,

    /// Call-control port (defaults to 2000 for SCCP or 5060 for SIP).
    #[arg(long, value_name = "PORT", requires = "mac")]
    port: Option<u16>,

    /// Call-control priority in the generated group.
    #[arg(long, requires = "mac")]
    priority: Option<u8>,

    /// Firmware/load identifier expected to be available to the phone.
    #[arg(long, value_name = "LOAD", requires = "mac")]
    firmware: Option<String>,

    /// SIP address-of-record user/extension for direct SIP generation.
    #[arg(long, value_name = "USER", required_if_eq("protocol", "sip"))]
    sip_user: Option<String>,

    /// Display name for the direct SIP line.
    #[arg(long, value_name = "NAME", requires = "sip_user")]
    sip_display_name: Option<String>,

    /// SIP digest username; defaults to --sip-user.
    #[arg(long, value_name = "USER", requires = "sip_user")]
    sip_auth_user: Option<String>,

    /// SIP digest password. Prefer a protected manifest to avoid shell history.
    #[arg(long, value_name = "SECRET", requires = "sip_user")]
    sip_password: Option<String>,

    /// Destination directory. The canonical Cisco filename is appended.
    #[arg(short, long, value_name = "DIR", conflicts_with = "stdout")]
    output: Option<PathBuf>,

    /// Write the generated artifact to standard output.
    #[arg(long, conflicts_with = "output")]
    stdout: bool,

    /// Replace an existing output file.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Args)]
struct BundleArgs {
    /// TOML or JSON file containing a `BundleSpec`.
    #[arg(long, value_name = "PATH")]
    manifest: PathBuf,

    /// Destination directory for generated artifacts and inventory.
    #[arg(short, long, value_name = "DIR", default_value = ".")]
    output: PathBuf,

    /// Replace files that already exist in the destination.
    #[arg(long)]
    force: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
enum OutputFormat {
    #[default]
    Human,
    Json,
}

#[derive(Debug, Args)]
struct ValidateArgs {
    /// Artifact to validate. Use the `bundle` subcommand for a directory.
    #[arg(value_name = "FILE")]
    input: Option<PathBuf>,

    /// Phone model used for profile-aware compatibility checks.
    #[arg(long, value_name = "MODEL")]
    model: Option<String>,

    /// Diagnostic output format.
    #[arg(long, value_enum, default_value_t, global = true)]
    format: OutputFormat,

    #[command(subcommand)]
    command: Option<ValidateCommand>,
}

#[derive(Debug, Subcommand)]
enum ValidateCommand {
    /// Validate the relationships and external dependencies in a directory.
    Bundle(ValidateBundleArgs),
}

#[derive(Debug, Args)]
struct ValidateBundleArgs {
    /// Directory containing generated artifacts and bootstrap-manifest.json.
    #[arg(value_name = "DIR")]
    directory: PathBuf,
}

#[derive(Debug, Args)]
struct ModelsArgs {
    /// Output format.
    #[arg(long, value_enum, default_value_t)]
    format: OutputFormat,
}

#[derive(Debug, Args)]
struct ExploreArgs {
    /// Phone model name, alias, or numeric identifier.
    #[arg(value_name = "MODEL")]
    model: String,

    /// Limit results to one protocol. The default covers every protocol
    /// supported by a known model, or both protocols for an unknown model.
    #[arg(long, value_enum)]
    protocol: Option<ProtocolArg>,

    /// Output format.
    #[arg(long, value_enum, default_value_t)]
    format: OutputFormat,
}

#[derive(Debug, Serialize)]
struct ExploreOutput {
    requested_model: String,
    resolved_model: Option<String>,
    options: Vec<PhoneOptionsCatalog>,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::from(EXIT_FAILURE)
        }
    }
}

fn run(cli: Cli) -> Result<u8> {
    match cli.command {
        Command::Generate(args) => match args.command {
            GenerateCommand::Device(args) => generate_one(&args),
            GenerateCommand::Bundle(args) => generate_many(&args),
        },
        Command::Validate(args) => validate_input(&args),
        Command::Models(args) => list_models(args.format),
        Command::Explore(args) => explore_model(&args),
    }
}

fn generate_one(args: &DeviceArgs) -> Result<u8> {
    let spec = if let Some(path) = &args.manifest {
        load_manifest(path)?
    } else {
        direct_device_spec(args)?
    };
    let artifact = generate_device(&spec).context("could not generate device configuration")?;
    print_generation_warnings(std::slice::from_ref(&artifact));

    if args.stdout {
        let mut stdout = io::stdout().lock();
        stdout
            .write_all(artifact.contents.as_bytes())
            .context("could not write generated configuration to stdout")?;
        if !artifact.contents.ends_with('\n') {
            stdout.write_all(b"\n")?;
        }
    } else {
        let directory = args.output.as_deref().unwrap_or_else(|| Path::new("."));
        write_artifacts(directory, std::slice::from_ref(&artifact), args.force)?;
        println!("wrote {}", directory.join(&artifact.filename).display());
    }

    Ok(EXIT_OK)
}

fn generate_many(args: &BundleArgs) -> Result<u8> {
    let spec: BundleSpec = load_manifest(&args.manifest)?;
    let bundle = generate_bundle(&spec).context("could not generate bootstrap bundle")?;
    print_generation_warnings(&bundle.artifacts);
    write_bundle(&args.output, &bundle, args.force)?;
    println!(
        "wrote {} configuration artifact(s) to {}",
        bundle.artifacts.len(),
        args.output.display()
    );
    Ok(EXIT_OK)
}

fn validate_input(args: &ValidateArgs) -> Result<u8> {
    let model_hint = args
        .model
        .as_deref()
        .map(str::parse::<PhoneModelId>)
        .transpose()
        .context("invalid --model")?;
    let diagnostics = match (&args.input, &args.command) {
        (Some(path), None) => {
            let filename = path.file_name().and_then(|name| name.to_str());
            if let Ok(detection) = detect_artifact("", filename)
                && matches!(
                    detection.dialect,
                    ArtifactDialect::CompiledBinary
                        | ArtifactDialect::SignedXml
                        | ArtifactDialect::EncryptedXml
                )
            {
                bail!(
                    "{} is a recognized {:?} artifact, but that representation cannot be decoded for semantic validation",
                    path.display(),
                    detection.dialect
                );
            }
            let source = fs::read_to_string(path)
                .with_context(|| format!("could not read {}", path.display()))?;
            let parsed = parse_artifact(&source, filename).with_context(|| {
                format!("could not parse bootstrap artifact {}", path.display())
            })?;
            validate(&parsed, model_hint.as_ref())
        }
        (None, Some(ValidateCommand::Bundle(bundle))) => {
            if model_hint.is_some() {
                bail!("--model applies to a single configuration, not bundle validation");
            }
            validate_bundle(&bundle.directory).with_context(|| {
                format!(
                    "could not validate bundle at {}",
                    bundle.directory.display()
                )
            })?
        }
        (Some(_), Some(_)) => bail!("FILE and the `bundle` subcommand are mutually exclusive"),
        (None, None) => bail!("provide FILE or use `sep-rs validate bundle DIR`"),
    };
    print_diagnostics(&diagnostics, args.format)?;
    Ok(validation_exit_code(&diagnostics))
}

fn list_models(format: OutputFormat) -> Result<u8> {
    let supported = profiles();
    match format {
        OutputFormat::Json => {
            let models = supported
                .iter()
                .map(|profile| {
                    serde_json::json!({
                        "id": profile.id,
                        "display_name": profile.display_name,
                        "aliases": profile.aliases,
                        "model_id": profile.model_id,
                        "protocols": profile.protocols,
                        "dialects": profile.dialects,
                        "load_prefixes": profile.load_prefixes,
                    })
                })
                .collect::<Vec<_>>();
            println!("{}", serde_json::to_string_pretty(&models)?);
        }
        OutputFormat::Human => {
            println!("MODEL\tMODEL ID\tPROTOCOLS");
            for profile in supported {
                let protocols = profile
                    .protocols
                    .iter()
                    .map(|protocol| format!("{protocol:?}").to_lowercase())
                    .collect::<Vec<_>>()
                    .join(",");
                println!("{}\t{}\t{}", profile.id, profile.model_id, protocols);
            }
        }
    }
    Ok(EXIT_OK)
}

fn explore_model(args: &ExploreArgs) -> Result<u8> {
    let model = args
        .model
        .parse::<PhoneModelId>()
        .context("invalid model")?;
    let profile = resolve_profile(&model);
    let protocols = match (args.protocol, profile) {
        (Some(ProtocolArg::Sccp), _) => vec![Protocol::Sccp],
        (Some(ProtocolArg::Sip), _) => vec![Protocol::Sip],
        (None, Some(profile)) => profile.protocols.to_vec(),
        (None, None) => Protocol::ALL.to_vec(),
    };
    let output = ExploreOutput {
        requested_model: model.to_string(),
        resolved_model: profile.map(|profile| profile.id.to_owned()),
        options: protocols
            .into_iter()
            .map(|protocol| phone_options(&model, protocol))
            .collect(),
    };

    match args.format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&output)?),
        OutputFormat::Human => print_exploration(&output, profile)?,
    }
    Ok(EXIT_OK)
}

fn print_exploration(output: &ExploreOutput, profile: Option<&sep_rs::PhoneProfile>) -> Result<()> {
    match profile {
        Some(profile) => println!(
            "{} — {} (model ID {})",
            profile.id, profile.display_name, profile.model_id
        ),
        None => println!(
            "{} — unrecognized model; showing generic enterprise options",
            output.requested_model
        ),
    }

    for options in &output.options {
        println!();
        println!("{} — {} settings", options.protocol, options.settings.len());
        if !options.supported && profile.is_some() {
            println!("This protocol is not supported by the selected model.");
        }
        println!("PATH\tSECTION\tTITLE\tCONSTRAINTS");
        for setting in &options.settings {
            println!(
                "{}\t{}\t{}\t{}",
                setting.path,
                setting.section,
                setting.title,
                constraint_summary(&setting.constraint)?
            );
        }
    }
    Ok(())
}

fn constraint_summary(constraint: &SettingVariant) -> Result<String> {
    let mut parts = vec![constraint.value_kind.to_string()];
    if constraint.nullable {
        parts.push("nullable".to_owned());
    }
    if constraint.multiple {
        parts.push("multiple".to_owned());
    }
    if let Some(default) = &constraint.default {
        parts.push(format!("default={}", serde_json::to_string(default)?));
    }
    if !constraint.allowed_values.is_empty() {
        let choices = constraint
            .allowed_values
            .iter()
            .map(|allowed| {
                Ok(format!(
                    "{} ({})",
                    serde_json::to_string(&allowed.value)?,
                    allowed.label
                ))
            })
            .collect::<Result<Vec<_>>>()?
            .join(", ");
        parts.push(format!("choices=[{choices}]"));
    }
    match (constraint.minimum, constraint.maximum) {
        (Some(minimum), Some(maximum)) => parts.push(format!("range={minimum}..={maximum}")),
        (Some(minimum), None) => parts.push(format!("minimum={minimum}")),
        (None, Some(maximum)) => parts.push(format!("maximum={maximum}")),
        (None, None) => {}
    }
    if let Some(maximum) = constraint.maximum_characters {
        parts.push(format!("maximum_characters={maximum}"));
    }
    if let Some(pattern) = &constraint.pattern {
        parts.push(format!("pattern={}", serde_json::to_string(pattern)?));
    }
    if constraint.secret {
        parts.push("secret".to_owned());
    }
    Ok(parts.join("; "))
}

fn direct_device_spec(args: &DeviceArgs) -> Result<DeviceSpec> {
    let mac = args
        .mac
        .as_deref()
        .ok_or_else(|| {
            anyhow!("provide --manifest or the direct flags --mac, --model, --protocol, and --host")
        })?
        .parse::<MacAddress>()
        .context("invalid --mac")?;
    let model = args
        .model
        .as_deref()
        .ok_or_else(|| anyhow!("--model is required"))?
        .parse::<PhoneModelId>()
        .context("invalid --model")?;
    let protocol = args
        .protocol
        .ok_or_else(|| anyhow!("--protocol is required"))?;
    let host = args
        .host
        .as_deref()
        .ok_or_else(|| anyhow!("--host is required"))?
        .parse::<Host>()
        .context("invalid --host")?;
    let port = args.port.unwrap_or(match protocol {
        ProtocolArg::Sccp => 2000,
        ProtocolArg::Sip => 5060,
    });

    let protocol = match protocol {
        ProtocolArg::Sccp => ProtocolSpec::Sccp(SccpSpec::default()),
        ProtocolArg::Sip => {
            let user = args
                .sip_user
                .as_deref()
                .ok_or_else(|| anyhow!("--sip-user is required with --protocol sip"))?;
            ProtocolSpec::Sip(SipSpec {
                lines: vec![SipLine {
                    index: 1,
                    directory_number: user.to_owned(),
                    display_name: args.sip_display_name.clone(),
                    auth_name: Some(args.sip_auth_user.as_deref().unwrap_or(user).to_owned()),
                    auth_secret: args.sip_password.clone().map(Secret::new),
                }],
                ..SipSpec::default()
            })
        }
    };

    Ok(DeviceSpec {
        mac,
        model,
        firmware: args.firmware.clone(),
        protocol,
        endpoints: vec![CallControlEndpoint {
            host,
            port,
            priority: args.priority.unwrap_or(0),
            transport: Transport::Tcp,
        }],
        phone_label: None,
        time_zone: None,
        date_template: None,
        ntp_server: None,
        locale: None,
        services: ServiceUrls::default(),
        settings: Vec::new(),
        allow_unknown_settings: false,
    })
}

fn load_manifest<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let source = fs::read_to_string(path)
        .with_context(|| format!("could not read manifest {}", path.display()))?;
    match path.extension().and_then(|extension| extension.to_str()) {
        Some(extension) if extension.eq_ignore_ascii_case("json") => serde_json::from_str(&source)
            .with_context(|| format!("invalid JSON in {}", path.display())),
        Some(extension) if extension.eq_ignore_ascii_case("toml") => {
            toml::from_str(&source).with_context(|| format!("invalid TOML in {}", path.display()))
        }
        _ => bail!(
            "manifest {} must have a .toml or .json extension",
            path.display()
        ),
    }
}

fn write_bundle(directory: &Path, bundle: &BootstrapBundle, force: bool) -> Result<()> {
    let inventory_path = directory.join("bootstrap-manifest.json");
    ensure_available(&inventory_path, force)?;
    write_artifacts(directory, &bundle.artifacts, force)?;
    let inventory = serde_json::to_vec_pretty(&bundle.inventory)?;
    fs::write(&inventory_path, inventory)
        .with_context(|| format!("could not write {}", inventory_path.display()))?;
    Ok(())
}

fn write_artifacts(directory: &Path, artifacts: &[GeneratedArtifact], force: bool) -> Result<()> {
    let destinations = artifacts
        .iter()
        .map(|artifact| directory.join(&artifact.filename))
        .collect::<Vec<_>>();
    for path in &destinations {
        ensure_available(path, force)?;
    }

    fs::create_dir_all(directory)
        .with_context(|| format!("could not create output directory {}", directory.display()))?;
    for (artifact, path) in artifacts.iter().zip(destinations) {
        fs::write(&path, artifact.contents.as_bytes())
            .with_context(|| format!("could not write {}", path.display()))?;
    }
    Ok(())
}

fn ensure_available(path: &Path, force: bool) -> Result<()> {
    if path.exists() && !force {
        bail!(
            "refusing to overwrite {}; pass --force to replace it",
            path.display()
        );
    }
    Ok(())
}

fn print_diagnostics(diagnostics: &[Diagnostic], format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(diagnostics)?),
        OutputFormat::Human if diagnostics.is_empty() => println!("valid"),
        OutputFormat::Human => {
            for diagnostic in diagnostics {
                println!("{diagnostic}");
            }
        }
    }
    Ok(())
}

fn print_generation_warnings(artifacts: &[GeneratedArtifact]) {
    for artifact in artifacts {
        for warning in &artifact.warnings {
            eprintln!("{}: {warning}", artifact.filename);
        }
    }
}

fn validation_exit_code(diagnostics: &[Diagnostic]) -> u8 {
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
    {
        EXIT_INVALID
    } else {
        EXIT_OK
    }
}
