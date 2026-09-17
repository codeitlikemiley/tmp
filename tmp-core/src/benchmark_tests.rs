use crate::benchmark::{compare, BenchmarkRun};

fn make_baseline() -> BenchmarkRun {
    BenchmarkRun {
        run_id: "2026-06-03T00-00-00Z-baseline".to_string(),
        task_id: "cli.parser_tests".to_string(),
        mode: "baseline".to_string(),
        surface: "cli".to_string(),
        agent: Some("codex".to_string()),
        model: Some("test-model".to_string()),
        tool_calls: 8,
        input_tokens: 5000,
        output_tokens: 3000,
        raw_output_bytes: 24000,
        shaped_output_bytes: 24000, // baseline has no shaping
        output_signal_preserved: true,
        wall_time_ms: 12000,
        success: true,
        invalid_operation_attempted: false,
        user_clarifications: 1,
        failed_attempts: 2,
        notes: String::new(),
    }
}

fn make_assisted() -> BenchmarkRun {
    BenchmarkRun {
        run_id: "2026-06-03T00-00-00Z-assisted".to_string(),
        task_id: "cli.parser_tests".to_string(),
        mode: "tmp_assisted".to_string(),
        surface: "cli".to_string(),
        agent: Some("codex".to_string()),
        model: Some("test-model".to_string()),
        tool_calls: 2,
        input_tokens: 1200,
        output_tokens: 300,
        raw_output_bytes: 24000,
        shaped_output_bytes: 2600,
        output_signal_preserved: true,
        wall_time_ms: 4200,
        success: true,
        invalid_operation_attempted: false,
        user_clarifications: 0,
        failed_attempts: 0,
        notes: String::new(),
    }
}

#[test]
fn benchmark_run_serialization_roundtrip() {
    let run = make_baseline();
    let json = serde_json::to_string_pretty(&run).expect("serialize");
    let deserialized: BenchmarkRun = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(deserialized.run_id, run.run_id);
    assert_eq!(deserialized.task_id, run.task_id);
    assert_eq!(deserialized.mode, run.mode);
    assert_eq!(deserialized.surface, run.surface);
    assert_eq!(deserialized.agent, run.agent);
    assert_eq!(deserialized.model, run.model);
    assert_eq!(deserialized.tool_calls, run.tool_calls);
    assert_eq!(deserialized.input_tokens, run.input_tokens);
    assert_eq!(deserialized.output_tokens, run.output_tokens);
    assert_eq!(deserialized.raw_output_bytes, run.raw_output_bytes);
    assert_eq!(deserialized.shaped_output_bytes, run.shaped_output_bytes);
    assert_eq!(
        deserialized.output_signal_preserved,
        run.output_signal_preserved
    );
    assert_eq!(deserialized.wall_time_ms, run.wall_time_ms);
    assert_eq!(deserialized.success, run.success);
    assert_eq!(
        deserialized.invalid_operation_attempted,
        run.invalid_operation_attempted
    );
    assert_eq!(deserialized.user_clarifications, run.user_clarifications);
    assert_eq!(deserialized.failed_attempts, run.failed_attempts);
}

#[test]
fn benchmark_run_validation_valid_modes() {
    let valid_modes = [
        "baseline",
        "tmp_assisted",
        "tmp_completion",
        "tmp_registry",
        "tmp_generated_schema",
        "tmp_output_policy",
        "tmp_rtk",
        "tmp_generated_rtk_filter",
    ];

    for mode in &valid_modes {
        let mut run = make_baseline();
        run.mode = mode.to_string();
        assert!(run.validate().is_ok(), "Mode '{}' should be valid", mode);
    }
}

#[test]
fn benchmark_run_validation_rejects_invalid_mode() {
    let mut run = make_baseline();
    run.mode = "invalid_mode".to_string();

    let result = run.validate();
    assert!(result.is_err(), "Invalid mode should be rejected");
    assert!(
        result.unwrap_err().contains("invalid mode"),
        "Error message should mention invalid mode"
    );
}

#[test]
fn benchmark_run_validation_rejects_empty_run_id() {
    let mut run = make_baseline();
    run.run_id = String::new();

    let result = run.validate();
    assert!(result.is_err(), "Empty run_id should be rejected");
    assert!(result.unwrap_err().contains("run_id"));
}

#[test]
fn benchmark_run_validation_rejects_empty_task_id() {
    let mut run = make_baseline();
    run.task_id = String::new();

    let result = run.validate();
    assert!(result.is_err(), "Empty task_id should be rejected");
    assert!(result.unwrap_err().contains("task_id"));
}

#[test]
fn benchmark_run_save_and_load_from_disk() {
    let run = make_assisted();
    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let path = tmp_dir.path().join("benchmark.json");

    run.save(&path).expect("save");
    assert!(path.exists(), "Saved file should exist");

    let loaded = BenchmarkRun::load(&path).expect("load");
    assert_eq!(loaded.run_id, run.run_id);
    assert_eq!(loaded.task_id, run.task_id);
    assert_eq!(loaded.mode, run.mode);
    assert_eq!(loaded.tool_calls, run.tool_calls);
    assert_eq!(loaded.input_tokens, run.input_tokens);
    assert_eq!(loaded.output_tokens, run.output_tokens);
    assert_eq!(loaded.raw_output_bytes, run.raw_output_bytes);
    assert_eq!(loaded.shaped_output_bytes, run.shaped_output_bytes);
    assert_eq!(loaded.wall_time_ms, run.wall_time_ms);
    assert_eq!(loaded.success, run.success);
}

#[test]
fn benchmark_compare_produces_correct_deltas() {
    let baseline = make_baseline();
    let assisted = make_assisted();

    let cmp = compare(&baseline, &assisted);

    assert_eq!(cmp.task_id, "cli.parser_tests");

    // Tool call reduction: 8 - 2 = 6
    assert_eq!(cmp.tool_call_reduction, 6);

    // Input token reduction: 5000 - 1200 = 3800
    assert_eq!(cmp.input_token_reduction, 3800);

    // Output token reduction: 3000 - 300 = 2700
    assert_eq!(cmp.output_token_reduction, 2700);

    // Output reduction ratio: 1 - (2600 / 24000) ≈ 0.8917
    let expected_ratio = 1.0 - (2600.0 / 24000.0);
    assert!(
        (cmp.output_reduction_ratio - expected_ratio).abs() < 0.001,
        "Output reduction ratio should be ~{:.4}, got {:.4}",
        expected_ratio,
        cmp.output_reduction_ratio
    );

    // Latency reduction: 12000 - 4200 = 7800
    assert_eq!(cmp.latency_reduction_ms, 7800);

    // Failed attempts delta: 2 - 0 = 2
    assert_eq!(cmp.invalid_operation_delta, 2);
}

#[test]
fn benchmark_compare_handles_zero_raw_bytes() {
    let mut baseline = make_baseline();
    baseline.raw_output_bytes = 0;
    let assisted = make_assisted();

    let cmp = compare(&baseline, &assisted);
    assert_eq!(
        cmp.output_reduction_ratio, 0.0,
        "Output reduction ratio should be 0.0 when baseline has 0 raw bytes"
    );
}

#[test]
fn benchmark_run_optional_fields_default() {
    let json = r#"{
        "run_id": "test-1",
        "task_id": "cli.test",
        "mode": "baseline",
        "surface": "cli",
        "tool_calls": 1,
        "input_tokens": 100,
        "output_tokens": 50,
        "raw_output_bytes": 1000,
        "shaped_output_bytes": 500,
        "output_signal_preserved": true,
        "wall_time_ms": 2000,
        "success": true,
        "invalid_operation_attempted": false,
        "user_clarifications": 0,
        "failed_attempts": 0
    }"#;

    let run: BenchmarkRun = serde_json::from_str(json).expect("deserialize with defaults");
    assert!(run.agent.is_none(), "agent should default to None");
    assert!(run.model.is_none(), "model should default to None");
    assert!(run.notes.is_empty(), "notes should default to empty string");
}
