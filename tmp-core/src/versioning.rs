use crate::schema::Schema;
use std::path::{Path, PathBuf};

pub fn schemas_dir() -> Option<PathBuf> {
    schemas_dir_for_config(None)
}

pub fn schemas_dir_for_config(config_path: Option<&Path>) -> Option<PathBuf> {
    if let Some(config_path) = config_path {
        return config_path.parent().map(|parent| parent.join("schemas"));
    }

    if let Ok(custom_dir) = std::env::var("TMP_CONFIG_DIR") {
        if !custom_dir.trim().is_empty() {
            let mut p = PathBuf::from(custom_dir);
            p.push("schemas");
            return Some(p);
        }
    }
    dirs::home_dir().map(|mut p| {
        p.push(".config");
        p.push("tmp");
        p.push("schemas");
        p
    })
}

pub fn active_schema_path(tool: &str) -> Result<PathBuf, String> {
    active_schema_path_for_config(tool, None)
}

pub fn active_schema_path_for_config(
    tool: &str,
    config_path: Option<&Path>,
) -> Result<PathBuf, String> {
    let dir = schemas_dir_for_config(config_path)
        .ok_or_else(|| "Could not determine schemas directory".to_string())?;
    Ok(dir.join(format!("{}.json", tool)))
}

pub fn version_schema_path(tool: &str, version: u32) -> Result<PathBuf, String> {
    version_schema_path_for_config(tool, version, None)
}

pub fn version_schema_path_for_config(
    tool: &str,
    version: u32,
    config_path: Option<&Path>,
) -> Result<PathBuf, String> {
    let dir = schemas_dir_for_config(config_path)
        .ok_or_else(|| "Could not determine schemas directory".to_string())?;
    Ok(dir
        .join("versions")
        .join(tool)
        .join(format!("v{}.json", version)))
}

pub fn save_schema(schema: &Schema) -> Result<(), String> {
    save_schema_for_config(schema, None)
}

pub fn save_schema_for_config(schema: &Schema, config_path: Option<&Path>) -> Result<(), String> {
    let active_path = active_schema_path_for_config(&schema.meta.tool, config_path)?;
    let version_path =
        version_schema_path_for_config(&schema.meta.tool, schema.meta.version, config_path)?;

    if let Some(parent) = active_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create active schema directory: {}", e))?;
    }
    if let Some(parent) = version_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create version schema directory: {}", e))?;
    }

    let content = serde_json::to_string_pretty(schema)
        .map_err(|e| format!("Failed to serialize schema: {}", e))?;
    std::fs::write(&active_path, &content)
        .map_err(|e| format!("Failed to write active schema file: {}", e))?;
    std::fs::write(&version_path, &content)
        .map_err(|e| format!("Failed to write version schema file: {}", e))?;
    Ok(())
}

pub fn get_latest_version(tool: &str) -> Result<Option<u32>, String> {
    get_latest_version_for_config(tool, None)
}

pub fn get_latest_version_for_config(
    tool: &str,
    config_path: Option<&Path>,
) -> Result<Option<u32>, String> {
    let dir = schemas_dir_for_config(config_path)
        .ok_or_else(|| "Could not determine schemas directory".to_string())?
        .join("versions")
        .join(tool);
    if !dir.exists() {
        return Ok(None);
    }
    let mut max_version = None;
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("Failed to read versions directory: {}", e))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
                if filename.starts_with('v') && filename.ends_with(".json") {
                    let version_part = &filename[1..filename.len() - 5];
                    if let Ok(ver) = version_part.parse::<u32>() {
                        max_version = Some(max_version.map_or(ver, |v| std::cmp::max(v, ver)));
                    }
                }
            }
        }
    }
    Ok(max_version)
}

pub fn load_schema(tool: &str, version: u32) -> Result<Schema, String> {
    load_schema_for_config(tool, version, None)
}

pub fn load_schema_for_config(
    tool: &str,
    version: u32,
    config_path: Option<&Path>,
) -> Result<Schema, String> {
    let path = version_schema_path_for_config(tool, version, config_path)?;
    if !path.exists() {
        return Err(format!("Schema file not found: {:?}", path));
    }
    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read schema file: {}", e))?;
    let schema =
        Schema::from_json(&content).map_err(|e| format!("Failed to deserialize schema: {}", e))?;
    Ok(schema)
}

pub fn get_history(tool: &str) -> Result<Vec<(u32, std::time::SystemTime)>, String> {
    get_history_for_config(tool, None)
}

pub fn get_history_for_config(
    tool: &str,
    config_path: Option<&Path>,
) -> Result<Vec<(u32, std::time::SystemTime)>, String> {
    let dir = schemas_dir_for_config(config_path)
        .ok_or_else(|| "Could not determine schemas directory".to_string())?
        .join("versions")
        .join(tool);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut history = Vec::new();
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("Failed to read versions directory: {}", e))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
                if filename.starts_with('v') && filename.ends_with(".json") {
                    let version_part = &filename[1..filename.len() - 5];
                    if let Ok(ver) = version_part.parse::<u32>() {
                        let metadata = entry
                            .metadata()
                            .map_err(|e| format!("Failed to read file metadata: {}", e))?;
                        let modified = metadata
                            .modified()
                            .unwrap_or_else(|_| std::time::SystemTime::now());
                        history.push((ver, modified));
                    }
                }
            }
        }
    }
    history.sort_by_key(|h| h.0);
    Ok(history)
}

pub fn rollback(tool: &str, target_version: u32) -> Result<(), String> {
    rollback_for_config(tool, target_version, None)
}

pub fn rollback_for_config(
    tool: &str,
    target_version: u32,
    config_path: Option<&Path>,
) -> Result<(), String> {
    let mut schema = load_schema_for_config(tool, target_version, config_path)?;
    let latest = get_latest_version_for_config(tool, config_path)?
        .ok_or_else(|| "No schema version history found".to_string())?;
    schema.meta.version = latest + 1;
    save_schema_for_config(&schema, config_path)?;
    Ok(())
}

pub fn generate_diff(old: &str, new: &str) -> String {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    let m = old_lines.len();
    let n = new_lines.len();

    let mut dp = vec![vec![0; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            if old_lines[i - 1] == new_lines[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = std::cmp::max(dp[i - 1][j], dp[i][j - 1]);
            }
        }
    }

    let mut result = Vec::new();
    let mut i = m;
    let mut j = n;

    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old_lines[i - 1] == new_lines[j - 1] {
            result.push(format!("  {}", old_lines[i - 1]));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
            result.push(format!("\x1b[32m+ {}\x1b[0m", new_lines[j - 1]));
            j -= 1;
        } else if i > 0 && (j == 0 || dp[i][j - 1] < dp[i - 1][j]) {
            result.push(format!("\x1b[31m- {}\x1b[0m", old_lines[i - 1]));
            i -= 1;
        }
    }

    result.reverse();
    result.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn dummy_schema(tool: &str, version: u32) -> Schema {
        Schema::from_json(&format!(
            r#"{{
                "meta": {{
                    "tool": "{}",
                    "version": {},
                    "verified": true,
                    "keywords": []
                }},
                "commands": []
            }}"#,
            tool, version
        ))
        .unwrap()
    }

    #[test]
    fn test_versioning_all() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let temp_dir = tempfile::tempdir().unwrap();
        std::env::set_var("TMP_CONFIG_DIR", temp_dir.path());

        let tool = "testtool";
        assert_eq!(get_latest_version(tool).unwrap(), None);

        let s1 = dummy_schema(tool, 1);
        save_schema(&s1).unwrap();
        assert_eq!(get_latest_version(tool).unwrap(), Some(1));

        let s2 = dummy_schema(tool, 2);
        save_schema(&s2).unwrap();
        assert_eq!(get_latest_version(tool).unwrap(), Some(2));

        // Verify active schema path exists: <temp_dir>/schemas/testtool.json
        let active_path = active_schema_path(tool).unwrap();
        assert!(active_path.exists());

        // Verify version schema paths exist:
        // <temp_dir>/schemas/versions/testtool/v1.json
        // <temp_dir>/schemas/versions/testtool/v2.json
        let v1_path = version_schema_path(tool, 1).unwrap();
        let v2_path = version_schema_path(tool, 2).unwrap();
        assert!(v1_path.exists());
        assert!(v2_path.exists());

        let loaded = load_schema(tool, 1).unwrap();
        assert_eq!(loaded.meta.version, 1);
        assert_eq!(loaded.meta.tool, tool);

        // Test history
        let history = get_history(tool).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].0, 1);
        assert_eq!(history[1].0, 2);

        // Test rollback
        rollback(tool, 1).unwrap();
        assert_eq!(get_latest_version(tool).unwrap(), Some(3));

        let loaded_rolled = load_schema(tool, 3).unwrap();
        assert_eq!(loaded_rolled.meta.version, 3);
        assert_eq!(loaded_rolled.meta.tool, tool);

        // Verify rollback generated version 3
        let v3_path = version_schema_path(tool, 3).unwrap();
        assert!(v3_path.exists());

        std::env::remove_var("TMP_CONFIG_DIR");
    }

    #[test]
    fn test_generate_diff() {
        let old = "line1\nline2\n";
        let new = "line1\nline3\n";
        let diff = generate_diff(old, new);
        assert!(diff.contains("\x1b[31m- line2\x1b[0m"));
        assert!(diff.contains("\x1b[32m+ line3\x1b[0m"));
    }

    #[test]
    fn test_schemas_dir_whitespace() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TMP_CONFIG_DIR", "   ");
        let dir = schemas_dir();
        // Since we checked whitespace, it should fall back to home_dir
        if let Some(h) = dirs::home_dir() {
            assert_eq!(dir, Some(h.join(".config").join("tmp").join("schemas")));
        }
        std::env::remove_var("TMP_CONFIG_DIR");
    }

    #[test]
    fn test_nonexistent_schema_load_and_rollback() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let temp_dir = tempfile::tempdir().unwrap();
        std::env::set_var("TMP_CONFIG_DIR", temp_dir.path());

        // Loading nonexistent schema version should fail
        let res = load_schema("nonexistent_tool", 1);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Schema file not found"));

        // Rollback on nonexistent schema tool/version should fail
        let res_roll = rollback("nonexistent_tool", 1);
        assert!(res_roll.is_err());

        std::env::remove_var("TMP_CONFIG_DIR");
    }

    #[test]
    fn test_invalid_version_filenames_ignored() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let temp_dir = tempfile::tempdir().unwrap();
        std::env::set_var("TMP_CONFIG_DIR", temp_dir.path());

        let tool = "weirdfiles";
        let versions_dir = temp_dir.path().join("schemas").join("versions").join(tool);
        std::fs::create_dir_all(&versions_dir).unwrap();

        // Write valid and invalid filename versions
        std::fs::write(versions_dir.join("v1.json"), "{}").unwrap();
        std::fs::write(versions_dir.join("vabc.json"), "{}").unwrap(); // non-numeric
        std::fs::write(versions_dir.join("readme.txt"), "{}").unwrap(); // wrong pattern
        std::fs::write(versions_dir.join("v2.json.bak"), "{}").unwrap(); // wrong suffix

        // get_latest_version should only parse v1.json and return Some(1)
        assert_eq!(get_latest_version(tool).unwrap(), Some(1));

        // get_history should only return version 1
        let history = get_history(tool).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].0, 1);

        std::env::remove_var("TMP_CONFIG_DIR");
    }
}
