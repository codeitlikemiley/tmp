use crate::compile::CompileOutput;

#[derive(Debug, Clone)]
pub struct CompletionCandidate {
    pub value: String,
    pub description: String,
    pub source: String, // which operation/parameter this came from
}

/// Generate completion candidates for a partial input
pub fn complete(input: &str, compile_output: &CompileOutput) -> Vec<CompletionCandidate> {
    let mut candidates = Vec::new();
    let parts: Vec<&str> = input.split_whitespace().collect();

    if parts.is_empty() {
        // Complete top-level commands
        for op in &compile_output.operations {
            let first_word = op.command.split_whitespace().next().unwrap_or(&op.command);
            if !candidates
                .iter()
                .any(|c: &CompletionCandidate| c.value == first_word)
            {
                candidates.push(CompletionCandidate {
                    value: first_word.to_string(),
                    description: op.description.clone(),
                    source: format!("{}.{}", op.group, first_word),
                });
            }
        }
        return candidates;
    }

    // Match operations that start with the input
    for op in &compile_output.operations {
        if op.command.starts_with(input) || input.starts_with(&op.command) {
            // If we're past the command, complete parameters
            if input.len() >= op.command.len() {
                let remaining = input[op.command.len()..].trim();
                for param in &op.parameters {
                    // If parameter has resolved values, offer them
                    if !param.values.is_empty() {
                        for val in &param.values {
                            if remaining.is_empty() || val.starts_with(remaining) {
                                candidates.push(CompletionCandidate {
                                    value: val.clone(),
                                    description: format!("{} ({})", param.description, param.name),
                                    source: format!(
                                        "{}.{}.{}",
                                        op.group,
                                        op.command.split_whitespace().last().unwrap_or_default(),
                                        param.name
                                    ),
                                });
                            }
                        }
                    }
                    // For flags, suggest the flag
                    if let Some(ref flag) = param.flag {
                        if remaining.is_empty() || flag.starts_with(remaining) {
                            candidates.push(CompletionCandidate {
                                value: flag.clone(),
                                description: format!(
                                    "{} ({})",
                                    param.description,
                                    if param.required {
                                        "required"
                                    } else {
                                        "optional"
                                    }
                                ),
                                source: format!("{}.{}", op.group, param.name),
                            });
                        }
                    }
                }
            } else {
                // Complete the command itself
                candidates.push(CompletionCandidate {
                    value: op.command.clone(),
                    description: op.description.clone(),
                    source: format!(
                        "{}.{}",
                        op.group,
                        op.command.split_whitespace().last().unwrap_or_default()
                    ),
                });
            }
        }
    }

    candidates
}

/// Generate zsh completion script for tmp CLI
pub fn generate_zsh_completions() -> String {
    r#"#compdef tmp

_tmp() {
    local -a commands
    commands=(
        'init:Download curated schemas and set up configuration'
        'schema:Manage schemas'
        'registry:Query and install schemas from the online registry'
        'compile:Compile workspace context and schemas'
        'generate:Generate schema for a tool'
        'resolve:Resolve NL query to command'
        'run:Run code contextually'
        'workflow:Manage and run workflows'
        'verify:Verify a schema'
        'output:Show output from previous runs'
        'benchmark:Run a benchmark'
        'completions:Print a shell completion script'
        'complete:Print context-aware completion candidates'
        'init-agent:Initialize instruction files for external coding agents'
    )

    _arguments -C \
        '-c[Custom configuration path]:config path:_files' \
        '--config[Custom configuration path]:config path:_files' \
        '1:command:->cmd' \
        '*::arg:->args'

    case $state in
        cmd)
            _describe 'command' commands
            ;;
    esac
}

_tmp "$@"
"#
    .to_string()
}

/// Generate bash completion script for tmp CLI
pub fn generate_bash_completions() -> String {
    r#"_tmp_completions() {
    local cur prev commands
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"
    commands="init schema registry compile generate resolve run workflow verify output benchmark completions complete init-agent"

    if [ $COMP_CWORD -eq 1 ]; then
        COMPREPLY=( $(compgen -W "$commands" -- "$cur") )
    fi
}
complete -F _tmp_completions tmp
"#
    .to_string()
}

/// Generate fish completion script for tmp CLI
pub fn generate_fish_completions() -> String {
    r#"complete -c tmp -n "__fish_use_subcommand" -a init -d "Download curated schemas and set up configuration"
complete -c tmp -n "__fish_use_subcommand" -a schema -d "Manage schemas"
complete -c tmp -n "__fish_use_subcommand" -a registry -d "Query and install schemas"
complete -c tmp -n "__fish_use_subcommand" -a compile -d "Compile workspace context and schemas"
complete -c tmp -n "__fish_use_subcommand" -a generate -d "Generate schema for a tool"
complete -c tmp -n "__fish_use_subcommand" -a resolve -d "Resolve NL query to command"
complete -c tmp -n "__fish_use_subcommand" -a run -d "Run code contextually"
complete -c tmp -n "__fish_use_subcommand" -a workflow -d "Manage and run workflows"
complete -c tmp -n "__fish_use_subcommand" -a verify -d "Verify a schema"
complete -c tmp -n "__fish_use_subcommand" -a output -d "Show output from previous runs"
complete -c tmp -n "__fish_use_subcommand" -a benchmark -d "Run a benchmark"
complete -c tmp -n "__fish_use_subcommand" -a completions -d "Print a shell completion script"
complete -c tmp -n "__fish_use_subcommand" -a complete -d "Print context-aware completion candidates"
complete -c tmp -n "__fish_use_subcommand" -a init-agent -d "Initialize instruction files for agents"
"#
    .to_string()
}

#[cfg(test)]
#[path = "completion_tests.rs"]
mod tests;
