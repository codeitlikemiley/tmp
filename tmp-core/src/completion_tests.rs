use crate::compile::{CompileOutput, ResolvedOperation, ResolvedParameter};
use crate::completion::{
    complete, generate_bash_completions, generate_fish_completions, generate_zsh_completions,
};
use crate::context::Context;
use crate::schema::ParameterType;

fn test_context() -> Context {
    Context {
        cwd: "/tmp/test".to_string(),
        project_root: None,
        build_system: "none".to_string(),
        file_kind: "standalone".to_string(),
        script_engine: None,
        recommended_target: None,
        package_name: None,
        packages: Vec::new(),
        bins: Vec::new(),
        examples: Vec::new(),
        features: Vec::new(),
        profiles: Vec::new(),
        tests: Vec::new(),
        benches: Vec::new(),
        git_branches: Vec::new(),
        git_remotes: Vec::new(),
        npm_scripts: Vec::new(),
    }
}

fn test_compile_output() -> CompileOutput {
    CompileOutput {
        context: test_context(),
        operations: vec![
            ResolvedOperation {
                command: "cargo test".to_string(),
                description: "Run tests".to_string(),
                group: "build".to_string(),
                verified: true,
                parameters: vec![
                    ResolvedParameter {
                        name: "filter".to_string(),
                        description: "Test filter pattern".to_string(),
                        required: false,
                        parameter_type: ParameterType::String,
                        default: None,
                        flag: None,
                        values: vec!["unit_tests".to_string(), "integration_tests".to_string()],
                    },
                    ResolvedParameter {
                        name: "verbose".to_string(),
                        description: "Enable verbose output".to_string(),
                        required: false,
                        parameter_type: ParameterType::Boolean,
                        default: None,
                        flag: Some("--verbose".to_string()),
                        values: Vec::new(),
                    },
                ],
            },
            ResolvedOperation {
                command: "cargo build".to_string(),
                description: "Build the project".to_string(),
                group: "build".to_string(),
                verified: true,
                parameters: vec![ResolvedParameter {
                    name: "release".to_string(),
                    description: "Build in release mode".to_string(),
                    required: false,
                    parameter_type: ParameterType::Boolean,
                    default: None,
                    flag: Some("--release".to_string()),
                    values: Vec::new(),
                }],
            },
            ResolvedOperation {
                command: "git status".to_string(),
                description: "Show working tree status".to_string(),
                group: "git".to_string(),
                verified: true,
                parameters: Vec::new(),
            },
        ],
    }
}

#[test]
fn complete_empty_input_returns_top_level_commands() {
    let output = test_compile_output();
    let candidates = complete("", &output);

    // Should return unique first words: "cargo" and "git"
    assert!(
        candidates.iter().any(|c| c.value == "cargo"),
        "Should include 'cargo' as a top-level command"
    );
    assert!(
        candidates.iter().any(|c| c.value == "git"),
        "Should include 'git' as a top-level command"
    );

    // Should not have duplicates (cargo appears in two operations)
    let cargo_count = candidates.iter().filter(|c| c.value == "cargo").count();
    assert_eq!(cargo_count, 1, "Should deduplicate top-level commands");
}

#[test]
fn complete_partial_command_matches_operations() {
    let output = test_compile_output();
    let candidates = complete("cargo", &output);

    // Should complete to "cargo test" and "cargo build"
    assert!(
        candidates.iter().any(|c| c.value == "cargo test"),
        "Should suggest 'cargo test'"
    );
    assert!(
        candidates.iter().any(|c| c.value == "cargo build"),
        "Should suggest 'cargo build'"
    );
}

#[test]
fn complete_full_command_completes_parameters() {
    let output = test_compile_output();
    let candidates = complete("cargo test", &output);

    // Should offer parameter values and flags
    assert!(
        candidates.iter().any(|c| c.value == "unit_tests"),
        "Should suggest parameter value 'unit_tests'"
    );
    assert!(
        candidates.iter().any(|c| c.value == "integration_tests"),
        "Should suggest parameter value 'integration_tests'"
    );
    assert!(
        candidates.iter().any(|c| c.value == "--verbose"),
        "Should suggest flag '--verbose'"
    );
}

#[test]
fn complete_with_parameter_value_filters() {
    let output = test_compile_output();
    let candidates = complete("cargo test unit", &output);

    // "unit" should match "unit_tests" but not "integration_tests"
    assert!(
        candidates.iter().any(|c| c.value == "unit_tests"),
        "Should suggest matching value 'unit_tests'"
    );
    assert!(
        !candidates.iter().any(|c| c.value == "integration_tests"),
        "Should NOT suggest non-matching value 'integration_tests'"
    );
}

#[test]
fn complete_unrelated_input_returns_empty() {
    let output = test_compile_output();
    let candidates = complete("python", &output);

    assert!(
        candidates.is_empty(),
        "Unrelated input should return no candidates"
    );
}

#[test]
fn generate_zsh_completions_returns_valid_script() {
    let script = generate_zsh_completions();

    assert!(
        script.contains("#compdef tmp"),
        "Zsh script must start with #compdef"
    );
    assert!(
        script.contains("_tmp"),
        "Zsh script must define _tmp function"
    );
    assert!(
        script.contains("compile"),
        "Zsh script must include 'compile' command"
    );
    assert!(
        script.contains("resolve"),
        "Zsh script must include 'resolve' command"
    );
    assert!(
        script.contains("benchmark"),
        "Zsh script must include 'benchmark' command"
    );
}

#[test]
fn generate_bash_completions_returns_valid_script() {
    let script = generate_bash_completions();

    assert!(
        script.contains("_tmp_completions"),
        "Bash script must define _tmp_completions function"
    );
    assert!(
        script.contains("complete -F"),
        "Bash script must register the completion function"
    );
    assert!(
        script.contains("compile"),
        "Bash script must include 'compile' command"
    );
    assert!(
        script.contains("init-agent"),
        "Bash script must include 'init-agent' command"
    );
}

#[test]
fn generate_fish_completions_returns_valid_script() {
    let script = generate_fish_completions();

    assert!(
        script.contains("complete -c tmp"),
        "Fish script must use 'complete -c tmp'"
    );
    assert!(
        script.contains("__fish_use_subcommand"),
        "Fish script must check for subcommand"
    );
    assert!(
        script.contains("compile"),
        "Fish script must include 'compile' command"
    );
    assert!(
        script.contains("workflow"),
        "Fish script must include 'workflow' command"
    );
}
