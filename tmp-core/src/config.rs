use serde::{Deserialize, Serialize};
use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Config {
    #[serde(default)]
    pub llm: LlmConfig,
}

fn default_strategy() -> String {
    "fallback".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LlmConfig {
    #[serde(default)]
    pub providers: Vec<ProviderConfig>,
    #[serde(default = "default_strategy")]
    pub strategy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderConfig {
    pub provider: String, // "gemini", "openai", "ollama", "openai-compatible"
    pub keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl Default for LlmConfig {
    fn default() -> Self {
        LlmConfig {
            providers: Vec::new(),
            strategy: "fallback".to_string(),
        }
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Toml(toml::de::Error),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io(err) => write!(f, "I/O error: {}", err),
            ConfigError::Toml(err) => write!(f, "TOML deserialization error: {}", err),
        }
    }
}

impl std::error::Error for ConfigError {}

pub fn default_config_path() -> Option<PathBuf> {
    if let Ok(custom_dir) = std::env::var("TMP_CONFIG_DIR") {
        if !custom_dir.trim().is_empty() {
            let mut p = PathBuf::from(custom_dir);
            p.push("config.toml");
            return Some(p);
        }
    }
    dirs::home_dir().map(|mut p| {
        p.push(".config");
        p.push("tmp");
        p.push("config.toml");
        p
    })
}

pub fn load_config(custom_path: Option<&Path>) -> Result<Config, ConfigError> {
    load_config_with_env(custom_path, |var| env::var(var).ok())
}

pub fn load_config_with_env<F>(
    custom_path: Option<&Path>,
    env_lookup: F,
) -> Result<Config, ConfigError>
where
    F: Fn(&str) -> Option<String>,
{
    let path = match custom_path {
        Some(p) => Some(p.to_path_buf()),
        None => default_config_path(),
    };

    let mut config = if let Some(p) = path {
        if p.exists() {
            let content = fs::read_to_string(&p).map_err(ConfigError::Io)?;
            if content.trim().is_empty() {
                Config::default()
            } else {
                toml::from_str(&content).map_err(ConfigError::Toml)?
            }
        } else {
            Config::default()
        }
    } else {
        Config::default()
    };

    // Apply environment variables fallback
    let provider_env_mapping = [
        (
            "gemini",
            "GEMINI_API_KEY",
            "GEMINI_BASE_URL",
            "GEMINI_MODEL",
        ),
        (
            "openai",
            "OPENAI_API_KEY",
            "OPENAI_BASE_URL",
            "OPENAI_MODEL",
        ),
        (
            "ollama",
            "OLLAMA_API_KEY",
            "OLLAMA_BASE_URL",
            "OLLAMA_MODEL",
        ),
        (
            "openai-compatible",
            "OPENAI_COMPATIBLE_API_KEY",
            "OPENAI_COMPATIBLE_BASE_URL",
            "OPENAI_COMPATIBLE_MODEL",
        ),
    ];

    for &(p_name, key_var, base_url_var, model_var) in &provider_env_mapping {
        let key_val = env_lookup(key_var).filter(|s| !s.trim().is_empty());
        let base_url_val = env_lookup(base_url_var).filter(|s| !s.trim().is_empty());
        let model_val = env_lookup(model_var).filter(|s| !s.trim().is_empty());

        if key_val.is_some() || base_url_val.is_some() || model_val.is_some() {
            if let Some(prov) = config
                .llm
                .providers
                .iter_mut()
                .find(|p| p.provider == p_name)
            {
                if let Some(key) = key_val {
                    prov.keys = vec![key];
                }
                if let Some(base_url) = base_url_val {
                    prov.base_url = Some(base_url);
                }
                if let Some(model) = model_val {
                    prov.model = Some(model);
                }
            } else {
                config.llm.providers.push(ProviderConfig {
                    provider: p_name.to_string(),
                    keys: key_val.map(|k| vec![k]).unwrap_or_default(),
                    base_url: base_url_val,
                    model: model_val,
                });
            }
        }
    }

    Ok(config)
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
