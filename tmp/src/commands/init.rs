use std::fs;
use std::path::PathBuf;
use tmp_core::config::default_config_path;

pub fn run(custom_config_path: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    println!("Initializing Terminal Meta Protocol (tmp)...");

    let config_file_path = match custom_config_path {
        Some(p) => PathBuf::from(p),
        None => match default_config_path() {
            Some(p) => p,
            None => {
                return Err(
                    "Could not determine default config directory. Please specify --config.".into(),
                )
            }
        },
    };

    let config_dir = config_file_path
        .parent()
        .ok_or("Invalid configuration path")?;

    // Create config directory and schemas subdirectory
    let schemas_dir = config_dir.join("schemas");
    fs::create_dir_all(&schemas_dir)?;
    println!("Created schemas directory at: {}", schemas_dir.display());

    // Write default config if it doesn't exist
    if !config_file_path.exists() {
        let default_config_content = r#"[llm]
strategy = "fallback"

[[llm.providers]]
provider = "gemini"
keys = []

[[llm.providers]]
provider = "openai"
keys = []
"#;
        fs::write(&config_file_path, default_config_content)?;
        println!(
            "Created default configuration file at: {}",
            config_file_path.display()
        );
    } else {
        println!(
            "Configuration file already exists at: {}",
            config_file_path.display()
        );
    }

    println!("Initialization complete!");
    Ok(())
}
