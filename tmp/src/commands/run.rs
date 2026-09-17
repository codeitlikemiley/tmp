use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use tmp_core::approval::{approval_for_run, check_approval, needs_approval};
use tmp_core::compile::ResolvedOperation;
use tmp_core::context::Context;
use tmp_core::output_policy::record_run;
use tmp_core::resolve::{OperationGate, ResolveResult};
use tmp_core::run::run as core_run;
use tmp_core::schema::{Operation, OutputMode, Surface};
use tmp_core::traits::RawOutput;

pub fn run(
    file: Option<&str>,
    dry_run: bool,
    custom_cwd: Option<&str>,
    yes: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let cwd = custom_cwd
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .ok_or("Could not determine working directory")?;
    let cwd_str = cwd.to_string_lossy().to_string();

    let context = Context::detect(&cwd, file, None);
    let project_root = context
        .project_root
        .as_deref()
        .map(Path::new)
        .unwrap_or(cwd.as_path());

    let last_resolve = if file.is_none() {
        load_last_resolve(project_root)
    } else {
        None
    };

    if let Some(ref resolved) = last_resolve {
        if let Some(ref gate) = resolved.operation {
            let op = operation_from_gate(&resolved.command, gate);
            let interactive = io::stdin().is_terminal();
            approval_for_run(&op, yes, interactive)?;
            if needs_approval(&op) && !yes {
                prompt_for_approval(&op)?;
            }
        }
    }

    let result = core_run(file, dry_run, &cwd_str)?;

    if !dry_run {
        let raw = RawOutput {
            stdout: result.stdout,
            stderr: result.stderr,
            exit_code: result.status.code().unwrap_or(1),
        };
        let mode = last_resolve
            .as_ref()
            .and_then(|r| r.operation.as_ref())
            .map(|g| g.output_policy.mode)
            .unwrap_or(OutputMode::Raw);
        let group = last_resolve
            .as_ref()
            .and_then(|r| r.operation.as_ref())
            .map(|g| g.group.clone())
            .unwrap_or_else(|| "run".to_string());
        let resolved_op = ResolvedOperation {
            command: result.command,
            description: String::new(),
            group,
            verified: last_resolve
                .as_ref()
                .and_then(|r| r.operation.as_ref())
                .map(|g| g.verified)
                .unwrap_or(false),
            parameters: vec![],
        };
        let envelope = record_run(&raw, &resolved_op, mode, project_root)?;
        match mode {
            OutputMode::Raw => {
                print!("{}", raw.stdout);
                eprint!("{}", raw.stderr);
            }
            _ => {
                if let Some(text) = envelope.summary.get("text").and_then(|v| v.as_str()) {
                    print!("{}", text);
                }
            }
        }
    }

    if !result.status.success() {
        return Err(format!("Command exited with status: {}", result.status).into());
    }

    Ok(())
}

fn load_last_resolve(project_root: &Path) -> Option<ResolveResult> {
    let path = project_root.join(".tmp").join("last_command.json");
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn operation_from_gate(command: &str, gate: &OperationGate) -> Operation {
    Operation {
        command: command.to_string(),
        description: String::new(),
        group: gate.group.clone(),
        verified: gate.verified,
        parameters: vec![],
        surface: Surface::Cli,
        effect: gate.effect,
        risk: gate.risk,
        approval: gate.approval,
        evidence: vec![],
        output_policy: gate.output_policy.clone(),
    }
}

fn prompt_for_approval(op: &Operation) -> Result<(), Box<dyn std::error::Error>> {
    let decision = check_approval(op, false);
    eprint!("⚠ {} Approve? [y/N] ", decision.reason);
    io::stderr().flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    let answer = line.trim().to_lowercase();
    if answer != "y" && answer != "yes" {
        return Err("Operation cancelled by user".into());
    }
    Ok(())
}
