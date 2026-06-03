# Original User Request

## Initial Request — 2026-06-03T04:27:15Z

Build **tmp** — the Terminal Meta Protocol. A standalone Rust CLI tool + library that makes CLI command schemas available to any AI coding agent (Claude Code, Codex, Antigravity, Cursor, Copilot, Windsurf, etc.) without MCP, without servers, and with near-zero context window overhead.

Unlike MCP (which requires a server, JSON-RPC, and hundreds of tokens of tool schemas in the AI's context), TMP uses a **file-based protocol**: pre-compiled context files that AI agents read as regular files, plus a CLI that agents call as plain shell commands.

The project is a fresh codebase, designed from scratch with a clean API. It is inspired by the schema system in waz but is NOT a port — it should have its own idiomatic Rust API design.

Working directory: /Volumes/goldcoders/tmp
Integrity mode: development

## Reference Material

The existing `waz` codebase at `/Volumes/goldcoders/waz` serves as **inspiration only** (not to be copied). Key concepts to study there:
- Schema format: `schemas/curated/cargo.json` — the SchemaFile/CommandEntry/TokenDef/DataSource structures
- Data source resolvers: `src/tui/cargo_schema.rs` — resolvers like `cargo:packages`, `git:branches`, `npm:scripts`
- AI schema generation: `src/generate.rs` — generating schemas from `--help` output via LLM
- NL→command resolution: `src/resolve.rs` — combining schemas + LLM for grounded commands
- Runtime context: `src/context.rs` — detecting project type, file kind, script engine
- LLM providers: `src/config.rs` + `src/llm.rs` — multi-provider rotation with fallback
- Schema verification TUI: `src/tui/verify.rs` — two-pane review interface with ratatui

## Requirements

### R1. Cargo Workspace Structure

A Cargo workspace at `/Volumes/goldcoders/tmp` with two crates:
- `tmp-core` — the library crate containing all protocol logic (schemas, compilation, resolution, registry, context detection, data resolvers, LLM integration). Other Rust tools (like waz) will depend on this as a library.
- `tmp` — the CLI binary crate that provides the `tmp` command. Thin wrapper over `tmp-core`.

### R2. Schema System

A JSON-based schema system for describing CLI tool commands. Each schema defines:
- **Meta**: tool name, version, author, verified status, coverage, keywords, requires_file, requires_binary
- **Commands**: each with a command string, description, group, verified flag, and tokens
- **Tokens**: each with name, description, required flag, type (String/Boolean/Enum/File/Number), default value, valid values list, CLI flag, and optional data source
- **Data Sources**: either a shell command to run (`{ "command": "brew list" }`) or a built-in resolver (`{ "resolver": "cargo:packages" }`)

Built-in resolvers must include at minimum: `cargo:packages`, `cargo:bins`, `cargo:examples`, `cargo:features`, `cargo:profiles`, `cargo:tests`, `cargo:benches`, `git:branches`, `git:remotes`, `npm:scripts`.

Curated schemas are NOT embedded in the binary. They live in the registry and are downloaded on `tmp init`.

### R3. Compiled Context System (`tmp compile`)

`tmp compile` scans the current project directory, loads relevant schemas, resolves all data sources, and generates a `.tmp/` directory containing:
- `.tmp/context.md` — a concise (30-80 lines) markdown file that any AI agent can read. Contains: project type, available commands with their key arguments (pre-resolved with real values), and instructions for using `tmp resolve` and `tmp run`.
- `.tmp/commands.json` — machine-readable full command bank with all resolved data source values.

The `.tmp/` directory should be gitignored by default. `tmp compile` should offer to add `.tmp/` to `.gitignore` if not already present.

### R4. CLI Contract

The `tmp` CLI must support these subcommands:
- `tmp compile [--cwd <dir>] [--watch]` — compile context for a project. `--watch` re-compiles on relevant file changes.
- `tmp resolve "query" --json [--cwd <dir>] [--tool <name>]` — natural language → grounded command using schemas + LLM.
- `tmp run [file[:line]] [--dry-run]` — execute the best command for the current file/project context.
- `tmp generate <tool> [--force] [--model <model>] [--provider <provider>] [--verify] [--history] [--rollback [version]]` — AI-powered schema generation from `--help` output.
- `tmp init` — download curated schemas from registry, set up config.
- `tmp init-agent <agent>` — generate AI agent config file (see R7).
- `tmp schema list` — list installed schemas.
- `tmp schema share <tool>` — export a schema as a shareable file.
- `tmp schema import <source>` — import from file or URL.
- `tmp schema keywords <tool> [words...]` — set/show custom keywords for AI matching.
- `tmp registry search <query>` — search the online registry.
- `tmp registry install <tool>` — install a schema from the registry.
- `tmp registry publish <tool>` — publish a schema to the registry.
- `tmp workflow list` — list available workflows.
- `tmp workflow run <name>` — execute a named workflow.
- `tmp workflow add <name> [--from <file>]` — add a new workflow.

### R5. AI Schema Generation

Generate schemas for any CLI tool using AI:
1. Run `<tool> --help` and recursively `<tool> <subcommand> --help`
2. Send help output to configured LLM
3. AI extracts commands, flags, argument types into structured JSON
4. Save to `~/.config/tmp/schemas/<tool>.json` with metadata

Must support multi-provider LLM configuration with:
- Providers: Gemini, OpenAI, Ollama, and any OpenAI-compatible API
- Strategies: single, fallback, round-robin
- Multiple API keys per provider with rotation
- Config at `~/.config/tmp/config.toml`
- Auto-detection from env vars (GEMINI_API_KEY, OPENAI_API_KEY, etc.)

### R6. Schema Verification TUI

A ratatui-based TUI for reviewing and approving schemas (`tmp generate <tool> --verify`):
- Two-pane layout: commands list + token details
- Toggle verified status per command
- Edit token properties (name, description, flag, type, required)
- Test data sources live
- Save changes back to JSON

### R7. Agent Bridge (`tmp init-agent`)

Generate the correct config/rule file for each AI agent that tells it about `.tmp/` and the `tmp` CLI:

| Agent | File generated |
|-------|---------------|
| `claude` | `CLAUDE.md` snippet (or `.claude/rules/tmp.md`) |
| `codex` | `AGENTS.md` snippet |
| `antigravity` | `AGENTS.md` snippet |
| `cursor` | `.cursor/rules/tmp.mdc` with proper YAML frontmatter |
| `copilot` | `.github/copilot-instructions.md` snippet |
| `windsurf` | `.windsurfrules` snippet |
| `all` | All of the above |

Generated content should be ~10-15 lines instructing the agent to read `.tmp/context.md` and use `tmp resolve` for grounded commands.

The command must detect if a target file already exists and append/merge rather than overwrite.

### R8. Schema Registry

A registry client that interacts with a GitHub-based schema repository (`codeitlikemiley/tmp-registry`):
- `index.json` at the repo root lists all published schemas with metadata (tool, version, author, commands count, verified, download URL)
- `tmp registry search <query>` — fetches and searches the index
- `tmp registry install <tool>` — downloads the schema JSON and saves to local config
- `tmp registry publish <tool>` — uploads/submits a schema to the registry (could be via GitHub API or generating a PR)

### R9. Workflows

Multi-step workflow support:
- Workflows are JSON files in `~/.config/tmp/workflows/` (and optionally `.tmp/workflows/` per-project)
- Each workflow has: name, description, steps (each with a command or schema reference), and tokens for parameterization
- `tmp workflow run <name>` executes steps sequentially, substituting tokens
- `tmp workflow list` shows available workflows
- `tmp workflow add` creates a new workflow

### R10. Runtime Context Detection

Detect project type and context from the working directory and optional file/line arguments:
- Detect build system: cargo, npm, go, python, etc.
- Detect file kind: cargo_project, single_file_script, standalone, etc.
- Detect script engine for single-file scripts (rust-script, cargo -Zscript)
- Resolve project root (walk up for Cargo.toml, package.json, etc.)
- Detect runnable kind and recommended target

### R11. Schema Versioning

Every `--force` regeneration auto-versions the old schema:
- Version history stored in `~/.config/tmp/schemas/versions/<tool>/v1.json`, `v2.json`, etc.
- `tmp generate <tool> --history` shows version timeline
- `tmp generate <tool> --rollback [n]` restores a previous version
- Show colorized diff on regeneration (added/removed/changed commands)

## Acceptance Criteria

### Build & Tests
- [ ] `cargo check` passes with zero warnings in the workspace
- [ ] `cargo test` passes with all tests green
- [ ] `cargo clippy -- -D warnings` passes
- [ ] `cargo fmt --check` passes
- [ ] Unit tests exist for: schema parsing, data source resolution, context detection, command building, registry index parsing, agent bridge generation, compile output generation
- [ ] Integration tests exist for: `tmp compile` producing valid `.tmp/` output, `tmp init-agent` generating correct files

### CLI Functionality
- [ ] `tmp compile` in a Cargo project produces a valid `.tmp/context.md` and `.tmp/commands.json`
- [ ] `tmp compile` in an npm project produces correct output
- [ ] `tmp generate <tool> --verify` launches the verification TUI
- [ ] `tmp init-agent claude` generates a valid CLAUDE.md snippet
- [ ] `tmp init-agent cursor` generates a valid `.cursor/rules/tmp.mdc` with YAML frontmatter
- [ ] `tmp init-agent all` generates files for all supported agents
- [ ] `tmp schema list` shows installed schemas with version/status
- [ ] `tmp registry search` returns results from the GitHub index (mock-testable)
- [ ] `tmp workflow list` shows available workflows
- [ ] `tmp workflow run` executes multi-step workflows with token substitution

### Library API
- [ ] `tmp-core` exports clean public types: `Schema`, `Command`, `Token`, `DataSource`, `Context`, `CompileOutput`, `ResolveResult`
- [ ] `tmp-core` can be used as a dependency by external Rust projects (clean, documented API)
- [ ] All public types implement `Serialize` + `Deserialize`

### Production Quality
- [ ] All errors are handled gracefully with user-friendly messages (no panics/unwraps in production code)
- [ ] CLI output is polished with colors, spinners, and clear formatting
- [ ] README.md documents all commands, the protocol concept, and setup instructions
- [ ] `tmp --help` and all subcommand `--help` messages are clear and complete
