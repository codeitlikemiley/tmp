use std::fs;
use std::path::{Path, PathBuf};
use tmp_core::context::Context;
use tmp_core::resolve::resolve;

pub fn sanitize_query(query: &str) -> String {
    query
        .chars()
        .filter(|&c| !";&|\\`$<>()[#]".contains(c))
        .collect()
}

pub fn run(
    query: &str,
    tool_filter: Option<&str>,
    json: bool,
    custom_cwd: Option<&str>,
    custom_config_path: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    if query.len() > 1000 {
        return Err("Query length exceeds maximum limit of 1000 characters.".into());
    }

    let sanitized_query = sanitize_query(query);

    let cwd = custom_cwd
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    let context = Context::detect(&cwd, None, None);

    let config_path = custom_config_path.map(PathBuf::from);

    let result = resolve(
        &sanitized_query,
        &context,
        tool_filter,
        config_path.as_deref(),
    )
    .map_err(|e| format!("Resolution error: {}", e))?;

    let project_root = context
        .project_root
        .clone()
        .unwrap_or_else(|| cwd.to_string_lossy().to_string());
    let tmp_dir = Path::new(&project_root).join(".tmp");
    fs::create_dir_all(&tmp_dir)?;
    let last_cmd_path = tmp_dir.join("last_command.json");
    let json_str = serde_json::to_string_pretty(&result)?;
    fs::write(last_cmd_path, &json_str)?;

    if json {
        println!("{}", json_str);
    } else {
        println!("{}", result.command);
    }

    Ok(())
}
