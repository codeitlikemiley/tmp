# PRD: Rebuilding TMP as a Well-Designed Rust Workspace

Version: Draft 0.1  
Date: 2026-06-03  
Primary reference: [Tool Mapping Protocol white paper](../whitepaper/tool-mapping-protocol.md)  
Audience: AI coding agents, maintainers, Rust engineers

## 1. Purpose

This PRD defines how to rebuild `tmp` into a well-designed Rust workspace that follows the Tool Mapping Protocol white paper and Rust best practices.

The document is intentionally implementation-oriented. An AI agent should be able to read it and execute the rebuild in phases without needing hidden context.

Plain English:

> TMP is a deterministic map from intent to verified operations. The crate should let humans, terminals, and agents ask "what should run?" before anything runs, then return a grounded result with evidence.

## 2. Source of Truth

Use these repository artifacts as authoritative inputs:

| Artifact | Role |
| --- | --- |
| `docs/whitepaper/tool-mapping-protocol.md` | Product and protocol vision. |
| `docs/whitepaper/benchmark-plan.md` | Hypotheses and measurement model. |
| `docs/whitepaper/benchmark-runs.schema.json` | Benchmark result contract. |
| `tmp-core/` | Existing deterministic core implementation. |
| `tmp/` | Existing CLI implementation. |
| `tests/` | Existing end-to-end behavior. |
| `PROJECT.md` | Existing rough phase notes; useful but secondary to this PRD. |

If this PRD conflicts with the white paper, prefer the white paper for product intent and this PRD for implementation sequence.

## 3. Product Goals

1. Rebuild TMP as a deterministic Rust core plus adapters.
2. Make the core embeddable, testable, and independent of AI providers.
3. Support CLI operation mapping first, while keeping API, SQL, workflow, script, completion, agent, and output-policy surfaces in the data model.
4. Fail closed when intent cannot be mapped.
5. Preserve raw execution evidence when output is summarized.
6. Provide benchmark data for claims about tool calls, token usage, output reduction, and correctness.
7. Make generated schemas and generated RTK filters draft-only until verified.

## 4. Non-Goals

| Non-Goal | Reason |
| --- | --- |
| Built-in LLM provider integration | Users should bring their own agent; TMP core remains deterministic. |
| Arbitrary SQL generation | TMP should map safe templates, not invent queries. |
| Replacing RTK | RTK can be an output adapter for supported CLI commands. |
| One giant crate | The core must stay separate from CLI/UI/server concerns. |
| Silent execution on uncertain intent | Unknown or ambiguous intent must not run. |

## 5. Success Criteria

The rebuild is successful when:

1. `tmp-core` exposes a stable operation-map API.
2. `tmp` CLI can execute the lifecycle: `init -> generate -> verify -> compile -> resolve -> run`.
3. Draft maps are never treated as verified.
4. Unknown intent does not invoke any operation.
5. High-risk operations require approval.
6. Output policies can return compact summaries and raw-output pointers.
7. `tmp generate rtk <operation-id>` can create an unverified RTK-compatible draft when applicable.
8. Benchmarks can record baseline vs TMP-assisted runs using the JSON schema.
9. Rust quality gates pass:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked
```

## 6. Users and Clients

| Client | Need |
| --- | --- |
| Human developer | Resolve and run correct commands safely. |
| Terminal | Ask for context-aware completions. |
| AI agent | Resolve intent in one grounded call. |
| Workflow runner | Invoke named multi-step operations. |
| Registry publisher | Share verified maps with metadata and checksums. |
| Tool author | Embed `tmp-core` without using the CLI. |

## 7. Architecture Overview

The workspace should keep protocol logic in the library and interface logic in adapters.

```text
tmp-core
  schema + context + discover + generate + verify + compile + resolve
  + invoke + output policy + registry + benchmark contracts

tmp
  CLI argument parsing + subcommand dispatch + TUI + user prompts

crates/command
  process execution helpers and cross-platform command abstractions

crates/tmp-agent
  optional agent-facing HTTP adapter or experimental harness
```

### Required Workspace Shape

| Crate | Responsibility | Must Not Own |
| --- | --- | --- |
| `tmp-core` | Protocol data model, deterministic algorithms, validation, adapters, benchmark models. | CLI parsing, terminal UI, HTTP server startup. |
| `tmp` | CLI UX, file output, prompts, command wiring, TUI verification. | Business logic that belongs in `tmp-core`. |
| `crates/command` | Process execution helper layer. | TMP schema semantics. |
| `crates/tmp-agent` | Optional HTTP/agent adapter. | Core resolver or schema authority. |

## 8. Rust Engineering Requirements

These requirements are mandatory for implementation.

### 8.1 Library vs Binary Errors

Use typed errors in `tmp-core`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("no operation matched intent: {intent}")]
    NoMatch { intent: String },
    #[error("schema error: {0}")]
    Schema(#[from] SchemaError),
}
```

Use `anyhow` only in binaries such as `tmp/src/main.rs` or CLI command handlers.

### 8.2 Ownership and API Shape

Use borrowed input parameters by default:

```rust
pub fn resolve(intent: &str, context: &Context, options: &ResolveOptions) -> Result<ResolvedOperation, ResolveError>
```

Avoid APIs that force unnecessary clones:

- Prefer `&str` over `String` for input.
- Prefer `&[T]` over `Vec<T>` for input.
- Return owned values only when the caller must store the result.
- Do not clone large schemas in loops.

### 8.3 Linting Discipline

- Fix warnings instead of suppressing them.
- If a Clippy lint must be suppressed, use `#[expect(...)]` with a short reason.
- Do not use global `allow` for convenience.
- Prefer workspace lint configuration once the crate stabilizes.

### 8.4 Testing as Documentation

Tests should read like behavior specifications:

```rust
#[test]
fn resolve_should_fail_closed_when_intent_has_no_matching_operation() {
    // ...
}
```

Prefer one behavior per test. Use integration tests for CLI flows and unit tests for parser/resolver/output-policy edge cases.

### 8.5 Production Panics

No `unwrap()` or `expect()` in production paths unless failure is proven impossible and documented. Tests and test helpers may use them.

### 8.6 Dependency Boundaries

Keep dependencies local to the crate that needs them.

| Dependency Class | Allowed Location | Rule |
| --- | --- | --- |
| `serde`, `serde_json`, `thiserror` | `tmp-core` | OK for protocol models and typed errors. |
| `anyhow` | `tmp`, examples, test helpers | Do not expose from library APIs. |
| `clap` | `tmp` | CLI parsing only. |
| `ratatui`/terminal UI | `tmp` | Never in `tmp-core`. |
| `axum`/HTTP server | `crates/tmp-agent` | Never in deterministic core. |
| async runtimes | adapter crates only until needed | Do not force async into core traits prematurely. |
| AI provider SDKs | external tooling only | Not allowed in `tmp-core` or deterministic resolver. |

No crate may introduce a dependency cycle. `tmp-core` must remain the lowest protocol layer.

### 8.7 Public API Documentation

All public `tmp-core` types and functions should have `///` doc comments before stabilization. Add doc examples for core flows where possible:

- Parse and validate an operation map.
- Compile context.
- Resolve intent.
- Shape output.
- Validate benchmark records.

## 9. Protocol Data Model

The new data model should align with the white paper while preserving backward compatibility with the current CLI schema.

### 9.1 Core Types

```rust
pub struct OperationMap {
    pub meta: MapMeta,
    pub operations: Vec<Operation>,
}

pub struct Operation {
    pub id: OperationId,
    pub surface: Surface,
    pub intents: Vec<String>,
    pub description: String,
    pub invocation: Invocation,
    pub parameters: Vec<Parameter>,
    pub effect: Effect,
    pub risk: Risk,
    pub approval: Approval,
    pub output_policy: OutputPolicyConfig,
    pub evidence: Vec<Evidence>,
    pub verified: bool,
}

pub struct Parameter {
    pub name: String,
    pub description: String,
    pub kind: ParameterKind,
    pub required: bool,
    pub default: Option<String>,
    pub values: Option<Vec<String>>,
    pub resolver: Option<ResolverRef>,
}
```

### 9.2 Enums

```rust
pub enum Surface {
    Cli,
    Api,
    Sql,
    Workflow,
    Script,
    Completion,
    Agent,
}

pub enum Effect {
    ReadOnly,
    LocalWrite,
    BuildTest,
    Network,
    DatabaseWrite,
    Deployment,
    Destructive,
}

pub enum Risk {
    Low,
    Medium,
    High,
}

pub enum Approval {
    NotRequired,
    Recommended,
    Required,
}
```

### 9.3 Backward Compatibility

The current schema names `Command` and `Token` may remain as compatibility aliases for one migration period.

Required serde behavior:

| Current Field | New Field | Requirement |
| --- | --- | --- |
| `commands` | `operations` | Deserialize both; serialize new name by default. |
| `tokens` | `parameters` | Deserialize both; serialize new name by default. |
| `command` | `invocation.template` | Preserve old CLI template import. |
| `verified` | `verified` | Preserve semantics. |

Migration must include tests for old and new JSON.

## 10. Core Traits

Define small traits around stable responsibilities.

```rust
pub trait SurfaceAdapter {
    type Error;

    fn discover(&self, context: &Context) -> Result<Vec<RawFact>, Self::Error>;
    fn map(&self, facts: &[RawFact]) -> Result<Vec<OperationDraft>, Self::Error>;
    fn invoke(&self, operation: &ResolvedOperation) -> Result<InvocationResult, Self::Error>;
}

pub trait ValueResolver {
    type Error;

    fn values(&self, parameter: &Parameter, context: &Context) -> Result<Vec<String>, Self::Error>;
}

pub trait OutputPolicy {
    type Error;

    fn shape(&self, raw: &RawOutput, operation: &ResolvedOperation) -> Result<OutputSummary, Self::Error>;
}

pub trait EvidenceCheck {
    type Error;

    fn verify(&self, operation: &Operation, context: &Context) -> Result<EvidenceResult, Self::Error>;
}
```

Trait design rules:

- Keep traits narrow.
- Prefer static dispatch in core paths.
- Use trait objects only at adapter registries or runtime plugin boundaries.
- Return typed errors.
- Avoid async traits until a concrete async adapter requires them.

## 11. CLI Contract

The CLI must be a thin adapter over `tmp-core`.

| Command | Required Behavior | Acceptance Criteria |
| --- | --- | --- |
| `tmp init` | Create config and directories. | Idempotent; does not overwrite user files without approval. |
| `tmp generate <tool>` | Draft operation map from help/discovery. | Writes `verified: false`; no model calls. |
| `tmp verify <schema>` | Verify schema or operation evidence. | Adds evidence or returns actionable errors. |
| `tmp compile` | Compile context and maps. | Writes machine and Markdown context artifacts. |
| `tmp resolve "<intent>"` | Resolve intent to operation. | Fails closed on no match. |
| `tmp run` | Invoke last resolved operation. | Checks risk/approval; stores raw and summary output. |
| `tmp output show --last --raw` | Reveal retained raw output. | Prints path/content for last run. |
| `tmp generate rtk <operation-id>` | Draft RTK-compatible filter. | Draft only; includes sample tests when possible. |
| `tmp benchmark run` | Record benchmark metrics. | Emits JSON matching `benchmark-runs.schema.json`. |
| `tmp init-agent <agent>` | Write external-agent instructions. | Tells agent to call TMP before unknown commands. |

## 12. Output Policy Requirements

Output policy is not just formatting. It is part of the protocol.

### 12.1 Result Envelope

```json
{
  "operation_id": "cargo.test",
  "invocation": "cargo test",
  "compressor": "tmp_builtin",
  "exit_status": 101,
  "success": false,
  "summary": {
    "failed_tests": 2,
    "first_failure": "parser::tests::rejects_invalid_escape"
  },
  "omitted": {
    "passing_test_lines": 148,
    "progress_lines": 22
  },
  "raw_output": {
    "retained": true,
    "location": ".tmp/runs/<run-id>/raw.log"
  }
}
```

### 12.2 Required Modes

| Mode | Minimum Behavior |
| --- | --- |
| `raw` | Pass through full output and still record metrics. |
| `test_summary` | Preserve failures, panic text, compiler errors, counts, exit status. |
| `diff_summary` | Preserve changed files, hunk headers, bounded relevant hunks. |
| `git_summary` | Preserve branch, staged, unstaged, untracked, ahead/behind. |
| `search_summary` | Preserve matched files, counts, bounded snippets. |
| `log_summary` | Preserve recent errors, warnings, service/timestamp identifiers. |
| `json_projection` | Preserve selected fields and schema hints. |

### 12.3 RTK Integration

TMP may use RTK when the operation is CLI-based and RTK supports the command.

Decision path:

```text
operation surface is CLI?
  no  -> use TMP-native output policy
  yes -> check RTK support
          supported -> invoke through RTK
          unsupported -> TMP policy, generated RTK draft, or raw passthrough
```

Generated RTK filters must be:

- Marked draft.
- Sample-based.
- Testable.
- Not trusted until reviewed.

## 13. Safety and Approval Requirements

Risk decisions should be deterministic.

| Effect | Default Risk | Approval |
| --- | --- | --- |
| `ReadOnly` | Low | Not required |
| `BuildTest` | Low | Not required |
| `LocalWrite` | Medium | Recommended |
| `Network` | Medium | Recommended |
| `DatabaseWrite` | High | Required |
| `Deployment` | High | Required |
| `Destructive` | High | Required |

Rules:

- Non-interactive mode must not prompt forever.
- High-risk operations require `--yes` or explicit approval.
- Approval prompts use stderr.
- Approval decisions are recorded in the run summary.

## 14. Benchmark Requirements

Benchmarking must prove or disprove TMP's claims.

### 14.1 Modes

| Mode | Meaning |
| --- | --- |
| `baseline` | Agent/user works without TMP. |
| `tmp_assisted` | Agent/user can compile, resolve, and run through TMP. |
| `tmp_completion` | Completion candidates come from TMP. |
| `tmp_output_policy` | Output shaping comes from TMP. |
| `tmp_rtk` | TMP resolves operation and RTK compresses output. |
| `tmp_generated_rtk_filter` | TMP drafts filter for unsupported command. |

### 14.2 Required Metrics

- Tool calls.
- Input tokens.
- Output tokens.
- Raw output bytes.
- Shaped output bytes.
- Wall time.
- Success.
- Invalid operation attempted.
- Clarifications.
- Failed attempts.
- Output signal preserved.

Benchmark records must validate against:

```text
docs/whitepaper/benchmark-runs.schema.json
```

## 15. Implementation Phases

Each phase should be independently shippable and tested.

### Phase 0: Stabilize Baseline

Goal: ensure current behavior is known before rebuilding.

Tasks:

1. Run `cargo fmt --all --check`.
2. Run `cargo clippy --all-targets --all-features --locked -- -D warnings`.
3. Run `cargo test --workspace --locked`.
4. Record current failures, if any, before changing architecture.

Acceptance:

- Baseline status is documented.
- No unrelated files are modified.

### Phase 1: Operation Model

Goal: introduce the white-paper operation model with backward compatibility.

Tasks:

1. Add `OperationMap`, `Operation`, `Parameter`, `Surface`, `Effect`, `Risk`, `Approval`, `Evidence`, `OutputPolicyConfig`.
2. Preserve old `Command`/`Token` deserialization.
3. Add validation for operation IDs, parameter names, required fields, and verified/draft state.
4. Update schema tests for old and new JSON.

Acceptance:

- Old schemas import.
- New schemas serialize with new names.
- Invalid names and missing fields fail with typed errors.

### Phase 2: Core Traits and Adapters

Goal: separate protocol logic from surfaces.

Tasks:

1. Add `SurfaceAdapter`, `ValueResolver`, `OutputPolicy`, `EvidenceCheck`.
2. Implement `CliSurfaceAdapter`.
3. Move CLI-specific discovery/invocation behind the adapter.
4. Keep API/SQL/workflow/script adapters as typed extension points if not implemented yet.

Acceptance:

- Existing CLI flows still work.
- Core APIs can be called without the CLI crate.
- No `anyhow` leaks into `tmp-core`.

### Phase 3: Compile and Resolve

Goal: make compiled context and intent resolution operation-based.

Tasks:

1. Compile relevant maps for the current workspace.
2. Resolve dynamic values through resolvers.
3. Resolve intent to operation ID and filled parameters.
4. Fail closed when no verified or draft-allowed operation matches.

Acceptance:

- `tmp compile` writes context artifacts.
- `tmp resolve "run tests"` returns a mapped operation.
- Unknown intent returns an error and does not write a runnable command.

### Phase 4: Invocation, Approval, and Output Policies

Goal: make `tmp run` safe and auditable.

Tasks:

1. Add approval checks.
2. Store raw output under `.tmp/runs/<run-id>/raw.log`.
3. Store summary envelope under `.tmp/runs/<run-id>/summary.json`.
4. Implement initial output modes: `raw`, `test_summary`, `git_summary`.
5. Add `tmp output show --last --raw`.

Acceptance:

- High-risk operations require approval.
- Raw output is retained when output is shaped.
- Output summary includes exit status, success, omitted counts, and raw pointer.

### Phase 5: RTK Filter Drafting

Goal: support `tmp generate rtk <operation-id>`.

Tasks:

1. Detect whether operation invocation is CLI and line-oriented.
2. Generate `.rtk/filters.toml` draft or TMP-native policy JSON.
3. Include sample tests when representative output exists.
4. Mark generated filter as unverified.

Acceptance:

- Generated filter is draft-only.
- Generated filter contains `match_command`, filter rules, and at least one test when samples exist.
- TMP refuses to treat generated filter as trusted without verification.

### Phase 6: Completion Adapter

Goal: expose operation and parameter candidates.

Tasks:

1. Add completion query API in `tmp-core`.
2. Add CLI command or shell integration output.
3. Support resolver-backed values such as `cargo:bins`, `git:branches`, `npm:scripts`.

Acceptance:

- Completion candidates reflect current workspace.
- Invalid candidates are filtered out when resolver data exists.

### Phase 7: Registry and Trust

Goal: share maps safely.

Tasks:

1. Add publisher metadata.
2. Add checksums.
3. Add compatibility metadata.
4. Add verification status.
5. Keep drafts separate from trusted maps.

Acceptance:

- Install verifies checksum.
- Registry listing shows verification/trust level.
- Draft maps cannot masquerade as verified maps.

### Phase 8: Benchmark Harness

Goal: prove or disprove TMP claims.

Tasks:

1. Add benchmark fixtures.
2. Record baseline and TMP-assisted runs.
3. Validate records against benchmark schema.
4. Report deltas for tool calls, tokens, output bytes, invalid attempts.

Acceptance:

- `tmp benchmark run` emits valid JSON.
- At least one CLI scenario and one output-policy scenario are measured.

## 16. Required Tests by Phase

| Phase | Unit Tests | Integration Tests |
| --- | --- | --- |
| 1 | schema parse/validate/compatibility | import old schema and new schema |
| 2 | trait adapter behavior | CLI lifecycle still works |
| 3 | resolver scoring/fail-closed | compile -> resolve |
| 4 | output policy parsers | resolve -> run -> output show |
| 5 | RTK TOML draft generation | generate rtk with sample output |
| 6 | completion value filtering | completion command in fixture repo |
| 7 | checksum/trust metadata | registry install/publish |
| 8 | benchmark schema validation | benchmark fixture run |

## 17. AI Agent Execution Rules

Any AI agent implementing this PRD must:

1. Read this PRD and the white paper first.
2. Inspect current code before editing.
3. Make one phase-sized change at a time.
4. Preserve unrelated user changes.
5. Use `rg` for search.
6. Avoid destructive git commands.
7. Prefer `apply_patch` for manual edits.
8. Run the relevant quality gates before claiming completion.
9. Report unverified assumptions explicitly.
10. Never add AI provider calls to `tmp-core`.

## 18. Definition of Done

For a phase:

- Required code is implemented.
- Tests for that phase exist and pass.
- `cargo fmt --all --check` passes.
- `cargo clippy --all-targets --all-features --locked -- -D warnings` passes or documented current blockers exist.
- Public APIs have doc comments.
- Failure cases use typed errors.
- No unrelated files are changed.

For the full rebuild:

- All phases pass.
- White-paper lifecycle is represented in code.
- CLI and library workflows are both documented.
- Benchmark data can be produced.
- Unknown intent fails closed.
- Raw output retention works.
- Draft generation remains untrusted until verified.

## 19. Open Questions

These should be resolved before final API stabilization:

1. Should `Command`/`Token` remain type aliases forever, or only for one major version?
2. Should `tmp-core` expose async traits for API/SQL adapters, or keep async out until needed?
3. Should `.tmp/runs/` retention have a default cleanup policy?
4. Should `tmp generate rtk` write `.rtk/filters.toml` directly or a review file first?
5. Should benchmark token counts come from agent logs, API usage, or approximate local tokenizers?
6. Should `crates/tmp-agent` remain in this workspace or become a separate adapter crate?

## 20. Agent Starter Checklist

Use this checklist before starting implementation:

```text
[ ] Read docs/whitepaper/tool-mapping-protocol.md
[ ] Read docs/prd/tmp-rebuild-prd.md
[ ] Inspect Cargo.toml workspace members
[ ] Inspect tmp-core/src/schema.rs
[ ] Inspect tmp-core/src/resolve.rs
[ ] Inspect tmp-core/src/compile.rs
[ ] Inspect tmp/src/main.rs
[ ] Run cargo fmt --all --check
[ ] Run cargo test --workspace --locked
[ ] Pick exactly one implementation phase
[ ] Implement only that phase
[ ] Add or update tests for that phase
[ ] Run phase quality gates
[ ] Summarize changes and remaining risks
```
