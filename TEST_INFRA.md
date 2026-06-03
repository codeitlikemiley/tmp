# E2E Test Infra: Terminal Meta Protocol (tmp)

## Test Philosophy
- Opaque-box, requirement-driven. No dependency on implementation design.
- Methodology: Category-Partition + BVA + Pairwise + Workload Testing.
- Executed via Cargo integration tests, invoking the compiled `tmp` binary.
- Complete isolation: Every test run uses a unique temporary directory, custom home directory, and custom config file to prevent interfering with local settings.

## Feature Inventory
| # | Feature | Source (requirement) | Tier 1 | Tier 2 | Tier 3 |
|---|---------|---------------------|:------:|:------:|:------:|
| 1 | `init` & Config | ORIGINAL_REQUEST §R1, R4 | 5      | 5      | ✓      |
| 2 | `init-agent` | ORIGINAL_REQUEST §R7 | 5      | 5      | ✓      |
| 3 | `schema` | ORIGINAL_REQUEST §R4 | 5      | 5      | ✓      |
| 4 | `registry` | ORIGINAL_REQUEST §R8 | 5      | 5      | ✓      |
| 5 | `compile` & Watcher | ORIGINAL_REQUEST §R3, R10 | 5      | 5      | ✓      |
| 6 | `resolve` & `run` | ORIGINAL_REQUEST §R4, R10 | 5      | 5      | ✓      |
| 7 | `generate` & Versioning | ORIGINAL_REQUEST §R5, R6, R11 | 5      | 5      | ✓      |
| 8 | `workflow` | ORIGINAL_REQUEST §R9 | 5      | 5      | ✓      |

## Test Architecture
- **Test Runner**: Standard cargo integration tests under `tests/`.
- **Command Invocation**: Executed using `std::process::Command` against the compiled `tmp` binary path.
- **Environment Isolation**: Custom `HOME` and `TMP_CONFIG_DIR` environment variables point to a per-test temporary directory (via `tempfile`).
- **Mock Servers**:
  - **Mock Registry**: A mock local HTTP server (or file-path override) to serve `index.json` and schema downloads.
  - **Mock LLM**: A mock local server responding to OpenAI/Gemini/Ollama API endpoints to test command grounding, schema generation, and fallback/rotation.

## Test Case Mapping

### Tier 1 - Feature Coverage (>=5 per feature)
- **F1 (Init & Config)**:
  1. `tmp init` with default options (verify config.toml creation).
  2. `tmp init` custom config path.
  3. Precedence: Config values read from custom environment variable versus config.toml.
  4. Invalid config validation: CLI fails with clean error when config.toml contains malformed syntax.
  5. Multi-provider key loading (Gemini, OpenAI, Ollama).
- **F2 (Agent Bridge)**:
  1. `tmp init-agent claude` (verify CLAUDE.md snippet created).
  2. `tmp init-agent cursor` (verify `.cursor/rules/tmp.mdc` created with YAML frontmatter).
  3. `tmp init-agent all` (verify all rules/bridge files created: claude, codex, antigravity, cursor, copilot, windsurf).
  4. Merging: `tmp init-agent claude` when CLAUDE.md already exists (must append/merge, not overwrite).
  5. `tmp init-agent` with invalid agent target returns a clean error.
- **F3 (Schema Management)**:
  1. `tmp schema list` displays installed schemas and metadata.
  2. `tmp schema share <tool>` exports to file.
  3. `tmp schema import <file>` imports from a local path.
  4. `tmp schema keywords <tool>` displays schema keywords.
  5. `tmp schema keywords <tool> [words...]` sets custom keywords.
- **F4 (Registry)**:
  1. `tmp registry search <query>` queries mock index and returns matches.
  2. `tmp registry install <tool>` downloads and saves schema to local config.
  3. `tmp registry publish <tool>` exports schema for submission.
  4. Search no results: `tmp registry search` returns clean zero results message.
  5. Offline fallback: client fails gracefully when registry server is unreachable.
- **F5 (Context & Compiler)**:
  1. `tmp compile` in a Cargo project (produces `.tmp/context.md` and `.tmp/commands.json`).
  2. `tmp compile` in an npm project (produces correct output).
  3. `tmp compile` auto-adds `.tmp/` to `.gitignore`.
  4. Project root resolution (walk up to locate `Cargo.toml`).
  5. Resolver execution: `cargo:packages` and `git:branches` resolve correctly.
- **F6 (Resolve & Run)**:
  1. `tmp resolve` grounding query returns command JSON.
  2. `tmp run --dry-run` prints the resolved command.
  3. `tmp run` executes the command and returns the output.
  4. Custom tool scoping: `tmp resolve "query" --tool <name>`.
  5. Resolution mismatch: outputs clear error when query does not match any schemas.
- **F7 (Generate, TUI & Versioning)**:
  1. `tmp generate <tool>` help-parser recursively executes `--help`.
  2. `tmp generate <tool> --verify` CLI routing arguments check (TUI launch verification).
  3. Regeneration backup: `--force` creates version backups in `versions/<tool>/vN.json`.
  4. `tmp generate <tool> --history` displays version timeline.
  5. `tmp generate <tool> --rollback [version]` restores previous schema version.
- **F8 (Workflows)**:
  1. `tmp workflow list` lists configured workflows.
  2. `tmp workflow add <name> --from <file>` adds a workflow.
  3. `tmp workflow run <name>` executes sequentially.
  4. Parameter substitution replaces tokens with runtime values.
  5. Workflow error propagation: subsequent steps are aborted if a step fails.

### Tier 2 - Boundary & Corner Cases (>=5 per feature)
- **F1 (Init & Config)**:
  1. Initializing in read-only home directories (fail gracefully).
  2. Missing API keys in both environment and config (returns clear error when running dependent commands).
  3. Overlapping API keys: environment variable vs config.toml (env var must take precedence).
  4. Malformed config values (e.g. non-numeric port in Ollama provider).
  5. Provider rotation fallback strategy when primary provider endpoint is unreachable.
- **F2 (Agent Bridge)**:
  1. Appending to write-protected target files (returns clear permission error).
  2. Repeated runs of `tmp init-agent` (should not add duplicate snippets to files).
  3. Merging with pre-existing rule files having custom content.
  4. Initializing agent bridge when project directory doesn't exist.
  5. Merging rule snippets into empty files (should format cleanly).
- **F3 (Schema Management)**:
  1. Importing a schema with corrupted or malformed JSON (fails with validation errors).
  2. Schema share target directory does not exist (creates directory or returns clean error).
  3. Keywords set with special characters and long inputs.
  4. Schema import with a tool name that conflicts with an existing curated schema.
  5. Schema share of non-existent tool.
- **F4 (Registry)**:
  1. Server returns malformed/empty JSON for registry search.
  2. Registry install of non-existent schema in mock registry.
  3. Submitting schema to registry with invalid auth token.
  4. Network timeouts during registry search and install.
  5. Downloading schema when registry rate-limit is exceeded (429 HTTP status).
- **F5 (Context & Compiler)**:
  1. Compiling in nested multi-project workspaces (e.g., Cargo inside npm).
  2. Compile when data resolver command fails (e.g. `npm run` command returns error).
  3. Watching directory with massive number of files (ensure file watcher doesn't overflow).
  4. `.gitignore` exists but has no newline at EOF (merge snippet cleanly).
  5. Resolver returns empty data (system compiles cleanly).
- **F6 (Resolve & Run)**:
  1. Grounding query containing command injection characters (e.g. `; rm -rf`).
  2. Grounding query exceeding model context limit.
  3. `tmp run` for command requiring missing system binary.
  4. `tmp run` executing interactive commands (ensure stdin pass-through or safe exit).
  5. `tmp run` when multiple matching commands are found (rank and select best).
- **F7 (Generate, TUI & Versioning)**:
  1. Help parser encounters cyclic subcommand structure.
  2. LLM response is malformed or invalid schema JSON.
  3. Rolling back schema to non-existent version.
  4. Version history list when no history exists.
  5. LLM key rotation exhaustion: all keys fail.
- **F8 (Workflows)**:
  1. Workflow steps containing cyclical workflow calls (infinite loop protection).
  2. Missing tokens required by the workflow (prompt or fail cleanly).
  3. Running non-existent workflow.
  4. Workflow file contains malformed YAML/JSON.
  5. Step execution timeout handling.

### Tier 3 - Cross-Feature Combinations (Pairwise Coverage)
1. **Init + Compile**: Initialize project, configure providers, compile context.
2. **Registry Install + Compile**: Install a custom schema from the registry, then compile to verify the new commands are included in `.tmp/commands.json`.
3. **Generate + Verify (TUI) + Schema List**: Generate a schema, verify/edit it, check `schema list` reflecting updated verified status.
4. **Compile + Resolve + Run**: Compile cargo context, resolve query to a specific cargo test command, and run it.
5. **Workflow Add + Run + Versioning**: Add a workflow that runs a command from a generated schema, roll back the schema, and verify the workflow still runs or handles the change gracefully.
6. **Init-Agent + Compile**: Initialize agent bridge files, then compile to ensure the `.tmp/context.md` path is properly aligned with agent instruction paths.
7. **Keywords + Resolve**: Add custom keywords to schema, resolve query matching those keywords.
8. **Registry Publish + Import**: Share a schema, publish to mock registry, import back under a new name.

### Tier 4 - Real-World Application Scenarios
1. **Cargo Workspace End-to-End Pipeline**:
   - Run `tmp init`.
   - Setup a Cargo project with 2 members, tests, and examples.
   - Run `tmp compile`.
   - Resolve "run unit tests for library" -> verify matches `cargo test -p tmp-core`.
   - Run `tmp run` -> verify unit tests execute.
2. **Node.js/npm App E2E Integration**:
   - Run `tmp init`.
   - Set up npm project with script `lint` and `test`.
   - Run `tmp compile`.
   - Resolve "run linting" -> matches `npm run lint`.
   - Run `tmp run` -> execute linting.
3. **New Tool Schema Bootstrapping**:
   - Run `tmp generate custom-tool` on a mock CLI tool.
   - Run `tmp generate custom-tool --verify` (simulate verify & save).
   - Check `tmp schema list` and ensure it is active.
   - Run `tmp compile` in a project utilizing the custom tool.
4. **Multi-Step Release Workflow**:
   - Create a workflow `release` with steps: `cargo test`, `tmp schema share`, `git status`.
   - Run `tmp workflow run release`.
   - Verify all steps executed sequentially.
5. **AI Agent Interaction Simulation**:
   - Generate `CLAUDE.md` and `.cursor/rules/tmp.mdc`.
   - Compile context.
   - Read `.tmp/context.md` and verify it contains clear instructions, project layout, and instructions for how the AI agent should call `tmp resolve` and `tmp run`.

## Coverage Thresholds
- Tier 1: ≥5 per feature (8 features = 40 tests)
- Tier 2: ≥5 per feature (8 features = 40 tests)
- Tier 3: Pairwise coverage of major feature interactions (≥8 tests)
- Tier 4: ≥5 realistic application scenarios
