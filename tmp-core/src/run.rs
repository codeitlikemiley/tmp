use crate::context::Context;
use command::Command;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;

#[derive(Debug, Clone)]
pub struct RunResult {
    pub command: String,
    pub working_dir: PathBuf,
    pub status: ExitStatus,
}

pub fn run(filepath_arg: Option<&str>, dry_run: bool, cwd: &str) -> Result<RunResult, String> {
    let context = Context::detect(Path::new(cwd), filepath_arg, None);
    let project_root = context.project_root.as_deref().unwrap_or(cwd);
    let working_dir = PathBuf::from(project_root);

    // 1. If filepath_arg is None, check for last resolved command
    let resolved_command = if filepath_arg.is_none() {
        let last_cmd_path = working_dir.join(".tmp").join("last_command.json");
        if last_cmd_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&last_cmd_path) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                    val["command"].as_str().map(|s| s.to_string())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    let command_to_run = if let Some(cmd) = resolved_command {
        cmd
    } else {
        resolve_locally(&context, filepath_arg)?
    };

    if dry_run {
        println!("Dry-run: {}", command_to_run);
        let status = success_status();
        return Ok(RunResult {
            command: command_to_run,
            working_dir,
            status,
        });
    }

    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = Command::new("cmd");
        c.args(["/C", &command_to_run]);
        c
    } else {
        let mut c = Command::new("sh");
        c.args(["-c", &command_to_run]);
        c
    };

    cmd.current_dir(&working_dir);
    cmd.stdin(std::process::Stdio::inherit());
    cmd.stdout(std::process::Stdio::inherit());
    cmd.stderr(std::process::Stdio::inherit());

    let status = cmd
        .status()
        .map_err(|e| format!("Failed to execute command: {}", e))?;

    Ok(RunResult {
        command: command_to_run,
        working_dir,
        status,
    })
}

fn success_status() -> ExitStatus {
    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = Command::new("cmd");
        c.args(["/C", "exit 0"]);
        c
    } else {
        Command::new("true")
    };
    cmd.status().unwrap()
}

fn resolve_locally(context: &Context, filepath_arg: Option<&str>) -> Result<String, String> {
    if context.file_kind == "single_file_script" {
        let file =
            filepath_arg.ok_or_else(|| "single-file scripts require a file path".to_string())?;
        return Ok(format!("rust-script {}", file));
    }

    if context.file_kind == "standalone" {
        let file =
            filepath_arg.ok_or_else(|| "standalone Rust files require a file path".to_string())?;
        let path = Path::new(file);
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("standalone");
        let temp_bin = std::env::temp_dir().join(format!("tmp-{}-{}", stem, std::process::id()));
        return Ok(format!(
            "rustc {} -o {} && {}",
            file,
            temp_bin.display(),
            temp_bin.display()
        ));
    }

    if context.packages.contains(&"cargo".to_string()) || context.build_system == "cargo" {
        if let Some(file) = filepath_arg {
            let normalized = file.replace('\\', "/");
            let path = Path::new(file);
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("main");

            if normalized.contains("/src/main.rs") || normalized.ends_with("src/main.rs") {
                return Ok("cargo run".to_string());
            }
            if normalized.contains("/src/bin/") || normalized.contains("src/bin/") {
                return Ok(format!("cargo run --bin {}", stem));
            }
            if normalized.contains("/examples/") || normalized.contains("examples/") {
                return Ok(format!("cargo run --example {}", stem));
            }
            if normalized.contains("/tests/") || normalized.contains("tests/") {
                return Ok(format!("cargo test --test {}", stem));
            }
            if normalized.contains("/benches/") || normalized.contains("benches/") {
                return Ok(format!("cargo bench --bench {}", stem));
            }
        }
        return Ok("cargo run".to_string());
    }

    if context.build_system == "npm" {
        return Ok("npm test".to_string());
    }

    Err("No runnable context detected".to_string())
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
