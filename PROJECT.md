# Project: TMP Phases 2-8

## Architecture

The project is a Rust workspace with 4 crates:
- `tmp-core` — Core library: schema model, context detection, compilation, resolution, execution, registry, versioning
- `tmp` — CLI binary: clap-based commands, ratatui TUI
- `crates/command` — Thin `std::process::Command` wrapper
- `crates/tmp-agent` — Axum REST server (not modified in this project)

Phase 1 (CLI mapping) is complete. All changes extend the existing foundation.

## Code Layout

```
/Volumes/goldcoders/tmp/
├── Cargo.toml                    # Workspace root
├── tmp-core/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs                # Module exports
│   │   ├── schema.rs             # Schema model (Command→Operation, Token→Parameter)
│   │   ├── compile.rs            # Context compilation
│   │   ├── config.rs             # Config struct
│   │   ├── context.rs            # Context detection
│   │   ├── generate.rs           # Schema generation
│   │   ├── help.rs               # Help text parsing
│   │   ├── registry.rs           # Schema registry
│   │   ├── resolve.rs            # Intent resolution (has is_binary_available)
│   │   ├── resolver.rs           # DataResolver struct
│   │   ├── run.rs                # Command execution
│   │   ├── versioning.rs         # Schema versioning
│   │   ├── traits.rs             # NEW: SurfaceAdapter, Resolver, OutputPolicy, EvidenceCheck
│   │   ├── output_policy.rs      # NEW: Output policy implementations
│   │   ├── approval.rs           # NEW: Effect/risk/approval system
│   │   ├── completion.rs         # NEW: Shell completion support
│   │   ├── benchmark.rs          # NEW: Benchmark harness
│   │   └── *_tests.rs            # Test files
│   └── examples/
│       └── basic_resolve.rs      # NEW: Runnable example
├── tmp/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs               # CLI entry point (Commands enum)
│       ├── commands/
│       │   ├── mod.rs
│       │   ├── compile.rs
│       │   ├── generate.rs
│       │   ├── init.rs
│       │   ├── init_agent.rs
│       │   ├── registry.rs
│       │   ├── resolve.rs
│       │   ├── run.rs
│       │   ├── schema.rs
│       │   ├── workflow.rs
│       │   ├── verify.rs         # NEW: tmp verify <schema>
│       │   ├── output.rs         # NEW: tmp output show
│       │   └── benchmark.rs      # NEW: tmp benchmark run
│       └── tui/
├── tests/
│   ├── common/
│   ├── e2e_tier1.rs
│   ├── e2e_tier2.rs
│   ├── e2e_tier3.rs
│   └── e2e_tier4.rs
└── docs/whitepaper/
    ├── tool-mapping-protocol.md
    ├── benchmark-plan.md
    └── benchmark-runs.schema.json
```

## Milestones

| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| 1 | Schema Model Evolution | R1: Rename Command→Operation, Token→Parameter. Add surface/effect/risk/output_policy/evidence/approval enums. Backward compat deserialization. Update all references across codebase. | none | PLANNED |
| 2 | Core Traits + Approval | R2 + R4: Define SurfaceAdapter/Resolver/OutputPolicy/EvidenceCheck traits. Implement CliSurfaceAdapter. Effect/risk/approval system. Extract `is_binary_available`. Config struct expansion. | M1 | PLANNED |
| 3 | Output Policy Engine | R3: Implement concrete output policies (test_summary, git_summary, diff_summary, search_summary, log_summary). Result envelope JSON. Raw output retention. | M1, M2 | PLANNED |
| 4 | New CLI Commands | R5: `tmp verify`, `tmp output show`, `tmp generate rtk`, `tmp benchmark run`. Wire into main.rs Commands enum. | M1, M2, M3 | PLANNED |
| 5 | Completion + Benchmark + Quality | R6 + R7 + code quality: Shell completions, benchmark harness, fail-closed invariant tests, runnable example. | M1, M2, M3, M4 | PLANNED |

## Interface Contracts

### Schema Model (M1)

The renamed types must be used throughout:
- `Command` → `Operation` (struct name, field names, variable names, error messages)
- `Token` → `Parameter` (struct name, field names, variable names, error messages)
- `commands` field in Schema → `operations`
- `tokens` field in Operation → `parameters`
- Backward compat: `#[serde(alias = "commands")]` for `operations`, `#[serde(alias = "tokens")]` for `parameters`

New enums:
```rust
enum Surface { Cli, Api, Sql, Workflow, Script, Completion, Agent }
enum Effect { ReadOnly, BuildTest, Network, Deployment, Destructive }
enum Risk { Low, Medium, High }
enum Approval { NotRequired, Recommended, Required }
enum EvidenceType { ParsedHelp, DryRun, HumanReview, RegistrySignature }
struct OutputPolicyConfig { mode: OutputMode, raw_retention: Option<String> }
enum OutputMode { Raw, TestSummary, DiffSummary, GitSummary, LogSummary, SearchSummary, JsonProjection }
```

### Traits (M2)

```rust
// tmp-core/src/traits.rs
pub trait SurfaceAdapter {
    fn discover(&self, context: &Context) -> Vec<RawFact>;
    fn map(&self, facts: &[RawFact]) -> Vec<OperationDraft>;
    fn invoke(&self, op: &ResolvedOperation) -> InvocationResult;
}

pub trait Resolver {
    fn values(&self, parameter: &Parameter, context: &Context) -> Result<Vec<String>, String>;
}

pub trait OutputPolicy {
    fn shape(&self, raw: &RawOutput, op: &ResolvedOperation) -> OutputSummary;
}

pub trait EvidenceCheck {
    fn verify(&self, operation: &Operation) -> EvidenceResult;
}
```

### Output Policy (M3) ↔ Run (M2)

Result envelope format:
```json
{
  "operation_id": "cargo.test",
  "invocation": "cargo test",
  "exit_status": 0,
  "success": true,
  "summary": { ... },
  "omitted": { ... },
  "raw_output": { "retained": true, "location": ".tmp/runs/<timestamp>/raw.log" }
}
```

### Approval System (M2) ↔ CLI (M4)

- `tmp run` checks resolved operation's risk + effect before execution
- `--yes` flag added to `tmp run` command
- Non-interactive detection via `atty::is(Stream::Stdin)` or similar
- Approval prompt uses stderr for the question, reads stdin

### CLI Commands (M4) ↔ Core (M2, M3)

- `tmp verify <schema>` calls `EvidenceCheck::verify()`
- `tmp output show` reads from `.tmp/runs/` directory
- `tmp generate rtk` produces TOML filter draft
- `tmp benchmark run` uses benchmark harness from M5

## Quality Requirements

- `cargo fmt --all` — no changes
- `cargo clippy --all-targets --all-features -- -D warnings` — zero warnings  
- `cargo test --workspace` — all tests pass
- All existing E2E tests pass (update for renamed types only)
