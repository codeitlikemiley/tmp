use super::*;
use clap::Parser;

#[test]
fn test_cli_parsing_init() {
    let args = vec!["tmp", "init"];
    let cli = Cli::try_parse_from(args).unwrap();
    assert_eq!(cli.config, None);
    assert_eq!(cli.command, Commands::Init);
}

#[test]
fn test_cli_parsing_custom_config() {
    let args = vec!["tmp", "--config", "/path/to/config.toml", "init"];
    let cli = Cli::try_parse_from(args).unwrap();
    assert_eq!(cli.config, Some("/path/to/config.toml".to_string()));
    assert_eq!(cli.command, Commands::Init);
}

#[test]
fn test_cli_parsing_schema_list() {
    let args = vec!["tmp", "schema", "list"];
    let cli = Cli::try_parse_from(args).unwrap();
    assert_eq!(
        cli.command,
        Commands::Schema {
            subcommand: SchemaSubcommands::List
        }
    );
}

#[test]
fn test_cli_parsing_registry_search() {
    let args = vec!["tmp", "registry", "search", "rust-cli"];
    let cli = Cli::try_parse_from(args).unwrap();
    assert_eq!(
        cli.command,
        Commands::Registry {
            subcommand: RegistrySubcommands::Search {
                query: "rust-cli".to_string()
            }
        }
    );
}

#[test]
fn test_cli_parsing_registry_install() {
    let args = vec!["tmp", "registry", "install", "cargo"];
    let cli = Cli::try_parse_from(args).unwrap();
    assert_eq!(
        cli.command,
        Commands::Registry {
            subcommand: RegistrySubcommands::Install {
                tool: "cargo".to_string()
            }
        }
    );
}

#[test]
fn test_cli_parsing_run_yes() {
    let args = vec!["tmp", "run", "--yes"];
    let cli = Cli::try_parse_from(args).unwrap();
    assert_eq!(
        cli.command,
        Commands::Run {
            file: None,
            dry_run: false,
            cwd: None,
            yes: true,
        }
    );
}

#[test]
fn test_cli_parsing_verify_command() {
    let args = vec!["tmp", "verify", "cargo"];
    let cli = Cli::try_parse_from(args).unwrap();
    assert_eq!(
        cli.command,
        Commands::Verify {
            schema: "cargo".to_string()
        }
    );
}

#[test]
fn test_cli_parsing_verify_command_with_config() {
    let args = vec!["tmp", "--config", "/custom/config.toml", "verify", "git"];
    let cli = Cli::try_parse_from(args).unwrap();
    assert_eq!(cli.config, Some("/custom/config.toml".to_string()));
    assert_eq!(
        cli.command,
        Commands::Verify {
            schema: "git".to_string()
        }
    );
}

#[test]
fn test_cli_parsing_output_show_list_mode() {
    let args = vec!["tmp", "output", "show"];
    let cli = Cli::try_parse_from(args).unwrap();
    assert_eq!(
        cli.command,
        Commands::Output {
            subcommand: OutputSubcommands::Show {
                last: false,
                raw: false,
                run_id: None,
                cwd: None,
            }
        }
    );
}

#[test]
fn test_cli_parsing_output_show_last() {
    let args = vec!["tmp", "output", "show", "--last"];
    let cli = Cli::try_parse_from(args).unwrap();
    assert_eq!(
        cli.command,
        Commands::Output {
            subcommand: OutputSubcommands::Show {
                last: true,
                raw: false,
                run_id: None,
                cwd: None,
            }
        }
    );
}

#[test]
fn test_cli_parsing_output_show_raw_with_run_id() {
    let args = vec!["tmp", "output", "show", "--raw", "1234567890"];
    let cli = Cli::try_parse_from(args).unwrap();
    assert_eq!(
        cli.command,
        Commands::Output {
            subcommand: OutputSubcommands::Show {
                last: false,
                raw: true,
                run_id: Some("1234567890".to_string()),
                cwd: None,
            }
        }
    );
}

#[test]
fn test_cli_parsing_output_show_with_cwd() {
    let args = vec!["tmp", "output", "show", "--last", "--cwd", "/some/path"];
    let cli = Cli::try_parse_from(args).unwrap();
    assert_eq!(
        cli.command,
        Commands::Output {
            subcommand: OutputSubcommands::Show {
                last: true,
                raw: false,
                run_id: None,
                cwd: Some("/some/path".to_string()),
            }
        }
    );
}

#[test]
fn test_cli_parsing_benchmark_run() {
    let args = vec!["tmp", "benchmark", "run", "task-001"];
    let cli = Cli::try_parse_from(args).unwrap();
    assert_eq!(
        cli.command,
        Commands::Benchmark {
            subcommand: BenchmarkSubcommands::Run {
                task_id: "task-001".to_string(),
                mode: "baseline".to_string(),
                cwd: None,
            }
        }
    );
}

#[test]
fn test_cli_parsing_benchmark_run_with_mode() {
    let args = vec![
        "tmp",
        "benchmark",
        "run",
        "task-002",
        "--mode",
        "tmp_assisted",
    ];
    let cli = Cli::try_parse_from(args).unwrap();
    assert_eq!(
        cli.command,
        Commands::Benchmark {
            subcommand: BenchmarkSubcommands::Run {
                task_id: "task-002".to_string(),
                mode: "tmp_assisted".to_string(),
                cwd: None,
            }
        }
    );
}

#[test]
fn test_cli_parsing_benchmark_run_with_cwd() {
    let args = vec![
        "tmp",
        "benchmark",
        "run",
        "task-003",
        "--cwd",
        "/project/dir",
    ];
    let cli = Cli::try_parse_from(args).unwrap();
    assert_eq!(
        cli.command,
        Commands::Benchmark {
            subcommand: BenchmarkSubcommands::Run {
                task_id: "task-003".to_string(),
                mode: "baseline".to_string(),
                cwd: Some("/project/dir".to_string()),
            }
        }
    );
}

#[test]
fn test_cli_parsing_generate_with_rtk() {
    let args = vec!["tmp", "generate", "cargo", "--rtk", "cargo.test"];
    let cli = Cli::try_parse_from(args).unwrap();
    match cli.command {
        Commands::Generate { tool, rtk, .. } => {
            assert_eq!(tool, "cargo");
            assert_eq!(rtk, Some("cargo.test".to_string()));
        }
        _ => panic!("Expected Generate command"),
    }
}

#[test]
fn test_cli_parsing_generate_without_rtk() {
    let args = vec!["tmp", "generate", "cargo"];
    let cli = Cli::try_parse_from(args).unwrap();
    match cli.command {
        Commands::Generate { tool, rtk, .. } => {
            assert_eq!(tool, "cargo");
            assert_eq!(rtk, None);
        }
        _ => panic!("Expected Generate command"),
    }
}

#[test]
fn test_cli_parsing_generate_rtk_subcommand_form() {
    let args = vec!["tmp", "generate", "rtk", "cargo.test"];
    let cli = Cli::try_parse_from(args).unwrap();
    match cli.command {
        Commands::Generate {
            tool,
            operation_id,
            rtk,
            ..
        } => {
            assert_eq!(tool, "rtk");
            assert_eq!(operation_id, Some("cargo.test".to_string()));
            assert_eq!(rtk, None);
        }
        other => panic!("Expected Generate, got {other:?}"),
    }
}

#[test]
fn test_cli_parsing_completions_zsh() {
    let args = vec!["tmp", "completions", "zsh"];
    let cli = Cli::try_parse_from(args).unwrap();
    assert_eq!(
        cli.command,
        Commands::Completions {
            shell: "zsh".to_string()
        }
    );
}

#[test]
fn test_cli_parsing_complete_input() {
    let args = vec!["tmp", "complete", "cargo run --bin"];
    let cli = Cli::try_parse_from(args).unwrap();
    match cli.command {
        Commands::Complete { input, cwd } => {
            assert_eq!(input, "cargo run --bin");
            assert_eq!(cwd, None);
        }
        other => panic!("Expected Complete, got {other:?}"),
    }
}

#[test]
fn test_cli_parsing_verify_requires_schema_arg() {
    let args = vec!["tmp", "verify"];
    let result = Cli::try_parse_from(args);
    assert!(result.is_err(), "verify should require a schema argument");
}

#[test]
fn test_cli_parsing_benchmark_run_requires_task_id() {
    let args = vec!["tmp", "benchmark", "run"];
    let result = Cli::try_parse_from(args);
    assert!(
        result.is_err(),
        "benchmark run should require a task_id argument"
    );
}
