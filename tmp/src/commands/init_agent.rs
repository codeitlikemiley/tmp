use std::fs;
use std::path::Path;

pub fn run(agent: &str) -> Result<(), Box<dyn std::error::Error>> {
    let project_dir = std::env::current_dir()?;

    let supported = [
        "claude",
        "cursor",
        "codex",
        "antigravity",
        "copilot",
        "windsurf",
        "all",
    ];

    if !supported.contains(&agent) {
        return Err(format!(
            "Unsupported agent: {}. Supported agents are 'claude', 'cursor', 'codex', 'antigravity', 'copilot', 'windsurf', or 'all'.",
            agent
        )
        .into());
    }

    let default_claude_content = "# Claude Agent Instructions\n\n## Build Command\ncargo build\n\n## Test Command\ncargo test\n\n## Resolve Commands\nBefore running any unknown command, use `tmp resolve \"<command_intent>\"` to find the correct command.\n";
    let default_cursor_content = "# Cursor Rules\n\nBefore running any unknown command, use `tmp resolve \"<command_intent>\"` to find the correct command.\n";
    let default_agents_content = "# AGENTS.md\n\nBefore running any unknown command, use `tmp resolve \"<command_intent>\"` to find the correct command.\n";
    let default_copilot_content = "# Copilot Instructions\n\nBefore running any unknown command, use `tmp resolve \"<command_intent>\"` to find the correct command.\n";
    let default_windsurf_content = "# Windsurf Rules\n\nBefore running any unknown command, use `tmp resolve \"<command_intent>\"` to find the correct command.\n";

    if agent == "claude" || agent == "all" {
        let claude_path = project_dir.join("CLAUDE.md");
        write_or_merge_rules(&claude_path, default_claude_content)?;
    }

    if agent == "cursor" || agent == "all" {
        let cursor_dir = project_dir.join(".cursor").join("rules");
        fs::create_dir_all(&cursor_dir)?;
        let cursor_path = cursor_dir.join("tmp.mdc");
        write_or_merge_rules(&cursor_path, default_cursor_content)?;
    }

    if agent == "codex" || agent == "antigravity" || agent == "all" {
        let agents_path = project_dir.join("AGENTS.md");
        write_or_merge_rules(&agents_path, default_agents_content)?;
    }

    if agent == "copilot" || agent == "all" {
        let github_dir = project_dir.join(".github");
        fs::create_dir_all(&github_dir)?;
        let copilot_path = github_dir.join("copilot-instructions.md");
        write_or_merge_rules(&copilot_path, default_copilot_content)?;
    }

    if agent == "windsurf" || agent == "all" {
        let windsurf_path = project_dir.join(".windsurfrules");
        write_or_merge_rules(&windsurf_path, default_windsurf_content)?;
    }

    println!("Initialized agent bridge instructions for: {}", agent);
    Ok(())
}

fn write_or_merge_rules(
    path: &Path,
    default_content: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() {
        let mut content = fs::read_to_string(path)?;
        if content.trim().is_empty() {
            fs::write(path, default_content)?;
        } else if !content.contains("tmp resolve") {
            content.push_str("\n\n## Resolve Commands\nBefore running any unknown command, use `tmp resolve \"<command_intent>\"` to find the correct command.\n");
            fs::write(path, content)?;
        }
    } else {
        fs::write(path, default_content)?;
    }
    Ok(())
}
