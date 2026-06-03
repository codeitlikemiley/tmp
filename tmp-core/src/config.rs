use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Config {}

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
    let path = match custom_path {
        Some(p) => Some(p.to_path_buf()),
        None => default_config_path(),
    };

    let config = if let Some(p) = path {
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

    Ok(config)
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
