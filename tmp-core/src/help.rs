use command::Command;
use std::collections::{HashSet, VecDeque};

pub fn parse_recursive_help(binary: &str) -> Result<String, String> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    let mut merged = String::new();

    queue.push_back(Vec::<String>::new());
    visited.insert(Vec::<String>::new());

    let mut count = 0;

    while let Some(subcmds) = queue.pop_front() {
        if count >= 20 {
            break;
        }
        count += 1;

        let mut cmd = Command::new(binary);
        cmd.args(&subcmds);
        cmd.arg("--help");

        let output = match cmd.output() {
            Ok(out) => out,
            Err(e) => {
                if subcmds.is_empty() {
                    return Err(format!("Failed to run root command '{}': {}", binary, e));
                } else {
                    continue;
                }
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let help_text = if stdout.is_empty() { stderr } else { stdout };

        let cmd_name = if subcmds.is_empty() {
            binary.to_string()
        } else {
            format!("{} {}", binary, subcmds.join(" "))
        };

        if !merged.is_empty() {
            merged.push('\n');
        }
        merged.push_str(&format!("=== Help for: {} ===\n", cmd_name));
        merged.push_str(&help_text);
        if !help_text.ends_with('\n') {
            merged.push('\n');
        }

        if subcmds.len() < 2 {
            let parsed_subs = parse_subcommands_from_help(&help_text);
            for sub in parsed_subs {
                let mut next_subcmds = subcmds.clone();
                next_subcmds.push(sub);
                if !visited.contains(&next_subcmds) {
                    visited.insert(next_subcmds.clone());
                    queue.push_back(next_subcmds);
                }
            }
        }
    }

    Ok(merged)
}

fn parse_subcommands_from_help(help_text: &str) -> Vec<String> {
    let mut subcommands = Vec::new();
    let mut in_subcommands_section = false;

    for line in help_text.lines() {
        let trimmed = line.trim().to_lowercase();
        if in_subcommands_section {
            if line.is_empty() || line.trim().is_empty() {
                continue;
            }
            let starts_with_whitespace = line.starts_with(' ') || line.starts_with('\t');
            if !starts_with_whitespace {
                in_subcommands_section = false;
                if (trimmed.contains("commands") || trimmed.contains("subcommands"))
                    && (trimmed.ends_with(':')
                        || trimmed.contains("commands:")
                        || trimmed.contains("subcommands:"))
                {
                    in_subcommands_section = true;
                }
            } else {
                if let Some(first_word) = line.split_whitespace().next() {
                    if !first_word.starts_with('-')
                        && first_word
                            .chars()
                            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
                    {
                        subcommands.push(first_word.to_string());
                    }
                }
            }
        } else {
            if (trimmed.contains("commands") || trimmed.contains("subcommands"))
                && (trimmed.ends_with(':')
                    || trimmed.contains("commands:")
                    || trimmed.contains("subcommands:"))
            {
                in_subcommands_section = true;
            }
        }
    }
    subcommands
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn test_parse_subcommands_from_help() {
        let help = r#"
Usage: my-cli <COMMAND>

Commands:
  start     Start the service
  stop      Stop the service
  restart-all Restart all services

Options:
  -h, --help  Show help
"#;
        let parsed = parse_subcommands_from_help(help);
        assert_eq!(parsed, vec!["start", "stop", "restart-all"]);
    }

    #[test]
    fn test_recursive_help_parser_with_mock_bin() {
        let temp_dir = tempfile::tempdir().unwrap();
        let script_path = temp_dir.path().join("mock_cli");

        let script_content = r#"#!/bin/sh
case "$*" in
    *sub1*sub2*)
        echo "Help for sub1 sub2"
        ;;
    *sub1*)
        echo "Help for sub1"
        echo "Commands:"
        echo "  sub2   Inner subcommand"
        ;;
    *)
        echo "Help for root"
        echo "Commands:"
        echo "  sub1   First subcommand"
        echo "  sub3   Ignored sub due to max depth/visited logic"
        ;;
esac
"#;

        fs::write(&script_path, script_content).unwrap();

        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&script_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).unwrap();
        }

        let binary_str = script_path.to_str().unwrap();
        let result = parse_recursive_help(binary_str).unwrap();

        assert!(result.contains("=== Help for:"));
        assert!(result.contains("Help for root"));
        assert!(result.contains("Help for sub1"));
        assert!(result.contains("Help for sub1 sub2"));
    }

    #[test]
    fn test_recursive_help_nonexistent_binary() {
        let result = parse_recursive_help("this-binary-does-not-exist-at-all-12345");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to run root command"));
    }

    #[test]
    fn test_parse_subcommands_various_formats() {
        let help = r#"
Subcommands:
  foo-bar       Description 1
  baz_qux       Description 2
  invalid@cmd   Should be filtered out
  -options      Should be filtered out
  --help        Should be filtered out
"#;
        let parsed = parse_subcommands_from_help(help);
        assert_eq!(parsed, vec!["foo-bar", "baz_qux"]);
    }
}
