use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Schema {
    pub meta: SchemaMeta,
    pub commands: Vec<Command>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaMeta {
    pub tool: String,
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_with: Option<String>,
    #[serde(default)]
    pub verified: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waz_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_file_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_binary: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Command {
    pub command: String,
    pub description: String,
    pub group: String,
    #[serde(default)]
    pub verified: bool,
    pub tokens: Vec<Token>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Token {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub required: bool,
    #[serde(rename = "type")]
    pub token_type: TokenType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_source: Option<DataSource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenType {
    String,
    Boolean,
    Enum,
    File,
    Number,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataSource {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolver: Option<String>,
    #[serde(default = "default_parse_mode")]
    pub parse: String,
}

fn default_parse_mode() -> String {
    "lines".to_string()
}

impl Schema {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn export_shareable(&self) -> Self {
        let mut cloned = self.clone();
        for cmd in &mut cloned.commands {
            for token in &mut cmd.tokens {
                if token.data_source.is_some() {
                    token.values = None;
                }
            }
        }
        cloned
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.meta.tool.trim().is_empty() {
            return Err("tool name cannot be empty".to_string());
        }
        if !self
            .meta
            .tool
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(
                "tool name must only contain alphanumeric characters, underscores, and dashes"
                    .to_string(),
            );
        }

        for cmd in &self.commands {
            if cmd.command.trim().is_empty() {
                return Err("command cannot be empty".to_string());
            }
            for token in &cmd.tokens {
                if token.name.is_empty() {
                    return Err("token name cannot be empty".to_string());
                }
                if token.name.chars().any(|c| c.is_whitespace()) {
                    return Err("token name cannot contain whitespace".to_string());
                }
                if !token
                    .name
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
                {
                    return Err(format!(
                        "token name '{}' contains invalid characters",
                        token.name
                    ));
                }
                if let Some(ref ds) = token.data_source {
                    if let Some(ref c) = ds.command {
                        if c.trim().is_empty() {
                            return Err("data source command cannot be empty".to_string());
                        }
                    }
                    if let Some(ref r) = ds.resolver {
                        if r.trim().is_empty() {
                            return Err("data source resolver cannot be empty".to_string());
                        }
                    }
                    if ds.command.is_none() && ds.resolver.is_none() {
                        return Err(
                            "data source must specify either command or resolver".to_string()
                        );
                    }
                    if ds.parse != "lines" && ds.parse != "words" {
                        return Err(format!(
                            "invalid parse mode '{}' (must be 'lines' or 'words')",
                            ds.parse
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for Schema {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct SchemaHelper {
            meta: SchemaMeta,
            commands: Vec<Command>,
        }

        let helper = SchemaHelper::deserialize(deserializer)?;

        let schema = Schema {
            meta: helper.meta,
            commands: helper.commands,
        };

        schema.validate().map_err(serde::de::Error::custom)?;

        Ok(schema)
    }
}

#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;
