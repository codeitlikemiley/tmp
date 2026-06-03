# Getting Started with TMP CLI

The **Terminal Meta Protocol (tmp)** is a file-based CLI utility that compiles development project context and matches natural language commands to exact, pre-validated CLI tools.

---

## Installation

### Prerequisites
- Rust and Cargo installed (stable release channel).

### Build & Install CLI
Clone the repository and install the binary globally using `cargo`:

```bash
git clone https://github.com/codeitlikemiley/tmp.git
cd tmp
cargo install --path tmp
```

Verify that the CLI is installed and check the subcommands:

```bash
tmp --help
```

---

## Quick Start Configuration

To initialize the configurations and set up default schemas, run:

```bash
tmp init
```

This creates the configuration directory at `~/.config/tmp/` (or follows the path specified by the `TMP_CONFIG_DIR` environment variable) and populates:
1. **`config.toml`**: The main options configuration.
2. **`schemas/`**: Subfolder containing your installed tool specifications.

### Customizing `config.toml`
Edit `~/.config/tmp/config.toml` to define your LLM provider keys and fallback rotation strategies:

```toml
[llm]
strategy = "fallback" # Option: "fallback" or "round_robin"

[[llm.providers]]
provider = "gemini"
keys = ["YOUR_GEMINI_API_KEY_1", "YOUR_GEMINI_API_KEY_2"]

[[llm.providers]]
provider = "openai"
keys = ["YOUR_OPENAI_API_KEY"]

[[llm.providers]]
provider = "ollama"
model = "llama3"
base_url = "http://localhost:11434"
```

*Note: You can also specify keys via environment variables (e.g., `GEMINI_API_KEY`, `OPENAI_API_KEY`), which take precedence over the file config.*

---

## Core CLI Usage Flow

```
[ Your Workspace ] ──(tmp compile)──► [ .tmp/context.md & commands.json ]
        │
        ├──(tmp resolve "run tests")──► [ grounded command ]
        │
        └──(tmp run)──────────────────► [ execute command ]
```

### 1. Compile Project Context
Run `compile` at your project root directory. This scans your workspace, detects the project type (e.g., Cargo, npm, Go, Python), resolves all dynamic data sources, and generates files in the local `.tmp/` directory:

```bash
tmp compile
```

#### Compiled Output:
* **`.tmp/context.md`**: A short, readable summary detailing the project type, layout, and a pre-resolved list of common commands. AI Coding Agents read this file to understand your project.
* **`.tmp/commands.json`**: A complete index of all tool schemas with parameters pre-populated with live data from resolvers.

#### Watch Daemon Mode:
To automatically re-compile the context whenever your source files change, run:

```bash
tmp compile --watch
```

---

### 2. Natural Language Query Resolution
You can resolve natural language prompts into executable CLI commands matching your loaded project schemas:

```bash
tmp resolve "run the unit tests for the tmp-core crate"
```

Output (JSON):
```json
{
  "command": "cargo test -p tmp-core --lib",
  "tool": "cargo",
  "confidence": 0.98,
  "explanation": "Resolved the unit testing command for the tmp-core package."
}
```

---

### 3. Executing Commands Contextually
To execute the best-matched command for a specific file or line, run:

```bash
tmp run tmp-core/src/resolver.rs:12
```

#### Dry-run mode:
To view the command without executing it, run:
```bash
tmp run tmp-core/src/resolver.rs:12 --dry-run
```

---

## Interactive Schema Verification TUI

If you generate a new CLI schema using the AI engine, you can review, edit, and test it interactively using the built-in Ratatui TUI:

```bash
tmp generate cargo --verify
```

### Keyboard Shortcuts in the TUI:
* `Tab` / `Shift-Tab`: Navigate between lists and details.
* `Enter`: Edit selected token parameters.
* `v`: Toggle verified flag for a command.
* `t`: Run the selected data source resolver live and inspect the output.
* `s`: Save changes back to the JSON file.
* `q`: Quit.
