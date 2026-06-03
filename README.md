# Terminal Meta Protocol (tmp)

**Terminal Meta Protocol (tmp)** makes CLI command schemas available to AI coding agents. It compiles context details (detected project type, dynamic resolvers) and outputs them to `.tmp/context.md` (concise instructions) and `.tmp/commands.json` (full command bank) for LLMs to read.

```
[Agent reads .tmp/context.md]
              │
              ▼
[Agent runs `tmp resolve "query"`] ──► [LLM matches queries against commands.json]
              │
              ▼
[Agent runs `tmp run <args>`] ─────► [Executes the resolved command safely]
```

## Crates

* **`tmp-core`**: The core library implementing config management, schema registry clients, context detection, and compilation logic.
* **`tmp`**: The CLI binary crate.

---

## Documentation Guides

Please refer to the following step-by-step guides under the [docs/](docs/) directory:

1. 🚀 **[Getting Started](docs/getting_started.md)**: Installing the CLI, setting up `config.toml`, compiling project contexts, and resolving queries.
2. 📦 **[Library Usage](docs/library_usage.md)**: Importing the `tmp-core` crate as a dependency in other Rust applications and utilizing public APIs.
3. 📋 **[Schema Specifications](docs/specifications.md)**: Detailed formats for schema files, built-in resolvers, whitelists, and versioning.
4. ⚙️ **[Workflows & Agent Integrations](docs/workflows.md)**: Parameterized multi-step workflows, registry searching, and AI agent bridge setup (`tmp init-agent`).
