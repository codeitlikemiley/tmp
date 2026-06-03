use super::*;
use crate::context::Context;
use crate::schema::{Command, Schema, SchemaMeta, Token, TokenType};

fn dummy_schema_for_resolve(tool: &str, command_str: &str, tokens: Vec<Token>) -> Schema {
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
        commands: vec![Command {
            command: command_str.to_string(),
            description: "Some command description".to_string(),
            group: tool.to_string(),
            verified: true,
            tokens,
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
        Token {
            name: "target".to_string(),
            description: "target token".to_string(),
            required: true,
            token_type: TokenType::String,
            default: None,
            values: None,
            flag: Some("--target".to_string()),
            data_source: None,
        },
        Token {
            name: "verbose".to_string(),
            description: "verbose option".to_string(),
            required: false,
            token_type: TokenType::Boolean,
            default: None,
            values: None,
            flag: Some("--verbose".to_string()),
            data_source: None,
        },
    ];

    // Case 1: Template contains placeholders
    let template = "run <target> {verbose}";
    let filled = vec![
        TokenFill {
            name: "target".to_string(),
            value: "my;val".to_string(),
            source: "test".to_string(),
        },
        TokenFill {
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
    let filled_only_required = vec![TokenFill {
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
        Token {
            name: "branch".to_string(),
            description: "branch name".to_string(),
            required: true,
            token_type: TokenType::Enum,
            default: Some("main".to_string()),
            values: Some(vec!["main".to_string(), "develop".to_string()]),
            flag: Some("-b".to_string()),
            data_source: None,
        },
        Token {
            name: "remote".to_string(),
            description: "remote name".to_string(),
            required: false,
            token_type: TokenType::String,
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
        .tokens_filled
        .iter()
        .find(|t| t.name == "branch")
        .unwrap();
    assert_eq!(fill1.value, "develop");
    assert_eq!(fill1.source, "Heuristic match from query");

    // Case 2: branch falls back to default "main" when not in query
    let res_default = heuristic_resolve("git checkout please", &schemas, &context, None).unwrap();
    assert_eq!(res_default.command, "git checkout main");
    let fill2 = res_default
        .tokens_filled
        .iter()
        .find(|t| t.name == "branch")
        .unwrap();
    assert_eq!(fill2.value, "main");
    assert_eq!(fill2.source, "Default value");
}
