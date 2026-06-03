use super::*;

#[test]
fn test_parse_valid_schema() {
    let json = r#"{
        "meta": {
            "tool": "git",
            "version": 1,
            "verified": true,
            "requires_binary": "git",
            "keywords": ["vcs", "git"]
        },
        "commands": [
            {
                "command": "git commit",
                "description": "Commit changes",
                "group": "git",
                "verified": true,
                "tokens": [
                    {
                        "name": "message",
                        "description": "Commit message",
                        "required": true,
                        "type": "String",
                        "flag": "-m"
                    },
                    {
                        "name": "branch",
                        "description": "Target branch",
                        "required": false,
                        "type": "Enum",
                        "data_source": {
                            "resolver": "git:branches"
                        }
                    }
                ]
            }
        ]
    }"#;

    let res = Schema::from_json(json);
    assert!(
        res.is_ok(),
        "Expected valid schema to parse: {:?}",
        res.err()
    );
    let schema = res.unwrap();
    assert_eq!(schema.meta.tool, "git");
    assert_eq!(schema.meta.version, 1);
    assert!(schema.meta.verified);
    assert_eq!(schema.meta.requires_binary, Some("git".to_string()));
    assert_eq!(
        schema.meta.keywords,
        vec!["vcs".to_string(), "git".to_string()]
    );

    assert_eq!(schema.operations.len(), 1);
    let op = &schema.operations[0];
    assert_eq!(op.command, "git commit");
    assert_eq!(op.parameters.len(), 2);

    let param1 = &op.parameters[0];
    assert_eq!(param1.name, "message");
    assert_eq!(param1.parameter_type, ParameterType::String);
    assert!(param1.required);
    assert_eq!(param1.flag, Some("-m".to_string()));
    assert!(param1.data_source.is_none());

    let param2 = &op.parameters[1];
    assert_eq!(param2.name, "branch");
    assert_eq!(param2.parameter_type, ParameterType::Enum);
    assert!(!param2.required);
    assert!(param2.data_source.is_some());
    let ds = param2.data_source.as_ref().unwrap();
    assert_eq!(ds.resolver, Some("git:branches".to_string()));
    // Check that parse mode defaulted to "lines"
    assert_eq!(ds.parse, "lines");

    // Check defaults for new metadata fields (backward compatibility)
    assert_eq!(op.surface, Surface::Cli);
    assert_eq!(op.effect, Effect::BuildTest);
    assert_eq!(op.risk, Risk::Low);
    assert_eq!(op.approval, Approval::NotRequired);
    assert!(op.evidence.is_empty());
    assert_eq!(op.output_policy.mode, OutputMode::Raw);
    assert_eq!(op.output_policy.raw_retention, None);
}

#[test]
fn test_validation_empty_tool() {
    let json = r#"{
        "meta": {
            "tool": "",
            "version": 1
        },
        "operations": []
    }"#;
    let res = Schema::from_json(json);
    assert!(res.is_err(), "Expected error for empty tool name");
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("tool name cannot be empty"));
}

#[test]
fn test_validation_empty_command() {
    let json = r#"{
        "meta": {
            "tool": "test",
            "version": 1
        },
        "operations": [
            {
                "command": "   ",
                "description": "empty command",
                "group": "test",
                "parameters": []
            }
        ]
    }"#;
    let res = Schema::from_json(json);
    assert!(res.is_err(), "Expected error for empty command string");
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("command cannot be empty"));
}

#[test]
fn test_validation_invalid_token_name() {
    // Test empty token name
    let json_empty = r#"{
        "meta": { "tool": "test", "version": 1 },
        "operations": [{
            "command": "test-cmd", "description": "desc", "group": "g", "parameters": [
                { "name": "", "description": "d", "type": "String" }
            ]
        }]
    }"#;
    let res = Schema::from_json(json_empty);
    assert!(res.is_err(), "Expected error for empty token name");
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("token name cannot be empty"));

    // Test token name with whitespace
    let json_space = r#"{
        "meta": { "tool": "test", "version": 1 },
        "operations": [{
            "command": "test-cmd", "description": "desc", "group": "g", "parameters": [
                { "name": "bad name", "description": "d", "type": "String" }
            ]
        }]
    }"#;
    let res = Schema::from_json(json_space);
    assert!(res.is_err(), "Expected error for space in token name");
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("token name cannot contain whitespace"));

    // Test token name with invalid characters
    let json_chars = r#"{
        "meta": { "tool": "test", "version": 1 },
        "operations": [{
            "command": "test-cmd", "description": "desc", "group": "g", "parameters": [
                { "name": "bad$name", "description": "d", "type": "String" }
            ]
        }]
    }"#;
    let res = Schema::from_json(json_chars);
    assert!(
        res.is_err(),
        "Expected error for special characters in token name"
    );
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("contains invalid characters"));
}

#[test]
fn test_validation_invalid_parse_mode() {
    let json = r#"{
        "meta": { "tool": "test", "version": 1 },
        "operations": [{
            "command": "test-cmd", "description": "desc", "group": "g", "parameters": [
                {
                    "name": "token", "description": "d", "type": "Enum",
                    "data_source": {
                        "resolver": "res",
                        "parse": "invalid_mode"
                    }
                }
            ]
        }]
    }"#;
    let res = Schema::from_json(json);
    assert!(res.is_err(), "Expected error for invalid parse mode");
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("invalid parse mode 'invalid_mode'"));
}

#[test]
fn test_validation_empty_data_source() {
    // Both command and resolver missing/None
    let json_none = r#"{
        "meta": { "tool": "test", "version": 1 },
        "operations": [{
            "command": "test-cmd", "description": "desc", "group": "g", "parameters": [
                {
                    "name": "token", "description": "d", "type": "Enum",
                    "data_source": {}
                }
            ]
        }]
    }"#;
    let res = Schema::from_json(json_none);
    assert!(res.is_err(), "Expected error for empty data source");
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("data source must specify either command or resolver"));

    // Command empty string
    let json_empty_cmd = r#"{
        "meta": { "tool": "test", "version": 1 },
        "operations": [{
            "command": "test-cmd", "description": "desc", "group": "g", "parameters": [
                {
                    "name": "token", "description": "d", "type": "Enum",
                    "data_source": {
                        "command": "   "
                    }
                }
            ]
        }]
    }"#;
    let res = Schema::from_json(json_empty_cmd);
    assert!(res.is_err(), "Expected error for empty data source command");
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("data source command cannot be empty"));

    // Resolver empty string
    let json_empty_res = r#"{
        "meta": { "tool": "test", "version": 1 },
        "operations": [{
            "command": "test-cmd", "description": "desc", "group": "g", "parameters": [
                {
                    "name": "token", "description": "d", "type": "Enum",
                    "data_source": {
                        "resolver": ""
                    }
                }
            ]
        }]
    }"#;
    let res = Schema::from_json(json_empty_res);
    assert!(
        res.is_err(),
        "Expected error for empty data source resolver"
    );
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("data source resolver cannot be empty"));
}

#[test]
fn test_serialize_schema() {
    let schema = Schema {
        meta: SchemaMeta {
            tool: "test".to_string(),
            version: 2,
            author: Some("Tester".to_string()),
            generated_by: None,
            generated_with: None,
            verified: false,
            verified_at: None,
            coverage: None,
            waz_version: None,
            requires_file: None,
            requires_file_kind: None,
            requires_binary: None,
            keywords: vec![],
        },
        operations: vec![Operation {
            command: "test run".to_string(),
            description: "runs test".to_string(),
            group: "test".to_string(),
            verified: false,
            parameters: vec![Parameter {
                name: "file".to_string(),
                description: "file target".to_string(),
                required: true,
                parameter_type: ParameterType::File,
                default: None,
                values: None,
                flag: None,
                data_source: None,
            }],
            surface: Surface::Cli,
            effect: Effect::BuildTest,
            risk: Risk::Low,
            approval: Approval::NotRequired,
            evidence: vec![],
            output_policy: OutputPolicyConfig {
                mode: OutputMode::Raw,
                raw_retention: None,
            },
        }],
    };

    let json_str_res = schema.to_json();
    assert!(json_str_res.is_ok());
    let json_str = json_str_res.unwrap();

    // Deserialize it back and ensure it is identical
    let deserialized_res = Schema::from_json(&json_str);
    assert!(deserialized_res.is_ok());
    let deserialized = deserialized_res.unwrap();
    assert_eq!(deserialized, schema);
}

#[test]
fn test_export_shareable() {
    let schema = Schema {
        meta: SchemaMeta {
            tool: "test".to_string(),
            version: 1,
            author: None,
            generated_by: None,
            generated_with: None,
            verified: false,
            verified_at: None,
            coverage: None,
            waz_version: None,
            requires_file: None,
            requires_file_kind: None,
            requires_binary: None,
            keywords: vec![],
        },
        operations: vec![Operation {
            command: "test run".to_string(),
            description: "runs test".to_string(),
            group: "test".to_string(),
            verified: false,
            parameters: vec![
                Parameter {
                    name: "dynamic-token".to_string(),
                    description: "token with source".to_string(),
                    required: false,
                    parameter_type: ParameterType::Enum,
                    default: None,
                    values: Some(vec!["val1".to_string(), "val2".to_string()]),
                    flag: None,
                    data_source: Some(DataSource {
                        command: None,
                        resolver: Some("test:resolver".to_string()),
                        parse: "lines".to_string(),
                    }),
                },
                Parameter {
                    name: "static-token".to_string(),
                    description: "token without source".to_string(),
                    required: false,
                    parameter_type: ParameterType::Enum,
                    default: None,
                    values: Some(vec!["valA".to_string(), "valB".to_string()]),
                    flag: None,
                    data_source: None,
                },
            ],
            surface: Surface::Cli,
            effect: Effect::BuildTest,
            risk: Risk::Low,
            approval: Approval::NotRequired,
            evidence: vec![],
            output_policy: OutputPolicyConfig {
                mode: OutputMode::Raw,
                raw_retention: None,
            },
        }],
    };

    let shareable = schema.export_shareable();

    // The first parameter (with data_source) should have values set to None
    let param1 = &shareable.operations[0].parameters[0];
    assert_eq!(param1.name, "dynamic-token");
    assert!(
        param1.values.is_none(),
        "Expected resolved values to be stripped"
    );

    // The second parameter (without data_source) should retain its values
    let param2 = &shareable.operations[0].parameters[1];
    assert_eq!(param2.name, "static-token");
    assert_eq!(
        param2.values,
        Some(vec!["valA".to_string(), "valB".to_string()])
    );
}

#[test]
fn test_validation_invalid_tool_characters() {
    let json_dots = r#"{
        "meta": {
            "tool": "../../bad",
            "version": 1
        },
        "operations": []
    }"#;
    let res = Schema::from_json(json_dots);
    assert!(res.is_err(), "Expected error for tool with dot/slashes");
    assert!(
        res.unwrap_err().to_string().contains("alphanumeric"),
        "Error message should mention 'alphanumeric'"
    );

    let json_slash = r#"{
        "meta": {
            "tool": "git/bad",
            "version": 1
        },
        "operations": []
    }"#;
    let res = Schema::from_json(json_slash);
    assert!(res.is_err(), "Expected error for tool with slash");
    assert!(
        res.unwrap_err().to_string().contains("alphanumeric"),
        "Error message should mention 'alphanumeric'"
    );
}
