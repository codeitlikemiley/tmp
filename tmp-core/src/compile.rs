use crate::context::Context;
use crate::resolver::DataResolver;
use crate::schema::{ParameterType, Schema};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileOutput {
    pub context: Context,
    #[serde(alias = "commands")]
    pub operations: Vec<ResolvedOperation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedOperation {
    pub command: String,
    pub description: String,
    pub group: String,
    pub verified: bool,
    #[serde(alias = "tokens")]
    pub parameters: Vec<ResolvedParameter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedParameter {
    pub name: String,
    pub description: String,
    pub required: bool,
    #[serde(rename = "type")]
    pub parameter_type: ParameterType,
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

        let mut resolved_operations = Vec::new();

        if schemas_dir.exists() {
            for entry in fs::read_dir(schemas_dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let path = entry.path();
                if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
                    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
                    if let Ok(schema) = Schema::from_json(&content) {
                        if Self::is_schema_relevant(&schema, context) {
                            for op in schema.operations {
                                let mut resolved_parameters = Vec::new();
                                for param in op.parameters {
                                    let resolved_values = match &param.data_source {
                                        Some(ds) => match DataResolver::resolve(ds, context) {
                                            Ok(vals) => vals,
                                            Err(err) => {
                                                eprintln!("Warning: Failed to resolve token data for '{}': {}", param.name, err);
                                                Vec::new()
                                            }
                                        },
                                        None => param.values.clone().unwrap_or_default(),
                                    };
                                    resolved_parameters.push(ResolvedParameter {
                                        name: param.name,
                                        description: param.description,
                                        required: param.required,
                                        parameter_type: param.parameter_type,
                                        default: param.default,
                                        flag: param.flag,
                                        values: resolved_values,
                                    });
                                }
                                resolved_operations.push(ResolvedOperation {
                                    command: op.command,
                                    description: op.description,
                                    group: op.group,
                                    verified: op.verified,
                                    parameters: resolved_parameters,
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(CompileOutput {
            context: context.clone(),
            operations: resolved_operations,
        })
    }

    pub fn generate_markdown(output: &CompileOutput) -> String {
        let mut md = String::new();
        md.push_str("# Tool Mapping Protocol (TMP) - Project Context\n\n");

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
        if output.operations.is_empty() {
            md.push_str("No active command schemas found matching project context.\n");
        } else {
            for op in &output.operations {
                md.push_str(&format!("### {} ({})\n", op.command, op.group));
                md.push_str(&format!("- **Description**: {}\n", op.description));
                if !op.parameters.is_empty() {
                    md.push_str("- **Tokens**:\n");
                    for param in &op.parameters {
                        let required_str = if param.required {
                            "required"
                        } else {
                            "optional"
                        };
                        let values_preview = if param.values.is_empty() {
                            "none".to_string()
                        } else if param.values.len() > 5 {
                            format!(
                                "{:?} ... (and {} more)",
                                &param.values[..5],
                                param.values.len() - 5
                            )
                        } else {
                            format!("{:?}", param.values)
                        };
                        md.push_str(&format!(
                            "  - `{}` ({}): {} | Resolved values: {}\n",
                            param.name, required_str, param.description, values_preview
                        ));
                    }
                }
            }
        }

        md.push_str("\n## External Agent Instructions\n");
        md.push_str("1. Use `tmp resolve \"<intent>\"` to ground a natural-language request into a schema-backed shell command.\n");
        md.push_str("2. Run `tmp run` after a successful resolve, or `tmp run <file>` when executing a file contextually.\n");
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
            if !crate::utils::is_binary_available(binary) {
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
}

#[cfg(test)]
#[path = "compile_tests.rs"]
mod tests;
