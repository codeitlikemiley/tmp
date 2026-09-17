use super::*;
use crate::compile::{ResolvedOperation, ResolvedParameter};
use crate::schema::{OutputMode, ParameterType};
use crate::traits::{OutputPolicy, OutputSummary, RawOutput};

// ---------------------------------------------------------------------------
// Helper to build a minimal ResolvedOperation for tests
// ---------------------------------------------------------------------------

fn test_op(command: &str, group: &str) -> ResolvedOperation {
    ResolvedOperation {
        command: command.to_string(),
        description: "test operation".to_string(),
        group: group.to_string(),
        verified: true,
        parameters: vec![ResolvedParameter {
            name: "filter".to_string(),
            description: "filter".to_string(),
            required: false,
            parameter_type: ParameterType::String,
            default: None,
            flag: None,
            values: vec![],
        }],
    }
}

// ===========================================================================
// strip_ansi tests
// ===========================================================================

#[test]
fn test_strip_ansi_plain_text() {
    assert_eq!(strip_ansi("hello world"), "hello world");
}

#[test]
fn test_strip_ansi_color_codes() {
    assert_eq!(
        strip_ansi("\x1b[32mPASS\x1b[0m: test completed"),
        "PASS: test completed"
    );
}

#[test]
fn test_strip_ansi_bold_and_reset() {
    assert_eq!(
        strip_ansi("\x1b[1m\x1b[31merror\x1b[0m: something broke"),
        "error: something broke"
    );
}

#[test]
fn test_strip_ansi_multiple_sequences() {
    let input = "\x1b[36mRunning\x1b[0m \x1b[33m42\x1b[0m tests";
    assert_eq!(strip_ansi(input), "Running 42 tests");
}

#[test]
fn test_strip_ansi_empty_string() {
    assert_eq!(strip_ansi(""), "");
}

#[test]
fn test_strip_ansi_no_ansi() {
    assert_eq!(strip_ansi("just plain text 123!"), "just plain text 123!");
}

// ===========================================================================
// RawPolicy tests
// ===========================================================================

#[test]
fn test_raw_policy_passthrough() {
    let policy = RawPolicy;
    let raw = RawOutput {
        stdout: "hello stdout\n".to_string(),
        stderr: "hello stderr\n".to_string(),
        exit_code: 0,
    };
    let op = test_op("echo hello", "test");
    let result = policy.shape(&raw, &op);
    assert!(result.summary.contains("hello stdout"));
    assert!(result.summary.contains("hello stderr"));
}

#[test]
fn test_raw_policy_empty_stderr() {
    let policy = RawPolicy;
    let raw = RawOutput {
        stdout: "only stdout".to_string(),
        stderr: String::new(),
        exit_code: 0,
    };
    let op = test_op("echo", "test");
    let result = policy.shape(&raw, &op);
    assert_eq!(result.summary, "only stdout");
}

// ===========================================================================
// TestSummaryPolicy tests
// ===========================================================================

#[test]
fn test_test_summary_all_passing() {
    let policy = TestSummaryPolicy;
    let raw = RawOutput {
        stdout: r#"
running 3 tests
test tests::test_one ... ok
test tests::test_two ... ok
test tests::test_three ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
"#
        .to_string(),
        stderr: String::new(),
        exit_code: 0,
    };
    let op = test_op("cargo test", "cargo");
    let result = policy.shape(&raw, &op);
    let v: serde_json::Value = serde_json::from_str(&result.summary).unwrap();
    assert_eq!(v["passed"], 3);
    assert_eq!(v["failed"], 0);
    assert_eq!(v["failures"].as_array().unwrap().len(), 0);
    assert_eq!(v["exit_code"], 0);
}

#[test]
fn test_test_summary_with_failures() {
    let policy = TestSummaryPolicy;
    let raw = RawOutput {
        stdout: r#"
running 5 tests
test tests::test_one ... ok
test tests::test_two ... ok
test tests::test_three ... FAILED
test tests::test_four ... ok
test tests::test_five ... FAILED

failures:

---- tests::test_three stdout ----
thread 'tests::test_three' panicked at 'assertion failed: `(left == right)`
  left: `1`,
 right: `2`', src/lib.rs:42:5

---- tests::test_five stdout ----
thread 'tests::test_five' panicked at 'not yet implemented', src/lib.rs:99:5

failures:
    tests::test_three
    tests::test_five

test result: FAILED. 3 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
"#
        .to_string(),
        stderr: String::new(),
        exit_code: 101,
    };
    let op = test_op("cargo test", "cargo");
    let result = policy.shape(&raw, &op);
    let v: serde_json::Value = serde_json::from_str(&result.summary).unwrap();
    assert_eq!(v["passed"], 3);
    assert_eq!(v["failed"], 2);
    assert_eq!(v["exit_code"], 101);
    let failures = v["failures"].as_array().unwrap();
    assert_eq!(failures.len(), 2);
    // Verify that failure content is captured
    assert!(failures[0].as_str().unwrap().contains("tests::test_three"));
    assert!(failures[1].as_str().unwrap().contains("tests::test_five"));
}

#[test]
fn test_test_summary_with_ignored() {
    let policy = TestSummaryPolicy;
    let raw = RawOutput {
        stdout: r#"
running 4 tests
test tests::test_one ... ok
test tests::test_two ... ok
test tests::test_three ... ignored
test tests::test_four ... ok

test result: ok. 3 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
"#
        .to_string(),
        stderr: String::new(),
        exit_code: 0,
    };
    let op = test_op("cargo test", "cargo");
    let result = policy.shape(&raw, &op);
    let v: serde_json::Value = serde_json::from_str(&result.summary).unwrap();
    assert_eq!(v["passed"], 3);
    assert_eq!(v["failed"], 0);
    assert_eq!(v["ignored"], 1);
}

#[test]
fn test_test_summary_compiler_errors() {
    let policy = TestSummaryPolicy;
    let raw = RawOutput {
        stdout: String::new(),
        stderr: r#"
error[E0308]: mismatched types
  --> src/lib.rs:10:5
   |
10 |     "hello"
   |     ^^^^^^^ expected `i32`, found `&str`

error: could not compile `my-crate` due to previous error
"#
        .to_string(),
        exit_code: 101,
    };
    let op = test_op("cargo test", "cargo");
    let result = policy.shape(&raw, &op);
    let v: serde_json::Value = serde_json::from_str(&result.summary).unwrap();
    let errors = v["compiler_errors"].as_array().unwrap();
    assert!(errors.len() >= 2);
    assert!(errors[0].as_str().unwrap().contains("E0308"));
}

#[test]
fn test_test_summary_ansi_stripped() {
    let policy = TestSummaryPolicy;
    let raw = RawOutput {
        stdout: "\x1b[32mtest result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\x1b[0m\n".to_string(),
        stderr: String::new(),
        exit_code: 0,
    };
    let op = test_op("cargo test", "cargo");
    let result = policy.shape(&raw, &op);
    let v: serde_json::Value = serde_json::from_str(&result.summary).unwrap();
    assert_eq!(v["passed"], 5);
    assert_eq!(v["failed"], 0);
}

// ===========================================================================
// GitSummaryPolicy tests
// ===========================================================================

#[test]
fn test_git_summary_long_form() {
    let policy = GitSummaryPolicy;
    let raw = RawOutput {
        stdout: r#"On branch main
Your branch is ahead of 'origin/main' by 3 commits.
  (use "git push" to publish your local commits)

Changes to be committed:
  (use "git restore --staged <file>..." to unstage)
	modified:   src/lib.rs
	new file:   src/output_policy.rs

Changes not staged for commit:
  (use "git add <file>..." to update what will be committed)
  (use "git restore <file>..." to discard changes in working directory)
	modified:   Cargo.toml

Untracked files:
  (use "git add <file>..." to include in what will be committed)
	TODO.md
	notes.txt
"#
        .to_string(),
        stderr: String::new(),
        exit_code: 0,
    };
    let op = test_op("git status", "git");
    let result = policy.shape(&raw, &op);
    let v: serde_json::Value = serde_json::from_str(&result.summary).unwrap();

    assert_eq!(v["branch"], "main");
    assert_eq!(v["ahead"], 3);
    assert_eq!(v["behind"], 0);

    let staged = v["staged"].as_array().unwrap();
    assert_eq!(staged.len(), 2);
    assert!(staged.iter().any(|s| s.as_str().unwrap() == "src/lib.rs"));
    assert!(staged
        .iter()
        .any(|s| s.as_str().unwrap() == "src/output_policy.rs"));

    let unstaged = v["unstaged"].as_array().unwrap();
    assert_eq!(unstaged.len(), 1);
    assert!(unstaged[0].as_str().unwrap() == "Cargo.toml");

    let untracked = v["untracked"].as_array().unwrap();
    assert_eq!(untracked.len(), 2);
}

#[test]
fn test_git_summary_short_form() {
    let policy = GitSummaryPolicy;
    let raw = RawOutput {
        stdout: "## feature-branch...origin/feature-branch [ahead 2, behind 1]\n".to_string(),
        stderr: String::new(),
        exit_code: 0,
    };
    let op = test_op("git status -sb", "git");
    let result = policy.shape(&raw, &op);
    let v: serde_json::Value = serde_json::from_str(&result.summary).unwrap();

    assert_eq!(v["branch"], "feature-branch");
    assert_eq!(v["ahead"], 2);
    assert_eq!(v["behind"], 1);
}

#[test]
fn test_git_summary_clean_working_tree() {
    let policy = GitSummaryPolicy;
    let raw = RawOutput {
        stdout: r#"On branch develop
nothing to commit, working tree clean
"#
        .to_string(),
        stderr: String::new(),
        exit_code: 0,
    };
    let op = test_op("git status", "git");
    let result = policy.shape(&raw, &op);
    let v: serde_json::Value = serde_json::from_str(&result.summary).unwrap();

    assert_eq!(v["branch"], "develop");
    assert_eq!(v["staged"].as_array().unwrap().len(), 0);
    assert_eq!(v["unstaged"].as_array().unwrap().len(), 0);
    assert_eq!(v["untracked"].as_array().unwrap().len(), 0);
}

// ===========================================================================
// DiffSummaryPolicy tests
// ===========================================================================

#[test]
fn test_diff_summary_basic() {
    let policy = DiffSummaryPolicy;
    let raw = RawOutput {
        stdout: r#"diff --git a/src/lib.rs b/src/lib.rs
index abc1234..def5678 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -10,6 +10,8 @@ fn main() {
     let x = 1;
+    let y = 2;
+    let z = x + y;
     println!("done");
 }
diff --git a/Cargo.toml b/Cargo.toml
index 1111111..2222222 100644
--- a/Cargo.toml
+++ b/Cargo.toml
@@ -5,3 +5,4 @@ edition = "2021"
 [dependencies]
 serde = "1.0"
+serde_json = "1.0"
"#
        .to_string(),
        stderr: String::new(),
        exit_code: 0,
    };
    let op = test_op("git diff", "git");
    let result = policy.shape(&raw, &op);
    let v: serde_json::Value = serde_json::from_str(&result.summary).unwrap();

    assert_eq!(v["files_changed"], 2);
    assert_eq!(v["total_additions"], 3);
    assert_eq!(v["total_deletions"], 0);

    let files = v["files"].as_array().unwrap();
    assert_eq!(files.len(), 2);

    // Check file names
    let file_names: Vec<&str> = files.iter().map(|f| f["file"].as_str().unwrap()).collect();
    assert!(file_names.contains(&"src/lib.rs"));
    assert!(file_names.contains(&"Cargo.toml"));
}

#[test]
fn test_diff_summary_with_deletions() {
    let policy = DiffSummaryPolicy;
    let raw = RawOutput {
        stdout: r#"--- a/file.txt
+++ b/file.txt
@@ -1,5 +1,3 @@
 line1
-line2
-line3
 line4
 line5
"#
        .to_string(),
        stderr: String::new(),
        exit_code: 0,
    };
    let op = test_op("diff", "diff");
    let result = policy.shape(&raw, &op);
    let v: serde_json::Value = serde_json::from_str(&result.summary).unwrap();

    assert_eq!(v["files_changed"], 1);
    assert_eq!(v["total_additions"], 0);
    assert_eq!(v["total_deletions"], 2);
}

#[test]
fn test_diff_summary_empty_diff() {
    let policy = DiffSummaryPolicy;
    let raw = RawOutput {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: 0,
    };
    let op = test_op("git diff", "git");
    let result = policy.shape(&raw, &op);
    let v: serde_json::Value = serde_json::from_str(&result.summary).unwrap();
    assert_eq!(v["files_changed"], 0);
    assert_eq!(v["total_additions"], 0);
    assert_eq!(v["total_deletions"], 0);
}

// ===========================================================================
// SearchSummaryPolicy tests
// ===========================================================================

#[test]
fn test_search_summary_ripgrep_output() {
    let policy = SearchSummaryPolicy;
    let raw = RawOutput {
        stdout: r#"src/lib.rs:10:    let x = 42;
src/lib.rs:25:    assert_eq!(x, 42);
src/main.rs:5:    let x = 42;
tests/integration.rs:12:    let result = 42;
tests/integration.rs:15:    assert!(result == 42);
tests/integration.rs:20:    // 42 is the answer
"#
        .to_string(),
        stderr: String::new(),
        exit_code: 0,
    };
    let op = test_op("rg 42", "search");
    let result = policy.shape(&raw, &op);
    let v: serde_json::Value = serde_json::from_str(&result.summary).unwrap();

    assert_eq!(v["total_matches"], 6);
    assert_eq!(v["files_matched"], 3);

    let files = v["files"].as_array().unwrap();
    assert_eq!(files.len(), 3);
}

#[test]
fn test_search_summary_no_matches() {
    let policy = SearchSummaryPolicy;
    let raw = RawOutput {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: 1,
    };
    let op = test_op("rg nonexistent", "search");
    let result = policy.shape(&raw, &op);
    let v: serde_json::Value = serde_json::from_str(&result.summary).unwrap();
    assert_eq!(v["total_matches"], 0);
    assert_eq!(v["files_matched"], 0);
}

#[test]
fn test_search_summary_snippet_limit() {
    let policy = SearchSummaryPolicy;
    // Create 10 matches in the same file — only first MAX_SNIPPETS_PER_FILE should be kept
    let mut lines = Vec::new();
    for i in 1..=10 {
        lines.push(format!("src/big.rs:{}:    match line {}", i, i));
    }
    let raw = RawOutput {
        stdout: lines.join("\n"),
        stderr: String::new(),
        exit_code: 0,
    };
    let op = test_op("rg match", "search");
    let result = policy.shape(&raw, &op);
    let v: serde_json::Value = serde_json::from_str(&result.summary).unwrap();
    assert_eq!(v["total_matches"], 10);
    assert_eq!(v["files_matched"], 1);

    let files = v["files"].as_array().unwrap();
    let snippets = files[0]["snippets"].as_array().unwrap();
    assert_eq!(snippets.len(), SearchSummaryPolicy::MAX_SNIPPETS_PER_FILE);
}

// ===========================================================================
// LogSummaryPolicy tests
// ===========================================================================

#[test]
fn test_log_summary_mixed_severities() {
    let policy = LogSummaryPolicy;
    let raw = RawOutput {
        stdout: r#"2026-06-03T10:00:00Z INFO Starting application
2026-06-03T10:00:01Z INFO Connected to database
2026-06-03T10:00:02Z WARNING Disk space low on /dev/sda1
2026-06-03T10:00:03Z ERROR Failed to connect to cache server
2026-06-03T10:00:04Z ERROR Failed to connect to cache server
2026-06-03T10:00:05Z ERROR Timeout on request /api/users
2026-06-03T10:00:06Z FATAL Out of memory
2026-06-03T10:00:07Z INFO Shutting down
"#
        .to_string(),
        stderr: String::new(),
        exit_code: 1,
    };
    let op = test_op("journalctl", "log");
    let result = policy.shape(&raw, &op);
    let v: serde_json::Value = serde_json::from_str(&result.summary).unwrap();

    assert_eq!(v["fatal_count"], 1);
    assert_eq!(v["error_count"], 3);
    assert_eq!(v["warning_count"], 1);
    assert_eq!(v["info_count"], 3);

    // Deduplicated: "Failed to connect to cache server" appears twice but
    // should be deduplicated to a single unique message
    let unique = v["unique_messages"].as_array().unwrap();
    let error_messages: Vec<&str> = unique
        .iter()
        .filter(|m| m["severity"].as_str().unwrap() == "error")
        .map(|m| m["message"].as_str().unwrap())
        .collect();
    // Should have 2 unique error messages, not 3
    assert_eq!(error_messages.len(), 2);
}

#[test]
fn test_log_summary_no_relevant_lines() {
    let policy = LogSummaryPolicy;
    let raw = RawOutput {
        stdout: "Just some output\nMore output\n".to_string(),
        stderr: String::new(),
        exit_code: 0,
    };
    let op = test_op("app run", "app");
    let result = policy.shape(&raw, &op);
    let v: serde_json::Value = serde_json::from_str(&result.summary).unwrap();
    assert_eq!(v["fatal_count"], 0);
    assert_eq!(v["error_count"], 0);
    assert_eq!(v["warning_count"], 0);
    assert_eq!(v["unique_messages"].as_array().unwrap().len(), 0);
}

#[test]
fn test_log_summary_bracket_severity() {
    let policy = LogSummaryPolicy;
    let raw = RawOutput {
        stdout: "[ERROR] something went wrong\n[WARN] low memory\n".to_string(),
        stderr: String::new(),
        exit_code: 1,
    };
    let op = test_op("app", "app");
    let result = policy.shape(&raw, &op);
    let v: serde_json::Value = serde_json::from_str(&result.summary).unwrap();
    assert_eq!(v["error_count"], 1);
    assert_eq!(v["warning_count"], 1);
}

// ===========================================================================
// retain_raw_output tests
// ===========================================================================

#[test]
fn test_retain_raw_output_writes_file() {
    let dir = tempfile::tempdir().unwrap();
    let raw = RawOutput {
        stdout: "test stdout content".to_string(),
        stderr: "test stderr content".to_string(),
        exit_code: 42,
    };

    let path = retain_raw_output(&raw, dir.path()).unwrap();
    assert!(std::path::Path::new(&path).exists());

    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("--- STDOUT ---"));
    assert!(content.contains("test stdout content"));
    assert!(content.contains("--- STDERR ---"));
    assert!(content.contains("test stderr content"));
    assert!(content.contains("--- EXIT: 42 ---"));
}

#[test]
fn test_retain_raw_output_creates_directories() {
    let dir = tempfile::tempdir().unwrap();
    let raw = RawOutput {
        stdout: "ok".to_string(),
        stderr: String::new(),
        exit_code: 0,
    };

    let path = retain_raw_output(&raw, dir.path()).unwrap();
    // Verify the .tmp/runs/<timestamp>/ directory structure was created
    let path_obj = std::path::Path::new(&path);
    assert!(path_obj.parent().unwrap().exists());
    assert!(path_obj
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .ends_with("runs"));
}

#[test]
fn record_run_writes_raw_log_and_summary_json_under_tmp_runs() {
    let dir = tempfile::tempdir().unwrap();
    let raw = RawOutput {
        stdout: "captured-stdout".to_string(),
        stderr: "captured-stderr".to_string(),
        exit_code: 0,
    };
    let op = test_op("echo captured-stdout", "run");

    let envelope = record_run(&raw, &op, OutputMode::Raw, dir.path()).unwrap();
    let raw_path = envelope
        .raw_output
        .location
        .expect("record_run should retain raw output");
    let path = std::path::Path::new(&raw_path);

    assert_eq!(path.file_name().and_then(|n| n.to_str()), Some("raw.log"));
    let run_dir = path.parent().expect("raw.log parent");
    assert_eq!(
        run_dir
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str()),
        Some("runs")
    );
    assert_eq!(
        run_dir
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str()),
        Some(".tmp")
    );

    let raw_content = fs::read_to_string(path).unwrap();
    assert!(raw_content.contains("captured-stdout"));

    let summary_path = run_dir.join("summary.json");
    let summary: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&summary_path).unwrap()).unwrap();
    assert_eq!(summary["invocation"], "echo captured-stdout");
    assert_eq!(summary["exit_status"], 0);
}

// ===========================================================================
// build_envelope tests
// ===========================================================================

#[test]
fn test_build_envelope_success() {
    let op = test_op("cargo test", "cargo");
    let raw = RawOutput {
        stdout: "all tests pass".to_string(),
        stderr: String::new(),
        exit_code: 0,
    };
    let summary = OutputSummary {
        summary: "3 passed, 0 failed".to_string(),
    };

    let envelope = build_envelope(&op, &raw, &summary, Some("/tmp/raw.log".to_string()));

    assert_eq!(envelope.operation_id, "cargo.test");
    assert_eq!(envelope.invocation, "cargo test");
    assert_eq!(envelope.exit_status, 0);
    assert!(envelope.success);
    assert_eq!(envelope.summary["text"], "3 passed, 0 failed");
    assert!(envelope.raw_output.retained);
    assert_eq!(
        envelope.raw_output.location.as_deref(),
        Some("/tmp/raw.log")
    );
}

#[test]
fn test_build_envelope_failure_no_raw() {
    let op = test_op("cargo build --release", "cargo");
    let raw = RawOutput {
        stdout: String::new(),
        stderr: "compilation failed".to_string(),
        exit_code: 1,
    };
    let summary = OutputSummary {
        summary: "build failed".to_string(),
    };

    let envelope = build_envelope(&op, &raw, &summary, None);

    assert_eq!(envelope.operation_id, "cargo.--release");
    assert_eq!(envelope.exit_status, 1);
    assert!(!envelope.success);
    assert!(!envelope.raw_output.retained);
    assert!(envelope.raw_output.location.is_none());
}

#[test]
fn test_build_envelope_json_serializable() {
    let op = test_op("cargo test", "cargo");
    let raw = RawOutput {
        stdout: "ok".to_string(),
        stderr: String::new(),
        exit_code: 0,
    };
    let summary = OutputSummary {
        summary: "passed".to_string(),
    };

    let envelope = build_envelope(&op, &raw, &summary, None);
    let json = serde_json::to_string_pretty(&envelope).unwrap();
    assert!(json.contains("\"operation_id\""));
    assert!(json.contains("\"success\": true"));

    // Verify round-trip
    let parsed: ResultEnvelope = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.operation_id, envelope.operation_id);
    assert_eq!(parsed.success, envelope.success);
}

// ===========================================================================
// get_policy tests
// ===========================================================================

#[test]
fn test_get_policy_returns_correct_type() {
    // We verify each mode returns a working policy by calling shape()
    let modes = vec![
        OutputMode::Raw,
        OutputMode::TestSummary,
        OutputMode::GitSummary,
        OutputMode::DiffSummary,
        OutputMode::SearchSummary,
        OutputMode::LogSummary,
        OutputMode::JsonProjection,
    ];
    let op = test_op("test", "test");
    let raw = RawOutput {
        stdout: "output".to_string(),
        stderr: String::new(),
        exit_code: 0,
    };

    for mode in modes {
        let policy = get_policy(&mode);
        let result = policy.shape(&raw, &op);
        // Every policy should produce a non-empty summary
        assert!(
            !result.summary.is_empty(),
            "Policy for {:?} produced empty summary",
            mode
        );
    }
}

// ===========================================================================
// Integration: full pipeline test
// ===========================================================================

#[test]
fn test_full_pipeline_test_summary() {
    // Simulate: run test -> shape output -> build envelope -> retain raw
    let raw = RawOutput {
        stdout: r#"running 10 tests
test tests::a ... ok
test tests::b ... ok
test tests::c ... ok
test tests::d ... ok
test tests::e ... ok
test tests::f ... ok
test tests::g ... ok
test tests::h ... ok
test tests::i ... FAILED
test tests::j ... ok

failures:

---- tests::i stdout ----
thread 'tests::i' panicked at 'assertion failed', src/lib.rs:99:5

failures:
    tests::i

test result: FAILED. 9 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
"#
        .to_string(),
        stderr: String::new(),
        exit_code: 101,
    };

    let op = test_op("cargo test", "cargo");
    let policy = get_policy(&OutputMode::TestSummary);
    let summary = policy.shape(&raw, &op);

    // Verify the summary is valid JSON
    let v: serde_json::Value = serde_json::from_str(&summary.summary).unwrap();
    assert_eq!(v["passed"], 9);
    assert_eq!(v["failed"], 1);

    // Build envelope
    let dir = tempfile::tempdir().unwrap();
    let raw_loc = retain_raw_output(&raw, dir.path()).unwrap();
    let envelope = build_envelope(&op, &raw, &summary, Some(raw_loc.clone()));

    assert!(!envelope.success);
    assert_eq!(envelope.exit_status, 101);
    assert!(envelope.raw_output.retained);

    // Verify raw file was written
    let raw_content = fs::read_to_string(&raw_loc).unwrap();
    assert!(raw_content.contains("tests::i"));
}

// ===========================================================================
// Edge cases
// ===========================================================================

#[test]
fn test_test_summary_empty_output() {
    let policy = TestSummaryPolicy;
    let raw = RawOutput {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: 0,
    };
    let op = test_op("cargo test", "cargo");
    let result = policy.shape(&raw, &op);
    let v: serde_json::Value = serde_json::from_str(&result.summary).unwrap();
    assert_eq!(v["passed"], 0);
    assert_eq!(v["failed"], 0);
}

#[test]
fn test_diff_summary_multiple_hunks() {
    let policy = DiffSummaryPolicy;
    let raw = RawOutput {
        stdout: r#"--- a/src/lib.rs
+++ b/src/lib.rs
@@ -10,3 +10,4 @@ fn foo() {
     let a = 1;
+    let b = 2;
 }
@@ -30,3 +31,4 @@ fn bar() {
     let c = 3;
+    let d = 4;
 }
@@ -50,4 +52,3 @@ fn baz() {
     let e = 5;
-    let f = 6;
 }
"#
        .to_string(),
        stderr: String::new(),
        exit_code: 0,
    };
    let op = test_op("git diff", "git");
    let result = policy.shape(&raw, &op);
    let v: serde_json::Value = serde_json::from_str(&result.summary).unwrap();

    assert_eq!(v["files_changed"], 1);
    assert_eq!(v["total_additions"], 2);
    assert_eq!(v["total_deletions"], 1);

    let files = v["files"].as_array().unwrap();
    assert_eq!(files[0]["hunks"], 3);
    assert_eq!(files[0]["hunk_headers"].as_array().unwrap().len(), 3);
}

#[test]
fn test_contains_severity_word_boundary() {
    // "error" should match, but "xerror" should not
    assert!(contains_severity("error: something", "error"));
    assert!(contains_severity("[error] something", "error"));
    assert!(!contains_severity("xerror something", "error"));
    assert!(contains_severity("warn: low disk", "warn"));
    assert!(contains_severity("warning: low disk", "warn")); // "warn" is prefix of "warning", boundary is 'i' which is alphabetic — let me check
}

#[test]
fn test_normalize_log_message_strips_timestamp() {
    assert_eq!(
        normalize_log_message("2026-06-03T10:00:00Z ERROR something broke"),
        "ERROR something broke"
    );
    assert_eq!(
        normalize_log_message("plain message without timestamp"),
        "plain message without timestamp"
    );
}

#[test]
fn test_parse_search_line_basic() {
    let (file, rest) = parse_search_line("src/lib.rs:10:let x = 42;").unwrap();
    assert_eq!(file, "src/lib.rs");
    assert_eq!(rest, "10:let x = 42;");
}

#[test]
fn test_parse_search_line_no_match() {
    assert!(parse_search_line("just some text without colons").is_none());
}
