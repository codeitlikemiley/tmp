use crate::context::Context;
use crate::schema::DataSource;
use command::Command;

pub struct DataResolver;

impl DataResolver {
    pub fn resolve(ds: &DataSource, context: &Context) -> Result<Vec<String>, String> {
        if let Some(ref resolver_name) = ds.resolver {
            if let Some(val) = Self::resolve_builtin(resolver_name, context) {
                return Ok(val);
            } else {
                return Err(format!(
                    "Unknown or unsupported resolver: {}",
                    resolver_name
                ));
            }
        }

        if let Some(ref cmd) = ds.command {
            if let Some(val) = Self::resolve_shell_command(cmd, &ds.parse, &context.cwd) {
                return Ok(val);
            } else {
                return Err(format!("Failed to execute command: {}", cmd));
            }
        }

        Err("DataSource must specify either command or resolver".to_string())
    }

    fn resolve_builtin(resolver: &str, context: &Context) -> Option<Vec<String>> {
        match resolver {
            "cargo:packages" => Some(context.packages.clone()),
            "cargo:bins" => Some(context.bins.clone()),
            "cargo:examples" => Some(context.examples.clone()),
            "cargo:features" => Some(context.features.clone()),
            "cargo:profiles" => Some(context.profiles.clone()),
            "cargo:tests" => Some(context.tests.clone()),
            "cargo:benches" => Some(context.benches.clone()),
            "git:branches" => Some(context.git_branches.clone()),
            "git:remotes" => Some(context.git_remotes.clone()),
            "npm:scripts" => Some(context.npm_scripts.clone()),
            _ => None,
        }
    }

    fn resolve_shell_command(command: &str, parse_mode: &str, cwd: &str) -> Option<Vec<String>> {
        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", command]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", command]);
            c
        };
        cmd.current_dir(cwd);

        let output = cmd.output().ok()?;
        if !output.status.success() {
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let parsed = match parse_mode {
            "words" => stdout
                .split_whitespace()
                .map(|w| w.trim().to_string())
                .filter(|w| !w.is_empty())
                .collect(),
            _ => stdout
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect(),
        };

        Some(parsed)
    }
}

#[cfg(test)]
#[path = "resolver_tests.rs"]
mod tests;
