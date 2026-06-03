use std::path::PathBuf;
use tmp_core::config::default_config_path;
use tmp_core::registry::RegistryClient;

const DEFAULT_REPO: &str = "codeitlikemiley/tmp-registry";

pub fn search(query: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Searching registry for '{}'...", query);
    let repo = std::env::var("TMP_REGISTRY_REPO").unwrap_or_else(|_| DEFAULT_REPO.to_string());
    let client = RegistryClient::new(&repo);

    match client.search(query) {
        Ok(results) => {
            if results.is_empty() {
                println!("No matching schemas found.");
            } else {
                println!(
                    "{:<15} {:<10} {:<20} {:<10} Description",
                    "Tool", "Version", "Author", "Verified"
                );
                println!("{}", "-".repeat(80));
                for meta in results {
                    let desc = meta.description.unwrap_or_else(|| "".to_string());
                    let verified_str = if meta.verified { "✓" } else { "✗" };
                    println!(
                        "{:<15} {:<10} {:<20} {:<10} {}",
                        meta.tool, meta.version, meta.author, verified_str, desc
                    );
                }
            }
        }
        Err(e) => {
            eprintln!("Error querying registry: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}

pub fn install(
    tool: &str,
    custom_config_path: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Installing schema for '{}'...", tool);
    let repo = std::env::var("TMP_REGISTRY_REPO").unwrap_or_else(|_| DEFAULT_REPO.to_string());
    let client = RegistryClient::new(&repo);

    let config_file_path = match custom_config_path {
        Some(p) => PathBuf::from(p),
        None => match default_config_path() {
            Some(p) => p,
            None => return Err("Could not determine default config directory.".into()),
        },
    };

    // Validate the configuration file
    tmp_core::config::load_config(Some(&config_file_path))?;

    let config_dir = config_file_path
        .parent()
        .ok_or("Invalid configuration path")?;
    let schemas_dir = config_dir.join("schemas");

    match client.install(tool, &schemas_dir) {
        Ok(_) => {
            println!(
                "Successfully installed schema for '{}' to {}",
                tool,
                schemas_dir.display()
            );
        }
        Err(e) => {
            eprintln!("Failed to install schema: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}

pub fn publish(
    tool: &str,
    _custom_config_path: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Publishing schema for '{}'...", tool);
    let repo = std::env::var("TMP_REGISTRY_REPO").unwrap_or_else(|_| DEFAULT_REPO.to_string());
    let client = RegistryClient::new(&repo);

    let schema_file = std::path::PathBuf::from(format!("{}.json", tool));
    if !schema_file.exists() {
        return Err(format!("Shared schema file not found in current directory: {}.json. Please run `tmp schema share {}` first.", tool, tool).into());
    }

    let schema_content = std::fs::read_to_string(&schema_file)?;

    match client.publish(tool, &schema_content) {
        Ok(_) => {
            println!("Successfully published schema for '{}' to registry", tool);
            let shared_file = std::path::PathBuf::from(format!("{}_shared.json", tool));
            std::fs::write(&shared_file, &schema_content)?;
        }
        Err(e) => {
            eprintln!("Failed to publish schema: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}
