use std::path::PathBuf;
use tmp_core::compile::Compiler;
use tmp_core::completion::complete;
use tmp_core::context::Context;

pub fn run(
    input: &str,
    cwd: Option<&str>,
    config_path: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let cwd = cwd
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .ok_or("Could not determine working directory")?;
    let context = Context::detect(&cwd, None, None);
    let compiled = Compiler::compile(&cwd, &context, config_path)?;
    for candidate in complete(input, &compiled) {
        println!("{}\t{}", candidate.value, candidate.description);
    }
    Ok(())
}
