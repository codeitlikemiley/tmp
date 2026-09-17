use crate::schema::{EvidenceType, Operation, Schema};
use crate::traits::{EvidenceCheck, EvidenceResult};
use command::Command;

/// Records parsed-help and dry-run evidence from the local machine.
///
/// Does not add `HumanReview`. That evidence is only written by an operator.
pub struct HelpEvidenceCheck;

impl EvidenceCheck for HelpEvidenceCheck {
    fn verify(&self, operation: &Operation) -> EvidenceResult {
        let mut evidence = operation.evidence.clone();
        let binary = operation
            .command
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();

        if binary.is_empty() || !crate::utils::is_binary_available(&binary) {
            return EvidenceResult {
                verified: false,
                evidence,
            };
        }

        push_unique(&mut evidence, EvidenceType::ParsedHelp);

        if help_invocation_completes(&binary) {
            push_unique(&mut evidence, EvidenceType::DryRun);
        }

        EvidenceResult {
            verified: evidence.contains(&EvidenceType::ParsedHelp),
            evidence,
        }
    }
}

/// Apply an evidence check to every operation and roll the results up to schema meta.
///
/// Re-running converges: evidence types are unioned, never duplicated.
pub fn apply_evidence(schema: &mut Schema, checker: &impl EvidenceCheck) {
    for op in &mut schema.operations {
        let result = checker.verify(op);
        op.evidence = result.evidence;
        op.verified = result.verified;
    }
    schema.meta.verified =
        !schema.operations.is_empty() && schema.operations.iter().all(|op| op.verified);
}

fn push_unique(evidence: &mut Vec<EvidenceType>, item: EvidenceType) {
    if !evidence.contains(&item) {
        evidence.push(item);
    }
}

fn help_invocation_completes(binary: &str) -> bool {
    Command::new(binary).arg("--help").output().is_ok()
}

#[cfg(test)]
#[path = "evidence_tests.rs"]
mod tests;
