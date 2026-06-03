use super::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_run_dry_run_with_last_command() {
    let temp_dir = tempdir().unwrap();
    let root = temp_dir.path();

    // Create .tmp/last_command.json
    let tmp_dir = root.join(".tmp");
    fs::create_dir_all(&tmp_dir).unwrap();
    let last_cmd_json = r#"{"command": "echo 'hello from last command'"}"#;
    fs::write(tmp_dir.join("last_command.json"), last_cmd_json).unwrap();

    let res = run(None, true, root.to_str().unwrap()).unwrap();
    assert_eq!(res.command, "echo 'hello from last command'");
    assert!(res.status.success());
}

#[test]
fn test_run_dry_run_with_malformed_last_command() {
    let temp_dir = tempdir().unwrap();
    let root = temp_dir.path();

    // Create a Cargo.toml so we resolve locally as cargo project when last_command.json fails
    fs::write(root.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();

    // Create malformed .tmp/last_command.json
    let tmp_dir = root.join(".tmp");
    fs::create_dir_all(&tmp_dir).unwrap();
    fs::write(tmp_dir.join("last_command.json"), "invalid json").unwrap();

    let res = run(None, true, root.to_str().unwrap()).unwrap();
    // Since Cargo project with no file arg, should fall back to `cargo run`
    assert_eq!(res.command, "cargo run");
}

#[test]
fn test_resolve_locally_single_file_script() {
    let context = Context {
        cwd: "/dummy".to_string(),
        project_root: None,
        build_system: "none".to_string(),
        file_kind: "single_file_script".to_string(),
        script_engine: Some("rust-script".to_string()),
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

    let res = resolve_locally(&context, Some("myscript.rs")).unwrap();
    assert_eq!(res, "rust-script myscript.rs");

    // Fails if file path missing
    let err = resolve_locally(&context, None).unwrap_err();
    assert!(err.contains("single-file scripts require a file path"));
}

#[test]
fn test_resolve_locally_standalone() {
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

    let res = resolve_locally(&context, Some("foo.rs")).unwrap();
    assert!(res.starts_with("rustc foo.rs -o "));
    assert!(res.contains("tmp-foo-"));

    // Fails if file path missing
    let err = resolve_locally(&context, None).unwrap_err();
    assert!(err.contains("standalone Rust files require a file path"));
}

#[test]
fn test_resolve_locally_cargo() {
    let context = Context {
        cwd: "/dummy".to_string(),
        project_root: None,
        build_system: "cargo".to_string(),
        file_kind: "cargo_project".to_string(),
        script_engine: None,
        recommended_target: None,
        package_name: Some("my-pkg".to_string()),
        packages: vec!["my-pkg".to_string()],
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

    // No file arg -> cargo run
    assert_eq!(resolve_locally(&context, None).unwrap(), "cargo run");

    // src/main.rs -> cargo run
    assert_eq!(
        resolve_locally(&context, Some("src/main.rs")).unwrap(),
        "cargo run"
    );

    // src/bin/foo.rs -> cargo run --bin foo
    assert_eq!(
        resolve_locally(&context, Some("src/bin/foo.rs")).unwrap(),
        "cargo run --bin foo"
    );

    // examples/bar.rs -> cargo run --example bar
    assert_eq!(
        resolve_locally(&context, Some("examples/bar.rs")).unwrap(),
        "cargo run --example bar"
    );

    // tests/test_foo.rs -> cargo test --test test_foo
    assert_eq!(
        resolve_locally(&context, Some("tests/test_foo.rs")).unwrap(),
        "cargo test --test test_foo"
    );

    // benches/bench_foo.rs -> cargo bench --bench bench_foo
    assert_eq!(
        resolve_locally(&context, Some("benches/bench_foo.rs")).unwrap(),
        "cargo bench --bench bench_foo"
    );
}

#[test]
fn test_resolve_locally_npm() {
    let context = Context {
        cwd: "/dummy".to_string(),
        project_root: None,
        build_system: "npm".to_string(),
        file_kind: "npm_project".to_string(),
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

    assert_eq!(resolve_locally(&context, None).unwrap(), "npm test");
}

#[test]
fn test_resolve_locally_none() {
    let context = Context {
        cwd: "/dummy".to_string(),
        project_root: None,
        build_system: "none".to_string(),
        file_kind: "standalone".to_string(), // but wait, context.file_kind is "standalone" but wait if we change both to make it not match any branches
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

    // If we make file_kind = "unknown", packages empty, build_system = "none":
    let mut custom_ctx = context.clone();
    custom_ctx.file_kind = "unknown".to_string();
    let err = resolve_locally(&custom_ctx, None).unwrap_err();
    assert_eq!(err, "No runnable context detected");
}
