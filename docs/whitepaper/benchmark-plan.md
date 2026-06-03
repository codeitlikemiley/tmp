# TMP Benchmark Plan

This plan defines how to test TMP's main hypotheses. It is meant to be updated as prototypes mature.

## Hypotheses

1. TMP reduces agent tool-call count.
2. TMP reduces input and output token usage.
3. TMP reduces invalid or hallucinated operations.
4. TMP reduces time-to-correct-action.
5. TMP improves terminal completion relevance.
6. TMP improves success rate when tasks span CLI, API, SQL, workflows, and scripts.

## Experimental Modes

Every task should be run in at least two modes:

- `baseline`: agent/user has normal repo access but no TMP map.
- `tmp_assisted`: agent/user can use TMP compile, list, resolve, and invoke flows.

Optional future modes:

- `tmp_completion`: terminal completion scenario using TMP candidates.
- `tmp_registry`: task begins by installing a registry schema.
- `tmp_generated_schema`: task begins with agent-generated schema draft plus verification.

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
- `latency_reduction_ms = baseline_wall_time_ms - tmp_wall_time_ms`
- `invalid_operation_delta = baseline_invalid_attempts - tmp_invalid_attempts`

## Initial Acceptance Targets

These are proposed targets for early validation:

- Reduce tool calls by at least 50% for common CLI resolution tasks.
- Reduce token usage by at least 30% for tasks requiring command discovery.
- Keep invalid operation attempts at zero for verified TMP mappings.
- Achieve completion precision at 5 of at least 90% for dynamic CLI completions.
- Fail closed for 100% of missing-schema tasks.

## Evidence to Capture

For each benchmark run, save:

- Agent transcript or terminal log.
- TMP command outputs.
- Generated `.tmp/context.md` and `.tmp/commands.json` when relevant.
- Token/tool-call accounting if available from the agent.
- Final operation invoked.
- Pass/fail outcome.

## Open Questions

- Which agents should be included in the first comparison?
- Which benchmark fixtures should be maintained in this repository?
- Should token counts be collected through agent APIs, logs, or manual estimates?
- Should TMP expose a benchmark command, or should benchmarks stay as external fixtures initially?
- What risk classification vocabulary should be stable before SQL/API benchmarks?

