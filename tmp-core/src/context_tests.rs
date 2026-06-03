use super::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_cargo_project_detection() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Write a Cargo.toml for the root/workspace
    let root_cargo = r#"
        [workspace]
        members = ["crates/*", "libs/my-lib"]
    "#;
    fs::write(root.join("Cargo.toml"), root_cargo).unwrap();

    // Create member directories
    let crate_a_dir = root.join("crates").join("crate-a");
    fs::create_dir_all(&crate_a_dir).unwrap();
    let crate_a_cargo = r#"
        [package]
        name = "crate-a"
        version = "0.1.0"

        [features]
        default = []
        feat_foo = []
        feat_bar = []

        [profile.release-custom]
        inherits = "release"
        codegen-units = 1

        [[bin]]
        name = "bin-custom"
        path = "src/main_custom.rs"
    "#;
    fs::write(crate_a_dir.join("Cargo.toml"), crate_a_cargo).unwrap();

    // Create file structure for crate-a
    let src_bin_dir = crate_a_dir.join("src").join("bin");
    fs::create_dir_all(&src_bin_dir).unwrap();
    fs::write(src_bin_dir.join("other_bin.rs"), "fn main() {}").unwrap();

    let nested_bin_dir = src_bin_dir.join("nested_bin");
    fs::create_dir_all(&nested_bin_dir).unwrap();
    fs::write(nested_bin_dir.join("main.rs"), "fn main() {}").unwrap();

    fs::create_dir_all(crate_a_dir.join("src")).unwrap();
    fs::write(crate_a_dir.join("src").join("main.rs"), "fn main() {}").unwrap();

    let examples_dir = crate_a_dir.join("examples");
    fs::create_dir_all(&examples_dir).unwrap();
    fs::write(examples_dir.join("ex1.rs"), "fn main() {}").unwrap();

    let tests_dir = crate_a_dir.join("tests");
    fs::create_dir_all(&tests_dir).unwrap();
    fs::write(tests_dir.join("t1.rs"), "fn main() {}").unwrap();

    let benches_dir = crate_a_dir.join("benches");
    fs::create_dir_all(&benches_dir).unwrap();
    fs::write(benches_dir.join("b1.rs"), "fn main() {}").unwrap();

    // Write a lib cargo member
    let my_lib_dir = root.join("libs").join("my-lib");
    fs::create_dir_all(&my_lib_dir).unwrap();
    let my_lib_cargo = r#"
        [package]
        name = "my-lib"
        version = "0.1.0"
    "#;
    fs::write(my_lib_dir.join("Cargo.toml"), my_lib_cargo).unwrap();

    // Detect context
    let ctx = Context::detect(root, None, None);

    assert_eq!(ctx.build_system, "cargo");
    assert_eq!(ctx.file_kind, "cargo_project");
    assert_eq!(ctx.project_root, Some(root.to_string_lossy().to_string()));

    // Assert cache fields
    let mut expected_packages = vec!["crate-a".to_string(), "my-lib".to_string()];
    expected_packages.sort();
    assert_eq!(ctx.packages, expected_packages);

    // bins should include crate-a (because of src/main.rs), bin-custom, other_bin, nested_bin
    let mut expected_bins = vec![
        "crate-a".to_string(),
        "bin-custom".to_string(),
        "other_bin".to_string(),
        "nested_bin".to_string(),
    ];
    expected_bins.sort();
    assert_eq!(ctx.bins, expected_bins);

    assert_eq!(ctx.examples, vec!["ex1".to_string()]);
    assert_eq!(ctx.tests, vec!["t1".to_string()]);
    assert_eq!(ctx.benches, vec!["b1".to_string()]);

    let mut expected_features = vec!["feat_foo".to_string(), "feat_bar".to_string()];
    expected_features.sort();
    assert_eq!(ctx.features, expected_features);

    let mut expected_profiles = vec![
        "dev".to_string(),
        "release".to_string(),
        "test".to_string(),
        "bench".to_string(),
        "release-custom".to_string(),
    ];
    expected_profiles.sort();
    expected_profiles.dedup();
    let mut actual_profiles = ctx.profiles.clone();
    actual_profiles.sort();
    assert_eq!(actual_profiles, expected_profiles);
}

#[test]
fn test_npm_project_detection() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let package_json = r#"
        {
            "name": "my-npm-pkg",
            "scripts": {
                "start": "node index.js",
                "test": "jest",
                "build": "tsc"
            }
        }
    "#;
    fs::write(root.join("package.json"), package_json).unwrap();

    let ctx = Context::detect(root, None, None);
    assert_eq!(ctx.build_system, "npm");
    assert_eq!(ctx.file_kind, "npm_project");
    assert_eq!(ctx.package_name, Some("my-npm-pkg".to_string()));
    assert_eq!(
        ctx.npm_scripts,
        vec!["build".to_string(), "start".to_string(), "test".to_string()]
    );
}

#[test]
fn test_single_file_script_detection() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Write a script file with rust-script shebang
    let script_path = root.join("my_script.rs");
    let script_content = r#"#!/usr/bin/env rust-script
        fn main() {
            println!("hello");
        }
    "#;
    fs::write(&script_path, script_content).unwrap();

    let ctx = Context::detect(root, Some(script_path.to_str().unwrap()), None);
    assert_eq!(ctx.file_kind, "single_file_script");
    assert_eq!(ctx.script_engine, Some("rust-script".to_string()));
    assert_eq!(
        ctx.recommended_target,
        Some(script_path.to_string_lossy().to_string())
    );
}

#[test]
fn test_git_commands_execution() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Initialize git repository
    let run_git = |args: &[&str]| {
        let mut cmd = std::process::Command::new("git");
        cmd.args(args);
        cmd.current_dir(root);
        cmd.output().is_ok()
    };

    // If git commands are available on the path, run the test, otherwise skip
    if run_git(&["init"]) {
        let _ = run_git(&["config", "user.name", "Test"]);
        let _ = run_git(&["config", "user.email", "test@example.com"]);

        fs::write(root.join("foo"), "bar").unwrap();
        let _ = run_git(&["add", "foo"]);
        let _ = run_git(&["commit", "-m", "initial commit"]);
        let _ = run_git(&["branch", "my-feature-branch"]);
        let _ = run_git(&["remote", "add", "origin", "git@github.com:foo/bar.git"]);

        let ctx = Context::detect(root, None, None);
        assert!(ctx.git_branches.contains(&"my-feature-branch".to_string()));
        assert!(ctx.git_remotes.contains(&"origin".to_string()));
    }
}

#[test]
fn test_context_structure_transition() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Initially standalone / empty
    let mut ctx = Context::detect(root, None, None);
    assert_eq!(ctx.build_system, "none");
    assert_eq!(ctx.file_kind, "standalone");

    // Write a Cargo.toml to transition the project structure
    let root_cargo = r#"
        [package]
        name = "transitioned-pkg"
        version = "0.1.0"
    "#;
    fs::write(root.join("Cargo.toml"), root_cargo).unwrap();

    // Call refresh, which should now re-detect and update the state
    ctx.refresh();
    assert_eq!(ctx.build_system, "cargo");
    assert_eq!(ctx.file_kind, "cargo_project");
    assert_eq!(ctx.package_name, Some("transitioned-pkg".to_string()));
}
