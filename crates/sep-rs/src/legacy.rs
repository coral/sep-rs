//! Codec for the colon/equal-delimited SIP configuration used by the 7940 and
//! 7960 firmware family.

use std::fmt::Write as _;

use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacySipConfig {
    pub entries: Vec<LegacyEntry>,
}

impl LegacySipConfig {
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.iter().rev().find_map(|entry| match entry {
            LegacyEntry::Assignment(assignment) if assignment.key.eq_ignore_ascii_case(key) => {
                Some(assignment.value.as_str())
            }
            _ => None,
        })
    }

    /// Serialize a stable, human-readable representation. Comments and unknown
    /// assignments remain in their original relative order.
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut output = String::new();
        for entry in &self.entries {
            match entry {
                LegacyEntry::Blank => output.push('\n'),
                LegacyEntry::Comment(comment) => {
                    output.push_str(comment);
                    output.push('\n');
                }
                LegacyEntry::Assignment(assignment) => {
                    output.push_str(&assignment.key);
                    output.push(assignment.separator.as_char());
                    output.push(' ');
                    if assignment.quoted {
                        output.push('"');
                        for character in assignment.value.chars() {
                            match character {
                                '\\' => output.push_str("\\\\"),
                                '"' => output.push_str("\\\""),
                                _ => output.push(character),
                            }
                        }
                        output.push('"');
                    } else {
                        output.push_str(&assignment.value);
                    }
                    if let Some(comment) = &assignment.trailing_comment {
                        output.push(' ');
                        output.push_str(comment);
                    }
                    output.push('\n');
                }
            }
        }
        output
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LegacyEntry {
    Assignment(LegacyAssignment),
    Comment(String),
    Blank,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyAssignment {
    pub key: String,
    pub separator: LegacySeparator,
    pub value: String,
    pub quoted: bool,
    pub trailing_comment: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacySeparator {
    Colon,
    Equals,
}

impl LegacySeparator {
    const fn as_char(self) -> char {
        match self {
            Self::Colon => ':',
            Self::Equals => '=',
        }
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum LegacyParseError {
    #[error("line {line}: expected a ':' or '=' assignment")]
    MissingSeparator { line: usize },
    #[error("line {line}: assignment key is empty")]
    EmptyKey { line: usize },
    #[error("line {line}: quoted value is not terminated")]
    UnterminatedQuote { line: usize },
    #[error("line {line}: unexpected characters after quoted value")]
    TrailingCharacters { line: usize },
}

/// Parse the legacy Cisco SIP format. Both separators are accepted per line;
/// blank values, quoted strings, whole-line comments and trailing comments are
/// retained.
pub fn parse_legacy(source: &str) -> Result<LegacySipConfig, LegacyParseError> {
    let mut entries = Vec::new();
    for (offset, raw_line) in source.lines().enumerate() {
        let line_number = offset + 1;
        let line = raw_line.trim();
        if line.is_empty() {
            entries.push(LegacyEntry::Blank);
            continue;
        }
        if line.starts_with('#') || line.starts_with(';') {
            entries.push(LegacyEntry::Comment(line.to_owned()));
            continue;
        }

        let Some((delimiter_index, delimiter)) = line
            .char_indices()
            .find(|(_, character)| matches!(character, ':' | '='))
        else {
            return Err(LegacyParseError::MissingSeparator { line: line_number });
        };

        let key = line[..delimiter_index].trim();
        if key.is_empty() {
            return Err(LegacyParseError::EmptyKey { line: line_number });
        }
        let separator = if delimiter == ':' {
            LegacySeparator::Colon
        } else {
            LegacySeparator::Equals
        };
        let rest = line[delimiter_index + delimiter.len_utf8()..].trim_start();
        let (value, quoted, trailing_comment) = parse_value(rest, line_number)?;
        entries.push(LegacyEntry::Assignment(LegacyAssignment {
            key: key.to_owned(),
            separator,
            value,
            quoted,
            trailing_comment,
        }));
    }
    // `str::lines` omits the final empty item. That is intentional: generation
    // always emits exactly one final newline rather than growing blank lines on
    // every parse/generate cycle.
    Ok(LegacySipConfig { entries })
}

fn parse_value(
    source: &str,
    line: usize,
) -> Result<(String, bool, Option<String>), LegacyParseError> {
    if !source.starts_with('"') {
        let (value, trailing) = split_unquoted_comment(source);
        return Ok((value.trim_end().to_owned(), false, trailing));
    }

    let mut value = String::new();
    let mut escaped = false;
    let mut closing_quote = None;
    for (index, character) in source[1..].char_indices() {
        if escaped {
            value.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            closing_quote = Some(index + 1);
            break;
        } else {
            value.push(character);
        }
    }
    if escaped {
        value.push('\\');
    }
    let Some(closing_quote) = closing_quote else {
        return Err(LegacyParseError::UnterminatedQuote { line });
    };
    let remainder = source[closing_quote + 1..].trim();
    let trailing_comment = if remainder.is_empty() {
        None
    } else if remainder.starts_with('#') || remainder.starts_with(';') {
        Some(remainder.to_owned())
    } else {
        return Err(LegacyParseError::TrailingCharacters { line });
    };
    Ok((value, true, trailing_comment))
}

fn split_unquoted_comment(source: &str) -> (&str, Option<String>) {
    let comment_start = source.char_indices().find_map(|(index, character)| {
        if matches!(character, '#' | ';')
            && (index == 0
                || source[..index]
                    .chars()
                    .next_back()
                    .is_some_and(char::is_whitespace))
        {
            Some(index)
        } else {
            None
        }
    });
    comment_start.map_or((source, None), |index| {
        (&source[..index], Some(source[index..].trim().to_owned()))
    })
}

pub(crate) fn assignment(key: impl Into<String>, value: impl Into<String>) -> LegacyEntry {
    LegacyEntry::Assignment(LegacyAssignment {
        key: key.into(),
        separator: LegacySeparator::Colon,
        value: value.into(),
        quoted: true,
        trailing_comment: None,
    })
}

pub(crate) fn section(entries: &mut Vec<LegacyEntry>, title: &str) {
    if !entries.is_empty() {
        entries.push(LegacyEntry::Blank);
    }
    let mut comment = String::from("# ");
    let _ = write!(comment, "{title}");
    entries.push(LegacyEntry::Comment(comment));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mixed_separators_quotes_blanks_and_numbered_keys() {
        let source = concat!(
            "# device\n",
            "proxy1_address: \"pbx.example.test\"\n",
            "proxy1_port=5060\n",
            "\n",
            "line1_name: \"1001\" # primary\n",
            "line2_name: \"\"\n",
        );
        let parsed = parse_legacy(source).expect("legacy config should parse");

        assert_eq!(parsed.get("proxy1_port"), Some("5060"));
        assert_eq!(parsed.get("line1_name"), Some("1001"));
        assert_eq!(parsed.get("line2_name"), Some(""));
        assert!(parsed.to_text().contains("line1_name: \"1001\" # primary"));
    }

    #[test]
    fn rejects_an_unterminated_quote() {
        assert_eq!(
            parse_legacy("line1_name: \"1001"),
            Err(LegacyParseError::UnterminatedQuote { line: 1 })
        );
    }
}
