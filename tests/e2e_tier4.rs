#![allow(dead_code, unused)]
// E2E Test - Tier 4: Real-World Application Scenarios
mod common;
use common::TestSandbox;
use std::fs;

/// Scenario 1: Cargo Workspace End-to-End Pipeline
#[test]
fn test_tier4_scenario1_cargo_workspace_pipeline() {
    let sandbox = TestSandbox::new();

    // 1. Initialize project config
    let output_init = sandbox.run(&["init"]);
    assert!(output_init.status.success());

    // Write cargo schema so resolve matches it
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
                "description": "run unit tests",
                "group": "test",
                "tokens": []
            }
        ]
    }"#;
    fs::write(schemas_dir.join("cargo.json"), cargo_schema).unwrap();

    // 2. Setup mock cargo workspace project with two packages: app and core_lib
    sandbox.setup_cargo_project("cargo-workspace-demo", &["app", "core_lib"]);

    // 3. Compile context
    let output_compile = sandbox.run(&["compile"]);
    assert!(output_compile.status.success());

    // 4. Resolve a query to run unit tests
    let output_resolve = sandbox.run(&["resolve", "run unit tests", "--json"]);
    assert!(output_resolve.status.success());
    let stdout_resolve = String::from_utf8_lossy(&output_resolve.stdout);
    assert!(stdout_resolve.contains("cargo test"));

    // 5. Run the resolved command
    let output_run = sandbox.run(&["run"]);
    assert!(output_run.status.success());
}

/// Scenario 2: Node.js/npm App E2E Integration
#[test]
fn test_tier4_scenario2_npm_project_pipeline() {
    let sandbox = TestSandbox::new();

    // 1. Initialize project config
    let output_init = sandbox.run(&["init"]);
    assert!(output_init.status.success());

    // Write npm schema so resolve matches it
    let schemas_dir = sandbox.config_dir.join("schemas");
    fs::create_dir_all(&schemas_dir).unwrap();
    let npm_schema = r#"{
        "meta": {
            "tool": "npm",
            "version": 1,
            "verified": true,
            "keywords": ["js"]
        },
        "commands": [
            {
                "command": "npm run lint",
                "description": "run linting",
                "group": "lint",
                "tokens": []
            }
        ]
    }"#;
    fs::write(schemas_dir.join("npm.json"), npm_schema).unwrap();

    // 2. Setup npm package.json with scripts
    let package_json = r#"{
        "name": "npm-e2e-demo",
        "version": "1.0.0",
        "scripts": {
            "test": "echo 'running tests'",
            "lint": "echo 'running lint'"
        }
    }"#;
    sandbox.setup_npm_project(package_json);

    // Create a mock npm executable to bypass Node.js/npm dependency
    let temp_bin_dir = tempfile::tempdir().unwrap();
    let script_path = temp_bin_dir.path().join(if cfg!(target_os = "windows") {
        "npm.cmd"
    } else {
        "npm"
    });
    let script_content = if cfg!(target_os = "windows") {
        "@exit /b 0\n"
    } else {
        "#!/bin/sh\nexit 0\n"
    };
    fs::write(&script_path, script_content).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).unwrap();
    }

    let current_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", temp_bin_dir.path().display(), current_path);

    // 3. Compile context
    let output_compile = std::process::Command::new(&sandbox.bin_path)
        .arg("compile")
        .current_dir(&sandbox.project_dir)
        .env("HOME", &sandbox.home_dir)
        .env("TMP_CONFIG_DIR", &sandbox.config_dir)
        .env("PATH", &new_path)
        .output()
        .expect("Failed compile");
    assert!(output_compile.status.success());

    // 4. Resolve a query for running lint
    let output_resolve = std::process::Command::new(&sandbox.bin_path)
        .args(["resolve", "run linting", "--json"])
        .current_dir(&sandbox.project_dir)
        .env("HOME", &sandbox.home_dir)
        .env("TMP_CONFIG_DIR", &sandbox.config_dir)
        .env("PATH", &new_path)
        .output()
        .expect("Failed resolve");
    assert!(output_resolve.status.success());
    let stdout_resolve = String::from_utf8_lossy(&output_resolve.stdout);
    assert!(stdout_resolve.contains("npm run lint"));

    // 5. Run the resolved command
    let output_run = std::process::Command::new(&sandbox.bin_path)
        .arg("run")
        .current_dir(&sandbox.project_dir)
        .env("HOME", &sandbox.home_dir)
        .env("TMP_CONFIG_DIR", &sandbox.config_dir)
        .env("PATH", &new_path)
        .output()
        .expect("Failed run");
    assert!(output_run.status.success());
}

/// Scenario 3: New Tool Schema Bootstrapping
#[test]
fn test_tier4_scenario3_bootstrap_new_tool() {
    let sandbox = TestSandbox::new();

    // 1. Initialize project config
    let output_init = sandbox.run(&["init"]);
    assert!(output_init.status.success());

    // Write a mock docker executable to bypass CommandNotFound errors
    let temp_bin_dir = tempfile::tempdir().unwrap();
    let script_path = temp_bin_dir.path().join("docker");
    let script_content = r#"#!/bin/sh
echo "Usage: docker [options]"
echo "Commands:"
echo "  run     Run a container"
"#;
    std::fs::write(&script_path, script_content).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script_path, perms).unwrap();
    }

    let response_body = serde_json::json!({
        "candidates": [
            {
                "content": {
                    "parts": [
                        {
                            "text": "{\n  \"meta\": {\n    \"tool\": \"docker\",\n    \"version\": 1,\n    \"verified\": true,\n    \"keywords\": []\n  },\n  \"commands\": []\n}"
                        }
                    ]
                }
            }
        ]
    }).to_string();

    let server = common::MockHttpServer::start(move |_req| response_body.clone());

    let current_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", temp_bin_dir.path().display(), current_path);

    // 2. Run generate to bootstrap a schema for a new tool (e.g. Docker CLI commands)
    let output_gen = std::process::Command::new(&sandbox.bin_path)
        .args(["generate", "docker"])
        .current_dir(&sandbox.project_dir)
        .env("HOME", &sandbox.home_dir)
        .env("TMP_CONFIG_DIR", &sandbox.config_dir)
        .env("GEMINI_API_KEY", "mock_key")
        .env(
            "GEMINI_BASE_URL",
            format!("http://127.0.0.1:{}", server.port),
        )
        .env("PATH", &new_path)
        .output()
        .expect("Failed to execute generate command");
    assert!(
        output_gen.status.success(),
        "generate command failed: {}",
        String::from_utf8_lossy(&output_gen.stderr)
    );

    // 3. Run generate with verify to start the verify flow
    let output_verify = std::process::Command::new(&sandbox.bin_path)
        .args(["generate", "docker", "--verify"])
        .current_dir(&sandbox.project_dir)
        .env("HOME", &sandbox.home_dir)
        .env("TMP_CONFIG_DIR", &sandbox.config_dir)
        .env("GEMINI_API_KEY", "mock_key")
        .env(
            "GEMINI_BASE_URL",
            format!("http://127.0.0.1:{}", server.port),
        )
        .env("PATH", &new_path)
        .output()
        .expect("Failed to execute generate command");
    assert!(
        output_verify.status.success(),
        "generate --verify command failed: {}",
        String::from_utf8_lossy(&output_verify.stderr)
    );

    // 4. Check if docker schema is listed in schema list
    let output_list = sandbox.run(&["schema", "list"]);
    assert!(output_list.status.success());
    let stdout_list = String::from_utf8_lossy(&output_list.stdout);
    assert!(stdout_list.contains("docker"));
}

/// Scenario 4: Multi-Step Release Workflow
#[test]
fn test_tier4_scenario4_release_workflow() {
    let sandbox = TestSandbox::new();

    // 1. Initialize project config
    let output_init = sandbox.run(&["init"]);
    assert!(output_init.status.success());

    // Write cargo schema to schemas directory so "tmp schema share cargo" works
    let schemas_dir = sandbox.config_dir.join("schemas");
    fs::create_dir_all(&schemas_dir).unwrap();
    let cargo_schema = r#"{
        "meta": {
            "tool": "cargo",
            "version": 1,
            "verified": true,
            "keywords": []
        },
        "commands": []
    }"#;
    fs::write(schemas_dir.join("cargo.json"), cargo_schema).unwrap();

    // Setup cargo project structure with a member so cargo test succeeds
    sandbox.setup_cargo_project("demo", &["my-member"]);

    // 2. Add a custom release workflow file
    let release_wf = r#"{
        "name": "release",
        "steps": [
            {
                "name": "test",
                "command": "cargo test"
            },
            {
                "name": "share-schema",
                "command": "tmp schema share cargo"
            }
        ]
    }"#;
    let wf_path = sandbox.temp_dir.path().join("release_wf.json");
    fs::write(&wf_path, release_wf).unwrap();

    // 3. Add the workflow to tmp
    let output_add = sandbox.run(&[
        "workflow",
        "add",
        "release",
        "--from",
        wf_path.to_str().unwrap(),
    ]);
    assert!(output_add.status.success());

    // 4. Run the release workflow with tmp binary path in PATH
    let bin_dir = sandbox.bin_path.parent().unwrap();
    let current_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), current_path);

    let output_run = std::process::Command::new(&sandbox.bin_path)
        .args(["workflow", "run", "release"])
        .current_dir(&sandbox.project_dir)
        .env("HOME", &sandbox.home_dir)
        .env("TMP_CONFIG_DIR", &sandbox.config_dir)
        .env("PATH", new_path)
        .output()
        .expect("Failed to execute workflow run");

    assert!(
        output_run.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output_run.stderr)
    );
}

/// Scenario 5: AI Agent Interaction Simulation
#[test]
fn test_tier4_scenario5_agent_interaction() {
    let sandbox = TestSandbox::new();

    // 1. Initialize agent bridge files
    let output_agent = sandbox.run(&["init-agent", "claude"]);
    assert!(output_agent.status.success());
    assert!(sandbox.project_dir.join("CLAUDE.md").exists());

    // 2. Setup project structure
    sandbox.setup_cargo_project("agent-demo-app", &[]);

    // 3. Compile context for agent use
    let output_compile = sandbox.run(&["compile"]);
    assert!(output_compile.status.success());

    // 4. Read compiled context and verify it references rule structures
    let context_md = sandbox.project_dir.join(".tmp").join("context.md");
    assert!(context_md.exists());
    let context_content = fs::read_to_string(context_md).unwrap();
    assert!(context_content.contains("CLAUDE.md"));
}
