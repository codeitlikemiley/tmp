use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    Cli,
    Api,
    Sql,
    Workflow,
    Script,
    Completion,
    Agent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Effect {
    #[serde(rename = "read-only")]
    ReadOnly,
    #[serde(rename = "build-test")]
    BuildTest,
    #[serde(rename = "network")]
    Network,
    #[serde(rename = "deployment")]
    Deployment,
    #[serde(rename = "destructive")]
    Destructive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Approval {
    NotRequired,
    Recommended,
    Required,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceType {
    ParsedHelp,
    DryRun,
    HumanReview,
    RegistrySignature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputMode {
    Raw,
    TestSummary,
    DiffSummary,
    GitSummary,
    LogSummary,
    SearchSummary,
    JsonProjection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputPolicyConfig {
    pub mode: OutputMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_retention: Option<String>,
}

fn default_surface() -> Surface {
    Surface::Cli
}

fn default_effect() -> Effect {
    Effect::BuildTest
}

fn default_risk() -> Risk {
    Risk::Low
}

fn default_approval() -> Approval {
    Approval::NotRequired
}

fn default_output_policy() -> OutputPolicyConfig {
    OutputPolicyConfig {
        mode: OutputMode::Raw,
        raw_retention: None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Schema {
    pub meta: SchemaMeta,
    pub operations: Vec<Operation>,
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
pub struct Operation {
    pub command: String,
    pub description: String,
    pub group: String,
    #[serde(default)]
    pub verified: bool,

    #[serde(alias = "tokens")]
    pub parameters: Vec<Parameter>,

    #[serde(default = "default_surface")]
    pub surface: Surface,
    #[serde(default = "default_effect")]
    pub effect: Effect,
    #[serde(default = "default_risk")]
    pub risk: Risk,
    #[serde(default = "default_approval")]
    pub approval: Approval,
    #[serde(default)]
    pub evidence: Vec<EvidenceType>,
    #[serde(default = "default_output_policy")]
    pub output_policy: OutputPolicyConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub required: bool,
    #[serde(rename = "type")]
    pub parameter_type: ParameterType,
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
pub enum ParameterType {
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
        for op in &mut cloned.operations {
            for param in &mut op.parameters {
                if param.data_source.is_some() {
                    param.values = None;
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

        for op in &self.operations {
            if op.command.trim().is_empty() {
                return Err("command cannot be empty".to_string());
            }
            for param in &op.parameters {
                if param.name.is_empty() {
                    return Err("token name cannot be empty".to_string());
                }
                if param.name.chars().any(|c| c.is_whitespace()) {
                    return Err("token name cannot contain whitespace".to_string());
                }
                if !param
                    .name
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
                {
                    return Err(format!(
                        "token name '{}' contains invalid characters",
                        param.name
                    ));
                }
                if let Some(ref ds) = param.data_source {
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

    /// Strict validation that checks for inconsistencies between risk, effect, and approval.
    ///
    /// Returns a list of warnings for operations with questionable metadata combinations.
    pub fn validate_strict(&self) -> Result<Vec<String>, String> {
        self.validate()?;
        let mut warnings = Vec::new();
        for op in &self.operations {
            if op.risk == Risk::High && op.approval == Approval::NotRequired {
                warnings.push(format!(
                    "operation '{}': high-risk operations should require approval",
                    op.command
                ));
            }
            if op.effect == Effect::Destructive && op.risk == Risk::Low {
                warnings.push(format!(
                    "operation '{}': destructive effects should not have low risk",
                    op.command
                ));
            }
        }
        Ok(warnings)
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
            #[serde(alias = "commands")]
            operations: Vec<Operation>,
        }

        let helper = SchemaHelper::deserialize(deserializer)?;

        let schema = Schema {
            meta: helper.meta,
            operations: helper.operations,
        };

        schema.validate().map_err(serde::de::Error::custom)?;

        Ok(schema)
    }
}

#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;
