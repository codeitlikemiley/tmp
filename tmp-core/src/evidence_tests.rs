use super::*;
use crate::schema::{
    Approval, Effect, Operation, OutputMode, OutputPolicyConfig, Risk, Schema, SchemaMeta, Surface,
};

fn op(command: &str) -> Operation {
    Operation {
        command: command.to_string(),
        description: "test".to_string(),
        group: "test".to_string(),
        verified: false,
        parameters: vec![],
        surface: Surface::Cli,
        effect: Effect::BuildTest,
        risk: Risk::Low,
        approval: Approval::NotRequired,
        evidence: vec![],
        output_policy: OutputPolicyConfig {
            mode: OutputMode::Raw,
            raw_retention: None,
        },
    }
}

fn schema_with(operations: Vec<Operation>) -> Schema {
    Schema {
        meta: SchemaMeta {
            tool: "test".to_string(),
            version: 1,
            author: None,
            generated_by: None,
            generated_with: None,
            verified: false,
            verified_at: None,
            coverage: None,
            waz_version: None,
            requires_file: None,
            requires_file_kind: None,
            requires_binary: None,
            keywords: vec![],
        },
        operations,
    }
}

#[test]
fn help_evidence_check_records_parsed_help_for_available_binary() {
    let result = HelpEvidenceCheck.verify(&op("sh"));
    assert!(
        result.evidence.contains(&EvidenceType::ParsedHelp),
        "expected ParsedHelp, got {:?}",
        result.evidence
    );
    assert!(result.verified);
}

#[test]
fn help_evidence_check_does_not_verify_missing_binary() {
    let result = HelpEvidenceCheck.verify(&op("tmp-missing-binary-xyz"));
    assert!(result.evidence.is_empty());
    assert!(!result.verified);
}

#[test]
fn apply_evidence_is_idempotent() {
    let mut schema = schema_with(vec![op("sh")]);
    let checker = HelpEvidenceCheck;
    apply_evidence(&mut schema, &checker);
    let first = schema.operations[0].evidence.clone();
    apply_evidence(&mut schema, &checker);
    assert_eq!(schema.operations[0].evidence, first);
    assert_eq!(
        first
            .iter()
            .filter(|e| **e == EvidenceType::ParsedHelp)
            .count(),
        1
    );
}

#[test]
fn apply_evidence_sets_meta_verified_when_every_operation_is_verified() {
    let mut schema = schema_with(vec![op("sh")]);
    apply_evidence(&mut schema, &HelpEvidenceCheck);
    assert!(schema.meta.verified);
    assert!(schema.operations[0].verified);
}
