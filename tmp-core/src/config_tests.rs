use super::*;
use std::fs;
use std::path::Path;

#[test]
fn test_default_config() {
    let config = load_config(Some(Path::new("non_existent_file_path_123.toml"))).unwrap();
    assert_eq!(config, Config::default());
}

#[test]
fn test_parse_empty_or_comment_only_toml() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    fs::write(&config_path, "# TMP configuration\n").unwrap();

    let config = load_config(Some(&config_path)).unwrap();
    assert_eq!(config, Config::default());
}

#[test]
fn test_parse_legacy_llm_toml_without_using_provider_settings() {
    let toml_content = r#"
[llm]
strategy = "round-robin"

[[llm.providers]]
provider = "gemini"
keys = ["key1", "key2"]
"#;
    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    fs::write(&config_path, toml_content).unwrap();

    let config = load_config(Some(&config_path)).unwrap();
    assert_eq!(config, Config::default());
}

#[test]
fn test_malformed_config_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    fs::write(&config_path, "[unclosed").unwrap();

    let res = load_config(Some(&config_path));
    assert!(res.is_err(), "Malformed TOML should fail to parse");
}

#[test]
fn test_config_empty_tmp_config_dir_env() {
    let original_val = std::env::var("TMP_CONFIG_DIR");
    std::env::set_var("TMP_CONFIG_DIR", "");
    let p = default_config_path();
    match original_val {
        Ok(val) => std::env::set_var("TMP_CONFIG_DIR", val),
        Err(_) => std::env::remove_var("TMP_CONFIG_DIR"),
    }

    let path = p.unwrap();
    let expected = dirs::home_dir()
        .map(|mut p| {
            p.push(".config");
            p.push("tmp");
            p.push("config.toml");
            p
        })
        .unwrap();
    assert_eq!(path, expected);
}
