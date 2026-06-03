#![allow(dead_code, unused)]
// E2E Test - Tier 3: Cross-Feature Combinations (Pairwise Coverage)
mod common;
use common::TestSandbox;
use std::fs;

/// Test 1: Init + Registry Install + Schema List (Runnable!)
/// Verifies the end-to-end integration of initializing a configuration,
/// installing a schema from a mock registry, and listing installed schemas.
#[test]
fn test_tier3_init_install_list() {
    let sandbox = TestSandbox::new();

    // 1. Run init to set up config
    let output_init = sandbox.run(&["init"]);
    assert!(output_init.status.success());
    assert!(sandbox.config_dir.join("config.toml").exists());

    // 2. Setup mock registry with cargo schema
    let index_json = r#"{
        "schemas": [
            {
                "tool": "cargo",
                "version": "0.1.0",
                "author": "E2E Test",
                "commands_count": 5,
                "verified": true,
                "download_url": "MOCK_URL",
                "description": "Cargo package manager schema"
            }
        ]
    }"#;
    let registry_dir = sandbox.temp_dir.path().join("mock_registry");
    let cargo_schema_file = registry_dir.join("cargo.json");
    let cargo_schema_url = format!("file://{}", cargo_schema_file.display());
    let index_json = index_json.replace("MOCK_URL", &cargo_schema_url);

    let cargo_schema_content = r#"{
        "meta": {
            "name": "cargo",
            "version": "0.1.0",
            "author": "E2E Test",
            "verified": true,
            "coverage": 100,
            "keywords": ["rust", "cargo"],
            "requires_file": "Cargo.toml",
            "requires_binary": "cargo"
        },
        "commands": []
    }"#;
    sandbox.write_mock_registry(&index_json, &[("cargo", cargo_schema_content)]);

    // 3. Install cargo schema
    let output_install = sandbox.run(&["registry", "install", "cargo"]);
    assert!(output_install.status.success());
    assert!(sandbox
        .config_dir
        .join("schemas")
        .join("cargo.json")
        .exists());

    // 4. List schemas and verify it shows cargo
    let output_list = sandbox.run(&["schema", "list"]);
    assert!(output_list.status.success());
    let stdout = String::from_utf8_lossy(&output_list.stdout);
    assert!(stdout.contains("cargo"));
}

/// Test 2: Init + Compile
#[test]
fn test_tier3_init_and_compile() {
    let sandbox = TestSandbox::new();
    sandbox.run(&["init"]);
    sandbox.setup_cargo_project("test-project", &[]);
    let output = sandbox.run(&["compile"]);
    assert!(output.status.success());
    assert!(sandbox.project_dir.join(".tmp").join("context.md").exists());
}

/// Test 3: Registry Install + Compile
#[test]
fn test_tier3_registry_install_and_compile() {
    let sandbox = TestSandbox::new();
    sandbox.run(&["init"]);

    let index_json = r#"{
        "schemas": [
            {
                "tool": "cargo",
                "version": "0.1.0",
                "author": "E2E Test",
                "commands_count": 5,
                "verified": true,
                "download_url": "MOCK_URL",
                "description": "Cargo package manager schema"
            }
        ]
    }"#;
    let registry_dir = sandbox.temp_dir.path().join("mock_registry");
    let cargo_schema_file = registry_dir.join("cargo.json");
    let cargo_schema_url = format!("file://{}", cargo_schema_file.display());
    let index_json = index_json.replace("MOCK_URL", &cargo_schema_url);

    let cargo_schema_content = r#"{
        "meta": {
            "name": "cargo",
            "version": "0.1.0",
            "author": "E2E Test",
            "verified": true,
            "coverage": 100,
            "keywords": ["rust", "cargo"],
            "requires_file": "Cargo.toml",
            "requires_binary": "cargo"
        },
        "commands": []
    }"#;
    sandbox.write_mock_registry(&index_json, &[("cargo", cargo_schema_content)]);

    sandbox.run(&["registry", "install", "cargo"]);
    sandbox.setup_cargo_project("test-project", &[]);
    let output = sandbox.run(&["compile"]);
    assert!(output.status.success());
    let commands_json =
        fs::read_to_string(sandbox.project_dir.join(".tmp").join("commands.json")).unwrap();
    assert!(commands_json.contains("cargo"));
}

/// Test 4: Generate + Verify (TUI) + Schema List (Runnable with mock server!)
#[test]
fn test_tier3_generate_verify_and_schema_list() {
    let sandbox = TestSandbox::new();
    sandbox.run(&["init"]);

    // Write a mock custom-tool executable to bypass CommandNotFound errors
    let temp_bin_dir = tempfile::tempdir().unwrap();
    let script_path = temp_bin_dir.path().join("custom-tool");
    let script_content = r#"#!/bin/sh
echo "Usage: custom-tool [options]"
echo "Commands:"
echo "  run     Run the custom tool"
"#;
    std::fs::write(&script_path, script_content).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script_path, perms).unwrap();
    }

    let current_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", temp_bin_dir.path().display(), current_path);

    let output_gen = std::process::Command::new(&sandbox.bin_path)
        .args(["generate", "custom-tool", "--verify"])
        .current_dir(&sandbox.project_dir)
        .env("HOME", &sandbox.home_dir)
        .env("TMP_CONFIG_DIR", &sandbox.config_dir)
        .env("PATH", new_path)
        .output()
        .expect("Failed to execute generate command");

    assert!(
        output_gen.status.success(),
        "generate command failed: {}",
        String::from_utf8_lossy(&output_gen.stderr)
    );

    let output_list = sandbox.run(&["schema", "list"]);
    assert!(output_list.status.success());
    assert!(String::from_utf8_lossy(&output_list.stdout).contains("custom-tool"));
}

/// Test 5: Compile + Resolve + Run
#[test]
fn test_tier3_compile_resolve_and_run() {
    let sandbox = TestSandbox::new();
    sandbox.run(&["init"]);

    // Setup a schema for cargo that has the 'run tests' command
    let schemas_dir = sandbox.config_dir.join("schemas");
    fs::create_dir_all(&schemas_dir).unwrap();
    let cargo_schema = r#"{
        "meta": {
            "tool": "cargo",
            "version": 1,
            "verified": true,
            "keywords": ["rust"]
        },
        "commands": [
            {
                "command": "cargo test",
                "description": "run tests",
                "group": "test",
                "tokens": []
            }
        ]
    }"#;
    fs::write(schemas_dir.join("cargo.json"), cargo_schema).unwrap();

    sandbox.setup_cargo_project("test-project", &["my-member"]);
    let compile_out = sandbox.run(&["compile"]);
    assert!(compile_out.status.success());

    let output_resolve = sandbox.run(&["resolve", "run tests"]);
    assert!(output_resolve.status.success());

    let output_run = sandbox.run(&["run"]);
    assert!(output_run.status.success());
}

/// Test 6: Workflow Add + Run + Versioning
#[test]
fn test_tier3_workflow_add_run_and_versioning() {
    let sandbox = TestSandbox::new();
    sandbox.run(&["init"]);

    let wf_content = r#"{
        "name": "release",
        "steps": [
            {
                "name": "step1",
                "command": "echo 'hello'"
            }
        ]
    }"#;
    fs::write(sandbox.project_dir.join("release_wf.json"), wf_content).unwrap();

    let output_add = sandbox.run(&["workflow", "add", "release", "--from", "release_wf.json"]);
    assert!(output_add.status.success());

    let output_run = sandbox.run(&["workflow", "run", "release"]);
    assert!(output_run.status.success());
}

/// Test 7: Init-Agent + Compile
#[test]
fn test_tier3_init_agent_and_compile() {
    let sandbox = TestSandbox::new();
    sandbox.run(&["init-agent", "claude"]);
    sandbox.setup_cargo_project("test-project", &[]);
    let output = sandbox.run(&["compile"]);
    assert!(output.status.success());
    let context_content =
        fs::read_to_string(sandbox.project_dir.join(".tmp").join("context.md")).unwrap();
    assert!(context_content.contains("CLAUDE.md"));
}

/// Test 8: Keywords + Resolve
#[test]
fn test_tier3_keywords_and_resolve() {
    let sandbox = TestSandbox::new();
    sandbox.run(&["init"]);

    // Write a cargo schema file first
    let schemas_dir = sandbox.config_dir.join("schemas");
    fs::create_dir_all(&schemas_dir).unwrap();
    let schema_content = r#"{
        "meta": {
            "tool": "cargo",
            "version": 1,
            "verified": true,
            "keywords": []
        },
        "commands": [
            {
                "command": "cargo build",
                "description": "build command",
                "group": "build",
                "tokens": []
            }
        ]
    }"#;
    fs::write(schemas_dir.join("cargo.json"), schema_content).unwrap();

    sandbox.run(&["schema", "keywords", "cargo", "rust-build"]);
    let output = sandbox.run(&["resolve", "rust-build"]);
    assert!(output.status.success());
}

/// Test 9: Registry Publish + Import
#[test]
fn test_tier3_registry_publish_and_import() {
    let sandbox = TestSandbox::new();
    sandbox.run(&["init"]);

    let schemas_dir = sandbox.config_dir.join("schemas");
    fs::create_dir_all(&schemas_dir).unwrap();
    let schema_content = r#"{
        "meta": {
            "tool": "cargo",
            "version": 1,
            "verified": true,
            "keywords": []
        },
        "commands": []
    }"#;
    fs::write(schemas_dir.join("cargo.json"), schema_content).unwrap();

    let output_share = sandbox.run(&["schema", "share", "cargo"]);
    assert!(output_share.status.success());

    let output_pub = sandbox.run(&["registry", "publish", "cargo"]);
    assert!(output_pub.status.success());

    let output = sandbox.run(&["schema", "import", "cargo.json"]);
    assert!(output.status.success());
}
