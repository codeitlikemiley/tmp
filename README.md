# Tool Mapping Protocol (`tmp`)

`tmp` is a deterministic CLI helper for making command knowledge available to external coding agents such as Claude Code, Codex, Cursor, Copilot, and similar tools. It stores command schemas, compiles workspace context, resolves natural-language intent against installed schemas with local heuristics, and runs the selected command or file context.

`tmp` does not call language model APIs or manage provider keys. If a user wants model-assisted schema authoring, they can use their preferred external agent to inspect help text, edit schema JSON, and then use `tmp schema import`, `tmp generate`, `tmp compile`, `tmp resolve`, and `tmp run`.

## Workspace

```text
.
├── Cargo.toml
├── crates/
│   └── command/                # Quiet std::process::Command wrapper
├── tmp-core/                   # Core library
│   └── src/
│       ├── compile.rs          # Workspace context and resolved schema output
│       ├── config.rs           # Config path loading
│       ├── context.rs          # Project/build/git context detection
│       ├── generate.rs         # Deterministic help-text draft schema generation
│       ├── help.rs             # Recursive `--help` scraper
│       ├── registry.rs         # Schema registry client
│       ├── resolve.rs          # Heuristic schema-backed command resolver
│       ├── resolver.rs         # Built-in dynamic token resolvers
│       ├── run.rs              # Contextual command runner
│       ├── schema.rs           # Schema data model and validation
│       └── versioning.rs       # Schema history, rollback, and diff helpers
├── tmp/                        # User-facing CLI binary
│   └── src/
│       ├── commands/
│       ├── main.rs
│       └── tui/
└── tests/                      # E2E tiers
```

## Quick Start

```bash
tmp init
tmp generate cargo
tmp generate cargo --verify
tmp compile
tmp resolve "run unit tests"
tmp run
```

For external agent setup:

```bash
tmp init-agent codex
tmp init-agent claude
```

The generated instruction files tell the external agent to use `tmp resolve "<intent>"` before running unknown commands.

## Commands

### `init`

Creates the config directory and `schemas/` directory. The default config is intentionally minimal and contains no API provider settings.

### `generate <tool>`

Generates an unverified draft schema from help text. If `--help-text` is omitted, `tmp` runs `<tool> --help` and recursively checks detected subcommands up to the scraper limits. Draft schemas are saved with version history.

Useful flags:

- `--help-text <PATH|DIR|COMMAND>`: Read help from a file, inspect a directory containing the tool binary, or run the value as a command.
- `--history`: Print schema version history.
- `--rollback <VERSION>`: Restore a prior schema as a new version.
- `--verify`: Launch the verification TUI when running interactively.
- `--non-interactive`: Save without prompting or launching TUI.
- `--force`: Save even when generated output matches the latest version.

Draft output is marked `verified: false`. Treat it as a bootstrap artifact, not a complete or trusted command contract.

### `schema`

Manages local schemas:

- `schema list`
- `schema share <tool>`
- `schema import <source>`
- `schema keywords <tool> [words...]`

### `registry`

Searches, installs, and publishes schemas through a registry source:

- `registry search <query>`
- `registry install <tool>`
- `registry publish <tool>`

### `compile`

Compiles project context and relevant schemas into:

- `.tmp/commands.json`
- `.tmp/context.md`

Use `--watch` to refresh context on file changes.

### `resolve "<query>"`

Resolves a natural-language query against installed schemas using local heuristic matching. If no schema match exists, the command fails closed instead of guessing.

Use `--json` for the full resolution structure. Successful resolution writes `.tmp/last_command.json`.

### `run [file]`

Runs the last resolved command when no file is provided. With a file, it chooses a contextual local command, such as `cargo run`, `cargo test --test <name>`, `rustc <file>`, or `npm test`.

Use `--dry-run` to preview the command.

### `workflow`

Imports and runs JSON/YAML workflow definitions:

- `workflow add <name> --from <path>`
- `workflow run <name>`
- `workflow list`

## Schema Notes

Schemas are JSON files under the active config directory’s `schemas/` folder. Tokens can use built-in data resolvers such as:

- `cargo:packages`
- `cargo:bins`
- `cargo:examples`
- `cargo:features`
- `cargo:tests`
- `git:branches`
- `git:remotes`
- `npm:scripts`

Custom token data sources can run shell commands and parse output as `lines` or `words`.

## Development

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace
```

