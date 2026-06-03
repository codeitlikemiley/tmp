use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use tmp_core::compile::Compiler;
use tmp_core::context::Context;

pub fn run(
    cwd: Option<&str>,
    watch: bool,
    custom_config_path: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let resolved_cwd = match cwd {
        Some(p) => PathBuf::from(p),
        None => std::env::current_dir()?,
    };
    let canonical_cwd = fs::canonicalize(&resolved_cwd).unwrap_or(resolved_cwd);

    if watch {
        watch_project(&canonical_cwd, custom_config_path)?;
    } else {
        let context = Context::detect(&canonical_cwd, None, None);
        let output = Compiler::compile(&canonical_cwd, &context, custom_config_path)?;
        Compiler::write_to_disk(&canonical_cwd, &output)?;
        println!("Compilation successful.");
    }

    Ok(())
}

fn watch_project(
    cwd: &Path,
    custom_config_path: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut context = Context::detect(cwd, None, None);
    let output = Compiler::compile(cwd, &context, custom_config_path)?;
    Compiler::write_to_disk(cwd, &output)?;
    println!("Initial compilation successful.");

    let (tx, rx) = channel();
    let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())?;
    watcher.watch(cwd, RecursiveMode::Recursive)?;

    // Watch custom schemas directory if set
    let config_file_path = match custom_config_path {
        Some(p) => Some(PathBuf::from(p)),
        None => tmp_core::config::default_config_path(),
    };
    if let Some(config_file_path) = config_file_path {
        if let Some(config_dir) = config_file_path.parent() {
            let schemas_dir = config_dir.join("schemas");
            if schemas_dir.exists() {
                let _ = watcher.watch(&schemas_dir, RecursiveMode::Recursive);
            }
        }
    }

    println!(
        "Watching for changes in {}... (Press Ctrl+C to exit)",
        cwd.display()
    );

    for res in rx {
        match res {
            Ok(event) => {
                let mut should_recompile = false;
                for path in event.paths {
                    // Check if path contains ignored directories to prevent recursive compile loops
                    let components = path
                        .components()
                        .map(|c| c.as_os_str().to_string_lossy().to_string())
                        .collect::<Vec<_>>();
                    if components.iter().any(|c| {
                        c == ".tmp"
                            || c == ".git"
                            || c == "target"
                            || c == "node_modules"
                            || c == ".gitignore"
                    }) {
                        continue;
                    }
                    should_recompile = true;
                    println!("Change detected: {}", path.display());
                    break;
                }

                if should_recompile {
                    println!("Re-compiling...");
                    context.refresh();
                    match Compiler::compile(cwd, &context, custom_config_path) {
                        Ok(output) => {
                            if let Err(e) = Compiler::write_to_disk(cwd, &output) {
                                eprintln!("Failed to write to disk: {}", e);
                            } else {
                                println!("Compilation successful.");
                            }
                        }
                        Err(e) => {
                            eprintln!("Compilation failed: {}", e);
                        }
                    }
                }
            }
            Err(e) => eprintln!("Watcher error: {:?}", e),
        }
    }

    Ok(())
}
