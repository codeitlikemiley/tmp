// Fail-closed invariant tests for TMP whitepaper §Fail-Closed Invariants.
//
// These tests verify the seven invariants listed in the whitepaper:
// 1. Unknown intent does not invoke anything.
// 2. Draft maps are never treated as verified.
// 3. High-risk effects require approval.
// 4. Dynamic resolver failure is visible.
// 5. Raw output is retained when output is shaped.
// 6. Generated RTK filters remain draft until tested.
// 7. Core resolver is deterministic.

use crate::approval::{check_approval, needs_approval};
use crate::compile::ResolvedOperation;
use crate::context::Context;
use crate::generate::generate_schema_from_help;
use crate::output_policy::retain_raw_output;
use crate::resolve::heuristic_resolve;
use crate::resolver::DataResolver;
use crate::schema::{
    Approval, DataSource, Effect, Operation, OutputMode, OutputPolicyConfig, Risk, Schema,
    SchemaMeta, Surface,
};
use crate::traits::RawOutput;

/// Helper: create a minimal context for testing.
fn test_context() -> Context {
    Context {
        cwd: "/tmp/test".to_string(),
        project_root: None,
        build_system: "none".to_string(),
        file_kind: "standalone".to_string(),
        script_engine: None,
        recommended_target: None,
        package_name: None,
        packages: vec!["myapp".to_string()],
        bins: vec!["mybin".to_string()],
        examples: Vec::new(),
        features: Vec::new(),
        profiles: Vec::new(),
        tests: vec!["test_core".to_string()],
        benches: Vec::new(),
        git_branches: vec!["main".to_string()],
        git_remotes: vec!["origin".to_string()],
        npm_scripts: Vec::new(),
    }
}

/// Helper: create a minimal valid schema for testing.
fn test_schema() -> Schema {
    Schema {
        meta: SchemaMeta {
            tool: "test-tool".to_string(),
            version: 1,
            author: None,
            generated_by: None,
            generated_with: None,
            verified: true,
            verified_at: None,
            coverage: None,
            waz_version: None,
            requires_file: None,
            requires_file_kind: None,
            requires_binary: None,
            keywords: vec!["test".to_string()],
        },
        operations: vec![Operation {
            command: "test-tool run".to_string(),
            description: "Run tests".to_string(),
            group: "testing".to_string(),
            verified: true,
            parameters: Vec::new(),
            surface: Surface::Cli,
            effect: Effect::BuildTest,
            risk: Risk::Low,
            approval: Approval::NotRequired,
            evidence: Vec::new(),
            output_policy: OutputPolicyConfig {
                mode: OutputMode::Raw,
                raw_retention: None,
            },
        }],
    }
}

/// Helper: build an operation with specific risk/effect/approval.
fn make_operation(command: &str, effect: Effect, risk: Risk, approval: Approval) -> Operation {
    Operation {
        command: command.to_string(),
        description: "test operation".to_string(),
        group: "test".to_string(),
        verified: true,
        parameters: Vec::new(),
        surface: Surface::Cli,
        effect,
        risk,
        approval,
        evidence: Vec::new(),
        output_policy: OutputPolicyConfig {
            mode: OutputMode::Raw,
            raw_retention: None,
        },
    }
}

// -----------------------------------------------------------------------
// Invariant 1: Unknown intent does not invoke anything.
// -----------------------------------------------------------------------

#[test]
fn invariant_unknown_intent_returns_none() {
    let ctx = test_context();
    let schemas = vec![test_schema()];

    // Completely unrelated gibberish should not match any operation.
    let result = heuristic_resolve("xyzzy flurble zogzog", &schemas, &ctx, None);
    assert!(
        result.is_none(),
        "Unknown intent must NOT resolve to any operation (fail closed)"
    );
}

#[test]
fn invariant_unknown_intent_returns_error_via_resolve_api() {
    // When using the resolve() wrapper, it should produce an Err for gibberish.
    // We can't call resolve() directly because it reads schemas from disk,
    // but we can verify that heuristic_resolve returns None for unknown intents,
    // which means the resolve() wrapper will produce the appropriate Err.
    let ctx = test_context();
    let schemas = vec![test_schema()];

    let result = heuristic_resolve("!@#$%^&*()_+", &schemas, &ctx, None);
    assert!(
        result.is_none(),
        "Gibberish input must fail closed — no operation invented"
    );
}

// -----------------------------------------------------------------------
// Invariant 2: Draft maps are never treated as verified.
// -----------------------------------------------------------------------

#[test]
fn invariant_generated_schema_is_never_verified() {
    let help_text = r#"
Usage: mytool <COMMAND>

Commands:
  deploy     Deploy the application
  rollback   Rollback to previous version

Options:
  -v, --verbose   Enable verbose output
"#;

    let schema = generate_schema_from_help("mytool", help_text);

    // Schema-level must be unverified
    assert!(
        !schema.meta.verified,
        "Generated schema meta must have verified=false"
    );

    // Every operation must also be unverified
    for op in &schema.operations {
        assert!(
            !op.verified,
            "Generated operation '{}' must have verified=false",
            op.command
        );
    }

    // generated_by should indicate draft provenance
    assert_eq!(
        schema.meta.generated_by.as_deref(),
        Some("tmp generate"),
        "Generated schema should record its provenance"
    );
}

#[test]
fn invariant_verified_false_persists_through_serialization() {
    let help_text = "Usage: tool\nCommands:\n  check  Check things\n";
    let schema = generate_schema_from_help("tool", help_text);

    let json = schema.to_json().expect("serialize");
    let roundtrip: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(
        roundtrip["meta"]["verified"],
        serde_json::Value::Bool(false),
        "verified=false must survive serialization round-trip"
    );
}

// -----------------------------------------------------------------------
// Invariant 3: High-risk effects require approval.
// -----------------------------------------------------------------------

#[test]
fn invariant_high_risk_required_approval_blocks_without_user_consent() {
    let op = make_operation(
        "deploy production",
        Effect::Destructive,
        Risk::High,
        Approval::Required,
    );

    let decision = check_approval(&op, false);
    assert!(
        !decision.allowed,
        "High-risk required-approval operation must be blocked without user consent"
    );
}

#[test]
fn invariant_high_risk_recommended_approval_blocks_without_user_consent() {
    let op = make_operation(
        "deploy staging",
        Effect::Deployment,
        Risk::High,
        Approval::Recommended,
    );

    let decision = check_approval(&op, false);
    assert!(
        !decision.allowed,
        "High-risk recommended-approval operation must be blocked without user consent"
    );
}

#[test]
fn invariant_destructive_high_risk_blocks_even_if_not_required() {
    // Even when approval is NotRequired, destructive + high-risk should still block.
    let op = make_operation(
        "rm -rf /important",
        Effect::Destructive,
        Risk::High,
        Approval::NotRequired,
    );

    let decision = check_approval(&op, false);
    assert!(
        !decision.allowed,
        "Destructive + high-risk must block even when approval=NotRequired (fail-closed)"
    );
    assert!(
        needs_approval(&op),
        "needs_approval() must return true for destructive + high-risk"
    );
}

#[test]
fn invariant_high_risk_allowed_with_user_approval() {
    let op = make_operation(
        "deploy production",
        Effect::Destructive,
        Risk::High,
        Approval::Required,
    );

    let decision = check_approval(&op, true);
    assert!(
        decision.allowed,
        "High-risk operation must be allowed when user explicitly approves"
    );
}

// -----------------------------------------------------------------------
// Invariant 4: Dynamic resolver failure is visible.
// -----------------------------------------------------------------------

#[test]
fn invariant_resolver_unknown_resolver_returns_error() {
    let ds = DataSource {
        command: None,
        resolver: Some("nonexistent:resolver".to_string()),
        parse: "lines".to_string(),
    };
    let ctx = test_context();

    let result = DataResolver::resolve(&ds, &ctx);
    assert!(
        result.is_err(),
        "Unknown resolver must return Err, not silently succeed"
    );

    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("Unknown") || err_msg.contains("unsupported"),
        "Error message must indicate the resolver is unknown: {}",
        err_msg
    );
}

#[test]
fn invariant_resolver_no_source_returns_error() {
    let ds = DataSource {
        command: None,
        resolver: None,
        parse: "lines".to_string(),
    };
    let ctx = test_context();

    let result = DataResolver::resolve(&ds, &ctx);
    assert!(
        result.is_err(),
        "DataSource with neither command nor resolver must return Err"
    );
}

// -----------------------------------------------------------------------
// Invariant 5: Raw output is retained when output is shaped.
// -----------------------------------------------------------------------

#[test]
fn invariant_raw_output_retained_on_disk() {
    let raw = RawOutput {
        stdout: "test result: ok. 5 passed; 0 failed; 0 ignored\n".to_string(),
        stderr: String::new(),
        exit_code: 0,
    };

    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let result = retain_raw_output(&raw, tmp_dir.path());
    assert!(result.is_ok(), "retain_raw_output must succeed");

    let path_str = result.unwrap();
    let path = std::path::Path::new(&path_str);
    assert!(path.exists(), "Raw output file must exist on disk");

    let content = std::fs::read_to_string(path).expect("read raw log");
    assert!(
        content.contains("STDOUT"),
        "Raw log must contain stdout marker"
    );
    assert!(
        content.contains("5 passed"),
        "Raw log must preserve the original stdout content"
    );
}

#[test]
fn invariant_shaped_output_is_smaller_than_raw() {
    let raw = RawOutput {
        stdout: "running 100 tests\ntest a1 ... ok\ntest a2 ... ok\ntest a3 ... ok\n\
                 test a4 ... ok\ntest a5 ... ok\ntest a6 ... ok\ntest a7 ... ok\n\
                 test a8 ... ok\ntest a9 ... ok\ntest a10 ... ok\n\
                 test result: ok. 100 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n"
            .to_string(),
        stderr: String::new(),
        exit_code: 0,
    };

    let op = ResolvedOperation {
        command: "cargo test".to_string(),
        description: "Run tests".to_string(),
        group: "build".to_string(),
        verified: true,
        parameters: Vec::new(),
    };

    let policy = crate::output_policy::get_policy(&OutputMode::TestSummary);
    let summary = policy.shape(&raw, &op);

    // The shaped output should be a structured JSON summary, not the full raw text.
    // Verify it is genuinely smaller than passing through everything.
    assert!(
        summary.summary.len() < raw.stdout.len() + raw.stderr.len(),
        "Shaped output ({} bytes) should be smaller than raw ({} bytes)",
        summary.summary.len(),
        raw.stdout.len() + raw.stderr.len()
    );
}

// -----------------------------------------------------------------------
// Invariant 6: Generated RTK filters remain draft until tested.
// -----------------------------------------------------------------------

#[test]
fn invariant_generated_schema_operations_are_drafts() {
    let help = "Usage: codegen\nOptions:\n  --out <DIR>  Output directory\n";
    let schema = generate_schema_from_help("codegen", help);

    // Every generated operation is a draft (verified=false)
    for op in &schema.operations {
        assert!(
            !op.verified,
            "Generated operation '{}' must remain unverified (draft) until tested",
            op.command
        );
    }

    // The generated_with field should indicate this is a draft
    assert!(
        schema
            .meta
            .generated_with
            .as_deref()
            .unwrap_or("")
            .contains("draft"),
        "generated_with should indicate draft status"
    );
}

#[test]
fn invariant_strict_validation_warns_on_high_risk_no_approval() {
    // Schemas with high-risk + NotRequired should produce a warning
    let schema = Schema {
        meta: SchemaMeta {
            tool: "danger-tool".to_string(),
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
            keywords: Vec::new(),
        },
        operations: vec![Operation {
            command: "danger-tool destroy".to_string(),
            description: "Destroy everything".to_string(),
            group: "danger".to_string(),
            verified: false,
            parameters: Vec::new(),
            surface: Surface::Cli,
            effect: Effect::Destructive,
            risk: Risk::High,
            approval: Approval::NotRequired,
            evidence: Vec::new(),
            output_policy: OutputPolicyConfig {
                mode: OutputMode::Raw,
                raw_retention: None,
            },
        }],
    };

    let warnings = schema.validate_strict().expect("validation should pass");
    assert!(
        !warnings.is_empty(),
        "Strict validation must warn about high-risk + NotRequired approval"
    );
}

// -----------------------------------------------------------------------
// Invariant 7: Core resolver is deterministic.
// -----------------------------------------------------------------------

#[test]
fn invariant_resolver_is_deterministic() {
    let ctx = test_context();
    let schemas = vec![test_schema()];

    // Run the same query multiple times — result must be identical each time.
    let query = "run tests";
    let results: Vec<_> = (0..5)
        .map(|_| heuristic_resolve(query, &schemas, &ctx, None))
        .collect();

    // All results must be identical
    for (i, result) in results.iter().enumerate().skip(1) {
        match (result, &results[0]) {
            (Some(a), Some(b)) => {
                assert_eq!(
                    a.command, b.command,
                    "Resolver must produce identical command on run {} vs run 0",
                    i
                );
                assert_eq!(
                    a.confidence, b.confidence,
                    "Resolver must produce identical confidence on run {} vs run 0",
                    i
                );
                assert_eq!(
                    a.tool, b.tool,
                    "Resolver must produce identical tool on run {} vs run 0",
                    i
                );
            }
            (None, None) => {} // both None is consistent
            _ => panic!(
                "Resolver must be deterministic: run {} differs from run 0",
                i
            ),
        }
    }
}

#[test]
fn invariant_builtin_resolver_is_deterministic() {
    let ds = DataSource {
        command: None,
        resolver: Some("cargo:packages".to_string()),
        parse: "lines".to_string(),
    };
    let ctx = test_context();

    let r1 = DataResolver::resolve(&ds, &ctx).expect("resolve 1");
    let r2 = DataResolver::resolve(&ds, &ctx).expect("resolve 2");
    let r3 = DataResolver::resolve(&ds, &ctx).expect("resolve 3");

    assert_eq!(
        r1, r2,
        "Builtin resolver must be deterministic (run 1 vs 2)"
    );
    assert_eq!(
        r2, r3,
        "Builtin resolver must be deterministic (run 2 vs 3)"
    );
    assert_eq!(
        r1,
        vec!["myapp".to_string()],
        "cargo:packages resolver must return context packages"
    );
}
