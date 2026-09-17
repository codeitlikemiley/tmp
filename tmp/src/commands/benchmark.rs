use command::Command;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tmp_core::benchmark::BenchmarkRun;

pub fn run(task_id: &str, mode: &str, cwd: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let base = cwd
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .ok_or("Could not determine working directory")?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let run_id = format!("{}-{}", timestamp, task_id);

    let started = Instant::now();
    let output = Command::new("sh")
        .args(["-c", "echo tmp-benchmark"])
        .current_dir(&base)
        .output()
        .map_err(|e| format!("Failed to run benchmark probe: {e}"))?;
    let wall_time_ms = started.elapsed().as_millis() as u64;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw_output_bytes = (stdout.len() + stderr.len()) as u64;

    let record = BenchmarkRun {
        run_id: run_id.clone(),
        task_id: task_id.to_string(),
        mode: mode.to_string(),
        surface: "cli".to_string(),
        agent: None,
        model: None,
        tool_calls: 1,
        input_tokens: 0,
        output_tokens: 0,
        raw_output_bytes,
        shaped_output_bytes: raw_output_bytes,
        output_signal_preserved: stdout.contains("tmp-benchmark"),
        wall_time_ms,
        success: output.status.success(),
        invalid_operation_attempted: false,
        user_clarifications: 0,
        failed_attempts: 0,
        notes: "Harness probe: echo tmp-benchmark".to_string(),
    };
    record
        .validate()
        .map_err(|e| format!("Invalid benchmark record: {e}"))?;

    let benchmark_dir = base.join(".tmp").join("benchmarks");
    let record_path = benchmark_dir.join(format!("{}.json", run_id));
    record
        .save(&record_path)
        .map_err(|e| format!("Failed to save benchmark: {e}"))?;

    println!("Benchmark record created: {}", record_path.display());
    println!("Run ID: {}", run_id);
    println!("Task: {} (mode: {})", task_id, mode);
    println!("wall_time_ms: {}", record.wall_time_ms);
    println!("raw_output_bytes: {}", record.raw_output_bytes);

    Ok(())
}
