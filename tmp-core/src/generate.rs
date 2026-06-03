use crate::schema::{Command, Schema, SchemaMeta, Token, TokenType};
use std::collections::BTreeSet;

struct HelpSection {
    command: String,
    body: String,
}

struct SubcommandEntry {
    name: String,
    description: String,
}

pub fn generate_schema_from_help(tool: &str, help_text: &str) -> Schema {
    let sections = parse_help_sections(tool, help_text);
    let explicit_commands = sections
        .iter()
        .map(|section| section.command.clone())
        .collect::<BTreeSet<_>>();

    let mut seen = BTreeSet::new();
    let mut commands = Vec::new();

    for section in &sections {
        if seen.insert(section.command.clone()) {
            commands.push(Command {
                command: section.command.clone(),
                description: description_for_section(&section.body, &section.command),
                group: "help".to_string(),
                verified: false,
                tokens: parse_options(&section.body),
            });
        }
    }

    for section in &sections {
        for subcommand in parse_subcommands(&section.body) {
            let command = format!("{} {}", section.command, subcommand.name);
            if explicit_commands.contains(&command) || !seen.insert(command.clone()) {
                continue;
            }

            commands.push(Command {
                command,
                description: subcommand.description,
                group: "help".to_string(),
                verified: false,
                tokens: Vec::new(),
            });
        }
    }

    Schema {
        meta: SchemaMeta {
            tool: tool.to_string(),
            version: 1,
            author: None,
            generated_by: Some("tmp generate".to_string()),
            generated_with: Some("help-text-draft".to_string()),
            verified: false,
            verified_at: None,
            coverage: Some("draft-from-help-text".to_string()),
            waz_version: None,
            requires_file: None,
            requires_file_kind: None,
            requires_binary: None,
            keywords: Vec::new(),
        },
        commands,
    }
}

fn parse_help_sections(tool: &str, help_text: &str) -> Vec<HelpSection> {
    let mut sections = Vec::new();
    let mut current_command = tool.to_string();
    let mut current_body = String::new();
    let mut saw_header = false;

    for line in help_text.lines() {
        if let Some(command) = parse_help_header(line) {
            if saw_header || !current_body.trim().is_empty() {
                sections.push(HelpSection {
                    command: current_command,
                    body: current_body,
                });
                current_body = String::new();
            }
            current_command = command;
            saw_header = true;
        } else {
            current_body.push_str(line);
            current_body.push('\n');
        }
    }

    if saw_header || !current_body.trim().is_empty() {
        sections.push(HelpSection {
            command: current_command,
            body: current_body,
        });
    }

    if sections.is_empty() {
        sections.push(HelpSection {
            command: tool.to_string(),
            body: String::new(),
        });
    }

    sections
}

fn parse_help_header(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let command = trimmed
        .strip_prefix("=== Help for: ")?
        .strip_suffix(" ===")?
        .trim();

    if command.is_empty() {
        None
    } else {
        Some(command.to_string())
    }
}

fn description_for_section(body: &str, command: &str) -> String {
    body.lines()
        .map(str::trim)
        .find(|line| {
            !line.is_empty()
                && !is_section_header(line)
                && !line.eq_ignore_ascii_case("commands:")
                && !line.eq_ignore_ascii_case("subcommands:")
                && !line.eq_ignore_ascii_case("options:")
                && !line.eq_ignore_ascii_case("flags:")
        })
        .map(str::to_string)
        .unwrap_or_else(|| format!("Draft command parsed from help for `{command}`"))
}

fn parse_subcommands(help_text: &str) -> Vec<SubcommandEntry> {
    let mut subcommands = Vec::new();
    let mut in_commands_section = false;

    for line in help_text.lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();

        if in_commands_section {
            if trimmed.is_empty() {
                continue;
            }

            if !starts_with_whitespace(line) {
                if is_commands_header(&lower) {
                    in_commands_section = true;
                    continue;
                }
                in_commands_section = false;
                continue;
            }

            let mut parts = trimmed.splitn(2, char::is_whitespace);
            let Some(name) = parts.next() else {
                continue;
            };
            if !is_identifier_like(name) || name.starts_with('-') {
                continue;
            }

            let description = parts.next().map(str::trim).unwrap_or_default();
            subcommands.push(SubcommandEntry {
                name: name.to_string(),
                description: if description.is_empty() {
                    format!("Draft subcommand parsed from help for `{name}`")
                } else {
                    description.to_string()
                },
            });
        } else if is_commands_header(&lower) {
            in_commands_section = true;
        }
    }

    subcommands
}

fn parse_options(help_text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut seen = BTreeSet::new();
    let mut in_options_section = false;

    for line in help_text.lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();

        if in_options_section {
            if trimmed.is_empty() {
                continue;
            }

            if !starts_with_whitespace(line) {
                if is_options_header(&lower) {
                    in_options_section = true;
                    continue;
                }
                in_options_section = false;
                continue;
            }

            let Some(token) = parse_option_line(trimmed) else {
                continue;
            };
            if seen.insert(token.name.clone()) {
                tokens.push(token);
            }
        } else if is_options_header(&lower) {
            in_options_section = true;
        }
    }

    tokens
}

fn parse_option_line(trimmed: &str) -> Option<Token> {
    if !trimmed.starts_with('-') {
        return None;
    }

    let split_at = find_description_split(trimmed).unwrap_or(trimmed.len());
    let spec = trimmed[..split_at].trim();
    let description = trimmed[split_at..].trim();
    let flag_spec = spec
        .split(',')
        .map(str::trim)
        .find(|part| part.starts_with("--"))
        .or_else(|| {
            spec.split(',')
                .map(str::trim)
                .find(|part| part.starts_with('-'))
        })?;

    let flag = flag_spec.split_whitespace().next()?;
    let name = normalize_token_name(flag)?;
    let takes_value = spec.contains('<')
        || spec.contains("=<")
        || spec.contains(" [")
        || spec
            .split_whitespace()
            .skip(1)
            .any(|part| !part.starts_with('-') && part.chars().any(|c| c.is_ascii_uppercase()));

    Some(Token {
        name,
        description: if description.is_empty() {
            format!("Draft option parsed from `{flag}`")
        } else {
            description.to_string()
        },
        required: false,
        token_type: if takes_value {
            TokenType::String
        } else {
            TokenType::Boolean
        },
        default: None,
        values: None,
        flag: Some(flag.to_string()),
        data_source: None,
    })
}

fn find_description_split(line: &str) -> Option<usize> {
    line.as_bytes()
        .windows(2)
        .position(|window| window == b"  ")
}

fn normalize_token_name(flag: &str) -> Option<String> {
    let normalized = flag
        .trim_start_matches('-')
        .trim()
        .replace('-', "_")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect::<String>();

    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn starts_with_whitespace(line: &str) -> bool {
    line.starts_with(' ') || line.starts_with('\t')
}

fn is_commands_header(lower: &str) -> bool {
    (lower.contains("commands") || lower.contains("subcommands"))
        && (lower.ends_with(':') || lower.contains("commands:") || lower.contains("subcommands:"))
}

fn is_options_header(lower: &str) -> bool {
    (lower.contains("options") || lower.contains("flags"))
        && (lower.ends_with(':') || lower.contains("options:") || lower.contains("flags:"))
}

fn is_section_header(line: &str) -> bool {
    let lower = line.to_lowercase();
    is_commands_header(&lower) || is_options_header(&lower)
}

fn is_identifier_like(value: &str) -> bool {
    value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_schema_from_help_creates_unverified_draft_commands() {
        let help = r#"
Usage: custom-tool <COMMAND>

Commands:
  run      Run the custom tool

Options:
  -v, --verbose       Enable verbose output
  --config <PATH>     Path to config file
"#;

        let schema = generate_schema_from_help("custom-tool", help);

        assert_eq!(schema.meta.tool, "custom-tool");
        assert!(!schema.meta.verified);
        assert!(schema
            .commands
            .iter()
            .any(|cmd| cmd.command == "custom-tool"));
        assert!(schema
            .commands
            .iter()
            .any(|cmd| cmd.command == "custom-tool run"));

        let root = schema
            .commands
            .iter()
            .find(|cmd| cmd.command == "custom-tool")
            .unwrap();
        assert!(root.tokens.iter().any(|token| token.name == "verbose"));
        assert!(root.tokens.iter().any(|token| token.name == "config"));
    }

    #[test]
    fn generate_schema_from_recursive_help_prefers_explicit_subcommand_sections() {
        let help = r#"=== Help for: tool ===
Usage: tool <COMMAND>
Commands:
  run    Run the tool
=== Help for: tool run ===
Usage: tool run [OPTIONS]
Options:
  --dry-run    Preview command
"#;

        let schema = generate_schema_from_help("tool", help);
        let run_commands = schema
            .commands
            .iter()
            .filter(|cmd| cmd.command == "tool run")
            .count();
        assert_eq!(run_commands, 1);
        assert!(schema
            .commands
            .iter()
            .find(|cmd| cmd.command == "tool run")
            .unwrap()
            .tokens
            .iter()
            .any(|token| token.name == "dry_run"));
    }
}
