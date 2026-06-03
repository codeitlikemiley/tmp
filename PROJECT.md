# Project: Terminal Meta Protocol (tmp)

## Architecture
TMP makes CLI command schemas available to AI coding agents. It compiles context details (detected project type, dynamic resolvers) and outputs them to `.tmp/context.md` (concise instructions) and `.tmp/commands.json` (full command bank).

```
[Agent reads .tmp/context.md]
              │
              ▼
[Agent runs `tmp resolve "query"`] ──► [LLM matches queries against commands.json]
              │
              ▼
[Agent runs `tmp run <args>`] ─────► [Executes the resolved command safely]
```

- **`tmp-core` (Library)**: Parses schemas, detects project context, runs data resolvers, integrates LLMs, manages schema versioning, and runs workflows.
- **`tmp` (CLI)**: CLI arguments parser, watcher logic, and Ratatui-based verification TUI.

## Code Layout
```
/Volumes/goldcoders/tmp/
├── Cargo.toml               # Workspace manifest
├── README.md                # Usage and setup instructions
├── tmp-core/                # Core Library Crate
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs           # Core API exports
│       ├── schema.rs        # Schema structs (Schema, Command, Token, DataSource)
│       ├── context.rs       # Directory detection, project inspection
│       ├── resolver.rs      # Data source resolvers (built-in + shell commands)
│       ├── compile.rs       # Output formatting (.tmp/context.md & .tmp/commands.json)
│       ├── config.rs        # Configuration loader
│       ├── llm/             # LLM API orchestration & key rotation
│       │   ├── mod.rs
│       │   ├── gemini.rs
│       │   ├── openai.rs
│       │   └── ollama.rs
│       ├── resolve.rs       # Natural language command resolution
│       ├── registry.rs      # GitHub-based registry client
│       ├── versioning.rs    # Versioning & schema diffs
│       ├── workflow.rs      # Multi-step JSON workflows
│       └── agent_bridge.rs  # Agent bridge rule file constructor
├── tmp/                     # CLI Binary Crate
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs          # CLI entrypoint
│       ├── commands/        # Subcommands handlers
│       └── tui/             # Schema verification TUI
└── tests/                   # Integration tests
```

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| 1 | Workspace Scaffold & Registry Client | Cargo Workspace, config config.toml, registry client, CLI clap routing | None | DONE |
| 2 | Schema Engine & Data Resolvers | Schema parsing, dynamic built-in & shell resolvers, schema CLI subcommands | M1 | DONE |
| 3 | Compiler & Watcher | Context detection, compile output generation, filesystem watcher | M2 | DONE |
| 4 | Help Schema Generator & TUI | --help extractor, LLM prompts/rotation, Ratatui verification TUI, versioning | M3 | DONE |
| 5 | Grounded Resolver, Workflows, & Agent Bridge | NL query grounding, command runner, workflows runner, agent configs snippet gen | M4 | DONE |

## Interface Contracts
### `tmp-core` Primary Public API
- `struct Schema`: Deserializes/serializes schema JSON.
- `struct Context`: Detects and represents the target workspace context.
- `struct CompileOutput`: Formats compilation results.
- `struct ResolveResult`: Represents parsed LLM resolution results.
- `struct LlmDispatcher`: Handles calls to LLM providers.
- `struct Workflow`: Represents multi-step JSON workflows.
