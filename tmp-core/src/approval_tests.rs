use super::*;
use crate::schema::{Approval, Effect, Operation, OutputMode, OutputPolicyConfig, Risk, Surface};

/// Helper to create a test operation with specified approval, risk, and effect.
fn make_op(approval: Approval, risk: Risk, effect: Effect) -> Operation {
    Operation {
        command: "test-cmd".to_string(),
        description: "test operation".to_string(),
        group: "test".to_string(),
        verified: false,
        parameters: vec![],
        surface: Surface::Cli,
        effect,
        risk,
        approval,
        evidence: vec![],
        output_policy: OutputPolicyConfig {
            mode: OutputMode::Raw,
            raw_retention: None,
        },
    }
}

#[test]
fn test_approval_required_blocks_without_consent() {
    let op = make_op(Approval::Required, Risk::High, Effect::Destructive);
    let decision = check_approval(&op, false);
    assert!(!decision.allowed);
    assert!(decision.reason.contains("requires approval"));
}

#[test]
fn test_approval_required_allows_with_consent() {
    let op = make_op(Approval::Required, Risk::High, Effect::Destructive);
    let decision = check_approval(&op, true);
    assert!(decision.allowed);
    assert!(decision.reason.contains("User approved"));
}

#[test]
fn test_recommended_blocks_high_risk_without_consent() {
    let op = make_op(Approval::Recommended, Risk::High, Effect::Network);
    let decision = check_approval(&op, false);
    assert!(!decision.allowed);
    assert!(decision.reason.contains("high risk"));
    assert!(decision.reason.contains("recommended"));
}

#[test]
fn test_recommended_allows_low_risk_without_consent() {
    let op = make_op(Approval::Recommended, Risk::Low, Effect::ReadOnly);
    let decision = check_approval(&op, false);
    assert!(decision.allowed);
    assert!(decision.reason.contains("not required"));
}

#[test]
fn test_recommended_allows_high_risk_with_consent() {
    let op = make_op(Approval::Recommended, Risk::High, Effect::Network);
    let decision = check_approval(&op, true);
    assert!(decision.allowed);
}

#[test]
fn test_not_required_allows_normal_operations() {
    let op = make_op(Approval::NotRequired, Risk::Low, Effect::ReadOnly);
    let decision = check_approval(&op, false);
    assert!(decision.allowed);
    assert!(decision.reason.contains("No approval required"));
}

#[test]
fn test_not_required_allows_medium_risk() {
    let op = make_op(Approval::NotRequired, Risk::Medium, Effect::BuildTest);
    let decision = check_approval(&op, false);
    assert!(decision.allowed);
}

#[test]
fn test_destructive_high_risk_fail_closed_invariant() {
    // Even with NotRequired approval, destructive + high risk is blocked
    let op = make_op(Approval::NotRequired, Risk::High, Effect::Destructive);
    let decision = check_approval(&op, false);
    assert!(!decision.allowed);
    assert!(decision.reason.contains("destructive with high risk"));
}

#[test]
fn test_destructive_high_risk_allowed_with_consent() {
    let op = make_op(Approval::NotRequired, Risk::High, Effect::Destructive);
    let decision = check_approval(&op, true);
    assert!(decision.allowed);
}

#[test]
fn test_destructive_low_risk_not_blocked() {
    // Destructive + low risk is not blocked by fail-closed
    let op = make_op(Approval::NotRequired, Risk::Low, Effect::Destructive);
    let decision = check_approval(&op, false);
    assert!(decision.allowed);
}

#[test]
fn test_needs_approval_required() {
    let op = make_op(Approval::Required, Risk::Low, Effect::ReadOnly);
    assert!(needs_approval(&op));
}

#[test]
fn test_needs_approval_recommended_high_risk() {
    let op = make_op(Approval::Recommended, Risk::High, Effect::Network);
    assert!(needs_approval(&op));
}

#[test]
fn test_needs_approval_recommended_low_risk() {
    let op = make_op(Approval::Recommended, Risk::Low, Effect::ReadOnly);
    assert!(!needs_approval(&op));
}

#[test]
fn test_needs_approval_destructive_high_risk() {
    let op = make_op(Approval::NotRequired, Risk::High, Effect::Destructive);
    assert!(needs_approval(&op));
}

#[test]
fn test_needs_approval_not_required_normal() {
    let op = make_op(Approval::NotRequired, Risk::Low, Effect::ReadOnly);
    assert!(!needs_approval(&op));
}

#[test]
fn test_needs_approval_not_required_medium_risk() {
    let op = make_op(Approval::NotRequired, Risk::Medium, Effect::BuildTest);
    assert!(!needs_approval(&op));
}

#[test]
fn approval_for_run_rejects_noninteractive_without_yes() {
    let op = make_op(Approval::Required, Risk::High, Effect::Destructive);
    let err = approval_for_run(&op, false, false).unwrap_err();
    assert!(err.contains("test-cmd"), "{err}");
    assert!(err.contains("approval"), "{err}");
}

#[test]
fn approval_for_run_skips_prompt_when_yes() {
    let op = make_op(Approval::Required, Risk::High, Effect::Destructive);
    assert_eq!(approval_for_run(&op, true, false), Ok(()));
}

#[test]
fn approval_for_run_allows_low_risk_without_yes() {
    let op = make_op(Approval::NotRequired, Risk::Low, Effect::ReadOnly);
    assert_eq!(approval_for_run(&op, false, false), Ok(()));
}
