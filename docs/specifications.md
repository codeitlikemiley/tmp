# TMP Schema Specifications

This document defines the schema specification formats, dynamic data source resolvers, AI extraction protocols, and version control mechanisms used by the Terminal Meta Protocol.

---

## 1. Schema JSON Specification

A schema file is a JSON document representing a specific command-line tool (e.g., `cargo.json`, `git.json`). The schema is structured into two main parts: `meta` and `commands`.

### Example Schema (abbreviated)
```json
{
  "meta": {
    "tool": "cargo",
    "version": "0.1.0",
    "author": "Core Devs",
    "description": "Cargo build system schema",
    "keywords": ["rust", "build", "test"],
    "requires_binary": "cargo"
  },
  "commands": [
    {
      "command": "cargo build",
      "description": "Compile the current package",
      "group": "build",
      "verified": true,
      "tokens": [
        {
          "name": "--package",
          "description": "Package to build",
          "token_type": "Enum",
          "required": false,
          "aliases": ["-p"],
          "data_source": {
            "resolver": "cargo:packages"
          }
        },
        {
          "name": "--release",
          "description": "Build artifacts in release mode",
          "token_type": "Boolean",
          "required": false,
          "aliases": ["-r"]
        }
      ]
    }
  ]
}
```

### Schema Properties

#### `meta`
* **`tool`** (String, required): The binary name of the tool (must only contain alphanumeric characters, dashes, and underscores).
* **`version`** (String, required): Semantic version of the schema.
* **`author`** (String): Author information.
* **`description`**: A high-level description of what the tool does.
* **`keywords`** (Array of Strings): Custom keywords to help the LLM match natural language queries to this tool.
* **`requires_binary`** (String): Expected system binary. Skip compilation if not installed on the system.

#### `commands[]`
* **`command`** (String, required): The command trigger string (e.g., `cargo test`).
* **`description`** (String, required): Explanation of the command.
* **`group`** (String): Tab categorization index.
* **`verified`** (Boolean): Indicates whether the command definition is curated/approved.
* **`tokens[]`** (Array): Parameters accepted by the command.

#### `tokens[]`
* **`name`** (String, required): Command flag parameter (e.g., `--target`).
* **`description`** (String, required): Explanation of parameter role.
* **`token_type`** (Enum, required): Must be one of `String`, `Boolean`, `Enum`, `File`, `Number`.
* **`required`** (Boolean): True if parameter is required.
* **`default_value`** (String): Default choice.
* **`valid_values`** (Array of Strings): Hardcoded values list.
* **`aliases`** (Array of Strings): Short-hand alternative flags (e.g., `["-p"]`).
* **`data_source`** (Object): Specifies how to populate the dropdown options list dynamically.

---

## 2. Data Source Resolvers

A `data_source` populates lists dynamically based on your workspace state. It supports two modes:
1. **`command`**: A shell command string executed within the project directory.
2. **`resolver`**: A built-in resolver key executed natively by the Rust core.

### Whitelisted Built-in Resolvers
* **`cargo:packages`**: Lists package names in the Cargo workspace (parsed from cargo metadata).
* **`cargo:bins`**: Lists defined binaries.
* **`cargo:examples`**: Lists code examples.
* **`cargo:features`**: Lists features defined in Cargo.toml.
* **`cargo:profiles`**: Lists build profiles.
* **`cargo:tests`**: Lists integration test targets.
* **`cargo:benches`**: Lists benchmark targets.
* **`git:branches`**: Lists local git branch names.
* **`git:remotes`**: Lists configured git remote names.
* **`npm:scripts`**: Lists script names from package.json.

### Security Whitelist
* Built-in resolvers are evaluated internally without running external shells, ensuring safety.
* Shell-based data sources (`command`) run inside the workspace root. Inputs are sanitized, and commands are parsed to prevent execution hijackings.

---

## 3. AI Schema Generation Protocol

When generating a schema via `tmp generate <tool>`, the system performs the following sequence:

```
[CLI Tool] ──(--help)──► [Extract menus] ──► [LLM Analysis] ──► [Filter & Save]
```

1. **Introspection**: Runs `<tool> --help` and recursively inspects `<tool> <subcommand> --help` up to three levels deep.
2. **LLM Formatting**: Sends the concatenated raw menus output to the configured LLM engine.
3. **Structured Extraction**: The LLM parses the command structures, arguments, types, and flags, returning a validated JSON schema.
4. **Validation Check**: The generated schema is audited:
   - Verifies all required fields exist.
   - Validates flags against the syntax regex: `^-{1,2}[a-zA-Z][a-zA-Z0-9-]*$`.
   - Prevents path traversals by ensuring the tool name matches `^[a-zA-Z0-9_-]+$`.
5. **Local Save**: Saves the validated output under `~/.config/tmp/schemas/<tool>.json`.

---

## 4. Schema Versioning & History

To prevent loss of local edits, the system implements automatic schema backups:

* **Automatic Rotation**: Running `tmp generate <tool> --force` moves the existing schema to `~/.config/tmp/schemas/versions/<tool>/v1.json`, `v2.json`, etc.
* **Rollbacks**: Users can inspect changes using `tmp generate <tool> --history` and rollback to a previous version with `tmp generate <tool> --rollback <version_number>`.
* **Structural Diff**: The rollback and regeneration workflows compare the abstract syntax tree of the old and new schema structures, displaying a color-coded command diff showing added, removed, or changed parameters.
