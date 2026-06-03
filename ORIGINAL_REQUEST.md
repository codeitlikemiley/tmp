# Original User Request

## Initial Request — 2026-06-03T16:19:32+08:00

An autonomous AI agent built in Rust using the `antigravity-sdk-rust` framework that adopts a dynamic workflow approach (orchestrating tasks via generated Python scripts) to automatically generate, verify, and register TMP command schemas and resolvers.

Working directory: /Volumes/goldcoders/tmp/crates/tmp-agent
Integrity mode: development

## Requirements

### R1. Crate Setup & Agent Harness
- Set up a new Rust crate member `tmp-agent` inside the `tmp` cargo workspace.
- Implement an agent initialization harness that configures `antigravity-sdk-rust` with the `GEMINI_API_KEY` and sets up the standard connection to `localharness`.

### R2. API Loopback Server (Rust Side)
- Implement a local REST HTTP server (using `axum` or another lightweight web framework) started by the Rust Agent.
- The server port should be dynamically allocated (or read from environment variable `TMP_AGENT_PORT`) and passed to the Python script in its environment.
- The server must expose the following REST endpoints:
  - `POST /execute` - Run a command on the host (with arguments and CWD) and return stdout/stderr.
  - `POST /read_file` - Read file content in the workspace (JSON payload with path).
  - `POST /write_file` - Write file content in the workspace.
  - `POST /subagent` - Spawn a background `antigravity-sdk-rust` subagent task with a specific prompt. Returns a unique `subagent_id` immediately.
  - `GET /subagent/:id` - Poll execution status (Running, Success, Failure) and retrieve the final generated output of the subagent.
  - `POST /log` - Log step-by-step progress from the script back to the main agent's console logs.
  - `POST /db/tables` - Given connection options (sqlite path or postgres URL) in payload, return list of tables.
  - `POST /db/columns` - Given table name and connection options, return columns and types.
  - `POST /db/query` - Given raw SQL SELECT statement and connection options, execute and return rows.

### R3. Dynamic Workflow Engine (Python Scripter)
- The main agent uses the `GEMINI_API_KEY` to plan schema/resolver generation by writing a Python script (`workflow.py`).
- The Python script must coordinate tasks concurrently (e.g., fetching DB schema and generating sub-parts of the JSON schema in parallel via parallel threads or processes) using the loopback REST HTTP server.
- The Rust Agent executes this script using `python3 workflow.py`.

### R4. Schema & Resolver Integration
- Once the Python script completes successfully, the final generated command schemas are validated and saved under the local TMP configuration directory (`schemas/`).
- Resolvers (dynamic autocomplete binders) are fully compiled and ready to be used by the main `tmp` CLI tool.

## Verification Plan

### Automated Tests
- Write integration tests inside the `tmp-agent` crate:
  1. Spin up the Rust Agent and its local loopback HTTP server.
  2. Mock/run a sample Python script that interacts with `/db/tables` (on a temporary SQLite database) and `/execute`.
  3. Verify that the Python script correctly retrieves the database structure, uses `/subagent` to generate parts of the schema, writes the final schema file, and completes successfully.
  4. Run `cargo test -p tmp-agent` to verify all tests.

## Acceptance Criteria

### Compilation & Workspace Registration
- [ ] Crate `tmp-agent` compiles without errors and clippy warnings.
- [ ] Crate is registered as a member in the workspace `Cargo.toml`.

### API Loopback Server Endpoints
- [ ] HTTP server starts on an allocated port and exposes all endpoints (`/execute`, `/read_file`, `/write_file`, `/subagent`, `/subagent/:id`, `/log`, `/db/tables`, `/db/columns`, `/db/query`).
- [ ] Native drivers (`rusqlite`, `tokio-postgres` or `postgres`) are used to execute database helpers.

### Dynamic Workflow Execution
- [ ] The agent writes a `workflow.py` script containing Python orchestration logic.
- [ ] The script executes successfully via `python3` and communicates with the Rust HTTP API server to spawn subagents and query data.
- [ ] The final output schema JSON matches the TMP schema format and is saved inside the local schemas folder.
