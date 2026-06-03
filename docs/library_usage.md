# Using `tmp-core` as a Library

The `tmp-core` crate is a self-contained, documented library that can be integrated as a dependency in other Rust applications (such as terminal shells, editors, or custom AI coding agents).

---

## Adding the Dependency

Add `tmp-core` to your `Cargo.toml` dependencies. Since it is located in the workspace, you can reference it via path:

```toml
[dependencies]
tmp-core = { path = "../tmp-core" }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

---

## Primary Public Types

`tmp-core` exposes clean public models for schema parsing, context inspection, and AI dispatch:

| Type | Purpose |
| :--- | :--- |
| **`Schema`** | Represens a CLI tool schema. Implements `Serialize` and `Deserialize`. |
| **`Context`** | Inspects directories to determine build system types, script engines, and paths. |
| **`CompileOutput`** | Compiles project configuration schemas into context representations. |
| **`LlmDispatcher`** | Coordinates LLM calls, key rotations, fallback providers, and prompt formatting. |
| **`ResolveResult`** | Holds the grounded output commands parsed from LLMs. |
| **`Workflow`** | Represents sequential multi-step commands parameterization scripts. |

---

## Code Examples

### 1. Parsing and Serializing a Schema
You can load and parse schema specifications programmatically:

```rust
use tmp_core::schema::Schema;
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raw_json = fs::read_to_string("~/.config/tmp/schemas/cargo.json")?;
    
    // Deserialize
    let schema: Schema = serde_json::from_str(&raw_json)?;
    println!("Loaded schema for tool: {}", schema.meta.tool);
    
    for cmd in &schema.commands {
        println!(" - Command: {}", cmd.command);
    }
    
    // Serialize
    let serialized = serde_json::to_string_pretty(&schema)?;
    
    Ok(())
}
```

---

### 2. Detecting Workspace Context
Detect project root build system types and script targets:

```rust
use tmp_core::context::{Context, ProjectType};
use std::path::Path;

fn main() {
    let current_dir = Path::new(".");
    let context = Context::detect(current_dir, None).unwrap();
    
    match context.project_type {
        ProjectType::Cargo => println!("Detected Cargo workspace at {:?}", context.root_dir),
        ProjectType::Npm => println!("Detected npm workspace at {:?}", context.root_dir),
        ProjectType::Go => println!("Detected Go project at {:?}", context.root_dir),
        ProjectType::Python => println!("Detected Python project at {:?}", context.root_dir),
        ProjectType::None => println!("No standard workspace detected."),
    }
}
```

---

### 3. Compiling Context to Disk
Run the compilation pipeline programmatically to generate context artifacts:

```rust
use tmp_core::compile::Compiler;
use tmp_core::config::Config;
use std::path::Path;

fn compile_project() -> Result<(), Box<dyn std::error::Error>> {
    let workspace_dir = Path::new("/my/project");
    let config = Config::load(None)?; // Load default configuration
    
    let compiler = Compiler::new(workspace_dir, config);
    let output = compiler.compile()?;
    
    // Write output to the workspace directory under .tmp/
    output.write_to_disk(workspace_dir)?;
    
    println!("Compiled context.md and commands.json written to .tmp/");
    Ok(())
}
```

---

### 4. Grounding a Query via LLM
Programmatically dispatch a natural language request to match against schemas using the configured LLM engines:

```rust
use tmp_core::config::Config;
use tmp_core::llm::LlmDispatcher;
use tmp_core::resolve::Resolver;
use std::path::Path;

fn resolve_query(query: &str) -> Result<String, Box<dyn std::error::Error>> {
    let config = Config::load(None)?;
    let dispatcher = LlmDispatcher::new(config.llm.clone())?;
    
    let project_dir = Path::new(".");
    let resolver = Resolver::new(project_dir, dispatcher);
    
    let result = resolver.resolve(query, None)?;
    
    println!("Resolved command: {}", result.command);
    println!("Confidence: {}", result.confidence);
    
    Ok(result.command)
}
```
