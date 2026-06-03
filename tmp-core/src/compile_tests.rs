use super::*;
use crate::context::Context;
use crate::schema::{Command, Schema, SchemaMeta, Token, TokenType};
use std::fs;
use tempfile::tempdir;

fn dummy_schema(
    tool: &str,
    binary: Option<&str>,
    file: Option<&str>,
    file_kind: Option<&str>,
) -> Schema {
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
            requires_file: file.map(|s| s.to_string()),
            requires_file_kind: file_kind.map(|s| s.to_string()),
            requires_binary: binary.map(|s| s.to_string()),
            keywords: vec![],
        },
        commands: vec![Command {
            command: format!("{} run", tool),
            description: "runs tool".to_string(),
            group: tool.to_string(),
            verified: true,
            tokens: vec![Token {
                name: "param".to_string(),
                description: "a param".to_string(),
                required: true,
                token_type: TokenType::String,
                default: Some("default_val".to_string()),
                values: None,
                flag: Some("--param".to_string()),
                data_source: None,
            }],
        }],
    }
}

#[test]
fn test_compiler_relevance_binary_available() {
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

    // "nonexistentbin12345" should not be available
    let schema = dummy_schema("mytool", Some("nonexistentbin12345"), None, None);
    assert!(!Compiler::is_schema_relevant(&schema, &context));

    // A common binary like "cargo" or "git" (or shell command "sh") might be available,
    // but to be safe we can check "sh".
    let has_sh = Compiler::is_binary_available("sh");
    let schema_sh = dummy_schema("mytool", Some("sh"), None, None);
    assert_eq!(Compiler::is_schema_relevant(&schema_sh, &context), has_sh);
}

#[test]
fn test_compiler_relevance_file_exists() {
    let temp_dir = tempdir().unwrap();
    let root = temp_dir.path();

    let context = Context {
        cwd: root.to_string_lossy().to_string(),
        project_root: Some(root.to_string_lossy().to_string()),
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

    // Requires file "target.txt" which does not exist
    let schema = dummy_schema("mytool", None, Some("target.txt"), None);
    assert!(!Compiler::is_schema_relevant(&schema, &context));

    // Create the file
    fs::write(root.join("target.txt"), "hello").unwrap();
    assert!(Compiler::is_schema_relevant(&schema, &context));
}

#[test]
fn test_compiler_relevance_file_kind() {
    let context = Context {
        cwd: "/dummy".to_string(),
        project_root: None,
        build_system: "none".to_string(),
        file_kind: "cargo_project".to_string(),
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

    let schema_cargo = dummy_schema("mytool", None, None, Some("cargo_project"));
    assert!(Compiler::is_schema_relevant(&schema_cargo, &context));

    let schema_npm = dummy_schema("mytool", None, None, Some("npm_project"));
    assert!(!Compiler::is_schema_relevant(&schema_npm, &context));
}

#[test]
fn test_compile_and_write_to_disk() {
    let temp_dir = tempdir().unwrap();
    let root = temp_dir.path();
    let config_dir = root.join("config");
    fs::create_dir_all(&config_dir).unwrap();

    let config_path = config_dir.join("config.toml");
    fs::write(&config_path, "[llm]").unwrap();

    let schemas_dir = config_dir.join("schemas");
    fs::create_dir_all(&schemas_dir).unwrap();

    let schema = dummy_schema("git", None, None, None);
    let schema_json = serde_json::to_string(&schema).unwrap();
    fs::write(schemas_dir.join("git.json"), schema_json).unwrap();

    let context = Context {
        cwd: root.to_string_lossy().to_string(),
        project_root: Some(root.to_string_lossy().to_string()),
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

    // Test Compiler::compile
    let output = Compiler::compile(root, &context, Some(config_path.to_str().unwrap())).unwrap();
    assert_eq!(output.commands.len(), 1);
    assert_eq!(output.commands[0].command, "git run");

    // Create CLAUDE.md to check if it's picked up in generate_markdown
    fs::write(root.join("CLAUDE.md"), "CLAUDE RULES").unwrap();

    let markdown = Compiler::generate_markdown(&output);
    assert!(markdown.contains("CLAUDE RULES"));
    assert!(markdown.contains("git run"));

    // Test write_to_disk
    Compiler::write_to_disk(root, &output).unwrap();

    let tmp_dir = root.join(".tmp");
    assert!(tmp_dir.join("commands.json").exists());
    assert!(tmp_dir.join("context.md").exists());

    // Check gitignore
    let gitignore = root.join(".gitignore");
    assert!(gitignore.exists());
    let gitignore_content = fs::read_to_string(gitignore).unwrap();
    assert!(gitignore_content.contains(".tmp/"));
}
