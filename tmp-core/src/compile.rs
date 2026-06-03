use crate::context::Context;
use crate::resolver::DataResolver;
use crate::schema::{Schema, TokenType};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileOutput {
    pub context: Context,
    pub commands: Vec<ResolvedCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedCommand {
    pub command: String,
    pub description: String,
    pub group: String,
    pub verified: bool,
    pub tokens: Vec<ResolvedToken>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedToken {
    pub name: String,
    pub description: String,
    pub required: bool,
    #[serde(rename = "type")]
    pub token_type: TokenType,
    pub default: Option<String>,
    pub flag: Option<String>,
    pub values: Vec<String>,
}

pub struct Compiler;

impl Compiler {
    pub fn compile(
        _cwd: &Path,
        context: &Context,
        custom_config_path: Option<&str>,
    ) -> Result<CompileOutput, String> {
        let config_file_path = match custom_config_path {
            Some(p) => std::path::PathBuf::from(p),
            None => crate::config::default_config_path()
                .ok_or_else(|| "Could not determine default config directory".to_string())?,
        };
        let config_dir = config_file_path
            .parent()
            .ok_or_else(|| "Invalid configuration path".to_string())?;
        let schemas_dir = config_dir.join("schemas");

        let mut resolved_commands = Vec::new();

        if schemas_dir.exists() {
            for entry in fs::read_dir(schemas_dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let path = entry.path();
                if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
                    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
                    if let Ok(schema) = Schema::from_json(&content) {
                        if Self::is_schema_relevant(&schema, context) {
                            for cmd in schema.commands {
                                let mut resolved_tokens = Vec::new();
                                for token in cmd.tokens {
                                    let resolved_values = match &token.data_source {
                                        Some(ds) => match DataResolver::resolve(ds, context) {
                                            Ok(vals) => vals,
                                            Err(err) => {
                                                eprintln!("Warning: Failed to resolve token data for '{}': {}", token.name, err);
                                                Vec::new()
                                            }
                                        },
                                        None => token.values.clone().unwrap_or_default(),
                                    };
                                    resolved_tokens.push(ResolvedToken {
                                        name: token.name,
                                        description: token.description,
                                        required: token.required,
                                        token_type: token.token_type,
                                        default: token.default,
                                        flag: token.flag,
                                        values: resolved_values,
                                    });
                                }
                                resolved_commands.push(ResolvedCommand {
                                    command: cmd.command,
                                    description: cmd.description,
                                    group: cmd.group,
                                    verified: cmd.verified,
                                    tokens: resolved_tokens,
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(CompileOutput {
            context: context.clone(),
            commands: resolved_commands,
        })
    }

    pub fn generate_markdown(output: &CompileOutput) -> String {
        let mut md = String::new();
        md.push_str("# Terminal Meta Protocol (TMP) - Project Context\n\n");

        md.push_str("## Project Information\n");
        md.push_str(&format!(
            "- **Build System**: {}\n",
            output.context.build_system
        ));
        md.push_str(&format!("- **File Kind**: {}\n", output.context.file_kind));
        if let Some(ref root) = output.context.project_root {
            md.push_str(&format!("- **Project Root**: {}\n", root));
        }
        if let Some(ref engine) = output.context.script_engine {
            md.push_str(&format!("- **Script Engine**: {}\n", engine));
        }
        if let Some(ref target) = output.context.recommended_target {
            md.push_str(&format!("- **Recommended Target**: {}\n", target));
        }
        if let Some(ref pkg) = output.context.package_name {
            md.push_str(&format!("- **Package Name**: {}\n", pkg));
        }

        md.push_str("\n## Project Structure & Caches\n");
        md.push_str(&format!(
            "- **Detected Packages**: {:?}\n",
            output.context.packages
        ));
        md.push_str(&format!(
            "- **Detected Binaries**: {:?}\n",
            output.context.bins
        ));
        md.push_str(&format!(
            "- **Detected Examples**: {:?}\n",
            output.context.examples
        ));
        md.push_str(&format!(
            "- **Detected Tests**: {:?}\n",
            output.context.tests
        ));
        md.push_str(&format!(
            "- **Detected Benches**: {:?}\n",
            output.context.benches
        ));
        md.push_str(&format!(
            "- **Git Branches**: {:?}\n",
            output.context.git_branches
        ));
        md.push_str(&format!(
            "- **Git Remotes**: {:?}\n",
            output.context.git_remotes
        ));
        md.push_str(&format!(
            "- **NPM Scripts**: {:?}\n",
            output.context.npm_scripts
        ));

        md.push_str("\n## Available Commands\n");
        if output.commands.is_empty() {
            md.push_str("No active command schemas found matching project context.\n");
        } else {
            for cmd in &output.commands {
                md.push_str(&format!("### {} ({})\n", cmd.command, cmd.group));
                md.push_str(&format!("- **Description**: {}\n", cmd.description));
                if !cmd.tokens.is_empty() {
                    md.push_str("- **Tokens**:\n");
                    for token in &cmd.tokens {
                        let required_str = if token.required {
                            "required"
                        } else {
                            "optional"
                        };
                        let values_preview = if token.values.is_empty() {
                            "none".to_string()
                        } else if token.values.len() > 5 {
                            format!(
                                "{:?} ... (and {} more)",
                                &token.values[..5],
                                token.values.len() - 5
                            )
                        } else {
                            format!("{:?}", token.values)
                        };
                        md.push_str(&format!(
                            "  - `{}` ({}): {} | Resolved values: {}\n",
                            token.name, required_str, token.description, values_preview
                        ));
                    }
                }
            }
        }

        md.push_str("\n## AI Agent Instructions\n");
        md.push_str("1. Use `tmp resolve \"<intent>\"` to ground your natural language request into a shell command.\n");
        md.push_str("2. Run `tmp run <command>` (or `tmp run` directly in file context) to execute the grounded commands safely.\n");
        md.push_str("3. Keep token context size small by reading `.tmp/context.md` instead of full tool manuals.\n");
        md.push_str(
            "4. If you need to refresh context, run `tmp compile` or check watch outputs.\n",
        );
        md.push_str(
            "5. Utilize detected binaries and targets to construct correct command arguments.\n",
        );
        md.push_str(
            "6. Verify output files match expectations and follow the project guidelines.\n",
        );

        let root_dir = output
            .context
            .project_root
            .as_deref()
            .map(Path::new)
            .unwrap_or_else(|| Path::new(&output.context.cwd));

        if let Ok(entries) = std::fs::read_dir(root_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                        if filename == "CLAUDE.md" || filename == "CHATGPT.md" {
                            md.push_str(&format!("\n### Agent Rules: {}\n", filename));
                            if let Ok(content) = std::fs::read_to_string(&path) {
                                md.push_str(&content);
                                md.push('\n');
                            }
                        }
                    }
                }
            }
        }

        md
    }

    pub fn write_to_disk(cwd: &Path, output: &CompileOutput) -> Result<(), std::io::Error> {
        let root_dir = output
            .context
            .project_root
            .as_deref()
            .map(Path::new)
            .unwrap_or(cwd);
        let tmp_dir = root_dir.join(".tmp");
        fs::create_dir_all(&tmp_dir)?;

        // Write commands.json
        let json_content = serde_json::to_string_pretty(output)?;
        fs::write(tmp_dir.join("commands.json"), json_content)?;

        // Write context.md
        let md_content = Self::generate_markdown(output);
        fs::write(tmp_dir.join("context.md"), md_content)?;

        // Update gitignore
        let gitignore_path = root_dir.join(".gitignore");
        if gitignore_path.exists() {
            let mut content = fs::read_to_string(&gitignore_path)?;
            if !content.contains(".tmp/") {
                let has_newline = content.ends_with('\n');
                let prefix = if has_newline { "" } else { "\n" };
                content.push_str(&format!("{}.tmp/\n", prefix));
                fs::write(&gitignore_path, content)?;
            }
        } else {
            fs::write(&gitignore_path, ".tmp/\n")?;
        }

        Ok(())
    }

    fn is_schema_relevant(schema: &Schema, context: &Context) -> bool {
        if let Some(ref binary) = schema.meta.requires_binary {
            if !Self::is_binary_available(binary) {
                return false;
            }
        }
        if let Some(ref file) = schema.meta.requires_file {
            let root = context
                .project_root
                .as_deref()
                .map(Path::new)
                .unwrap_or_else(|| Path::new(&context.cwd));
            if !root.join(file).exists() {
                return false;
            }
        }
        if let Some(ref kind) = schema.meta.requires_file_kind {
            if context.file_kind != *kind {
                return false;
            }
        }
        true
    }

    fn is_binary_available(binary: &str) -> bool {
        let paths = match std::env::var_os("PATH") {
            Some(val) => std::env::split_paths(&val).collect::<Vec<_>>(),
            None => return false,
        };
        for mut path in paths {
            path.push(binary);
            if path.is_file() {
                return true;
            }
            if cfg!(target_os = "windows") {
                let mut exe_path = path.clone();
                exe_path.set_extension("exe");
                if exe_path.is_file() {
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
#[path = "compile_tests.rs"]
mod tests;
