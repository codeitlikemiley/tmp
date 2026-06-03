use std::fs;
use std::path::PathBuf;
use tmp_core::config::default_config_path;

pub fn list(custom_config_path: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
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

    if !schemas_dir.exists() {
        println!("No schemas directory found. Have you run `tmp init`?");
        return Ok(());
    }

    println!("Installed schemas in {}:", schemas_dir.display());
    let mut count = 0;
    for entry in fs::read_dir(schemas_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                println!(" - {}", stem);
                count += 1;
            }
        }
    }

    if count == 0 {
        println!(" (none)");
    }

    Ok(())
}

fn get_schemas_dir(
    custom_config_path: Option<&str>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let config_file_path = match custom_config_path {
        Some(p) => PathBuf::from(p),
        None => match default_config_path() {
            Some(p) => p,
            None => return Err("Could not determine default config directory.".into()),
        },
    };
    tmp_core::config::load_config(Some(&config_file_path))?;
    let config_dir = config_file_path
        .parent()
        .ok_or("Invalid configuration path")?;
    Ok(config_dir.join("schemas"))
}

pub fn share(
    tool: &str,
    custom_config_path: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    if tool.is_empty()
        || !tool
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return Err("Invalid tool name".into());
    }
    let schemas_dir = get_schemas_dir(custom_config_path)?;
    let schema_file = schemas_dir.join(format!("{}.json", tool));
    if !schema_file.exists() {
        return Err(format!("Schema not found for tool: {}", tool).into());
    }
    let content = fs::read_to_string(&schema_file)?;
    let schema = tmp_core::schema::Schema::from_json(&content)?;
    let shareable = schema.export_shareable();
    let shareable_json = serde_json::to_string_pretty(&shareable)?;
    let output_file = PathBuf::from(format!("{}.json", tool));
    fs::write(&output_file, shareable_json)?;
    Ok(())
}

pub fn import(
    source: &str,
    custom_config_path: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = if source.starts_with("http://") || source.starts_with("https://") {
        ureq::get(source).call()?.into_string()?
    } else if let Some(stripped) = source.strip_prefix("file://") {
        fs::read_to_string(stripped)?
    } else {
        fs::read_to_string(source)?
    };

    let schema = tmp_core::schema::Schema::from_json(&content)?;
    let schemas_dir = get_schemas_dir(custom_config_path)?;
    fs::create_dir_all(&schemas_dir)?;
    let target_file = schemas_dir.join(format!("{}.json", schema.meta.tool));
    let pretty_json = serde_json::to_string_pretty(&schema)?;
    fs::write(&target_file, pretty_json)?;
    Ok(())
}

pub fn keywords(
    tool: &str,
    words: Vec<String>,
    custom_config_path: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    if tool.is_empty()
        || !tool
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return Err("Invalid tool name".into());
    }
    let schemas_dir = get_schemas_dir(custom_config_path)?;
    let schema_file = schemas_dir.join(format!("{}.json", tool));
    if !schema_file.exists() {
        return Err(format!("Schema not found for tool: {}", tool).into());
    }
    let content = fs::read_to_string(&schema_file)?;
    let mut schema = tmp_core::schema::Schema::from_json(&content)?;
    if words.is_empty() {
        for word in &schema.meta.keywords {
            println!("{}", word);
        }
    } else {
        schema.meta.keywords = words;
        let pretty_json = serde_json::to_string_pretty(&schema)?;
        fs::write(&schema_file, pretty_json)?;
    }
    Ok(())
}
