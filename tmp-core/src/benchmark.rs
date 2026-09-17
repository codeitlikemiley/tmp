use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkRun {
    pub run_id: String,
    pub task_id: String,
    pub mode: String,
    pub surface: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub tool_calls: u32,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub raw_output_bytes: u64,
    pub shaped_output_bytes: u64,
    pub output_signal_preserved: bool,
    pub wall_time_ms: u64,
    pub success: bool,
    pub invalid_operation_attempted: bool,
    pub user_clarifications: u32,
    pub failed_attempts: u32,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkComparison {
    pub task_id: String,
    pub baseline: BenchmarkRun,
    pub assisted: BenchmarkRun,
    pub tool_call_reduction: i32,
    pub input_token_reduction: i32,
    pub output_token_reduction: i32,
    pub output_reduction_ratio: f64,
    pub latency_reduction_ms: i64,
    pub invalid_operation_delta: i32,
}

impl BenchmarkRun {
    pub fn load(path: &Path) -> Result<Self, String> {
        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(path, json).map_err(|e| e.to_string())
    }

    /// Validate against the benchmark schema
    pub fn validate(&self) -> Result<(), String> {
        if self.run_id.is_empty() {
            return Err("run_id cannot be empty".to_string());
        }
        if self.task_id.is_empty() {
            return Err("task_id cannot be empty".to_string());
        }
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
        if !valid_modes.contains(&self.mode.as_str()) {
            return Err(format!(
                "invalid mode '{}'. Valid: {:?}",
                self.mode, valid_modes
            ));
        }
        Ok(())
    }
}

pub fn compare(baseline: &BenchmarkRun, assisted: &BenchmarkRun) -> BenchmarkComparison {
    let output_reduction_ratio = if baseline.raw_output_bytes > 0 {
        1.0 - (assisted.shaped_output_bytes as f64 / baseline.raw_output_bytes as f64)
    } else {
        0.0
    };

    BenchmarkComparison {
        task_id: baseline.task_id.clone(),
        baseline: baseline.clone(),
        assisted: assisted.clone(),
        tool_call_reduction: baseline.tool_calls as i32 - assisted.tool_calls as i32,
        input_token_reduction: baseline.input_tokens as i32 - assisted.input_tokens as i32,
        output_token_reduction: baseline.output_tokens as i32 - assisted.output_tokens as i32,
        output_reduction_ratio,
        latency_reduction_ms: baseline.wall_time_ms as i64 - assisted.wall_time_ms as i64,
        invalid_operation_delta: baseline.failed_attempts as i32 - assisted.failed_attempts as i32,
    }
}

#[cfg(test)]
#[path = "benchmark_tests.rs"]
mod tests;
