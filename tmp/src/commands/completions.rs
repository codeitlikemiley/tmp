use tmp_core::completion::{
    generate_bash_completions, generate_fish_completions, generate_zsh_completions,
};

pub fn run(shell: &str) -> Result<(), Box<dyn std::error::Error>> {
    let script = match shell {
        "zsh" => generate_zsh_completions(),
        "bash" => generate_bash_completions(),
        "fish" => generate_fish_completions(),
        other => {
            return Err(format!("Unsupported shell '{other}'. Use zsh, bash, or fish.").into());
        }
    };
    print!("{script}");
    Ok(())
}
