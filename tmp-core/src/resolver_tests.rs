use super::*;
use crate::context::Context;
use crate::schema::DataSource;

#[test]
fn test_resolve_builtins() {
    let context = Context {
        cwd: "/dummy".to_string(),
        project_root: None,
        build_system: "cargo".to_string(),
        file_kind: "cargo_project".to_string(),
        script_engine: None,
        recommended_target: None,
        package_name: Some("my-package".to_string()),
        packages: vec!["pkg1".to_string(), "pkg2".to_string()],
        bins: vec!["bin1".to_string()],
        examples: vec!["ex1".to_string()],
        features: vec!["feat1".to_string()],
        profiles: vec!["dev".to_string(), "release".to_string()],
        tests: vec!["test1".to_string()],
        benches: vec!["bench1".to_string()],
        git_branches: vec!["main".to_string()],
        git_remotes: vec!["origin".to_string()],
        npm_scripts: vec!["start".to_string()],
    };

    let test_cases = vec![
        ("cargo:packages", vec!["pkg1", "pkg2"]),
        ("cargo:bins", vec!["bin1"]),
        ("cargo:examples", vec!["ex1"]),
        ("cargo:features", vec!["feat1"]),
        ("cargo:profiles", vec!["dev", "release"]),
        ("cargo:tests", vec!["test1"]),
        ("cargo:benches", vec!["bench1"]),
        ("git:branches", vec!["main"]),
        ("git:remotes", vec!["origin"]),
        ("npm:scripts", vec!["start"]),
    ];

    for (resolver_name, expected) in test_cases {
        let ds = DataSource {
            command: None,
            resolver: Some(resolver_name.to_string()),
            parse: "lines".to_string(),
        };
        let res = DataResolver::resolve(&ds, &context).unwrap();
        let expected_vec: Vec<String> = expected.into_iter().map(|s| s.to_string()).collect();
        assert_eq!(res, expected_vec);
    }
}

#[test]
fn test_resolve_shell_command_lines() {
    let context = Context {
        cwd: std::env::current_dir()
            .unwrap()
            .to_string_lossy()
            .to_string(),
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

    let ds = DataSource {
        command: Some("echo line1 && echo line2".to_string()),
        resolver: None,
        parse: "lines".to_string(),
    };

    let res = DataResolver::resolve(&ds, &context).unwrap();
    assert_eq!(res, vec!["line1".to_string(), "line2".to_string()]);
}

#[test]
fn test_resolve_shell_command_words() {
    let context = Context {
        cwd: std::env::current_dir()
            .unwrap()
            .to_string_lossy()
            .to_string(),
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

    let ds = DataSource {
        command: Some("echo word1 word2 word3".to_string()),
        resolver: None,
        parse: "words".to_string(),
    };

    let res = DataResolver::resolve(&ds, &context).unwrap();
    assert_eq!(
        res,
        vec![
            "word1".to_string(),
            "word2".to_string(),
            "word3".to_string()
        ]
    );
}

#[test]
fn test_resolve_errors() {
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

    // Unknown resolver
    let ds_unknown = DataSource {
        command: None,
        resolver: Some("unknown:resolver".to_string()),
        parse: "lines".to_string(),
    };
    assert!(DataResolver::resolve(&ds_unknown, &context).is_err());

    // Failed command
    let ds_failed = DataSource {
        command: Some("nonexistent_command_12345".to_string()),
        resolver: None,
        parse: "lines".to_string(),
    };
    assert!(DataResolver::resolve(&ds_failed, &context).is_err());

    // Command exits with status non-zero
    let exit_cmd = if cfg!(target_os = "windows") {
        "cmd /C exit 1"
    } else {
        "false"
    };
    let ds_exit_fail = DataSource {
        command: Some(exit_cmd.to_string()),
        resolver: None,
        parse: "lines".to_string(),
    };
    assert!(DataResolver::resolve(&ds_exit_fail, &context).is_err());

    // Neither command nor resolver specified
    let ds_empty = DataSource {
        command: None,
        resolver: None,
        parse: "lines".to_string(),
    };
    let err = DataResolver::resolve(&ds_empty, &context).unwrap_err();
    assert!(err.contains("DataSource must specify either command or resolver"));
}
