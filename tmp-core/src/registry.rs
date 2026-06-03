use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone)]
pub struct RegistryClient {
    pub repo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryIndex {
    pub schemas: Vec<RegistrySchemaMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistrySchemaMeta {
    pub tool: String,
    pub version: String,
    pub author: String,
    pub commands_count: usize,
    pub verified: bool,
    pub download_url: String,
    pub description: Option<String>,
}

#[derive(Debug)]
pub enum RegistryError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Http(String),
    ToolNotFound(String),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryError::Io(err) => write!(f, "Registry I/O error: {}", err),
            RegistryError::Json(err) => write!(f, "Registry JSON parse error: {}", err),
            RegistryError::Http(err) => write!(f, "Registry HTTP error: {}", err),
            RegistryError::ToolNotFound(tool) => write!(f, "Tool '{}' not found in registry", tool),
        }
    }
}

impl std::error::Error for RegistryError {}

impl RegistryClient {
    pub fn new(repo: &str) -> Self {
        Self {
            repo: repo.to_string(),
        }
    }

    pub fn fetch_index(&self) -> Result<RegistryIndex, RegistryError> {
        let mut url = if self.repo.starts_with("http://")
            || self.repo.starts_with("https://")
            || self.repo.starts_with("file://")
        {
            self.repo.clone()
        } else {
            format!(
                "https://raw.githubusercontent.com/{}/main/index.json",
                self.repo
            )
        };

        if (url.starts_with("file://") || url.starts_with("http://") || url.starts_with("https://"))
            && !url.ends_with("index.json")
        {
            if !url.ends_with('/') {
                url.push('/');
            }
            url.push_str("index.json");
        }

        let content = if url.starts_with("file://") {
            let path = url.trim_start_matches("file://");
            std::fs::read_to_string(path).map_err(RegistryError::Io)?
        } else {
            ureq::get(&url)
                .call()
                .map_err(|e| RegistryError::Http(e.to_string()))?
                .into_string()
                .map_err(RegistryError::Io)?
        };

        let index: RegistryIndex = serde_json::from_str(&content).map_err(RegistryError::Json)?;
        Ok(index)
    }

    pub fn search(&self, query: &str) -> Result<Vec<RegistrySchemaMeta>, RegistryError> {
        let index = self.fetch_index()?;
        let query_lower = query.to_lowercase();
        let matches = index
            .schemas
            .into_iter()
            .filter(|schema| {
                schema.tool.to_lowercase().contains(&query_lower)
                    || schema
                        .description
                        .as_ref()
                        .map(|d| d.to_lowercase().contains(&query_lower))
                        .unwrap_or(false)
            })
            .collect();
        Ok(matches)
    }

    pub fn install(&self, tool: &str, target_dir: &std::path::Path) -> Result<(), RegistryError> {
        if tool.is_empty()
            || !tool
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(RegistryError::ToolNotFound(tool.to_string()));
        }

        let index = self.fetch_index()?;
        let meta = index
            .schemas
            .into_iter()
            .find(|schema| schema.tool == tool)
            .ok_or_else(|| RegistryError::ToolNotFound(tool.to_string()))?;

        let content = if meta.download_url.starts_with("file://") {
            if !self.repo.starts_with("file://") {
                return Err(RegistryError::Http(
                    "Local file schema not allowed for remote registry".to_string(),
                ));
            }
            let path = meta.download_url.trim_start_matches("file://");
            std::fs::read_to_string(path).map_err(RegistryError::Io)?
        } else {
            ureq::get(&meta.download_url)
                .call()
                .map_err(|e| RegistryError::Http(e.to_string()))?
                .into_string()
                .map_err(RegistryError::Io)?
        };

        // Validate JSON
        let _: serde_json::Value = serde_json::from_str(&content).map_err(RegistryError::Json)?;

        std::fs::create_dir_all(target_dir).map_err(RegistryError::Io)?;
        let dest_path = target_dir.join(format!("{}.json", tool));
        std::fs::write(&dest_path, content).map_err(RegistryError::Io)?;

        Ok(())
    }

    pub fn publish(&self, tool: &str, schema_content: &str) -> Result<(), RegistryError> {
        if self.repo.starts_with("file://") {
            let registry_dir = std::path::PathBuf::from(self.repo.trim_start_matches("file://"));
            let registry_dir =
                if registry_dir.file_name().and_then(|s| s.to_str()) == Some("index.json") {
                    registry_dir.parent().unwrap().to_path_buf()
                } else {
                    registry_dir
                };

            std::fs::create_dir_all(&registry_dir).map_err(RegistryError::Io)?;

            let dest_schema_path = registry_dir.join(format!("{}.json", tool));
            std::fs::write(&dest_schema_path, schema_content).map_err(RegistryError::Io)?;

            let index_path = registry_dir.join("index.json");
            let mut index = if index_path.exists() {
                let index_content =
                    std::fs::read_to_string(&index_path).map_err(RegistryError::Io)?;
                serde_json::from_str::<RegistryIndex>(&index_content).unwrap_or(RegistryIndex {
                    schemas: Vec::new(),
                })
            } else {
                RegistryIndex {
                    schemas: Vec::new(),
                }
            };

            let schema_val: serde_json::Value =
                serde_json::from_str(schema_content).map_err(RegistryError::Json)?;

            // Handle both numeric and string values for version
            let version = if let Some(v_num) = schema_val["meta"]["version"].as_u64() {
                v_num.to_string()
            } else if let Some(v_str) = schema_val["meta"]["version"].as_str() {
                v_str.to_string()
            } else {
                "1".to_string()
            };

            let author = schema_val["meta"]["author"]
                .as_str()
                .unwrap_or("Unknown")
                .to_string();
            let description = schema_val["meta"]["description"]
                .as_str()
                .map(|s| s.to_string());

            let meta = RegistrySchemaMeta {
                tool: tool.to_string(),
                version,
                author,
                commands_count: schema_val["commands"]
                    .as_array()
                    .map(|a| a.len())
                    .unwrap_or(0),
                verified: schema_val["meta"]["verified"].as_bool().unwrap_or(false),
                download_url: format!("file://{}", dest_schema_path.to_string_lossy()),
                description,
            };

            if let Some(pos) = index.schemas.iter().position(|s| s.tool == tool) {
                index.schemas[pos] = meta;
            } else {
                index.schemas.push(meta);
            }

            let new_index_json =
                serde_json::to_string_pretty(&index).map_err(RegistryError::Json)?;
            std::fs::write(&index_path, new_index_json).map_err(RegistryError::Io)?;

            Ok(())
        } else {
            Err(RegistryError::Http(
                "Publishing to remote registries is not implemented".to_string(),
            ))
        }
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
