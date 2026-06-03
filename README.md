# Tool Mapping Protocol (`tmp`)

<p align="center">
  <img src="logo.svg" alt="Tool Mapping Protocol (TMP) Logo" width="400" />
</p>

**TMP is a protocol for turning intent into verified operations.** The first implementation is a Rust workspace that ships both a user-facing CLI and a set of embeddable library crates.

> TMP is a map. Before a human, terminal, or AI agent runs something, TMP shows the correct road, the required inputs, the possible side effects, and the shape of the result.

TMP does not call language model APIs or manage provider keys. The deterministic core stays independent of AI providers. If a user wants model-assisted schema authoring, they use their preferred external agent to inspect help text, edit schema JSON, and then call into TMP.

---

## What TMP Maps

TMP is bigger than a CLI helper. It can map any surface where an action can be invoked:

| Surface    | Example               | TMP Value                |
| ---------- | --------------------- | ------------------------ |
| CLI        | `cargo test`          | known flags & parameters |
| API        | `POST /deployments`   | schema + effects         |
| SQL        | `recent_failed_jobs`  | safe query templates     |
| Workflow   | `release_candidate`   | ordered steps            |
| Script     | `sync-data`           | documented args          |
| Completion | `<TAB>`               | dynamic values           |
| Agent tool | `resolve_intent`      | grounded lookup          |
| Output     | test summary          | less noise               |

---

## Workspace

```text
.
├── Cargo.toml                  # Workspace root
├── tmp-core/                   # Core library crate (embeddable)
│   ├── examples/
│   └── src/
│       ├── compile.rs          # Workspace context compiler
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
├── crates/
│   ├── command/                # Quiet std::process::Command wrapper (library)
│   └── tmp-agent/              # Agent adapter server (library + binary)
│       ├── src/
│       │   ├── lib.rs          # Axum server, DB helpers, subagent orchestration
│       │   └── main.rs
│       └── tests/
├── docs/
│   └── whitepaper/             # Protocol whitepaper (Draft 0.4)
├── tests/                      # E2E test tiers (Tier 1–4)
└── scripts/
```

---

## Using TMP as a Library

The core value of TMP lives in its library crates. You can embed schema resolution, context detection, and compilation directly into your own tools without going through the CLI.

### `tmp-core` — The Protocol Engine

`tmp-core` is the heart of TMP. It exposes all protocol primitives as a Rust library:

```toml
[dependencies]
tmp-core = { path = "tmp-core" }
```

#### Modules

| Module       | Purpose                                              |
| ------------ | ---------------------------------------------------- |
| `schema`     | Parse, validate, and serialize operation schemas      |
| `resolve`    | Match natural-language intent to a schema-backed command |
| `compile`    | Build workspace context and resolved command maps     |
| `context`    | Detect project structure, build system, git state     |
| `generate`   | Draft schemas from `--help` text                      |
| `help`       | Recursive help-text scraper                           |
| `resolver`   | Built-in dynamic token resolvers (`cargo:*`, `git:*`, `npm:*`) |
| `run`        | Execute resolved commands with contextual inference   |
| `registry`   | Interact with schema registries (search, install, publish) |
| `versioning` | Schema history, rollback, and diff                    |
| `config`     | Configuration path resolution                         |

#### Key Types

```rust
use tmp_core::schema::{Schema, Command, Token, DataSource, TokenType};
use tmp_core::resolve::{ResolveResult, TokenFill};
use tmp_core::compile::{Compiler, CompileOutput, ResolvedCommand, ResolvedToken};
use tmp_core::context::Context;
```

#### Example: Resolve Intent Programmatically

```rust
use tmp_core::context::Context;
use tmp_core::resolve;

fn main() {
    let context = Context::detect(".");
    match resolve::resolve("run unit tests", &context, None, None) {
        Ok(result) => {
            println!("Command: {}", result.command);
            println!("Confidence: {}", result.confidence);
            for fill in &result.tokens_filled {
                println!("  {} = {} ({})", fill.name, fill.value, fill.source);
            }
        }
        Err(e) => eprintln!("Resolution failed: {}", e),
    }
}
```

#### Example: Compile Context

```rust
use tmp_core::compile::Compiler;
use tmp_core::context::Context;
use std::path::Path;

fn main() {
    let cwd = Path::new(".");
    let context = Context::detect(".");
    let output = Compiler::compile(cwd, &context, None).unwrap();

    // Write .tmp/commands.json and .tmp/context.md
    Compiler::write_to_disk(cwd, &output).unwrap();

    // Or generate markdown programmatically
    let markdown = Compiler::generate_markdown(&output);
    println!("{}", markdown);
}
```

---

### `command` — Quiet Process Execution

A thin wrapper around `std::process::Command` that suppresses console window creation on Windows. Useful as a drop-in replacement in cross-platform tools.

```toml
[dependencies]
command = { path = "crates/command" }
```

```rust
use command::Command;

let output = Command::new("cargo")
    .arg("test")
    .output()
    .expect("failed to execute");
println!("{}", String::from_utf8_lossy(&output.stdout));
```

---

### `tmp-agent` — Agent Adapter Server

An Axum-based HTTP server that exposes TMP capabilities to AI agents. It provides REST endpoints for command execution, file operations, chat (via Antigravity SDK), subagent orchestration, database introspection, and structured logging.

```toml
[dependencies]
tmp-agent = { path = "crates/tmp-agent" }
```

#### Endpoints

| Method | Path             | Purpose                           |
| ------ | ---------------- | --------------------------------- |
| GET    | `/status`        | Health check and agent state      |
| POST   | `/chat`          | Send a message to the AI agent    |
| POST   | `/execute`       | Run a shell command                |
| POST   | `/read_file`     | Read file contents (sandboxed)    |
| POST   | `/write_file`    | Write file contents (sandboxed)   |
| POST   | `/subagent`      | Spawn an async subagent task      |
| GET    | `/subagent/:id`  | Poll subagent status              |
| POST   | `/log`           | Structured logging                |
| POST   | `/db/tables`     | List database tables (SQLite/PG)  |
| POST   | `/db/columns`    | Get column info for a table       |
| POST   | `/db/query`      | Execute read-only SQL queries     |

Security: file operations are sandboxed to the workspace directory. SQL queries are restricted to `SELECT`/`WITH` statements with mutating keyword detection.

---

## CLI Quick Start

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

## CLI Commands

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

---

## Schema Notes

Schemas are JSON files under the active config directory's `schemas/` folder. Tokens can use built-in data resolvers such as:

- `cargo:packages`
- `cargo:bins`
- `cargo:examples`
- `cargo:features`
- `cargo:tests`
- `git:branches`
- `git:remotes`
- `npm:scripts`

Custom token data sources can run shell commands and parse output as `lines` or `words`.

---

## Core Invariants

These rules are tested across the E2E tier suite:

| Invariant                                        | Why It Matters                          |
| ------------------------------------------------ | --------------------------------------- |
| Unknown intent does not invoke anything           | Prevents hallucinated operations        |
| Draft maps are never treated as verified          | Prevents false trust                    |
| High-risk effects require approval                | Prevents accidental destructive actions |
| Dynamic resolver failure is visible               | Prevents hidden wrong defaults          |
| Raw output is retained when output is shaped      | Preserves auditability                  |
| The core resolver is deterministic                | Keeps TMP independent of AI providers   |

---

## Architecture

```text
┌─────────────────────────────────────────────────┐
│  External Agents (Claude, Codex, Copilot, …)    │
└──────────────────────┬──────────────────────────┘
                       │
          ┌────────────┼────────────┐
          ▼            ▼            ▼
   ┌──────────┐  ┌──────────┐  ┌──────────────┐
   │ tmp CLI  │  │tmp-agent │  │ Your Tool    │
   │ (binary) │  │ (server) │  │ (embeds      │
   │          │  │          │  │  tmp-core)    │
   └────┬─────┘  └────┬─────┘  └──────┬───────┘
        │              │               │
        └──────────────┼───────────────┘
                       ▼
              ┌────────────────┐
              │   tmp-core     │
              │  (library)     │
              │                │
              │ schema         │
              │ resolve        │
              │ compile        │
              │ context        │
              │ generate       │
              │ registry       │
              │ versioning     │
              │ run            │
              └───────┬────────┘
                      │
                      ▼
              ┌────────────────┐
              │   command      │
              │  (library)     │
              └────────────────┘
```

---

## Development

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace
```

### Test Tiers

| Tier | Scope                                          |
| ---- | ---------------------------------------------- |
| 1    | Schema, context, config, resolver fundamentals |
| 2    | Generate, compile, resolve, run integration    |
| 3    | Registry, workflow, versioning                 |
| 4    | Agent adapter, subagent orchestration          |

---

## Further Reading

- [Whitepaper — Tool Mapping Protocol (Draft 0.4)](docs/whitepaper/tool-mapping-protocol.md)
- [Benchmark Plan](docs/whitepaper/benchmark-plan.md)
