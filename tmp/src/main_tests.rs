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
