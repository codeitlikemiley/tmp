use clap::{Parser, Subcommand};
use std::process;

mod commands;
pub mod tui;

#[derive(Parser)]
#[command(name = "tmp")]
#[command(about = "Terminal Meta Protocol CLI", long_about = None)]
pub struct Cli {
    #[arg(short, long, help = "Custom configuration path")]
    pub config: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum Commands {
    #[command(about = "Download curated schemas and set up configuration")]
    Init,

    #[command(about = "Manage schemas")]
    Schema {
        #[command(subcommand)]
        subcommand: SchemaSubcommands,
    },

    #[command(about = "Query and install schemas from the online registry")]
    Registry {
        #[command(subcommand)]
        subcommand: RegistrySubcommands,
    },

    #[command(about = "Compile workspace context and schemas")]
    Compile {
        #[arg(long, help = "Current working directory to run compile in")]
        cwd: Option<String>,

        #[arg(long, help = "Watch workspace and recompile automatically on changes")]
        watch: bool,
    },

    #[command(about = "Generate schema for a tool")]
    Generate {
        #[arg(help = "The tool name to generate schema for")]
        tool: String,

        #[arg(
            long,
            help = "Path to a file containing help text, or a directory to recursively check help"
        )]
        help_text: Option<String>,

        #[arg(long, help = "Override LLM provider")]
        provider: Option<String>,

        #[arg(long, help = "Override LLM model")]
        model: Option<String>,

        #[arg(long, help = "Rollback schema to a specified version")]
        rollback: Option<u32>,

        #[arg(long, help = "List all schema versions")]
        history: bool,

        #[arg(
            long,
            help = "Bypass TUI verification and save generated schema directly"
        )]
        non_interactive: bool,

        #[arg(long, help = "Launch verification TUI on the generated schema")]
        verify: bool,

        #[arg(long, help = "Force generation and backup")]
        force: bool,
    },

    #[command(about = "Resolve NL query to command")]
    Resolve {
        #[arg(help = "The query to resolve")]
        query: String,

        #[arg(long, help = "Filter commands by tool")]
        tool: Option<String>,

        #[arg(long, help = "Output as JSON")]
        json: bool,

        #[arg(long, help = "Current working directory")]
        cwd: Option<String>,
    },

    #[command(about = "Run code contextually")]
    Run {
        #[arg(help = "Optional file path (with optional :line)")]
        file: Option<String>,

        #[arg(long, help = "Dry run command preview")]
        dry_run: bool,

        #[arg(long, help = "Current working directory")]
        cwd: Option<String>,
    },

    #[command(about = "Manage and run workflows")]
    Workflow {
        #[command(subcommand)]
        subcommand: WorkflowSubcommands,
    },

    #[command(name = "init-agent", about = "Initialize files for an AI agent")]
    InitAgent {
        #[arg(help = "Name of the agent (e.g. claude, chatgpt)")]
        agent: String,
    },
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum SchemaSubcommands {
    #[command(about = "List installed schemas")]
    List,

    #[command(about = "Share a schema")]
    Share {
        #[arg(help = "The tool name to share")]
        tool: String,
    },

    #[command(about = "Import a schema")]
    Import {
        #[arg(help = "The source path or URL of the schema")]
        source: String,
    },

    #[command(about = "Manage keywords for a schema")]
    Keywords {
        #[arg(help = "The tool name")]
        tool: String,
        #[arg(help = "The keywords to set (leave empty to show current)")]
        words: Vec<String>,
    },
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum RegistrySubcommands {
    #[command(about = "Search the online registry")]
    Search {
        #[arg(help = "The query to search for")]
        query: String,
    },

    #[command(about = "Install a schema from the registry")]
    Install {
        #[arg(help = "The tool name to install")]
        tool: String,
    },

    #[command(about = "Publish a schema to the registry")]
    Publish {
        #[arg(help = "The tool name to publish")]
        tool: String,
    },
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum WorkflowSubcommands {
    #[command(about = "Add a workflow")]
    Add {
        #[arg(help = "Name of the workflow")]
        name: String,

        #[arg(long, help = "Path to JSON definition file")]
        from: String,
    },

    #[command(about = "Run a workflow")]
    Run {
        #[arg(help = "Name of the workflow")]
        name: String,
    },

    #[command(about = "List all workflows")]
    List,
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run_cli(cli) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

pub fn run_cli(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = match cli.config.as_deref() {
        Some(p) => Some(std::path::PathBuf::from(p)),
        None => tmp_core::config::default_config_path(),
    };
    if let Some(ref p) = config_path {
        if p.exists() {
            tmp_core::config::load_config(Some(p))?;
        }
    }

    match cli.command {
        Commands::Init => {
            commands::init::run(cli.config.as_deref())?;
        }
        Commands::Schema { subcommand } => match subcommand {
            SchemaSubcommands::List => {
                commands::schema::list(cli.config.as_deref())?;
            }
            SchemaSubcommands::Share { tool } => {
                commands::schema::share(&tool, cli.config.as_deref())?;
            }
            SchemaSubcommands::Import { source } => {
                commands::schema::import(&source, cli.config.as_deref())?;
            }
            SchemaSubcommands::Keywords { tool, words } => {
                commands::schema::keywords(&tool, words, cli.config.as_deref())?;
            }
        },
        Commands::Registry { subcommand } => match subcommand {
            RegistrySubcommands::Search { query } => {
                commands::registry::search(&query)?;
            }
            RegistrySubcommands::Install { tool } => {
                commands::registry::install(&tool, cli.config.as_deref())?;
            }
            RegistrySubcommands::Publish { tool } => {
                commands::registry::publish(&tool, cli.config.as_deref())?;
            }
        },
        Commands::Compile { cwd, watch } => {
            commands::compile::run(cwd.as_deref(), watch, cli.config.as_deref())?;
        }
        Commands::Generate {
            tool,
            help_text,
            provider,
            model,
            rollback,
            history,
            non_interactive,
            verify,
            force,
        } => {
            commands::generate::run(
                &tool,
                cli.config.as_deref(),
                help_text.as_deref(),
                provider.as_deref(),
                model.as_deref(),
                rollback,
                history,
                verify,
                non_interactive,
                force,
            )?;
        }
        Commands::Resolve {
            query,
            tool,
            json,
            cwd,
        } => {
            commands::resolve::run(
                &query,
                tool.as_deref(),
                json,
                cwd.as_deref(),
                cli.config.as_deref(),
            )?;
        }
        Commands::Run { file, dry_run, cwd } => {
            commands::run::run(file.as_deref(), dry_run, cwd.as_deref())?;
        }
        Commands::Workflow { subcommand } => match subcommand {
            WorkflowSubcommands::Add { name, from } => {
                commands::workflow::add(&name, &from)?;
            }
            WorkflowSubcommands::Run { name } => {
                commands::workflow::run(&name)?;
            }
            WorkflowSubcommands::List => {
                commands::workflow::list()?;
            }
        },
        Commands::InitAgent { agent } => {
            commands::init_agent::run(&agent)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
