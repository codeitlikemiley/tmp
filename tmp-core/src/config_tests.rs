use super::*;
use std::fs;
use std::path::Path;

#[test]
fn test_default_config() {
    let mock_env = |_: &str| None;
    let config =
        load_config_with_env(Some(Path::new("non_existent_file_path_123.toml")), mock_env).unwrap();
    assert_eq!(config.llm.strategy, "fallback");
    assert!(config.llm.providers.is_empty());
}

#[test]
fn test_parse_valid_toml() {
    let toml_content = r#"
[llm]
strategy = "round-robin"
providers = [
    { provider = "gemini", keys = ["key1", "key2"] },
    { provider = "openai", keys = ["op1"] }
]
"#;
    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    fs::write(&config_path, toml_content).unwrap();

    let mock_env = |_: &str| None;
    let config = load_config_with_env(Some(&config_path), mock_env).unwrap();
    assert_eq!(config.llm.strategy, "round-robin");
    assert_eq!(config.llm.providers.len(), 2);

    let gemini = config
        .llm
        .providers
        .iter()
        .find(|p| p.provider == "gemini")
        .unwrap();
    assert_eq!(gemini.keys, vec!["key1", "key2"]);
}

#[test]
fn test_env_fallback_missing_provider() {
    let toml_content = r#"
[llm]
strategy = "single"
providers = [
    { provider = "openai", keys = ["toml_openai_key"] }
]
"#;
    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    fs::write(&config_path, toml_content).unwrap();

    let mock_env = |var: &str| {
        if var == "GEMINI_API_KEY" {
            Some("env_gemini_key".to_string())
        } else {
            None
        }
    };

    let config = load_config_with_env(Some(&config_path), mock_env).unwrap();
    assert_eq!(config.llm.strategy, "single");

    // gemini should be appended from env
    let gemini = config
        .llm
        .providers
        .iter()
        .find(|p| p.provider == "gemini")
        .unwrap();
    assert_eq!(gemini.keys, vec!["env_gemini_key"]);

    // openai should remain as is
    let openai = config
        .llm
        .providers
        .iter()
        .find(|p| p.provider == "openai")
        .unwrap();
    assert_eq!(openai.keys, vec!["toml_openai_key"]);
}

#[test]
fn test_env_fallback_empty_keys() {
    let toml_content = r#"
[llm]
strategy = "fallback"
providers = [
    { provider = "openai", keys = [] }
]
"#;
    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    fs::write(&config_path, toml_content).unwrap();

    let mock_env = |var: &str| {
        if var == "OPENAI_API_KEY" {
            Some("env_openai_key".to_string())
        } else {
            None
        }
    };

    let config = load_config_with_env(Some(&config_path), mock_env).unwrap();
    let openai = config
        .llm
        .providers
        .iter()
        .find(|p| p.provider == "openai")
        .unwrap();
    assert_eq!(openai.keys, vec!["env_openai_key"]);
}

#[test]
fn test_env_override() {
    let toml_content = r#"
[llm]
strategy = "fallback"
providers = [
    { provider = "openai", keys = ["toml_openai_key"] }
]
"#;
    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    fs::write(&config_path, toml_content).unwrap();

    let mock_env = |var: &str| {
        if var == "OPENAI_API_KEY" {
            Some("env_openai_key".to_string())
        } else {
            None
        }
    };

    let config = load_config_with_env(Some(&config_path), mock_env).unwrap();
    let openai = config
        .llm
        .providers
        .iter()
        .find(|p| p.provider == "openai")
        .unwrap();
    // Should be overridden by env variable
    assert_eq!(openai.keys, vec!["env_openai_key"]);
}

#[test]
fn test_empty_config_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    fs::write(&config_path, "").unwrap();

    let mock_env = |_: &str| None;
    let config = load_config_with_env(Some(&config_path), mock_env).unwrap();
    assert_eq!(config.llm.strategy, "fallback");
    assert!(config.llm.providers.is_empty());
}

#[test]
fn test_malformed_config_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    fs::write(&config_path, "llm = 42").unwrap(); // llm should be a table, not an integer

    let mock_env = |_: &str| None;
    let res = load_config_with_env(Some(&config_path), mock_env);
    assert!(res.is_err(), "Malformed config file should fail to parse");
}

#[test]
fn test_empty_env_variables() {
    let mock_env = |var: &str| {
        if var == "GEMINI_API_KEY" {
            Some("  ".to_string())
        } else {
            None
        }
    };
    let config = load_config_with_env(Some(Path::new("non_existent_file.toml")), mock_env).unwrap();
    let gemini = config.llm.providers.iter().find(|p| p.provider == "gemini");
    assert!(gemini.is_none());
}

#[test]
fn test_config_invalid_toml_structural_mismatch() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    fs::write(&config_path, "[llm]\nproviders = \"not_an_array\"").unwrap();

    let mock_env = |_: &str| None;
    let res = load_config_with_env(Some(&config_path), mock_env);
    assert!(res.is_err(), "Structural mismatch should fail to parse");
}

#[test]
fn test_config_empty_tmp_config_dir_env() {
    let original_val = std::env::var("TMP_CONFIG_DIR");
    std::env::set_var("TMP_CONFIG_DIR", "");
    let p = default_config_path();
    // Clean up
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

#[test]
fn test_config_env_base_url_and_model() {
    let mock_env = |var: &str| match var {
        "GEMINI_BASE_URL" => Some("https://gemini.example.com".to_string()),
        "GEMINI_MODEL" => Some("gemini-1.5-pro".to_string()),
        "OPENAI_BASE_URL" => Some("https://openai.example.com".to_string()),
        "OPENAI_MODEL" => Some("gpt-4o".to_string()),
        "OLLAMA_BASE_URL" => Some("http://localhost:11434".to_string()),
        "OLLAMA_MODEL" => Some("llama3".to_string()),
        "OPENAI_COMPATIBLE_BASE_URL" => Some("https://compatible.example.com".to_string()),
        "OPENAI_COMPATIBLE_MODEL" => Some("custom-model".to_string()),
        _ => None,
    };
    let config = load_config_with_env(Some(Path::new("non_existent_file.toml")), mock_env).unwrap();

    let gemini = config
        .llm
        .providers
        .iter()
        .find(|p| p.provider == "gemini")
        .unwrap();
    assert_eq!(
        gemini.base_url.as_deref(),
        Some("https://gemini.example.com")
    );
    assert_eq!(gemini.model.as_deref(), Some("gemini-1.5-pro"));
    assert!(gemini.keys.is_empty());

    let openai = config
        .llm
        .providers
        .iter()
        .find(|p| p.provider == "openai")
        .unwrap();
    assert_eq!(
        openai.base_url.as_deref(),
        Some("https://openai.example.com")
    );
    assert_eq!(openai.model.as_deref(), Some("gpt-4o"));

    let ollama = config
        .llm
        .providers
        .iter()
        .find(|p| p.provider == "ollama")
        .unwrap();
    assert_eq!(ollama.base_url.as_deref(), Some("http://localhost:11434"));
    assert_eq!(ollama.model.as_deref(), Some("llama3"));

    let compatible = config
        .llm
        .providers
        .iter()
        .find(|p| p.provider == "openai-compatible")
        .unwrap();
    assert_eq!(
        compatible.base_url.as_deref(),
        Some("https://compatible.example.com")
    );
    assert_eq!(compatible.model.as_deref(), Some("custom-model"));
}
