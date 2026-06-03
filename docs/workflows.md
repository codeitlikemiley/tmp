# Workflows & Integrations

This guide details the multi-step workflows engine, registry client commands, and how to bridge local context compiling to AI coding agents.

---

## 1. Multi-Step Workflows

Workflows are parameterized JSON or YAML files allowing sequential execution of shell commands and schema actions. They are loaded from `~/.config/tmp/workflows/` (global) or `.tmp/workflows/` (local to project).

### Example Workflow JSON
```json
{
  "name": "release",
  "description": "Run tests and publish schema",
  "steps": [
    {
      "name": "Unit Testing",
      "command": "cargo test --workspace",
      "timeout_seconds": 60
    },
    {
      "name": "Registry Check",
      "command": "tmp schema share cargo",
      "timeout_seconds": 15
    }
  ]
}
```

### Workflow Commands
* **List Workflows**: Displays all globally and locally registered workflows.
  ```bash
  tmp workflow list
  ```
* **Add Workflow**: Creates a workflow profile from a template path.
  ```bash
  tmp workflow add release --from ./my-release-workflow.json
  ```
* **Run Workflow**: Executes the steps in sequence. If a step fails (non-zero exit code or timeout), execution halts immediately.
  ```bash
  tmp workflow run release
  ```

---

## 2. Schema Registry Client

TMP provides native support for connecting to a shared GitHub repository index (by default, `codeitlikemiley/tmp-registry`).

* **Search Registry**: Query schemas available in the remote index.
  ```bash
  tmp registry search cargo
  ```
* **Install Schema**: Downloads and registers the specified tool schema.
  ```bash
  tmp registry install cargo
  ```
* **Publish Schema**: Generates the package distribution payload to submit to the repository registry.
  ```bash
  tmp registry publish cargo
  ```

---

## 3. AI Agent Bridge (`tmp init-agent`)

The `tmp init-agent` command configures instructions and rules files for various AI coding assistants, letting them read the `.tmp/context.md` file and utilize the CLI grounding features natively.

```bash
tmp init-agent all
```

The system generates/merges configuration rule segments for the following targets:

| Target Agent | File Affected | Instructions Injected |
| :--- | :--- | :--- |
| **Claude** | `CLAUDE.md` or `.claude/rules/tmp.md` | Tells Claude Code to read `.tmp/context.md` for project layout and call `tmp resolve` for build targets. |
| **Cursor** | `.cursor/rules/tmp.mdc` | Creates rules with frontmatter matching workspace queries. |
| **Antigravity / Codex** | `AGENTS.md` | Injects instructions into the code map instructions directory. |
| **Copilot** | `.github/copilot-instructions.md` | Configures system prompts for Copilot chat. |
| **Windsurf** | `.windsurfrules` | Injects project-specific context mappings. |

### Idempotent Merging Behavior
If a configuration file (like `CLAUDE.md` or `AGENTS.md`) already exists, `tmp init-agent` **will not overwrite** your custom content. Instead, it reads the file, identifies whether a TMP configuration block exists, and appends or updates the rule snippet at the end of the file safely.
