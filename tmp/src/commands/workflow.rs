use command::Command;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tmp_core::context::Context;

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub name: String,
    pub command: String,
    pub timeout_ms: Option<u64>,
    pub timeout: Option<u64>, // seconds
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Workflow {
    pub name: String,
    pub steps: Vec<WorkflowStep>,
}

fn get_global_workflows_dir() -> Option<PathBuf> {
    if let Ok(custom_dir) = std::env::var("TMP_CONFIG_DIR") {
        if !custom_dir.trim().is_empty() {
            let mut p = PathBuf::from(custom_dir);
            p.push("workflows");
            return Some(p);
        }
    }
    dirs::home_dir().map(|mut p| {
        p.push(".config");
        p.push("tmp");
        p.push("workflows");
        p
    })
}

fn get_local_workflows_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let cwd = std::env::current_dir()?;
    let context = Context::detect(&cwd, None, None);
    let root = context
        .project_root
        .unwrap_or_else(|| cwd.to_string_lossy().to_string());
    Ok(Path::new(&root).join(".tmp").join("workflows"))
}

pub fn list() -> Result<(), Box<dyn std::error::Error>> {
    let mut search_paths = Vec::new();
    if let Ok(local_dir) = get_local_workflows_dir() {
        search_paths.push(local_dir);
    }
    if let Some(global_dir) = get_global_workflows_dir() {
        search_paths.push(global_dir);
    }

    let mut workflow_names = std::collections::BTreeSet::new();

    for dir in search_paths {
        if dir.exists() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "json" {
                            if let Some(stem) = path.file_stem() {
                                workflow_names.insert(stem.to_string_lossy().into_owned());
                            }
                        }
                    }
                }
            }
        }
    }

    for name in workflow_names {
        println!("{}", name);
    }
    Ok(())
}

pub fn add(name: &str, from_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Adding workflow '{}' from {}...", name, from_path);
    let content = fs::read_to_string(from_path)?;

    // Parse JSON or YAML
    let workflow: Workflow = if from_path.ends_with(".yaml") || from_path.ends_with(".yml") {
        serde_yaml::from_str(&content)?
    } else {
        match serde_json::from_str(&content) {
            Ok(wf) => wf,
            Err(_) => serde_yaml::from_str(&content)?,
        }
    };

    let cwd = std::env::current_dir()?;
    let context = Context::detect(&cwd, None, None);
    let target_dir = if let Some(ref root) = context.project_root {
        Path::new(root).join(".tmp").join("workflows")
    } else {
        match get_global_workflows_dir() {
            Some(dir) => dir,
            None => return Err("Could not determine global config directory.".into()),
        }
    };

    fs::create_dir_all(&target_dir)?;
    let target_file = target_dir.join(format!("{}.json", name));
    // Save internally as JSON
    let json_str = serde_json::to_string_pretty(&workflow)?;
    fs::write(&target_file, json_str)?;
    println!("Workflow '{}' successfully added.", name);
    Ok(())
}

fn substitute_tokens(command: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut result = command.to_string();

    // 1. Find all {{TOKEN}} placeholders and replace them
    let mut start = 0;
    while let Some(pos) = result[start..].find("{{") {
        let actual_pos = start + pos;
        if let Some(end_pos) = result[actual_pos + 2..].find("}}") {
            let actual_end = actual_pos + 2 + end_pos;
            let token_name = &result[actual_pos + 2..actual_end];
            match std::env::var(token_name) {
                Ok(val) => {
                    result = result.replace(&format!("{{{{{}}}}}", token_name), &val);
                    start = 0;
                }
                Err(_) => {
                    return Err(
                        format!("Missing environment variable for token: {}", token_name).into(),
                    );
                }
            }
        } else {
            break;
        }
    }

    // 2. Find all <TOKEN> placeholders and replace them
    let mut start = 0;
    while let Some(pos) = result[start..].find('<') {
        let actual_pos = start + pos;
        if let Some(end_pos) = result[actual_pos + 1..].find('>') {
            let actual_end = actual_pos + 1 + end_pos;
            let token_name = &result[actual_pos + 1..actual_end];
            if !token_name.is_empty() && token_name.chars().all(|c| c.is_alphanumeric() || c == '_')
            {
                match std::env::var(token_name) {
                    Ok(val) => {
                        result = result.replace(&format!("<{}>", token_name), &val);
                        start = 0;
                    }
                    Err(_) => {
                        return Err(format!(
                            "Missing environment variable for token: {}",
                            token_name
                        )
                        .into());
                    }
                }
            } else {
                start = actual_end + 1;
            }
        } else {
            break;
        }
    }

    Ok(result)
}

pub fn run(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let current_depth: u32 = std::env::var("TMP_WORKFLOW_DEPTH")
        .unwrap_or_default()
        .parse()
        .unwrap_or(0);
    if current_depth > 10 {
        return Err("Max workflow recursion depth exceeded (detected loop).".into());
    }
    let next_depth = current_depth + 1;

    println!("Running workflow '{}'...", name);

    // Search both paths for the workflow file
    let mut target_file = None;
    if let Ok(local_dir) = get_local_workflows_dir() {
        let path = local_dir.join(format!("{}.json", name));
        if path.exists() {
            target_file = Some(path);
        }
    }

    if target_file.is_none() {
        if let Some(global_dir) = get_global_workflows_dir() {
            let path = global_dir.join(format!("{}.json", name));
            if path.exists() {
                target_file = Some(path);
            }
        }
    }

    let target_file = match target_file {
        Some(f) => f,
        None => return Err(format!("Workflow '{}' not found.", name).into()),
    };

    let content = fs::read_to_string(&target_file)?;
    let workflow: Workflow = serde_json::from_str(&content)?;

    let cwd = std::env::current_dir()?;
    let context = Context::detect(&cwd, None, None);
    let root = context
        .project_root
        .unwrap_or_else(|| cwd.to_string_lossy().to_string());
    let working_dir = Path::new(&root);

    for step in &workflow.steps {
        let substituted_command = substitute_tokens(&step.command)?;

        println!("► Running step '{}': {}", step.name, substituted_command);
        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", &substituted_command]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", &substituted_command]);
            c
        };
        cmd.current_dir(working_dir);
        cmd.stdin(std::process::Stdio::inherit());
        cmd.stdout(std::process::Stdio::inherit());
        cmd.stderr(std::process::Stdio::inherit());
        cmd.env("TMP_WORKFLOW_DEPTH", next_depth.to_string());

        let timeout_limit_ms = step.timeout_ms.or_else(|| step.timeout.map(|s| s * 1000));

        let status = if let Some(limit_ms) = timeout_limit_ms {
            let mut child = cmd
                .spawn()
                .map_err(|e| format!("Failed to spawn step command: {}", e))?;
            let start_time = std::time::Instant::now();
            let mut exited_status = None;
            while start_time.elapsed().as_millis() < limit_ms as u128 {
                match child.try_wait() {
                    Ok(Some(st)) => {
                        exited_status = Some(st);
                        break;
                    }
                    Ok(None) => {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                    Err(e) => {
                        return Err(format!("Error polling step process: {}", e).into());
                    }
                }
            }
            if let Some(st) = exited_status {
                st
            } else {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "Workflow step '{}' timed out after {}ms.",
                    step.name, limit_ms
                )
                .into());
            }
        } else {
            cmd.status()
                .map_err(|e| format!("Failed to execute step command: {}", e))?
        };

        if !status.success() {
            return Err(format!(
                "Workflow failed at step '{}': Command exited with status {}",
                step.name, status
            )
            .into());
        }
    }

    println!("Workflow '{}' completed successfully.", name);
    Ok(())
}
