use crate::compile::ResolvedOperation;
use crate::schema::OutputMode;
use crate::traits::{OutputPolicy, OutputSummary, RawOutput};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Result envelope (whitepaper spec §Output Shaping)
// ---------------------------------------------------------------------------

/// Structured result envelope matching the whitepaper spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultEnvelope {
    pub operation_id: String,
    pub invocation: String,
    pub exit_status: i32,
    pub success: bool,
    pub summary: serde_json::Value,
    pub omitted: serde_json::Value,
    pub raw_output: RawOutputRef,
}

/// Reference to the retained raw output on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawOutputRef {
    pub retained: bool,
    pub location: Option<String>,
}

// ---------------------------------------------------------------------------
// ANSI stripping utility
// ---------------------------------------------------------------------------

/// Strip ANSI escape sequences from a string.
///
/// Handles `ESC [ ... <letter>` (CSI) sequences and `ESC <letter>` (two-byte)
/// sequences commonly found in terminal output.
pub fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_escape = false;
    let mut in_csi = false;
    for c in s.chars() {
        if in_csi {
            // CSI sequences end at the first ASCII letter
            if c.is_ascii_alphabetic() {
                in_csi = false;
            }
        } else if in_escape {
            in_escape = false;
            if c == '[' {
                in_csi = true;
            }
            // For two-byte escapes (e.g. ESC M) or unknown chars, just consume
        } else if c == '\x1b' {
            in_escape = true;
        } else {
            result.push(c);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Raw output retention
// ---------------------------------------------------------------------------

fn chrono_lite_timestamp() -> String {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", duration.as_secs())
}

/// Retain the raw output on disk under `base_dir/.tmp/runs/<timestamp>/raw.log`.
///
/// Returns the absolute path to the written log file.
pub fn retain_raw_output(raw: &RawOutput, base_dir: &Path) -> Result<String, std::io::Error> {
    let timestamp = chrono_lite_timestamp();
    let run_dir = base_dir.join(".tmp").join("runs").join(&timestamp);
    fs::create_dir_all(&run_dir)?;
    let raw_path = run_dir.join("raw.log");
    fs::write(
        &raw_path,
        format!(
            "--- STDOUT ---\n{}\n--- STDERR ---\n{}\n--- EXIT: {} ---",
            raw.stdout, raw.stderr, raw.exit_code
        ),
    )?;
    Ok(raw_path.to_string_lossy().to_string())
}

// ---------------------------------------------------------------------------
// Envelope builder
// ---------------------------------------------------------------------------

/// Build a `ResultEnvelope` from a resolved operation, raw output, shaped
/// summary, and optional retained-raw-output path.
pub fn build_envelope(
    op: &ResolvedOperation,
    raw: &RawOutput,
    summary: &OutputSummary,
    raw_location: Option<String>,
) -> ResultEnvelope {
    ResultEnvelope {
        operation_id: format!(
            "{}.{}",
            op.group,
            op.command.split_whitespace().last().unwrap_or(&op.command)
        ),
        invocation: op.command.clone(),
        exit_status: raw.exit_code,
        success: raw.exit_code == 0,
        summary: serde_json::json!({ "text": summary.summary }),
        omitted: serde_json::json!({}),
        raw_output: RawOutputRef {
            retained: raw_location.is_some(),
            location: raw_location,
        },
    }
}

// ---------------------------------------------------------------------------
// Policy dispatcher
// ---------------------------------------------------------------------------

/// Retain raw output, shape it, write `summary.json` next to `raw.log`, and return the envelope.
pub fn record_run(
    raw: &RawOutput,
    resolved_op: &ResolvedOperation,
    mode: OutputMode,
    base_dir: &Path,
) -> Result<ResultEnvelope, std::io::Error> {
    let raw_path = retain_raw_output(raw, base_dir)?;
    let summary = get_policy(&mode).shape(raw, resolved_op);
    let envelope = build_envelope(resolved_op, raw, &summary, Some(raw_path.clone()));
    let Some(run_dir) = Path::new(&raw_path).parent() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "retained raw output path has no parent directory",
        ));
    };
    let json = serde_json::to_string_pretty(&envelope)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(run_dir.join("summary.json"), json)?;
    Ok(envelope)
}

/// Return a boxed `OutputPolicy` implementation for the given `OutputMode`.
pub fn get_policy(mode: &OutputMode) -> Box<dyn OutputPolicy> {
    match mode {
        OutputMode::Raw => Box::new(RawPolicy),
        OutputMode::TestSummary => Box::new(TestSummaryPolicy),
        OutputMode::GitSummary => Box::new(GitSummaryPolicy),
        OutputMode::DiffSummary => Box::new(DiffSummaryPolicy),
        OutputMode::SearchSummary => Box::new(SearchSummaryPolicy),
        OutputMode::LogSummary => Box::new(LogSummaryPolicy),
        OutputMode::JsonProjection => Box::new(RawPolicy), // fallback to raw
    }
}

// ---------------------------------------------------------------------------
// 1. RawPolicy — passthrough
// ---------------------------------------------------------------------------

pub struct RawPolicy;

impl OutputPolicy for RawPolicy {
    fn shape(&self, raw: &RawOutput, _op: &ResolvedOperation) -> OutputSummary {
        let mut combined = raw.stdout.clone();
        if !raw.stderr.is_empty() {
            combined.push_str(&raw.stderr);
        }
        OutputSummary { summary: combined }
    }
}

// ---------------------------------------------------------------------------
// 2. TestSummaryPolicy — parse cargo-test / pytest / jest output
// ---------------------------------------------------------------------------

pub struct TestSummaryPolicy;

impl TestSummaryPolicy {
    /// Maximum number of failure blocks to include verbatim in the summary.
    const MAX_FAILURE_BLOCKS: usize = 5;
    /// Maximum lines per failure block.
    const MAX_LINES_PER_FAILURE: usize = 30;
}

impl OutputPolicy for TestSummaryPolicy {
    fn shape(&self, raw: &RawOutput, _op: &ResolvedOperation) -> OutputSummary {
        let combined = format!("{}{}", raw.stdout, raw.stderr);
        let lines: Vec<&str> = combined.lines().collect();

        let mut failures: Vec<String> = Vec::new();
        let mut passed_count: u64 = 0;
        let mut failed_count: u64 = 0;
        let mut ignored_count: u64 = 0;
        let mut result_line = String::new();
        let mut compiler_errors: Vec<String> = Vec::new();

        // State machine for collecting failure blocks
        let mut in_failure_block = false;
        let mut failure_block: Vec<String> = Vec::new();
        let mut failure_name = String::new();

        for raw_line in &lines {
            let line = strip_ansi(raw_line);
            let trimmed = line.trim();

            // ---- Detect cargo test result summary line ----
            // e.g. "test result: ok. 10 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out"
            // e.g. "test result: FAILED. 8 passed; 2 failed; 0 ignored; ..."
            if trimmed.starts_with("test result:") {
                result_line = trimmed.to_string();
                // Parse the counts from this line
                for part in trimmed.split(';') {
                    let part = part.trim();
                    if let Some(rest) = part.strip_suffix("passed") {
                        if let Ok(n) = rest.trim().parse::<u64>() {
                            passed_count = n;
                        }
                    } else if let Some(rest) = part.strip_suffix("failed") {
                        if let Ok(n) = rest.trim().parse::<u64>() {
                            failed_count = n;
                        }
                    } else if let Some(rest) = part.strip_suffix("ignored") {
                        if let Ok(n) = rest.trim().parse::<u64>() {
                            ignored_count = n;
                        }
                    }
                    // Also match "test result: ok. N passed"
                    if part.contains("passed") {
                        let after_dot = part.split('.').next_back().unwrap_or(part).trim();
                        if let Some(rest) = after_dot.strip_suffix("passed") {
                            if let Ok(n) = rest.trim().parse::<u64>() {
                                passed_count = n;
                            }
                        }
                    }
                }
                continue;
            }

            // ---- Detect failure block start ----
            // cargo test outputs "---- test_name stdout ----" before each failure
            if trimmed.starts_with("----") && trimmed.ends_with("----") {
                if in_failure_block && !failure_block.is_empty() {
                    // Save previous block
                    if failures.len() < Self::MAX_FAILURE_BLOCKS {
                        failures.push(format!(
                            "--- {} ---\n{}",
                            failure_name,
                            failure_block.join("\n")
                        ));
                    }
                    failure_block.clear();
                }
                // Extract name between the dashes
                let inner = trimmed.trim_start_matches('-').trim_end_matches('-').trim();
                // Strip " stdout" suffix if present
                failure_name = inner
                    .strip_suffix("stdout")
                    .unwrap_or(inner)
                    .trim()
                    .to_string();
                in_failure_block = true;
                continue;
            }

            // ---- Detect "failures:" header which ends failure blocks ----
            if trimmed == "failures:" {
                if in_failure_block && !failure_block.is_empty() {
                    if failures.len() < Self::MAX_FAILURE_BLOCKS {
                        failures.push(format!(
                            "--- {} ---\n{}",
                            failure_name,
                            failure_block.join("\n")
                        ));
                    }
                    failure_block.clear();
                }
                in_failure_block = false;
                continue;
            }

            // ---- Collect failure block lines ----
            if in_failure_block && failure_block.len() < Self::MAX_LINES_PER_FAILURE {
                failure_block.push(line.clone());
                continue;
            }

            // ---- Detect compiler errors ----
            // "error[E0308]: mismatched types" or "error: ..."
            if trimmed.starts_with("error[") || trimmed.starts_with("error:") {
                compiler_errors.push(trimmed.to_string());
            }
        }

        // Flush any dangling failure block
        if in_failure_block
            && !failure_block.is_empty()
            && failures.len() < Self::MAX_FAILURE_BLOCKS
        {
            failures.push(format!(
                "--- {} ---\n{}",
                failure_name,
                failure_block.join("\n")
            ));
        }

        // Build structured JSON summary
        let summary_json = serde_json::json!({
            "passed": passed_count,
            "failed": failed_count,
            "ignored": ignored_count,
            "result_line": result_line,
            "failures": failures,
            "compiler_errors": compiler_errors,
            "exit_code": raw.exit_code,
        });

        OutputSummary {
            summary: serde_json::to_string_pretty(&summary_json).unwrap_or_default(),
        }
    }
}

// ---------------------------------------------------------------------------
// 3. GitSummaryPolicy — parse git status / git diff --stat
// ---------------------------------------------------------------------------

pub struct GitSummaryPolicy;

impl OutputPolicy for GitSummaryPolicy {
    fn shape(&self, raw: &RawOutput, _op: &ResolvedOperation) -> OutputSummary {
        let combined = format!("{}{}", raw.stdout, raw.stderr);
        let lines: Vec<&str> = combined.lines().collect();

        let mut branch = String::new();
        let mut staged: Vec<String> = Vec::new();
        let mut unstaged: Vec<String> = Vec::new();
        let mut untracked: Vec<String> = Vec::new();
        let mut ahead: u64 = 0;
        let mut behind: u64 = 0;

        // State tracking for long-form `git status`
        let mut section = Section::None;

        for raw_line in &lines {
            let line = strip_ansi(raw_line);
            let trimmed = line.trim();

            // Skip hint lines (parenthetical instructions)
            if trimmed.starts_with("(use \"git") || trimmed.starts_with("(use 'git") {
                continue;
            }

            // ---- Branch detection ----
            // Long form: "On branch main"
            if let Some(rest) = trimmed.strip_prefix("On branch ") {
                branch = rest.trim().to_string();
                continue;
            }
            // Short form: "## main...origin/main [ahead 2, behind 1]"
            if trimmed.starts_with("## ") {
                let status_part = trimmed.strip_prefix("## ").unwrap_or(trimmed);
                // Branch name is before "..."
                if let Some(dotdot_idx) = status_part.find("...") {
                    branch = status_part[..dotdot_idx].to_string();
                } else {
                    // No tracking info, just the branch name (may have trailing space)
                    branch = status_part
                        .split_whitespace()
                        .next()
                        .unwrap_or(status_part)
                        .to_string();
                }
                // Parse ahead/behind
                if let Some(bracket_start) = status_part.find('[') {
                    if let Some(bracket_end) = status_part.find(']') {
                        let info = &status_part[bracket_start + 1..bracket_end];
                        for part in info.split(',') {
                            let part = part.trim();
                            if let Some(rest) = part.strip_prefix("ahead ") {
                                ahead = rest.trim().parse().unwrap_or(0);
                            } else if let Some(rest) = part.strip_prefix("behind ") {
                                behind = rest.trim().parse().unwrap_or(0);
                            }
                        }
                    }
                }
                continue;
            }

            // ---- Ahead/behind from long-form ----
            // "Your branch is ahead of 'origin/main' by 2 commits."
            if trimmed.contains("ahead of") {
                for word in trimmed.split_whitespace() {
                    if let Ok(n) = word.parse::<u64>() {
                        ahead = n;
                        break;
                    }
                }
                continue;
            }
            if trimmed.contains("behind") && trimmed.contains("by") {
                for word in trimmed.split_whitespace() {
                    if let Ok(n) = word.parse::<u64>() {
                        behind = n;
                        break;
                    }
                }
                continue;
            }

            // ---- Section headers (long-form git status) ----
            if trimmed.starts_with("Changes to be committed:") {
                section = Section::Staged;
                continue;
            }
            if trimmed.starts_with("Changes not staged for commit:") {
                section = Section::Unstaged;
                continue;
            }
            if trimmed.starts_with("Untracked files:") {
                section = Section::Untracked;
                continue;
            }

            // Empty line resets section
            if trimmed.is_empty() {
                section = Section::None;
                continue;
            }

            // ---- Collect files per section ----
            match section {
                Section::Staged => {
                    // Lines like "	modified:   src/main.rs" or "	new file:   foo.rs"
                    if let Some(colon_idx) = trimmed.find(':') {
                        let file = trimmed[colon_idx + 1..].trim().to_string();
                        if !file.is_empty() {
                            staged.push(file);
                        }
                    }
                }
                Section::Unstaged => {
                    if let Some(colon_idx) = trimmed.find(':') {
                        let file = trimmed[colon_idx + 1..].trim().to_string();
                        if !file.is_empty() {
                            unstaged.push(file);
                        }
                    }
                }
                Section::Untracked => {
                    // Just filenames, no prefix
                    if !trimmed.is_empty() {
                        untracked.push(trimmed.to_string());
                    }
                }
                Section::None => {}
            }

            // ---- Short-form status (porcelain-like) ----
            // "M  src/lib.rs" / " M src/lib.rs" / "?? new_file.rs" / "A  added.rs"
            if trimmed.len() >= 4 && raw_line.len() >= 3 {
                let first = raw_line.as_bytes().first().copied().unwrap_or(b' ');
                let second = raw_line.as_bytes().get(1).copied().unwrap_or(b' ');
                let third = raw_line.as_bytes().get(2).copied().unwrap_or(b' ');
                if third == b' ' && section == Section::None {
                    let file = raw_line.get(3..).unwrap_or("").trim().to_string();
                    if !file.is_empty() {
                        if first == b'?' && second == b'?' {
                            untracked.push(file);
                        } else {
                            if first != b' ' {
                                staged.push(file.clone());
                            }
                            if second != b' ' {
                                unstaged.push(file);
                            }
                        }
                    }
                }
            }
        }

        // Deduplicate (short-form may add same file to both collections above)
        staged.dedup();
        unstaged.dedup();
        untracked.dedup();

        let summary_json = serde_json::json!({
            "branch": branch,
            "staged": staged,
            "unstaged": unstaged,
            "untracked": untracked,
            "ahead": ahead,
            "behind": behind,
            "exit_code": raw.exit_code,
        });

        OutputSummary {
            summary: serde_json::to_string_pretty(&summary_json).unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    None,
    Staged,
    Unstaged,
    Untracked,
}

// ---------------------------------------------------------------------------
// 4. DiffSummaryPolicy — parse unified diff output
// ---------------------------------------------------------------------------

pub struct DiffSummaryPolicy;

impl DiffSummaryPolicy {
    /// Maximum hunk headers to include per file.
    const MAX_HUNKS_PER_FILE: usize = 10;
}

impl OutputPolicy for DiffSummaryPolicy {
    fn shape(&self, raw: &RawOutput, _op: &ResolvedOperation) -> OutputSummary {
        let combined = format!("{}{}", raw.stdout, raw.stderr);
        let lines: Vec<&str> = combined.lines().collect();

        let mut files: Vec<DiffFileSummary> = Vec::new();
        let mut current_file = String::new();
        let mut hunks: Vec<String> = Vec::new();
        let mut additions: u64 = 0;
        let mut deletions: u64 = 0;
        let mut total_additions: u64 = 0;
        let mut total_deletions: u64 = 0;

        for raw_line in &lines {
            let line = strip_ansi(raw_line);
            let trimmed = line.trim();

            // Detect file names from "+++ b/path" lines
            if let Some(rest) = trimmed.strip_prefix("+++ ") {
                // Flush previous file
                if !current_file.is_empty() {
                    files.push(DiffFileSummary {
                        file: current_file.clone(),
                        hunks: hunks.len() as u64,
                        hunk_headers: hunks.clone(),
                        additions,
                        deletions,
                    });
                    total_additions += additions;
                    total_deletions += deletions;
                    hunks.clear();
                    additions = 0;
                    deletions = 0;
                }
                // Strip "b/" prefix if present
                current_file = rest.strip_prefix("b/").unwrap_or(rest).trim().to_string();
                continue;
            }

            // Skip "--- a/path" lines (we already got file name from +++)
            if trimmed.starts_with("--- ") {
                continue;
            }

            // Hunk headers: @@ -old,count +new,count @@ optional context
            if trimmed.starts_with("@@ ") {
                if hunks.len() < Self::MAX_HUNKS_PER_FILE {
                    hunks.push(trimmed.to_string());
                }
                continue;
            }

            // Count additions and deletions within hunks
            if trimmed.starts_with('+') && !trimmed.starts_with("+++") {
                additions += 1;
            } else if trimmed.starts_with('-') && !trimmed.starts_with("---") {
                deletions += 1;
            }
        }

        // Flush last file
        if !current_file.is_empty() {
            total_additions += additions;
            total_deletions += deletions;
            files.push(DiffFileSummary {
                file: current_file,
                hunks: hunks.len() as u64,
                hunk_headers: hunks,
                additions,
                deletions,
            });
        }

        let file_summaries: Vec<serde_json::Value> = files
            .iter()
            .map(|f| {
                serde_json::json!({
                    "file": f.file,
                    "hunks": f.hunks,
                    "hunk_headers": f.hunk_headers,
                    "additions": f.additions,
                    "deletions": f.deletions,
                })
            })
            .collect();

        let summary_json = serde_json::json!({
            "files_changed": files.len(),
            "total_additions": total_additions,
            "total_deletions": total_deletions,
            "files": file_summaries,
            "exit_code": raw.exit_code,
        });

        OutputSummary {
            summary: serde_json::to_string_pretty(&summary_json).unwrap_or_default(),
        }
    }
}

struct DiffFileSummary {
    file: String,
    hunks: u64,
    hunk_headers: Vec<String>,
    additions: u64,
    deletions: u64,
}

// ---------------------------------------------------------------------------
// 5. SearchSummaryPolicy — parse grep / ripgrep output
// ---------------------------------------------------------------------------

pub struct SearchSummaryPolicy;

impl SearchSummaryPolicy {
    /// Maximum snippet lines to include per file in the summary.
    const MAX_SNIPPETS_PER_FILE: usize = 5;
}

impl OutputPolicy for SearchSummaryPolicy {
    fn shape(&self, raw: &RawOutput, _op: &ResolvedOperation) -> OutputSummary {
        let combined = format!("{}{}", raw.stdout, raw.stderr);
        let lines: Vec<&str> = combined.lines().collect();

        // Map of filename -> list of matching lines
        let mut file_matches: HashMap<String, Vec<String>> = HashMap::new();
        let mut total_matches: u64 = 0;

        for raw_line in &lines {
            let line = strip_ansi(raw_line);
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // ripgrep / grep format: "file:line:content" or "file:content"
            // Also handle "file-line-content" (context lines in rg)
            if let Some((file, _rest)) = parse_search_line(trimmed) {
                total_matches += 1;
                let snippets = file_matches.entry(file).or_default();
                if snippets.len() < Self::MAX_SNIPPETS_PER_FILE {
                    snippets.push(trimmed.to_string());
                }
            }
        }

        let file_summaries: Vec<serde_json::Value> = file_matches
            .iter()
            .map(|(file, snippets)| {
                serde_json::json!({
                    "file": file,
                    "match_count": snippets.len(),
                    "snippets": snippets,
                })
            })
            .collect();

        let summary_json = serde_json::json!({
            "total_matches": total_matches,
            "files_matched": file_matches.len(),
            "files": file_summaries,
            "exit_code": raw.exit_code,
        });

        OutputSummary {
            summary: serde_json::to_string_pretty(&summary_json).unwrap_or_default(),
        }
    }
}

/// Try to parse a grep/rg output line into (filename, rest).
/// Handles `file:line:content` and `file:content` formats.
fn parse_search_line(line: &str) -> Option<(String, String)> {
    // Find the first colon that looks like a file separator.
    // Skip Windows drive letters like "C:\"
    let start = if line.len() >= 2
        && line.as_bytes()[0].is_ascii_alphabetic()
        && line.as_bytes()[1] == b':'
    {
        2
    } else {
        0
    };

    if let Some(colon_idx) = line[start..].find(':') {
        let file = &line[..start + colon_idx];
        let rest = &line[start + colon_idx + 1..];
        // Validate: file part should not be empty and should look like a path
        if !file.is_empty() && !file.contains(' ') {
            return Some((file.to_string(), rest.to_string()));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// 6. LogSummaryPolicy — parse log output
// ---------------------------------------------------------------------------

pub struct LogSummaryPolicy;

impl LogSummaryPolicy {
    /// Maximum unique error/warning messages to include.
    const MAX_UNIQUE_MESSAGES: usize = 20;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
enum Severity {
    Fatal,
    Error,
    Warning,
    Info,
}

impl OutputPolicy for LogSummaryPolicy {
    fn shape(&self, raw: &RawOutput, _op: &ResolvedOperation) -> OutputSummary {
        let combined = format!("{}{}", raw.stdout, raw.stderr);
        let lines: Vec<&str> = combined.lines().collect();

        let mut counts: HashMap<Severity, u64> = HashMap::new();
        let mut unique_messages: Vec<(Severity, String)> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

        for raw_line in &lines {
            let line = strip_ansi(raw_line);
            let lower = line.to_ascii_lowercase();
            let trimmed = line.trim();

            if trimmed.is_empty() {
                continue;
            }

            let severity = if contains_severity(&lower, "fatal") {
                Some(Severity::Fatal)
            } else if contains_severity(&lower, "error") {
                Some(Severity::Error)
            } else if contains_severity(&lower, "warn") {
                Some(Severity::Warning)
            } else if contains_severity(&lower, "info") {
                Some(Severity::Info)
            } else {
                None
            };

            if let Some(sev) = severity {
                *counts.entry(sev).or_insert(0) += 1;

                // Deduplicate: only keep first occurrence of each unique message
                // Normalize by stripping timestamps (common ISO-like prefixes)
                let normalized = normalize_log_message(trimmed);
                if !seen.contains(&normalized) && unique_messages.len() < Self::MAX_UNIQUE_MESSAGES
                {
                    seen.insert(normalized);
                    unique_messages.push((sev, trimmed.to_string()));
                }
            }
        }

        let messages: Vec<serde_json::Value> = unique_messages
            .iter()
            .map(|(sev, msg)| {
                serde_json::json!({
                    "severity": sev,
                    "message": msg,
                })
            })
            .collect();

        let summary_json = serde_json::json!({
            "fatal_count": counts.get(&Severity::Fatal).unwrap_or(&0),
            "error_count": counts.get(&Severity::Error).unwrap_or(&0),
            "warning_count": counts.get(&Severity::Warning).unwrap_or(&0),
            "info_count": counts.get(&Severity::Info).unwrap_or(&0),
            "unique_messages": messages,
            "exit_code": raw.exit_code,
        });

        OutputSummary {
            summary: serde_json::to_string_pretty(&summary_json).unwrap_or_default(),
        }
    }
}

/// Check if a lowered line contains a severity keyword in a meaningful position
/// (start of line, after a timestamp, or surrounded by brackets/delimiters).
/// The keyword may appear as a prefix of a longer word (e.g. "warn" matches
/// "warning"), as long as it starts at a word boundary.
fn contains_severity(lower: &str, keyword: &str) -> bool {
    let mut search_from = 0;
    while let Some(rel_idx) = lower[search_from..].find(keyword) {
        let idx = search_from + rel_idx;
        let before_ok = idx == 0 || {
            let prev = lower.as_bytes()[idx - 1];
            prev == b' '
                || prev == b'['
                || prev == b':'
                || prev == b'|'
                || prev == b'\t'
                || prev == b'('
        };
        if before_ok {
            return true;
        }
        search_from = idx + keyword.len();
    }
    false
}

/// Strip common timestamp prefixes from a log line for deduplication.
/// E.g. "2026-06-03T10:30:00Z ERROR something" -> "ERROR something"
fn normalize_log_message(line: &str) -> String {
    let trimmed = line.trim();
    // Try to strip an ISO-like timestamp (at least "YYYY-MM-DD" prefix)
    if trimmed.len() >= 10
        && trimmed.as_bytes()[4] == b'-'
        && trimmed.as_bytes()[7] == b'-'
        && trimmed.as_bytes()[0..4].iter().all(|b| b.is_ascii_digit())
    {
        // Skip until we find a space after the timestamp portion
        // Timestamps can be up to ~25 chars: "2026-06-03T10:30:00.123Z "
        let after_ts = trimmed
            .char_indices()
            .skip(10)
            .find(|(_, c)| *c == ' ' || *c == '\t')
            .map(|(i, _)| i + 1)
            .unwrap_or(0);
        return trimmed[after_ts..].trim().to_string();
    }
    trimmed.to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "output_policy_tests.rs"]
mod tests;
