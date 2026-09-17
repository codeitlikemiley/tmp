use std::path::PathBuf;

pub fn show(
    last: bool,
    raw: bool,
    run_id: Option<&str>,
    cwd: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let base = cwd
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .ok_or("Could not determine working directory")?;
    let runs_dir = base.join(".tmp").join("runs");

    if !runs_dir.exists() {
        println!("No output runs found. Run 'tmp run' first.");
        return Ok(());
    }

    // Find the target run
    let target_dir = if let Some(id) = run_id {
        runs_dir.join(id)
    } else if last {
        // Find most recent run directory
        let mut entries: Vec<_> = std::fs::read_dir(&runs_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();
        entries.sort_by_key(|e| e.file_name());
        entries.last().map(|e| e.path()).ok_or("No runs found")?
    } else {
        // List all runs
        let mut entries: Vec<_> = std::fs::read_dir(&runs_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();
        entries.sort_by_key(|e| e.file_name());

        println!("Available runs:");
        for entry in &entries {
            let name = entry.file_name();
            let raw_exists = entry.path().join("raw.log").exists();
            let summary_exists = entry.path().join("summary.json").exists();
            println!(
                "  {} [raw: {}, summary: {}]",
                name.to_string_lossy(),
                raw_exists,
                summary_exists
            );
        }
        return Ok(());
    };

    if !target_dir.exists() {
        return Err(format!("Run directory not found: {}", target_dir.display()).into());
    }

    if raw {
        let raw_path = target_dir.join("raw.log");
        if raw_path.exists() {
            let content = std::fs::read_to_string(&raw_path)?;
            println!("{}", content);
        } else {
            println!("No raw output found for this run.");
        }
    } else {
        let summary_path = target_dir.join("summary.json");
        if summary_path.exists() {
            let content = std::fs::read_to_string(&summary_path)?;
            println!("{}", content);
        } else {
            // Fall back to raw
            let raw_path = target_dir.join("raw.log");
            if raw_path.exists() {
                let content = std::fs::read_to_string(&raw_path)?;
                println!("{}", content);
            } else {
                println!("No output found for this run.");
            }
        }
    }

    Ok(())
}
