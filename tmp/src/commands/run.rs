use std::path::PathBuf;
use tmp_core::run::run as core_run;

pub fn run(
    file: Option<&str>,
    dry_run: bool,
    custom_cwd: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let cwd = custom_cwd
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap());
    let cwd_str = cwd.to_string_lossy().to_string();

    let result = core_run(file, dry_run, &cwd_str)?;

    if !result.status.success() {
        return Err(format!("Command exited with status: {}", result.status).into());
    }

    Ok(())
}
