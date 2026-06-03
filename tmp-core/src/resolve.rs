use crate::context::Context;
use crate::schema::{Parameter, Schema};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterFill {
    pub name: String,
    pub value: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveResult {
    pub command: String,
    pub tool: String,
    pub explanation: String,
    pub confidence: String,
    #[serde(alias = "tokens_filled")]
    pub parameters_filled: Vec<ParameterFill>,
}

pub fn load_all_schemas(config_path: Option<&Path>) -> Result<Vec<Schema>, String> {
    let config_file_path = match config_path {
        Some(p) => p.to_path_buf(),
        None => crate::config::default_config_path()
            .ok_or_else(|| "Could not determine default config directory".to_string())?,
    };
    let config_dir = config_file_path
        .parent()
        .ok_or_else(|| "Invalid configuration path".to_string())?;
    let schemas_dir = config_dir.join("schemas");
    let mut schemas = Vec::new();
    if schemas_dir.exists() {
        for entry in std::fs::read_dir(schemas_dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
                let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
                if let Ok(schema) = Schema::from_json(&content) {
                    schemas.push(schema);
                }
            }
        }
    }
    Ok(schemas)
}

fn escape_token_value(val: &str) -> String {
    let mut escaped = String::new();
    for c in val.chars() {
        if ";&|<>`$\\()\"'*?[]!{}\n\r".contains(c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

fn construct_final_command(
    template: &str,
    parameters: &[Parameter],
    filled: &[ParameterFill],
) -> String {
    let mut cmd = template.to_string();

    // 1. Replace placeholders first
    for tf in filled {
        let escaped_value = escape_token_value(&tf.value);
        let placeholder1 = format!("<{}>", tf.name);
        let placeholder2 = format!("{{{}}}", tf.name);
        if cmd.contains(&placeholder1) {
            cmd = cmd.replace(&placeholder1, &escaped_value);
        } else if cmd.contains(&placeholder2) {
            cmd = cmd.replace(&placeholder2, &escaped_value);
        }
    }

    // 2. Append flags for filled tokens that don't have placeholders in the template
    for tf in filled {
        let placeholder1 = format!("<{}>", tf.name);
        let placeholder2 = format!("{{{}}}", tf.name);
        if !template.contains(&placeholder1) && !template.contains(&placeholder2) {
            // Find the parameter definition to get its flag
            if let Some(param) = parameters.iter().find(|p| p.name == tf.name) {
                if let Some(ref flag) = param.flag {
                    if !cmd.contains(flag) {
                        let escaped_value = escape_token_value(&tf.value);
                        cmd.push_str(&format!(" {} {}", flag, escaped_value));
                    }
                }
            }
        }
    }

    // 3. Remove optional placeholders that were not filled
    for param in parameters {
        if !filled.iter().any(|tf| tf.name == param.name) {
            let placeholder1 = format!("<{}>", param.name);
            let placeholder2 = format!("{{{}}}", param.name);

            // Remove `--flag <placeholder>` or just `<placeholder>`
            if let Some(ref flag) = param.flag {
                let pattern1 = format!("{} {}", flag, placeholder1);
                let pattern2 = format!("{} {}", flag, placeholder2);
                if cmd.contains(&pattern1) {
                    cmd = cmd.replace(&pattern1, "");
                } else if cmd.contains(&pattern2) {
                    cmd = cmd.replace(&pattern2, "");
                }
            }

            cmd = cmd.replace(&placeholder1, "");
            cmd = cmd.replace(&placeholder2, "");
        }
    }

    // Clean up double spaces
    while cmd.contains("  ") {
        cmd = cmd.replace("  ", " ");
    }

    cmd.trim().to_string()
}

pub fn heuristic_resolve(
    query: &str,
    schemas: &[Schema],
    context: &Context,
    tool_filter: Option<&str>,
) -> Option<ResolveResult> {
    let query_lower = query.to_lowercase();
    let query_words: Vec<&str> = query_lower.split_whitespace().collect();

    let mut best_op: Option<(&crate::schema::Operation, &Schema)> = None;
    let mut best_score = 0;

    for schema in schemas {
        if let Some(tool) = tool_filter {
            if schema.meta.tool.to_lowercase() != tool.to_lowercase() {
                continue;
            }
        }

        // Filter schema relevance like compile.rs does
        if let Some(ref binary) = schema.meta.requires_binary {
            if !crate::utils::is_binary_available(binary) {
                continue;
            }
        }
        if let Some(ref file) = schema.meta.requires_file {
            let root = context
                .project_root
                .as_deref()
                .map(Path::new)
                .unwrap_or_else(|| Path::new(&context.cwd));
            if !root.join(file).exists() {
                continue;
            }
        }
        if let Some(ref kind) = schema.meta.requires_file_kind {
            if context.file_kind != *kind {
                continue;
            }
        }

        for op in &schema.operations {
            let mut score = 0;
            let cmd_lower = op.command.to_lowercase();
            let desc_lower = op.description.to_lowercase();

            // Match words in query with command words
            for &word in &query_words {
                if cmd_lower.contains(word) {
                    score += 15;
                }
                if desc_lower.contains(word) {
                    score += 5;
                }
                for kw in &schema.meta.keywords {
                    if kw.to_lowercase().contains(word) {
                        score += 8;
                    }
                }
                if op.group.to_lowercase().contains(word) {
                    score += 5;
                }
            }

            if score > best_score {
                best_score = score;
                best_op = Some((op, schema));
            }
        }
    }

    if let Some((op, schema)) = best_op {
        if best_score > 0 {
            let mut parameters_filled = Vec::new();

            for param in &op.parameters {
                let resolved_values = match &param.data_source {
                    Some(ds) => {
                        crate::resolver::DataResolver::resolve(ds, context).unwrap_or_default()
                    }
                    None => param.values.clone().unwrap_or_default(),
                };

                let mut filled_val = None;
                let mut source = "";

                // 1. Try to find a matching resolved value in the query
                for val in &resolved_values {
                    if query_lower.contains(&val.to_lowercase()) {
                        filled_val = Some(val.clone());
                        source = "Heuristic match from query";
                        break;
                    }
                }

                // 2. Try default value
                if filled_val.is_none() {
                    if let Some(ref def) = param.default {
                        filled_val = Some(def.clone());
                        source = "Default value";
                    }
                }

                // 3. If required, and still none, try to guess
                if filled_val.is_none() && param.required {
                    if let Some(&last_word) = query_words.last() {
                        if last_word != "test"
                            && last_word != "run"
                            && last_word != "build"
                            && last_word != "check"
                        {
                            filled_val = Some(last_word.to_string());
                            source = "Heuristic positional guess";
                        }
                    }
                }

                if let Some(val) = filled_val {
                    parameters_filled.push(ParameterFill {
                        name: param.name.clone(),
                        value: val,
                        source: source.to_string(),
                    });
                }
            }

            let final_cmd =
                construct_final_command(&op.command, &op.parameters, &parameters_filled);

            return Some(ResolveResult {
                command: final_cmd,
                tool: schema.meta.tool.clone(),
                explanation: op.description.clone(),
                confidence: if best_score > 20 {
                    "high".to_string()
                } else {
                    "medium".to_string()
                },
                parameters_filled,
            });
        }
    }

    None
}

pub fn resolve(
    query: &str,
    context: &Context,
    tool_filter: Option<&str>,
    config_path: Option<&Path>,
) -> Result<ResolveResult, String> {
    let schemas = load_all_schemas(config_path)?;

    if let Some(result) = heuristic_resolve(query, &schemas, context, tool_filter) {
        return Ok(result);
    }

    Err(format!(
        "Failed to resolve query '{}' to any command.",
        query
    ))
}

#[cfg(test)]
#[path = "resolve_tests.rs"]
mod tests;
