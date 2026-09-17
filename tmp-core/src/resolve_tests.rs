use super::*;
use crate::context::Context;
use crate::schema::{
    Approval, Effect, Operation, OutputMode, OutputPolicyConfig, Parameter, ParameterType, Risk,
    Schema, SchemaMeta, Surface,
};

fn dummy_schema_for_resolve(tool: &str, command_str: &str, tokens: Vec<Parameter>) -> Schema {
    Schema {
        meta: SchemaMeta {
            tool: tool.to_string(),
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
            keywords: vec!["mykeyword".to_string()],
        },
        operations: vec![Operation {
            command: command_str.to_string(),
            description: "Some command description".to_string(),
            group: tool.to_string(),
            verified: true,
            parameters: tokens,
            surface: Surface::Cli,
            effect: Effect::BuildTest,
            risk: Risk::Low,
            approval: Approval::NotRequired,
            evidence: vec![],
            output_policy: OutputPolicyConfig {
                mode: OutputMode::Raw,
                raw_retention: None,
            },
        }],
    }
}

#[test]
fn test_escape_token_value() {
    assert_eq!(escape_token_value("simple"), "simple");
    assert_eq!(escape_token_value("val;echo 1"), "val\\;echo 1");
    assert_eq!(
        escape_token_value("a&b|c<d>e`f$g\\h(i)j\"k'l*m?n[o]p!q{r}s\nt"),
        "a\\&b\\|c\\<d\\>e\\`f\\$g\\\\h\\(i\\)j\\\"k\\'l\\*m\\?n\\[o\\]p\\!q\\{r\\}s\\\nt"
    );
}

#[test]
fn test_construct_final_command() {
    let tokens = vec![
        Parameter {
            name: "target".to_string(),
            description: "target token".to_string(),
            required: true,
            parameter_type: ParameterType::String,
            default: None,
            values: None,
            flag: Some("--target".to_string()),
            data_source: None,
        },
        Parameter {
            name: "verbose".to_string(),
            description: "verbose option".to_string(),
            required: false,
            parameter_type: ParameterType::Boolean,
            default: None,
            values: None,
            flag: Some("--verbose".to_string()),
            data_source: None,
        },
    ];

    // Case 1: Template contains placeholders
    let template = "run <target> {verbose}";
    let filled = vec![
        ParameterFill {
            name: "target".to_string(),
            value: "my;val".to_string(),
            source: "test".to_string(),
        },
        ParameterFill {
            name: "verbose".to_string(),
            value: "true".to_string(),
            source: "test".to_string(),
        },
    ];
    let res = construct_final_command(template, &tokens, &filled);
    assert_eq!(res, "run my\\;val true");

    // Case 2: Template doesn't contain placeholders, append flag
    let template_no_placeholders = "run";
    let res = construct_final_command(template_no_placeholders, &tokens, &filled);
    assert_eq!(res, "run --target my\\;val --verbose true");

    // Case 3: Optional placeholder not filled -> flag and placeholder removed
    let filled_only_required = vec![ParameterFill {
        name: "target".to_string(),
        value: "val".to_string(),
        source: "test".to_string(),
    }];
    let template_with_optional = "run --target <target> --verbose {verbose}";
    let res = construct_final_command(template_with_optional, &tokens, &filled_only_required);
    assert_eq!(res, "run --target val");
}

#[test]
fn test_heuristic_resolve_match_scores() {
    let context = Context {
        cwd: "/dummy".to_string(),
        project_root: None,
        build_system: "none".to_string(),
        file_kind: "standalone".to_string(),
        script_engine: None,
        recommended_target: None,
        package_name: None,
        packages: vec![],
        bins: vec![],
        examples: vec![],
        features: vec![],
        profiles: vec![],
        tests: vec![],
        benches: vec![],
        git_branches: vec![],
        git_remotes: vec![],
        npm_scripts: vec![],
    };

    let schema1 = dummy_schema_for_resolve("git", "git status", vec![]);
    let schema2 = dummy_schema_for_resolve("cargo", "cargo build", vec![]);
    let schemas = vec![schema1, schema2];

    // Query "git" should match git status
    let res = heuristic_resolve("git status command", &schemas, &context, None).unwrap();
    assert_eq!(res.tool, "git");
    assert_eq!(res.command, "git status");
    assert_eq!(res.confidence, "high");

    // Query "build" should match cargo build
    let res2 = heuristic_resolve("run cargo build", &schemas, &context, None).unwrap();
    assert_eq!(res2.tool, "cargo");
    assert_eq!(res2.command, "cargo build");

    // Scoped resolution with tool filter
    let res_scoped = heuristic_resolve("build", &schemas, &context, Some("cargo")).unwrap();
    assert_eq!(res_scoped.tool, "cargo");

    // Scoring should filter out non-matching tool
    let res_scoped_fail = heuristic_resolve("build", &schemas, &context, Some("git"));
    assert!(res_scoped_fail.is_none());
}

#[test]
fn test_heuristic_resolve_token_filling() {
    let context = Context {
        cwd: "/dummy".to_string(),
        project_root: None,
        build_system: "none".to_string(),
        file_kind: "standalone".to_string(),
        script_engine: None,
        recommended_target: None,
        package_name: None,
        packages: vec![],
        bins: vec![],
        examples: vec![],
        features: vec![],
        profiles: vec![],
        tests: vec![],
        benches: vec![],
        git_branches: vec![],
        git_remotes: vec![],
        npm_scripts: vec![],
    };

    let tokens = vec![
        Parameter {
            name: "branch".to_string(),
            description: "branch name".to_string(),
            required: true,
            parameter_type: ParameterType::Enum,
            default: Some("main".to_string()),
            values: Some(vec!["main".to_string(), "develop".to_string()]),
            flag: Some("-b".to_string()),
            data_source: None,
        },
        Parameter {
            name: "remote".to_string(),
            description: "remote name".to_string(),
            required: false,
            parameter_type: ParameterType::String,
            default: Some("origin".to_string()),
            values: None,
            flag: None,
            data_source: None,
        },
    ];

    let schema = dummy_schema_for_resolve("git", "git checkout <branch>", tokens);
    let schemas = vec![schema];

    // Case 1: branch matched from query
    let res = heuristic_resolve("git checkout develop", &schemas, &context, None).unwrap();
    assert_eq!(res.command, "git checkout develop");
    let fill1 = res
        .parameters_filled
        .iter()
        .find(|t| t.name == "branch")
        .unwrap();
    assert_eq!(fill1.value, "develop");
    assert_eq!(fill1.source, "Heuristic match from query");

    // Case 2: branch falls back to default "main" when not in query
    let res_default = heuristic_resolve("git checkout please", &schemas, &context, None).unwrap();
    assert_eq!(res_default.command, "git checkout main");
    let fill2 = res_default
        .parameters_filled
        .iter()
        .find(|t| t.name == "branch")
        .unwrap();
    assert_eq!(fill2.value, "main");
    assert_eq!(fill2.source, "Default value");
}

fn empty_context() -> Context {
    Context {
        cwd: "/dummy".to_string(),
        project_root: None,
        build_system: "none".to_string(),
        file_kind: "standalone".to_string(),
        script_engine: None,
        recommended_target: None,
        package_name: None,
        packages: vec![],
        bins: vec![],
        examples: vec![],
        features: vec![],
        profiles: vec![],
        tests: vec![],
        benches: vec![],
        git_branches: vec![],
        git_remotes: vec![],
        npm_scripts: vec![],
    }
}

#[test]
fn heuristic_resolve_includes_operation_risk_and_approval_from_schema() {
    let mut schema = dummy_schema_for_resolve("cargo", "cargo test", vec![]);
    schema.operations[0].risk = Risk::High;
    schema.operations[0].approval = Approval::Required;
    schema.operations[0].effect = Effect::Destructive;
    schema.operations[0].group = "test".to_string();

    let res = heuristic_resolve("cargo test", &[schema], &empty_context(), None).unwrap();
    let gate = res
        .operation
        .expect("resolve result should include operation gate");
    assert_eq!(gate.risk, Risk::High);
    assert_eq!(gate.approval, Approval::Required);
}

#[test]
fn old_last_command_json_deserializes_with_operation_none() {
    let parsed: ResolveResult = serde_json::from_str(r#"{"command":"echo hi"}"#).unwrap();
    assert_eq!(parsed.command, "echo hi");
    assert!(parsed.operation.is_none());
}

#[test]
fn resolve_result_with_operation_round_trips_command_and_gate() {
    let original = ResolveResult {
        command: "cargo test".to_string(),
        tool: "cargo".to_string(),
        explanation: "run tests".to_string(),
        confidence: "high".to_string(),
        parameters_filled: vec![],
        operation: Some(OperationGate {
            effect: Effect::BuildTest,
            risk: Risk::High,
            approval: Approval::Required,
            output_policy: OutputPolicyConfig {
                mode: OutputMode::TestSummary,
                raw_retention: None,
            },
            verified: true,
            group: "test".to_string(),
        }),
    };

    let value = serde_json::to_value(&original).unwrap();
    assert_eq!(value["command"], "cargo test");
    assert_eq!(value["operation"]["risk"], "high");
    assert_eq!(value["operation"]["approval"], "required");
    assert_eq!(value["operation"]["effect"], "build-test");
    assert_eq!(value["operation"]["group"], "test");
    assert_eq!(value["operation"]["output_policy"]["mode"], "test_summary");

    let parsed: ResolveResult = serde_json::from_value(value).unwrap();
    let gate = parsed.operation.expect("gate should round-trip");
    assert_eq!(parsed.command, "cargo test");
    assert_eq!(gate.risk, Risk::High);
    assert_eq!(gate.approval, Approval::Required);
}
