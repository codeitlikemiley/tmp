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

    assert_eq!(schema.commands.len(), 1);
    let cmd = &schema.commands[0];
    assert_eq!(cmd.command, "git commit");
    assert_eq!(cmd.tokens.len(), 2);

    let token1 = &cmd.tokens[0];
    assert_eq!(token1.name, "message");
    assert_eq!(token1.token_type, TokenType::String);
    assert!(token1.required);
    assert_eq!(token1.flag, Some("-m".to_string()));
    assert!(token1.data_source.is_none());

    let token2 = &cmd.tokens[1];
    assert_eq!(token2.name, "branch");
    assert_eq!(token2.token_type, TokenType::Enum);
    assert!(!token2.required);
    assert!(token2.data_source.is_some());
    let ds = token2.data_source.as_ref().unwrap();
    assert_eq!(ds.resolver, Some("git:branches".to_string()));
    // Check that parse mode defaulted to "lines"
    assert_eq!(ds.parse, "lines");
}

#[test]
fn test_validation_empty_tool() {
    let json = r#"{
        "meta": {
            "tool": "",
            "version": 1
        },
        "commands": []
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
        "commands": [
            {
                "command": "   ",
                "description": "empty command",
                "group": "test",
                "tokens": []
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
        "commands": [{
            "command": "test-cmd", "description": "desc", "group": "g", "tokens": [
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
        "commands": [{
            "command": "test-cmd", "description": "desc", "group": "g", "tokens": [
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
        "commands": [{
            "command": "test-cmd", "description": "desc", "group": "g", "tokens": [
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
        "commands": [{
            "command": "test-cmd", "description": "desc", "group": "g", "tokens": [
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
        "commands": [{
            "command": "test-cmd", "description": "desc", "group": "g", "tokens": [
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
        "commands": [{
            "command": "test-cmd", "description": "desc", "group": "g", "tokens": [
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
        "commands": [{
            "command": "test-cmd", "description": "desc", "group": "g", "tokens": [
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
        commands: vec![Command {
            command: "test run".to_string(),
            description: "runs test".to_string(),
            group: "test".to_string(),
            verified: false,
            tokens: vec![Token {
                name: "file".to_string(),
                description: "file target".to_string(),
                required: true,
                token_type: TokenType::File,
                default: None,
                values: None,
                flag: None,
                data_source: None,
            }],
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
        commands: vec![Command {
            command: "test run".to_string(),
            description: "runs test".to_string(),
            group: "test".to_string(),
            verified: false,
            tokens: vec![
                Token {
                    name: "dynamic-token".to_string(),
                    description: "token with source".to_string(),
                    required: false,
                    token_type: TokenType::Enum,
                    default: None,
                    values: Some(vec!["val1".to_string(), "val2".to_string()]),
                    flag: None,
                    data_source: Some(DataSource {
                        command: None,
                        resolver: Some("test:resolver".to_string()),
                        parse: "lines".to_string(),
                    }),
                },
                Token {
                    name: "static-token".to_string(),
                    description: "token without source".to_string(),
                    required: false,
                    token_type: TokenType::Enum,
                    default: None,
                    values: Some(vec!["valA".to_string(), "valB".to_string()]),
                    flag: None,
                    data_source: None,
                },
            ],
        }],
    };

    let shareable = schema.export_shareable();

    // The first token (with data_source) should have values set to None
    let token1 = &shareable.commands[0].tokens[0];
    assert_eq!(token1.name, "dynamic-token");
    assert!(
        token1.values.is_none(),
        "Expected resolved values to be stripped"
    );

    // The second token (without data_source) should retain its values
    let token2 = &shareable.commands[0].tokens[1];
    assert_eq!(token2.name, "static-token");
    assert_eq!(
        token2.values,
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
        "commands": []
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
        "commands": []
    }"#;
    let res = Schema::from_json(json_slash);
    assert!(res.is_err(), "Expected error for tool with slash");
    assert!(
        res.unwrap_err().to_string().contains("alphanumeric"),
        "Error message should mention 'alphanumeric'"
    );
}
