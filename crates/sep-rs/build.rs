use std::collections::{HashMap, HashSet};
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

const CATALOG_PATH: &str = "data/sep-settings.json";
const OPTIONS_DEFINITIONS_PATH: &str = "data/options-definitions.json";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    schema_version: u32,
    settings: Vec<SettingDefinition>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingDefinition {
    path: String,
    name: String,
    title: String,
    section: String,
    variants: Vec<SettingVariant>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingVariant {
    value_kind: ValueKind,
    #[serde(default)]
    nullable: bool,
    #[serde(default)]
    default: Option<SettingValue>,
    #[serde(default)]
    allowed_values: Vec<AllowedValue>,
    #[serde(default)]
    minimum: Option<i64>,
    #[serde(default)]
    maximum: Option<i64>,
    #[serde(default)]
    maximum_characters: Option<usize>,
    #[serde(default)]
    pattern: Option<String>,
    #[serde(default)]
    secret: bool,
    #[serde(default)]
    multiple: bool,
    #[serde(default)]
    selectors: Vec<Selector>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ValueKind {
    Boolean,
    Integer,
    String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(untagged)]
enum SettingValue {
    Boolean(bool),
    Integer(i64),
    String(String),
    List(Vec<Self>),
    Null,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AllowedValue {
    value: SettingValue,
    label: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selector {
    model: String,
    protocol: Protocol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Protocol {
    Sccp,
    Sip,
}

fn main() {
    println!("cargo::rerun-if-changed={CATALOG_PATH}");
    println!("cargo::rerun-if-changed={OPTIONS_DEFINITIONS_PATH}");

    let source = fs::read_to_string(CATALOG_PATH).expect("read SEP settings catalog");
    let catalog: Catalog = serde_json::from_str(&source).expect("decode SEP settings catalog");
    validate_catalog(&catalog).expect("validate SEP settings catalog");
    let definitions_source =
        fs::read_to_string(OPTIONS_DEFINITIONS_PATH).expect("read options definitions");
    let definitions: serde_json::Value =
        serde_json::from_str(&definitions_source).expect("decode options definitions");
    validate_schema_definitions(&definitions).expect("validate options definitions");
    let output_directory = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set"));
    fs::write(
        output_directory.join("sep_settings_catalog.rs"),
        render_catalog(&catalog),
    )
    .expect("write generated Rust settings catalog");

    let generated_definitions = format!(
        "fn generated_schema_definitions() -> SchemaValue {{\n    {}\n}}\n",
        render_json_value(&definitions)
    );
    fs::write(
        output_directory.join("options_schema.rs"),
        generated_definitions,
    )
    .expect("write generated Rust options schema");
}

fn validate_schema_definitions(definitions: &serde_json::Value) -> Result<(), String> {
    let definitions = definitions
        .as_object()
        .filter(|definitions| !definitions.is_empty())
        .ok_or_else(|| "options definitions must be a non-empty object".to_owned())?;
    for name in [
        "DeviceSpec",
        "DefaultSpec",
        "BundleSpec",
        "ArtifactValidationRequest",
        "BundleValidationRequest",
    ] {
        if !definitions
            .get(name)
            .is_some_and(serde_json::Value::is_object)
        {
            return Err(format!("missing options schema definition `{name}`"));
        }
    }

    for definition in definitions.values() {
        validate_references(definition, definitions)?;
    }
    Ok(())
}

fn validate_references(
    value: &serde_json::Value,
    definitions: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                validate_references(value, definitions)?;
            }
        }
        serde_json::Value::Object(values) => {
            if let Some(reference) = values.get("$ref").and_then(serde_json::Value::as_str) {
                let name = reference
                    .strip_prefix("#/$defs/")
                    .ok_or_else(|| format!("unsupported options schema reference `{reference}`"))?;
                if !definitions.contains_key(name) {
                    return Err(format!("missing options schema definition `{name}`"));
                }
            }
            for value in values.values() {
                validate_references(value, definitions)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_catalog(catalog: &Catalog) -> Result<(), String> {
    if catalog.schema_version != 1 {
        return Err(format!(
            "unsupported schema version {}",
            catalog.schema_version
        ));
    }
    if catalog.settings.is_empty() {
        return Err("catalog must contain at least one setting".to_owned());
    }

    let mut paths = HashSet::new();
    for setting in &catalog.settings {
        if !paths.insert(setting.path.as_str()) {
            return Err(format!("duplicate setting path `{}`", setting.path));
        }
        validate_catalog_path(setting)?;
        if setting.title.trim().is_empty() || setting.title.chars().any(char::is_control) {
            return Err(format!("`{}` has an invalid title", setting.path));
        }
        if setting.section.trim().is_empty() || setting.section.chars().any(char::is_control) {
            return Err(format!("`{}` has an invalid section", setting.path));
        }
        if setting.variants.is_empty() {
            return Err(format!("`{}` has no variants", setting.path));
        }

        let mut selector_owners = HashMap::new();
        let mut common_variant = None;
        for (variant_index, variant) in setting.variants.iter().enumerate() {
            if variant.selectors.is_empty()
                && let Some(previous) = common_variant.replace(variant_index)
            {
                return Err(format!(
                    "`{}` has common variants at indexes {previous} and {variant_index}",
                    setting.path
                ));
            }
            for selector in &variant.selectors {
                if selector.model.trim().is_empty() || selector.model.chars().any(char::is_control)
                {
                    return Err(format!(
                        "`{}` variant {variant_index} has an invalid model selector",
                        setting.path
                    ));
                }
                let key = (selector.model.as_str(), selector.protocol);
                if let Some(previous) = selector_owners.insert(key, variant_index) {
                    return Err(format!(
                        "`{}` selector `{}:{:?}` occurs in variants {previous} and {variant_index}",
                        setting.path, selector.model, selector.protocol
                    ));
                }
            }
            validate_variant(setting, variant_index, variant)?;
        }
    }
    Ok(())
}

fn validate_catalog_path(setting: &SettingDefinition) -> Result<(), String> {
    let Some(path) = setting.path.strip_prefix("/device/") else {
        return Err(format!("invalid setting path `{}`", setting.path));
    };
    let segments = path.split('/').collect::<Vec<_>>();
    if segments.iter().any(|segment| segment.is_empty()) {
        return Err(format!("invalid setting path `{}`", setting.path));
    }
    for (index, segment) in segments.iter().enumerate() {
        let is_last = index + 1 == segments.len();
        if let Some(attribute) = segment.strip_prefix('@') {
            if !is_last || !valid_xml_name(attribute) {
                return Err(format!("invalid setting path `{}`", setting.path));
            }
            continue;
        }
        let name = segment.strip_suffix("[*]").unwrap_or(segment);
        if !valid_xml_name(name) {
            return Err(format!("invalid setting path `{}`", setting.path));
        }
    }

    let leaf = segments
        .last()
        .expect("a catalog path has at least one segment");
    let leaf = leaf.strip_prefix('@').unwrap_or(leaf);
    let leaf = leaf.strip_suffix("[*]").unwrap_or(leaf);
    if setting.name != leaf {
        return Err(format!(
            "`{}` has name `{}` instead of `{leaf}`",
            setting.path, setting.name
        ));
    }
    Ok(())
}

fn validate_variant(
    setting: &SettingDefinition,
    variant_index: usize,
    variant: &SettingVariant,
) -> Result<(), String> {
    let location = format!("`{}` variant {variant_index}", setting.path);
    if variant.minimum.is_some() || variant.maximum.is_some() {
        if !matches!(variant.value_kind, ValueKind::Integer) {
            return Err(format!(
                "{location} applies an integer range to another type"
            ));
        }
        if variant
            .minimum
            .zip(variant.maximum)
            .is_some_and(|(minimum, maximum)| minimum > maximum)
        {
            return Err(format!("{location} has an inverted integer range"));
        }
    }
    if variant.maximum_characters.is_some() && !matches!(variant.value_kind, ValueKind::String) {
        return Err(format!(
            "{location} applies a string length to another type"
        ));
    }
    let pattern = if let Some(pattern) = &variant.pattern {
        if !matches!(variant.value_kind, ValueKind::String) {
            return Err(format!(
                "{location} applies a string pattern to another type"
            ));
        }
        Some(
            regex::Regex::new(&format!("^(?:{pattern})$"))
                .map_err(|error| format!("{location} has an invalid pattern: {error}"))?,
        )
    } else {
        None
    };

    let mut allowed_values = HashSet::new();
    for allowed in &variant.allowed_values {
        if !scalar_matches_kind(&allowed.value, variant.value_kind) {
            return Err(format!("{location} has an allowed value of the wrong type"));
        }
        if allowed.label.trim().is_empty() || allowed.label.chars().any(char::is_control) {
            return Err(format!("{location} has an invalid allowed-value label"));
        }
        if !allowed_values.insert(&allowed.value) {
            return Err(format!("{location} has a duplicate allowed value"));
        }
    }
    if let Some(default) = &variant.default {
        validate_default(default, variant, pattern.as_ref(), &location)?;
    }
    Ok(())
}

fn validate_default(
    value: &SettingValue,
    variant: &SettingVariant,
    pattern: Option<&regex::Regex>,
    location: &str,
) -> Result<(), String> {
    if matches!(value, SettingValue::Null) {
        return if variant.nullable {
            Ok(())
        } else {
            Err(format!("{location} has a null default but is not nullable"))
        };
    }

    let values = match (variant.multiple, value) {
        (true, SettingValue::List(values)) => values.as_slice(),
        (true, _) => return Err(format!("{location} has a scalar default for a list")),
        (false, SettingValue::List(_)) => {
            return Err(format!("{location} has a list default for a scalar"));
        }
        (false, value) => std::slice::from_ref(value),
    };
    for value in values {
        if !scalar_matches_kind(value, variant.value_kind) {
            return Err(format!("{location} has a default of the wrong type"));
        }
        if !variant.allowed_values.is_empty()
            && !variant
                .allowed_values
                .iter()
                .any(|allowed| allowed.value == *value)
        {
            return Err(format!(
                "{location} has a default outside its allowed values"
            ));
        }
        match value {
            SettingValue::Integer(value)
                if variant.minimum.is_some_and(|minimum| *value < minimum)
                    || variant.maximum.is_some_and(|maximum| *value > maximum) =>
            {
                return Err(format!(
                    "{location} has a default outside its integer range"
                ));
            }
            SettingValue::String(value)
                if variant
                    .maximum_characters
                    .is_some_and(|maximum| value.chars().count() > maximum) =>
            {
                return Err(format!("{location} has a default longer than its maximum"));
            }
            SettingValue::String(value)
                if pattern.is_some_and(|pattern| !pattern.is_match(value)) =>
            {
                return Err(format!(
                    "{location} has a default that does not match its pattern"
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

const fn scalar_matches_kind(value: &SettingValue, kind: ValueKind) -> bool {
    matches!(
        (kind, value),
        (ValueKind::Boolean, SettingValue::Boolean(_))
            | (ValueKind::Integer, SettingValue::Integer(_))
            | (ValueKind::String, SettingValue::String(_))
    )
}

fn valid_xml_name(name: &str) -> bool {
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && characters.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
        })
}

fn render_catalog(catalog: &Catalog) -> String {
    let mut rust = String::from(
        "#[allow(clippy::too_many_lines, reason = \"generated directly from the SEP settings catalog\")]\n\
         fn generated_sep_settings_catalog() -> SepSettingsCatalog {\n\
         \x20   SepSettingsCatalog {\n",
    );
    writeln!(rust, "        schema_version: {},", catalog.schema_version).expect("write to string");
    rust.push_str("        settings: vec![\n");
    for definition in &catalog.settings {
        rust.push_str("            SepSettingDefinition {\n");
        push_string_field(&mut rust, 16, "path", &definition.path);
        push_string_field(&mut rust, 16, "name", &definition.name);
        push_string_field(&mut rust, 16, "title", &definition.title);
        push_string_field(&mut rust, 16, "section", &definition.section);
        rust.push_str("                variants: vec![\n");
        for variant in &definition.variants {
            render_variant(&mut rust, variant);
        }
        rust.push_str("                ],\n            },\n");
    }
    rust.push_str("        ],\n    }\n}\n");
    rust
}

fn render_variant(rust: &mut String, variant: &SettingVariant) {
    rust.push_str("                    SettingVariant {\n");
    let kind = match variant.value_kind {
        ValueKind::Boolean => "SettingValueKind::Boolean",
        ValueKind::Integer => "SettingValueKind::Integer",
        ValueKind::String => "SettingValueKind::String",
    };
    writeln!(rust, "                        value_kind: {kind},").expect("write to string");
    writeln!(
        rust,
        "                        nullable: {},",
        variant.nullable
    )
    .expect("write to string");
    writeln!(
        rust,
        "                        default: {},",
        render_optional_value(variant.default.as_ref())
    )
    .expect("write to string");
    rust.push_str("                        allowed_values: vec![\n");
    for allowed in &variant.allowed_values {
        writeln!(
            rust,
            "                            SettingAllowedValue {{ value: {}, label: String::from({:?}) }},",
            render_value(&allowed.value),
            allowed.label
        )
        .expect("write to string");
    }
    rust.push_str("                        ],\n");
    push_option(rust, "minimum", variant.minimum);
    push_option(rust, "maximum", variant.maximum);
    push_option(rust, "maximum_characters", variant.maximum_characters);
    writeln!(
        rust,
        "                        pattern: {},",
        render_optional_string(variant.pattern.as_deref())
    )
    .expect("write to string");
    writeln!(rust, "                        secret: {},", variant.secret).expect("write to string");
    writeln!(
        rust,
        "                        multiple: {},",
        variant.multiple
    )
    .expect("write to string");
    rust.push_str("                        selectors: vec![\n");
    for selector in &variant.selectors {
        let protocol = match selector.protocol {
            Protocol::Sccp => "Protocol::Sccp",
            Protocol::Sip => "Protocol::Sip",
        };
        writeln!(
            rust,
            "                            SettingSelector {{ model: String::from({:?}), protocol: {protocol} }},",
            selector.model
        )
        .expect("write to string");
    }
    rust.push_str("                        ],\n                    },\n");
}

fn push_string_field(rust: &mut String, indentation: usize, name: &str, value: &str) {
    writeln!(rust, "{:indentation$}{name}: String::from({value:?}),", "").expect("write to string");
}

fn push_option<T: std::fmt::Display>(rust: &mut String, name: &str, value: Option<T>) {
    let value = value.map_or_else(|| "None".to_owned(), |value| format!("Some({value})"));
    writeln!(rust, "                        {name}: {value},").expect("write to string");
}

fn render_optional_string(value: Option<&str>) -> String {
    value.map_or_else(
        || "None".to_owned(),
        |value| format!("Some(String::from({value:?}))"),
    )
}

fn render_optional_value(value: Option<&SettingValue>) -> String {
    value.map_or_else(
        || "None".to_owned(),
        |value| format!("Some({})", render_value(value)),
    )
}

fn render_value(value: &SettingValue) -> String {
    match value {
        SettingValue::Boolean(value) => format!("SepSettingValue::Boolean({value})"),
        SettingValue::Integer(value) => format!("SepSettingValue::Integer({value})"),
        SettingValue::String(value) => {
            format!("SepSettingValue::String(String::from({value:?}))")
        }
        SettingValue::List(values) => format!(
            "SepSettingValue::List(vec![{}])",
            values
                .iter()
                .map(render_value)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        SettingValue::Null => "SepSettingValue::Null".to_owned(),
    }
}

fn render_json_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "SchemaValue::Null".to_owned(),
        serde_json::Value::Bool(value) => format!("SchemaValue::Boolean({value})"),
        serde_json::Value::Number(value) => {
            let value = value
                .as_i64()
                .expect("options schema numbers must fit in an i64");
            format!("SchemaValue::Integer({})", readable_integer(value))
        }
        serde_json::Value::String(value) => {
            format!("SchemaValue::String(String::from({value:?}))")
        }
        serde_json::Value::Array(values) => format!(
            "SchemaValue::Array(vec![{}])",
            values
                .iter()
                .map(render_json_value)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        serde_json::Value::Object(values) if values.is_empty() => {
            "SchemaValue::Object(std::collections::BTreeMap::new())".to_owned()
        }
        serde_json::Value::Object(values) => format!(
            "SchemaValue::Object(std::collections::BTreeMap::from([{}]))",
            values
                .iter()
                .map(|(key, value)| format!(
                    "(String::from({key:?}), {})",
                    render_json_value(value)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn readable_integer(value: i64) -> String {
    let raw = value.to_string();
    let (sign, digits) = raw
        .strip_prefix('-')
        .map_or(("", raw.as_str()), |digits| ("-", digits));
    let mut output = String::with_capacity(raw.len() + raw.len() / 3);
    output.push_str(sign);
    for (index, character) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            output.push('_');
        }
        output.push(character);
    }
    output
}
