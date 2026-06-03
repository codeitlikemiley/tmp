use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use tmp_core::config::{default_config_path, load_config};
use tmp_core::context::Context;
use tmp_core::help::parse_recursive_help;
use tmp_core::llm::LlmDispatcher;
use tmp_core::versioning::{
    generate_diff, get_history, get_latest_version, load_schema, rollback, save_schema,
};

#[allow(clippy::too_many_arguments)]
pub fn run(
    tool: &str,
    custom_config_path: Option<&str>,
    help_text_opt: Option<&str>,
    provider_opt: Option<&str>,
    model_opt: Option<&str>,
    rollback_opt: Option<u32>,
    history: bool,
    verify: bool,
    non_interactive: bool,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Input validation on tool name
    if tool.is_empty()
        || !tool
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return Err("Invalid tool name".into());
    }

    // 2. Load config
    let config_file_path = match custom_config_path {
        Some(p) => PathBuf::from(p),
        None => match default_config_path() {
            Some(p) => p,
            None => return Err("Could not determine default config directory.".into()),
        },
    };
    let config = load_config(Some(&config_file_path))?;

    // 3. Handle history tracking
    if history {
        let hist = get_history(tool).map_err(|e| format!("History error: {e}"))?;
        if hist.is_empty() {
            return Err(format!("No history found for tool: {tool}").into());
        }
        for (ver, modified) in hist {
            let duration = modified
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default();
            println!("Version: {ver}, Modified: {}", duration.as_secs());
        }
        return Ok(());
    }

    // 4. Handle rollback
    if let Some(target_ver) = rollback_opt {
        rollback(tool, target_ver).map_err(|e| format!("Rollback failed: {e}"))?;
        println!("Rolled back schema for {tool} to version {target_ver}");
        return Ok(());
    }

    // 5. Gather help text
    let help_content = if let Some(path_str) = help_text_opt {
        let path = PathBuf::from(path_str);
        if path.is_file() {
            fs::read_to_string(&path)?
        } else if path.is_dir() {
            let bin_to_run = if path.join(tool).is_file() {
                path.join(tool).to_string_lossy().into_owned()
            } else {
                tool.to_string()
            };
            parse_recursive_help(&bin_to_run).map_err(|e| format!("Recursive help failed: {e}"))?
        } else {
            fs::read_to_string(&path)
                .unwrap_or_else(|_| parse_recursive_help(path_str).unwrap_or_default())
        }
    } else {
        parse_recursive_help(tool).map_err(|e| format!("Recursive help failed: {e}"))?
    };

    // 6. Generate schema via LLM
    let mut dispatcher = LlmDispatcher::new(config);
    let mut new_schema = dispatcher
        .generate_schema(tool, &help_content, provider_opt, model_opt)
        .map_err(|e| format!("LLM generation failed: {e}"))?;

    // 7. Determine version and perform diff check
    let latest_ver = get_latest_version(tool).unwrap_or(None);
    let old_schema_str = if let Some(ver) = latest_ver {
        let mut old_schema = load_schema(tool, ver)?;
        new_schema.meta.version = ver + 1;
        old_schema.meta.version = ver + 1;
        serde_json::to_string_pretty(&old_schema).unwrap_or_default()
    } else {
        new_schema.meta.version = 1;
        String::new()
    };

    let new_schema_str = serde_json::to_string_pretty(&new_schema).unwrap_or_default();
    let mut is_different = true;
    if !old_schema_str.is_empty() {
        if old_schema_str == new_schema_str {
            is_different = false;
            if !force {
                println!("Schema is identical to the latest version. Use --force to overwrite.");
                return Ok(());
            }
        } else {
            let diff = generate_diff(&old_schema_str, &new_schema_str);
            println!("Schema differs from latest version:\n{diff}");
        }
    }

    // 8. Decide on TUI run or direct save
    let mut should_save = true;
    let mut run_tui = verify;

    if is_different && !verify {
        if io::stdin().is_terminal() && io::stdout().is_terminal() && !non_interactive {
            print!("Do you want to save the generated schema? [y/N]: ");
            let _ = io::stdout().flush();
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let trimmed = input.trim().to_lowercase();
            if trimmed == "y" || trimmed == "yes" {
                run_tui = true;
            } else {
                should_save = false;
            }
        } else {
            should_save = true;
        }
    }

    if should_save {
        if run_tui && io::stdin().is_terminal() && io::stdout().is_terminal() && !non_interactive {
            let context = Context::detect(&std::env::current_dir().unwrap_or_default(), None, None);
            let saved = crate::tui::verify::run(&mut new_schema, &context)?;

            if !saved {
                println!("Verification TUI exited without saving.");
                return Ok(());
            }
        } else {
            save_schema(&new_schema).map_err(|e| format!("Failed to save schema: {e}"))?;
            println!(
                "Schema saved successfully (version {}).",
                new_schema.meta.version
            );
        }
    }

    Ok(())
}
