use crate::schema::{Approval, Effect, Operation, Risk};

/// The result of an approval check for an operation.
pub struct ApprovalDecision {
    pub allowed: bool,
    pub reason: String,
}

/// Check whether an operation is approved to proceed.
///
/// Implements a fail-closed invariant: high-risk effects require approval
/// even when the schema says `NotRequired`.
pub fn check_approval(op: &Operation, user_approved: bool) -> ApprovalDecision {
    match op.approval {
        Approval::Required => {
            if user_approved {
                ApprovalDecision {
                    allowed: true,
                    reason: "User approved high-risk operation".to_string(),
                }
            } else {
                ApprovalDecision {
                    allowed: false,
                    reason: format!(
                        "Operation '{}' requires approval (risk: {:?}, effect: {:?})",
                        op.command, op.risk, op.effect
                    ),
                }
            }
        }
        Approval::Recommended => {
            if op.risk == Risk::High && !user_approved {
                ApprovalDecision {
                    allowed: false,
                    reason: format!(
                        "Operation '{}' has high risk and approval is recommended",
                        op.command
                    ),
                }
            } else {
                ApprovalDecision {
                    allowed: true,
                    reason: "Approval recommended but not required".to_string(),
                }
            }
        }
        Approval::NotRequired => {
            // Still enforce fail-closed: destructive + high risk always needs approval
            if op.effect == Effect::Destructive && op.risk == Risk::High && !user_approved {
                ApprovalDecision {
                    allowed: false,
                    reason: format!(
                        "Operation '{}' is destructive with high risk — approval required",
                        op.command
                    ),
                }
            } else {
                ApprovalDecision {
                    allowed: true,
                    reason: "No approval required".to_string(),
                }
            }
        }
    }
}

/// Determine if an operation needs interactive approval based on its metadata.
pub fn needs_approval(op: &Operation) -> bool {
    matches!(op.approval, Approval::Required)
        || (op.approval == Approval::Recommended && op.risk == Risk::High)
        || (op.effect == Effect::Destructive && op.risk == Risk::High)
}

/// Decide whether `tmp run` may proceed without a prompt.
///
/// `yes` is `--yes`. `interactive` is whether stdin is a terminal.
/// `Ok` means the command may run, or the caller should prompt.
/// `Err` means approval is required and the session is not a terminal.
pub fn approval_for_run(op: &Operation, yes: bool, interactive: bool) -> Result<(), String> {
    if yes || !needs_approval(op) {
        return Ok(());
    }
    if interactive {
        return Ok(());
    }
    Err(check_approval(op, false).reason)
}

#[cfg(test)]
#[path = "approval_tests.rs"]
mod tests;
