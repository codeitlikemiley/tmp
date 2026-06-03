use crate::compile::ResolvedOperation;
use crate::context::Context;
use crate::schema::{EvidenceType, Operation, Parameter};
use command::Command;

#[derive(Debug, Clone)]
pub struct RawFact {
    pub source: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct OperationDraft {
    pub operation: Operation,
}

#[derive(Debug, Clone)]
pub struct InvocationResult {
    pub success: bool,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone)]
pub struct RawOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Debug, Clone)]
pub struct OutputSummary {
    pub summary: String,
}

#[derive(Debug, Clone)]
pub struct EvidenceResult {
    pub verified: bool,
    pub evidence: Vec<EvidenceType>,
}

pub trait SurfaceAdapter {
    fn discover(&self, context: &Context) -> Vec<RawFact>;
    fn map(&self, facts: &[RawFact]) -> Vec<OperationDraft>;
    fn invoke(&self, op: &ResolvedOperation) -> InvocationResult;
}

pub trait Resolver {
    fn values(&self, parameter: &Parameter, context: &Context) -> Result<Vec<String>, String>;
}

pub trait OutputPolicy {
    fn shape(&self, raw: &RawOutput, op: &ResolvedOperation) -> OutputSummary;
}

pub trait EvidenceCheck {
    fn verify(&self, operation: &Operation) -> EvidenceResult;
}

pub struct CliSurfaceAdapter;

impl SurfaceAdapter for CliSurfaceAdapter {
    fn discover(&self, context: &Context) -> Vec<RawFact> {
        let binary = context
            .recommended_target
            .clone()
            .or_else(|| context.package_name.clone())
            .unwrap_or_else(|| "cargo".to_string());
        if let Ok(merged_help) = crate::help::parse_recursive_help(&binary) {
            vec![RawFact {
                source: binary,
                content: merged_help,
            }]
        } else {
            vec![]
        }
    }

    fn map(&self, facts: &[RawFact]) -> Vec<OperationDraft> {
        let mut drafts = Vec::new();
        for fact in facts {
            let schema = crate::generate::generate_schema_from_help(&fact.source, &fact.content);
            for op in schema.operations {
                drafts.push(OperationDraft { operation: op });
            }
        }
        drafts
    }

    fn invoke(&self, op: &ResolvedOperation) -> InvocationResult {
        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", &op.command]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", &op.command]);
            c
        };

        match cmd.output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                InvocationResult {
                    success: output.status.success(),
                    exit_code: output.status.code().unwrap_or(0),
                    stdout,
                    stderr,
                }
            }
            Err(e) => InvocationResult {
                success: false,
                exit_code: -1,
                stdout: String::new(),
                stderr: e.to_string(),
            },
        }
    }
}
