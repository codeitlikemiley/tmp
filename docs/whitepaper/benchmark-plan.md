# TMP Benchmark Plan

This plan defines how to test TMP's main hypotheses. It is meant to be updated as prototypes mature.

## Hypotheses

1. TMP reduces agent tool-call count.
2. TMP reduces input and output token usage.
3. TMP reduces invalid or hallucinated operations.
4. TMP reduces time-to-correct-action.
5. TMP improves terminal completion relevance.
6. TMP improves success rate when tasks span CLI, API, SQL, workflows, and scripts.
7. TMP reduces noisy command output while preserving the signal needed for the next action.

## Experimental Modes

Every task should be run in at least two modes:

- `baseline`: agent/user has normal repo access but no TMP map.
- `tmp_assisted`: agent/user can use TMP compile, list, resolve, and invoke flows.

Optional future modes:

- `tmp_completion`: terminal completion scenario using TMP candidates.
- `tmp_registry`: task begins by installing a registry schema.
- `tmp_generated_schema`: task begins with agent-generated schema draft plus verification.
- `tmp_output_policy`: task invokes a mapped command and receives an operation-aware output summary.
- `tmp_rtk`: task resolves through TMP and compresses CLI output through RTK or an RTK-compatible adapter.
- `tmp_generated_rtk_filter`: task uses TMP to draft an RTK-compatible filter for a command RTK does not already support.

## Metrics

Record these fields for each run:

```json
{
  "run_id": "2026-06-03T00-00-00Z-example",
  "task_id": "cli.parser_tests",
  "mode": "tmp_assisted",
  "surface": "cli",
  "agent": "codex",
  "model": "user_selected",
  "tool_calls": 0,
  "input_tokens": 0,
  "output_tokens": 0,
  "raw_output_bytes": 0,
  "shaped_output_bytes": 0,
  "output_signal_preserved": true,
  "wall_time_ms": 0,
  "success": false,
  "invalid_operation_attempted": false,
  "user_clarifications": 0,
  "failed_attempts": 0,
  "notes": ""
}
```

## Scenario Matrix

| ID | Surface | Task | Success Criteria |
| --- | --- | --- | --- |
| `cli.unit_tests` | CLI | Run unit tests for a named package or module. | Correct test command runs. |
| `cli.bin_completion` | Completion | Complete `cargo run --bin <TAB>`. | Candidates match workspace binaries. |
| `api.deploy_staging` | API | Create a staging deployment. | Correct endpoint and approval classification. |
| `sql.recent_failures` | SQL | Show recent failed jobs. | Valid read-only query with bounded limit. |
| `sql.block_mutation` | SQL | Try to delete old rows. | TMP classifies as write/destructive and blocks or requests approval. |
| `workflow.release_candidate` | Workflow | Run release candidate workflow. | Correct ordered workflow selected. |
| `script.sync_data` | Script | Run data sync for a target environment. | Correct script args and environment selected. |
| `output.cargo_test` | Output policy | Run tests and summarize failures. | Summary preserves failures, panics, compiler errors, counts, exit status, and raw-output pointer. |
| `output.tmp_rtk_cargo_test` | Output policy | Resolve tests with TMP and compress command output with RTK. | TMP records operation metadata and RTK reduces shell noise without losing failure signal. |
| `output.generated_rtk_filter` | Output policy | Generate an RTK-compatible filter for an unsupported local command. | Draft filter includes samples, tests, raw-output pointer, and remains unverified until reviewed. |
| `output.unsupported_rtk_command` | Output policy | Run a command that RTK does not rewrite. | TMP detects no RTK compressor and chooses generated draft, TMP-native policy, or raw passthrough. |
| `output.git_status` | Output policy | Summarize repository status. | Summary preserves branch and grouped changed paths without boilerplate. |
| `output.git_diff` | Output policy | Summarize a large diff. | Summary preserves changed files and bounded relevant hunks with raw-output pointer. |
| `output.rg` | Output policy | Summarize search results. | Summary preserves match files, counts, and bounded snippets. |
| `output.logs` | Output policy | Summarize service logs. | Summary preserves recent errors, warnings, service names, timestamps, and trace IDs. |
| `registry.install_reuse` | Registry | Install and reuse a schema. | Schema installs and resolves a task. |
| `missing.schema` | Failure | Ask for an unmapped action. | TMP fails closed with no invented operation. |

## Measurement Procedure

1. Prepare a fixture repo or environment for the scenario.
2. Reset generated files and caches.
3. Run the baseline mode.
4. Record metrics.
5. Reset the environment again.
6. Run the TMP-assisted mode.
7. Record metrics.
8. Compare deltas.

Recommended derived metrics:

- `tool_call_reduction = baseline_tool_calls - tmp_tool_calls`
- `token_reduction = baseline_tokens - tmp_tokens`
- `output_reduction_ratio = 1 - shaped_output_bytes / raw_output_bytes`
- `latency_reduction_ms = baseline_wall_time_ms - tmp_wall_time_ms`
- `invalid_operation_delta = baseline_invalid_attempts - tmp_invalid_attempts`

For TMP plus RTK scenarios, compare at least three modes:

- `baseline`: raw command output reaches the agent.
- `tmp_output_policy`: TMP resolves and shapes output with a built-in policy.
- `tmp_rtk`: TMP resolves the operation and delegates CLI output compression to RTK.
- `tmp_generated_rtk_filter`: TMP drafts an RTK-compatible filter when RTK does not already support the command.

This tells us whether the combined approach improves both sides of the problem: fewer exploratory calls from TMP and smaller command results from RTK.

## Initial Acceptance Targets

These are proposed targets for early validation:

- Reduce tool calls by at least 50% for common CLI resolution tasks.
- Reduce token usage by at least 30% for tasks requiring command discovery.
- Reduce shaped command output by at least 60% for noisy command classes while preserving required failure/status signal.
- Generate RTK-compatible draft filters for unsupported but line-oriented commands with at least one passing sample test.
- Keep invalid operation attempts at zero for verified TMP mappings.
- Achieve completion precision at 5 of at least 90% for dynamic CLI completions.
- Fail closed for 100% of missing-schema tasks.

## Evidence to Capture

For each benchmark run, save:

- Agent transcript or terminal log.
- TMP command outputs.
- Raw command output and TMP-shaped output for output-policy scenarios.
- Generated `.tmp/context.md` and `.tmp/commands.json` when relevant.
- Token/tool-call accounting if available from the agent.
- Final operation invoked.
- Whether the agent needed to request raw output to proceed.
- Whether generated output filters were reviewed, trusted, and verified before use.
- Pass/fail outcome.

## Open Questions

- Which agents should be included in the first comparison?
- Which benchmark fixtures should be maintained in this repository?
- Should token counts be collected through agent APIs, logs, or manual estimates?
- Should TMP expose a benchmark command, or should benchmarks stay as external fixtures initially?
- What risk classification vocabulary should be stable before SQL/API benchmarks?
- Which output policies should be built first: tests, diffs, search, status, or logs?
- What threshold defines "signal preserved" for each output policy?
- Should `tmp generate rtk` emit `.rtk/filters.toml`, TMP-native policy JSON, or both?
- How should TMP detect that RTK already supports a command: call `rtk rewrite`, inspect a registry export, or maintain compatibility metadata?
