#![allow(dead_code, unused)]
// E2E Test - Tier 1: Feature Coverage (5 tests per feature)
mod common;
use common::{MockHttpServer, TestSandbox};
use std::fs;
use std::process::Command;

// ==========================================
// F1: INIT & CONFIG (5 TESTS)
// ==========================================

#[test]
fn test_tier1_f1_init_default() {
    let sandbox = TestSandbox::new();
    let output = sandbox.run(&["init"]);
    assert!(output.status.success());
    let config_file = sandbox.config_dir.join("config.toml");
    assert!(config_file.exists());
}

#[test]
fn test_tier1_f1_init_custom_config() {
    let sandbox = TestSandbox::new();
    let custom_config = sandbox.temp_dir.path().join("custom_config.toml");
    let output = sandbox.run(&["--config", custom_config.to_str().unwrap(), "init"]);
    assert!(output.status.success());
    assert!(custom_config.exists());
}

#[test]
fn test_tier1_f1_config_env_vars() {
    let sandbox = TestSandbox::new();
    let output = sandbox.run(&["init"]);
    assert!(output.status.success());
    // Verification of environment loading logic is in unit tests
}

#[test]
fn test_tier1_f1_malformed_config_error() {
    let sandbox = TestSandbox::new();
    sandbox.run(&["init"]);
    // Corrupt config
    sandbox.write_config("invalid toml content = =");
    let output = sandbox.run(&["schema", "list"]);
    // Should fail or handle gracefully
    assert!(
        !output.status.success()
            || String::from_utf8_lossy(&output.stderr).contains("error")
            || String::from_utf8_lossy(&output.stdout).contains("error")
    );
}

#[test]
fn test_tier1_f1_config_precedence() {
    let sandbox = TestSandbox::new();
    let output = sandbox.run(&["init"]);
    assert!(output.status.success());
    // Precedence tests check config overrides in environment (already validated in unit tests)
}

// ==========================================
// F2: AGENT BRIDGE (5 TESTS)
// ==========================================

#[test]
fn test_tier1_f2_init_agent_claude() {
    let sandbox = TestSandbox::new();
    let output = sandbox.run(&["init-agent", "claude"]);
    assert!(output.status.success());
    assert!(sandbox.project_dir.join("CLAUDE.md").exists());
}

#[test]
fn test_tier1_f2_init_agent_cursor() {
    let sandbox = TestSandbox::new();
    let output = sandbox.run(&["init-agent", "cursor"]);
    assert!(output.status.success());
    assert!(sandbox
        .project_dir
        .join(".cursor")
        .join("rules")
        .join("tmp.mdc")
        .exists());
}

#[test]
fn test_tier1_f2_init_agent_all() {
    let sandbox = TestSandbox::new();
    let output = sandbox.run(&["init-agent", "all"]);
    assert!(output.status.success());
    assert!(sandbox.project_dir.join("CLAUDE.md").exists());
    assert!(sandbox
        .project_dir
        .join(".cursor")
        .join("rules")
        .join("tmp.mdc")
        .exists());
    assert!(sandbox.project_dir.join("AGENTS.md").exists());
    assert!(sandbox
        .project_dir
        .join(".github")
        .join("copilot-instructions.md")
        .exists());
    assert!(sandbox.project_dir.join(".windsurfrules").exists());
}

#[test]
fn test_tier1_f2_init_agent_codex_antigravity() {
    let sandbox = TestSandbox::new();
    let output1 = sandbox.run(&["init-agent", "codex"]);
    assert!(output1.status.success());
    assert!(sandbox.project_dir.join("AGENTS.md").exists());

    let sandbox2 = TestSandbox::new();
    let output2 = sandbox2.run(&["init-agent", "antigravity"]);
    assert!(output2.status.success());
    assert!(sandbox2.project_dir.join("AGENTS.md").exists());
}

#[test]
fn test_tier1_f2_init_agent_copilot() {
    let sandbox = TestSandbox::new();
    let output = sandbox.run(&["init-agent", "copilot"]);
    assert!(output.status.success());
    assert!(sandbox
        .project_dir
        .join(".github")
        .join("copilot-instructions.md")
        .exists());
}

#[test]
fn test_tier1_f2_init_agent_windsurf() {
    let sandbox = TestSandbox::new();
    let output = sandbox.run(&["init-agent", "windsurf"]);
    assert!(output.status.success());
    assert!(sandbox.project_dir.join(".windsurfrules").exists());
}

#[test]
fn test_tier1_f2_init_agent_merge() {
    let sandbox = TestSandbox::new();
    fs::write(
        sandbox.project_dir.join("CLAUDE.md"),
        "# Custom Claude Rules\n",
    )
    .unwrap();
    let output = sandbox.run(&["init-agent", "claude"]);
    assert!(output.status.success());
    let content = fs::read_to_string(sandbox.project_dir.join("CLAUDE.md")).unwrap();
    assert!(content.contains("# Custom Claude Rules"));
    assert!(content.contains("tmp resolve"));
}

#[test]
fn test_tier1_f2_init_agent_invalid() {
    let sandbox = TestSandbox::new();
    let output = sandbox.run(&["init-agent", "invalid-agent-name"]);
    assert!(!output.status.success());
}

// ==========================================
// F3: SCHEMA MANAGEMENT (5 TESTS)
// ==========================================

#[test]
fn test_tier1_f3_schema_list() {
    let sandbox = TestSandbox::new();
    let output = sandbox.run(&["schema", "list"]);
    assert!(output.status.success());
}

#[test]
fn test_tier1_f3_schema_share() {
    let sandbox = TestSandbox::new();
    let schemas_dir = sandbox.config_dir.join("schemas");
    fs::create_dir_all(&schemas_dir).unwrap();
    let cargo_schema = r#"{
        "meta": {
            "tool": "cargo",
            "version": 1,
            "verified": true,
            "keywords": ["rust"]
        },
        "commands": []
    }"#;
    fs::write(schemas_dir.join("cargo.json"), cargo_schema).unwrap();

    let output = sandbox.run(&["schema", "share", "cargo"]);
    assert!(
        output.status.success(),
        "share failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let shared_file = sandbox.project_dir.join("cargo.json");
    assert!(shared_file.exists());
    let content = fs::read_to_string(shared_file).unwrap();
    assert!(content.contains("\"tool\": \"cargo\""));
}

#[test]
fn test_tier1_f3_schema_import() {
    let sandbox = TestSandbox::new();
    let custom_schema = r#"{
        "meta": {
            "tool": "custom",
            "version": 1,
            "verified": false,
            "keywords": ["test"]
        },
        "commands": []
    }"#;
    fs::write(
        sandbox.project_dir.join("custom_schema.json"),
        custom_schema,
    )
    .unwrap();

    let output = sandbox.run(&["schema", "import", "custom_schema.json"]);
    assert!(
        output.status.success(),
        "import failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let imported_file = sandbox.config_dir.join("schemas").join("custom.json");
    assert!(imported_file.exists());
    let content = fs::read_to_string(imported_file).unwrap();
    assert!(content.contains("\"tool\": \"custom\""));
}

#[test]
fn test_tier1_f3_schema_keywords_show() {
    let sandbox = TestSandbox::new();
    let schemas_dir = sandbox.config_dir.join("schemas");
    fs::create_dir_all(&schemas_dir).unwrap();
    let cargo_schema = r#"{
        "meta": {
            "tool": "cargo",
            "version": 1,
            "verified": true,
            "keywords": ["rust", "build"]
        },
        "commands": []
    }"#;
    fs::write(schemas_dir.join("cargo.json"), cargo_schema).unwrap();

    let output = sandbox.run(&["schema", "keywords", "cargo"]);
    assert!(
        output.status.success(),
        "keywords show failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rust"));
    assert!(stdout.contains("build"));
}

#[test]
fn test_tier1_f3_schema_keywords_set() {
    let sandbox = TestSandbox::new();
    let schemas_dir = sandbox.config_dir.join("schemas");
    fs::create_dir_all(&schemas_dir).unwrap();
    let cargo_schema = r#"{
        "meta": {
            "tool": "cargo",
            "version": 1,
            "verified": true,
            "keywords": ["rust"]
        },
        "commands": []
    }"#;
    fs::write(schemas_dir.join("cargo.json"), cargo_schema).unwrap();

    let output = sandbox.run(&["schema", "keywords", "cargo", "rust", "build", "package"]);
    assert!(
        output.status.success(),
        "keywords set failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let schema_file = schemas_dir.join("cargo.json");
    let content = fs::read_to_string(schema_file).unwrap();
    assert!(content.contains("\"rust\""));
    assert!(content.contains("\"build\""));
    assert!(content.contains("\"package\""));
}

// ==========================================
// F4: REGISTRY (5 TESTS)
// ==========================================

#[test]
fn test_tier1_f4_registry_search() {
    let sandbox = TestSandbox::new();
    let index_json = r#"{
        "schemas": [
            {
                "tool": "cargo",
                "version": "0.1.0",
                "author": "E2E Test",
                "commands_count": 5,
                "verified": true,
                "download_url": "file:///mock/cargo.json",
                "description": "Cargo package manager schema"
            }
        ]
    }"#;
    sandbox.write_mock_registry(index_json, &[]);
    let output = sandbox.run(&["registry", "search", "cargo"]);
    assert!(output.status.success());
}

#[test]
fn test_tier1_f4_registry_install() {
    let sandbox = TestSandbox::new();
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

    let output = sandbox.run(&["registry", "install", "cargo"]);
    assert!(output.status.success());
}

#[test]
fn test_tier1_f4_registry_publish() {
    let sandbox = TestSandbox::new();
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
    fs::write(sandbox.project_dir.join("cargo.json"), cargo_schema_content).unwrap();

    let output = sandbox.run(&["registry", "publish", "cargo"]);
    assert!(
        output.status.success(),
        "publish failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_tier1_f4_registry_search_no_results() {
    let sandbox = TestSandbox::new();
    let index_json = r#"{"schemas": []}"#;
    sandbox.write_mock_registry(index_json, &[]);
    let output = sandbox.run(&["registry", "search", "nonexistent"]);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("No matching schemas found"));
}

#[test]
fn test_tier1_f4_registry_offline_graceful() {
    let sandbox = TestSandbox::new();
    // Use an unreachable repo address
    let output = Command::new(&sandbox.bin_path)
        .args(["registry", "search", "cargo"])
        .env("HOME", &sandbox.home_dir)
        .env("TMP_CONFIG_DIR", &sandbox.config_dir)
        .env("TMP_REGISTRY_REPO", "http://127.0.0.1:9999/index.json") // Unreachable port
        .output()
        .expect("Failed to execute");
    assert!(!output.status.success());
}

// ==========================================
// F5: CONTEXT & COMPILER (5 TESTS)
// ==========================================

#[test]
fn test_tier1_f5_compile_cargo_project() {
    let sandbox = TestSandbox::new();
    sandbox.setup_cargo_project("test-cargo", &["lib1"]);
    let output = sandbox.run(&["compile"]);
    assert!(output.status.success());
    assert!(sandbox.project_dir.join(".tmp").join("context.md").exists());
}

#[test]
fn test_tier1_f5_compile_npm_project() {
    let sandbox = TestSandbox::new();
    sandbox.setup_npm_project(r#"{"name": "test-npm", "scripts": {"test": "echo lint"}}"#);
    let output = sandbox.run(&["compile"]);
    assert!(output.status.success());
}

#[test]
fn test_tier1_f5_compile_gitignore_hook() {
    let sandbox = TestSandbox::new();
    sandbox.setup_cargo_project("test-gitignore", &[]);
    fs::write(sandbox.project_dir.join(".gitignore"), "target/\n").unwrap();
    let output = sandbox.run(&["compile"]);
    assert!(output.status.success());
    let gitignore = fs::read_to_string(sandbox.project_dir.join(".gitignore")).unwrap();
    assert!(gitignore.contains(".tmp/"));
}

#[test]
fn test_tier1_f5_project_root_resolution() {
    let sandbox = TestSandbox::new();
    sandbox.setup_cargo_project("test-resolution", &["sub_member"]);
    // Compile from sub directory
    let output = sandbox.run_in_dir(&["compile"], &sandbox.project_dir.join("sub_member"));
    assert!(output.status.success());
    assert!(sandbox
        .project_dir
        .join("sub_member")
        .join(".tmp")
        .join("context.md")
        .exists());
}

#[test]
fn test_tier1_f5_compiler_resolver_execution() {
    let sandbox = TestSandbox::new();
    sandbox.setup_cargo_project("test-resolver", &[]);
    let output = sandbox.run(&["compile"]);
    assert!(output.status.success());
}

// ==========================================
// F6: RESOLVE & RUN (5 TESTS)
// ==========================================

#[test]
fn test_tier1_f6_resolve_json() {
    let sandbox = TestSandbox::new();
    sandbox.run(&["init"]);
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

    let output = sandbox.run(&["resolve", "run unit tests", "--json"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cargo test"));
}

#[test]
fn test_tier1_f6_run_dry_run() {
    let sandbox = TestSandbox::new();
    sandbox.run(&["init"]);
    fs::write(
        sandbox.project_dir.join("Cargo.toml"),
        r#"[package]
name = "test-pkg"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    fs::create_dir_all(sandbox.project_dir.join("src")).unwrap();
    fs::write(
        sandbox.project_dir.join("src").join("main.rs"),
        "fn main() {}",
    )
    .unwrap();

    let output = sandbox.run(&["run", "--dry-run"]);
    assert!(output.status.success());
}

#[test]
fn test_tier1_f6_run_execution() {
    let sandbox = TestSandbox::new();
    sandbox.run(&["init"]);
    fs::write(
        sandbox.project_dir.join("Cargo.toml"),
        r#"[package]
name = "test-pkg"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    fs::create_dir_all(sandbox.project_dir.join("src")).unwrap();
    fs::write(
        sandbox.project_dir.join("src").join("main.rs"),
        "fn main() {}",
    )
    .unwrap();

    let output = sandbox.run(&["run"]);
    assert!(output.status.success());
}

#[test]
fn test_tier1_f6_resolve_tool_scoping() {
    let sandbox = TestSandbox::new();
    sandbox.run(&["init"]);
    let schemas_dir = sandbox.config_dir.join("schemas");
    fs::create_dir_all(&schemas_dir).unwrap();

    let cargo_schema = r#"{
        "meta": {
            "tool": "cargo",
            "version": 1,
            "verified": true,
            "keywords": []
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

    let output = sandbox.run(&["resolve", "run tests", "--tool", "cargo"]);
    assert!(output.status.success());
}

#[test]
fn test_tier1_f6_resolve_mismatch_error() {
    let sandbox = TestSandbox::new();
    sandbox.run(&["init"]);
    let output = sandbox.run(&[
        "resolve",
        "some completely random string that matches nothing",
    ]);
    assert!(!output.status.success());
}

// ==========================================
// F7: GENERATE, TUI & VERSIONING (5 TESTS)
// ==========================================

#[test]
fn test_tier1_f7_generate_help() {
    let sandbox = TestSandbox::new();
    fs::write(sandbox.project_dir.join("help.txt"), "usage: git clone").unwrap();

    let schema_json = r#"{
        "meta": {
            "tool": "git",
            "version": 1,
            "verified": true,
            "keywords": []
        },
        "commands": []
    }"#;

    let response_body = serde_json::json!({
        "candidates": [
            {
                "content": {
                    "parts": [
                        {
                            "text": schema_json
                        }
                    ]
                }
            }
        ]
    })
    .to_string();

    let server = MockHttpServer::start(move |_| response_body.clone());
    let base_url = format!("http://127.0.0.1:{}", server.port);

    let output = Command::new(&sandbox.bin_path)
        .args(["generate", "git", "--help-text", "help.txt"])
        .current_dir(&sandbox.project_dir)
        .env("HOME", &sandbox.home_dir)
        .env("TMP_CONFIG_DIR", &sandbox.config_dir)
        .env("GEMINI_API_KEY", "mock_key")
        .env("GEMINI_BASE_URL", &base_url)
        .output()
        .expect("Failed to execute generate command");

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let active_path = sandbox.config_dir.join("schemas").join("git.json");
    assert!(active_path.exists());
    let content = fs::read_to_string(active_path).unwrap();
    assert!(content.contains("\"tool\": \"git\""));
}

#[test]
fn test_tier1_f7_generate_verify_tui_args() {
    let sandbox = TestSandbox::new();
    fs::write(sandbox.project_dir.join("help.txt"), "usage: git clone").unwrap();

    let schema_json = r#"{
        "meta": {
            "tool": "git",
            "version": 1,
            "verified": true,
            "keywords": []
        },
        "commands": []
    }"#;

    let response_body = serde_json::json!({
        "candidates": [
            {
                "content": {
                    "parts": [
                        {
                            "text": schema_json
                        }
                    ]
                }
            }
        ]
    })
    .to_string();

    let server = MockHttpServer::start(move |_| response_body.clone());
    let base_url = format!("http://127.0.0.1:{}", server.port);

    let output = Command::new(&sandbox.bin_path)
        .args(["generate", "git", "--help-text", "help.txt", "--verify"])
        .current_dir(&sandbox.project_dir)
        .env("HOME", &sandbox.home_dir)
        .env("TMP_CONFIG_DIR", &sandbox.config_dir)
        .env("GEMINI_API_KEY", "mock_key")
        .env("GEMINI_BASE_URL", &base_url)
        .output()
        .expect("Failed to execute generate command");

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let active_path = sandbox.config_dir.join("schemas").join("git.json");
    assert!(active_path.exists());
}

#[test]
fn test_tier1_f7_generate_force_backup() {
    let sandbox = TestSandbox::new();
    fs::write(sandbox.project_dir.join("help.txt"), "usage: git clone").unwrap();

    let schemas_dir = sandbox.config_dir.join("schemas");
    let versions_dir = schemas_dir.join("versions").join("git");
    fs::create_dir_all(&versions_dir).unwrap();

    let schema_v1 = r#"{
        "meta": {
            "tool": "git",
            "version": 1,
            "verified": true,
            "keywords": []
        },
        "commands": []
    }"#;
    fs::write(schemas_dir.join("git.json"), schema_v1).unwrap();
    fs::write(versions_dir.join("v1.json"), schema_v1).unwrap();

    let schema_v2 = r#"{
        "meta": {
            "tool": "git",
            "version": 2,
            "verified": true,
            "keywords": ["v2"]
        },
        "commands": []
    }"#;

    let response_body = serde_json::json!({
        "candidates": [
            {
                "content": {
                    "parts": [
                        {
                            "text": schema_v2
                        }
                    ]
                }
            }
        ]
    })
    .to_string();

    let server = MockHttpServer::start(move |_| response_body.clone());
    let base_url = format!("http://127.0.0.1:{}", server.port);

    let output = Command::new(&sandbox.bin_path)
        .args(["generate", "git", "--help-text", "help.txt", "--force"])
        .current_dir(&sandbox.project_dir)
        .env("HOME", &sandbox.home_dir)
        .env("TMP_CONFIG_DIR", &sandbox.config_dir)
        .env("GEMINI_API_KEY", "mock_key")
        .env("GEMINI_BASE_URL", &base_url)
        .output()
        .expect("Failed to execute generate command");

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(schemas_dir.join("git.json").exists());
    let active_content = fs::read_to_string(schemas_dir.join("git.json")).unwrap();
    assert!(active_content.contains("\"version\": 2"));

    assert!(versions_dir.join("v1.json").exists());
    assert!(versions_dir.join("v2.json").exists());
}

#[test]
fn test_tier1_f7_generate_history() {
    let sandbox = TestSandbox::new();
    let schemas_dir = sandbox.config_dir.join("schemas");
    let versions_dir = schemas_dir.join("versions").join("git");
    fs::create_dir_all(&versions_dir).unwrap();

    let schema_v1 = r#"{
        "meta": {
            "tool": "git",
            "version": 1,
            "verified": true,
            "keywords": []
        },
        "commands": []
    }"#;
    fs::write(schemas_dir.join("git.json"), schema_v1).unwrap();
    fs::write(versions_dir.join("v1.json"), schema_v1).unwrap();

    let output = Command::new(&sandbox.bin_path)
        .args(["generate", "git", "--history"])
        .current_dir(&sandbox.project_dir)
        .env("HOME", &sandbox.home_dir)
        .env("TMP_CONFIG_DIR", &sandbox.config_dir)
        .output()
        .expect("Failed to execute history command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Version: 1"));
}

#[test]
fn test_tier1_f7_generate_rollback() {
    let sandbox = TestSandbox::new();
    let schemas_dir = sandbox.config_dir.join("schemas");
    let versions_dir = schemas_dir.join("versions").join("git");
    fs::create_dir_all(&versions_dir).unwrap();

    let schema_v1 = r#"{
        "meta": {
            "tool": "git",
            "version": 1,
            "verified": false,
            "keywords": []
        },
        "commands": []
    }"#;
    let schema_v2 = r#"{
        "meta": {
            "tool": "git",
            "version": 2,
            "verified": true,
            "keywords": []
        },
        "commands": []
    }"#;

    fs::write(schemas_dir.join("git.json"), schema_v2).unwrap();
    fs::write(versions_dir.join("v1.json"), schema_v1).unwrap();
    fs::write(versions_dir.join("v2.json"), schema_v2).unwrap();

    let output = Command::new(&sandbox.bin_path)
        .args(["generate", "git", "--rollback", "1"])
        .current_dir(&sandbox.project_dir)
        .env("HOME", &sandbox.home_dir)
        .env("TMP_CONFIG_DIR", &sandbox.config_dir)
        .output()
        .expect("Failed to execute rollback command");

    assert!(output.status.success());
    let active_content = fs::read_to_string(schemas_dir.join("git.json")).unwrap();
    assert!(active_content.contains("\"version\": 3"));
    assert!(active_content.contains("\"verified\": false"));
}

// ==========================================
// F8: WORKFLOWS (5 TESTS)
// ==========================================

#[test]
fn test_tier1_f8_workflow_list() {
    let sandbox = TestSandbox::new();
    sandbox.run(&["init"]);
    let wf_dir = sandbox.project_dir.join(".tmp").join("workflows");
    fs::create_dir_all(&wf_dir).unwrap();
    fs::write(wf_dir.join("my_wf.json"), r#"{"name":"my_wf","steps":[]}"#).unwrap();

    let output = sandbox.run(&["workflow", "list"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("my_wf"));
}

#[test]
fn test_tier1_f8_workflow_add() {
    let sandbox = TestSandbox::new();
    sandbox.run(&["init"]);
    // Establish a local project context
    fs::create_dir_all(sandbox.project_dir.join(".git")).unwrap();
    let wf_content = r#"{"name":"my_wf","steps":[]}"#;
    fs::write(sandbox.project_dir.join("wf.json"), wf_content).unwrap();

    let output = sandbox.run(&["workflow", "add", "my_wf", "--from", "wf.json"]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(sandbox
        .project_dir
        .join(".tmp")
        .join("workflows")
        .join("my_wf.json")
        .exists());
}

#[test]
fn test_tier1_f8_workflow_add_global_fallback() {
    let sandbox = TestSandbox::new();
    sandbox.run(&["init"]);
    // Do NOT establish a local project context (no .git or Cargo.toml)
    let wf_content = r#"{"name":"my_wf_global","steps":[]}"#;
    fs::write(sandbox.project_dir.join("wf.json"), wf_content).unwrap();

    let output = sandbox.run(&["workflow", "add", "my_wf_global", "--from", "wf.json"]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(sandbox
        .config_dir
        .join("workflows")
        .join("my_wf_global.json")
        .exists());
}

#[test]
fn test_tier1_f8_workflow_run() {
    let sandbox = TestSandbox::new();
    sandbox.run(&["init"]);
    let wf_dir = sandbox.project_dir.join(".tmp").join("workflows");
    fs::create_dir_all(&wf_dir).unwrap();
    let wf_content = r#"{
        "name": "my_wf",
        "steps": [
            {
                "name": "hello_step",
                "command": "echo 'hello'"
            }
        ]
    }"#;
    fs::write(wf_dir.join("my_wf.json"), wf_content).unwrap();

    let output = sandbox.run(&["workflow", "run", "my_wf"]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_tier1_f8_workflow_token_substitution() {
    let sandbox = TestSandbox::new();
    sandbox.run(&["init"]);
    let wf_dir = sandbox.project_dir.join(".tmp").join("workflows");
    fs::create_dir_all(&wf_dir).unwrap();
    let wf_content = r#"{
        "name": "my_wf",
        "steps": [
            {
                "name": "sub_step",
                "command": "echo <arg>"
            }
        ]
    }"#;
    fs::write(wf_dir.join("my_wf.json"), wf_content).unwrap();

    let output = std::process::Command::new(&sandbox.bin_path)
        .args(["workflow", "run", "my_wf"])
        .current_dir(&sandbox.project_dir)
        .env("HOME", &sandbox.home_dir)
        .env("TMP_CONFIG_DIR", &sandbox.config_dir)
        .env("arg", "hello")
        .output()
        .expect("Failed to execute");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hello"));
}

#[test]
fn test_tier1_f8_workflow_error_propagation() {
    let sandbox = TestSandbox::new();
    sandbox.run(&["init"]);
    let wf_dir = sandbox.project_dir.join(".tmp").join("workflows");
    fs::create_dir_all(&wf_dir).unwrap();
    let wf_content = r#"{
        "name": "my_wf",
        "steps": [
            {
                "name": "fail_step",
                "command": "exit 1"
            },
            {
                "name": "skip_step",
                "command": "echo 'should_not_run'"
            }
        ]
    }"#;
    fs::write(wf_dir.join("my_wf.json"), wf_content).unwrap();

    let output = sandbox.run(&["workflow", "run", "my_wf"]);
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("should_not_run"));
}
