use super::*;
use std::fs;

#[test]
fn test_registry_fetch_and_search() {
    let temp_dir = tempfile::tempdir().unwrap();
    let index_path = temp_dir.path().join("index.json");

    let mock_index = RegistryIndex {
        schemas: vec![
            RegistrySchemaMeta {
                tool: "cargo".to_string(),
                version: "0.1.0".to_string(),
                author: "Test Author".to_string(),
                commands_count: 5,
                verified: true,
                download_url: "file:///mock/cargo.json".to_string(),
                description: Some("Cargo build tool schema".to_string()),
            },
            RegistrySchemaMeta {
                tool: "npm".to_string(),
                version: "1.0.0".to_string(),
                author: "NPM Author".to_string(),
                commands_count: 3,
                verified: false,
                download_url: "file:///mock/npm.json".to_string(),
                description: Some("Node Package Manager".to_string()),
            },
        ],
    };

    let index_json = serde_json::to_string(&mock_index).unwrap();
    fs::write(&index_path, index_json).unwrap();

    let repo_url = format!("file://{}", index_path.to_str().unwrap());
    let client = RegistryClient::new(&repo_url);

    // Test fetch index
    let fetched = client.fetch_index().unwrap();
    assert_eq!(fetched.schemas.len(), 2);

    // Test search by tool name
    let cargo_results = client.search("cargo").unwrap();
    assert_eq!(cargo_results.len(), 1);
    assert_eq!(cargo_results[0].tool, "cargo");

    // Test search by description
    let npm_results = client.search("package").unwrap();
    assert_eq!(npm_results.len(), 1);
    assert_eq!(npm_results[0].tool, "npm");

    // Test search case insensitivity
    let cap_results = client.search("NPM").unwrap();
    assert_eq!(cap_results.len(), 1);
    assert_eq!(cap_results[0].tool, "npm");
}

#[test]
fn test_registry_install() {
    let temp_dir = tempfile::tempdir().unwrap();

    // Create mock schema file
    let schema_content = r#"{"tool": "cargo", "commands": []}"#;
    let schema_file_path = temp_dir.path().join("cargo_schema.json");
    fs::write(&schema_file_path, schema_content).unwrap();

    // Create mock index file referencing the mock schema file
    let download_url = format!("file://{}", schema_file_path.to_str().unwrap());
    let mock_index = RegistryIndex {
        schemas: vec![RegistrySchemaMeta {
            tool: "cargo".to_string(),
            version: "0.1.0".to_string(),
            author: "Test Author".to_string(),
            commands_count: 0,
            verified: true,
            download_url,
            description: None,
        }],
    };

    let index_file_path = temp_dir.path().join("index.json");
    let index_json = serde_json::to_string(&mock_index).unwrap();
    fs::write(&index_file_path, index_json).unwrap();

    let repo_url = format!("file://{}", index_file_path.to_str().unwrap());
    let client = RegistryClient::new(&repo_url);

    // Target directory where schema will be installed
    let install_dir = temp_dir.path().join("installed_schemas");

    client.install("cargo", &install_dir).unwrap();

    client.install("cargo", &install_dir).unwrap();

    // Verify file got written
    let installed_file = install_dir.join("cargo.json");
    assert!(installed_file.exists());
    let content = fs::read_to_string(installed_file).unwrap();
    assert_eq!(content, schema_content);
}

#[test]
fn test_debug_trim() {
    let url = "file:///Volumes/goldcoders/tmp/Cargo.toml";
    let path = url.trim_start_matches("file://");
    println!("trimmed path: {}", path);
    assert_eq!(path, "/Volumes/goldcoders/tmp/Cargo.toml");
}

#[test]
fn test_registry_missing_description() {
    let temp_dir = tempfile::tempdir().unwrap();
    let index_path = temp_dir.path().join("index.json");

    // "description" is missing, which is an Option
    let index_json = r#"{
        "schemas": [
            {
                "tool": "cargo",
                "version": "0.1.0",
                "author": "Test Author",
                "commands_count": 5,
                "verified": true,
                "download_url": "file:///mock/cargo.json"
            }
        ]
    }"#;
    fs::write(&index_path, index_json).unwrap();

    let repo_url = format!("file://{}", index_path.to_str().unwrap());
    let client = RegistryClient::new(&repo_url);

    // Should fetch and deserialize successfully
    let fetched = client.fetch_index().unwrap();
    assert_eq!(fetched.schemas.len(), 1);
    assert_eq!(fetched.schemas[0].description, None);

    // Search should still work
    let results = client.search("cargo").unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn test_registry_missing_required_property() {
    let temp_dir = tempfile::tempdir().unwrap();
    let index_path = temp_dir.path().join("index.json");

    // "verified" and "commands_count" are missing (not options, no defaults)
    let index_json = r#"{
        "schemas": [
            {
                "tool": "cargo",
                "version": "0.1.0",
                "author": "Test Author",
                "download_url": "file:///mock/cargo.json"
            }
        ]
    }"#;
    fs::write(&index_path, index_json).unwrap();

    let repo_url = format!("file://{}", index_path.to_str().unwrap());
    let client = RegistryClient::new(&repo_url);

    // fetch should fail due to missing required property
    let res = client.fetch_index();
    assert!(res.is_err());
    if let Err(RegistryError::Json(err)) = res {
        assert!(err.to_string().contains("missing field"));
    } else {
        panic!("Expected JSON parsing error");
    }
}

#[test]
fn test_registry_offline_behavior() {
    // Attempting to query an unreachable HTTP domain to simulate network outage
    let client = RegistryClient::new("http://unreachable-offline-test-12345.local/index.json");
    let res = client.fetch_index();
    assert!(res.is_err());
    if let Err(RegistryError::Http(err)) = res {
        // Assert it failed due to some network connectivity issue
        assert!(!err.is_empty());
    } else {
        panic!("Expected HTTP error when offline");
    }
}

#[test]
fn test_registry_malformed_index_json() {
    let temp_dir = tempfile::tempdir().unwrap();
    let index_path = temp_dir.path().join("index.json");
    fs::write(&index_path, "not a valid json {{{{").unwrap();

    let repo_url = format!("file://{}", index_path.to_str().unwrap());
    let client = RegistryClient::new(&repo_url);

    let res = client.fetch_index();
    assert!(res.is_err());
    assert!(matches!(res.unwrap_err(), RegistryError::Json(_)));
}

#[test]
fn test_registry_install_traversal() {
    let temp_dir = tempfile::tempdir().unwrap();
    let client = RegistryClient::new("file:///mock/index.json");
    let install_dir = temp_dir.path().join("installed_schemas");

    let res = client.install("../path/to/tool", &install_dir);
    assert!(res.is_err());
    assert!(matches!(res.unwrap_err(), RegistryError::ToolNotFound(_)));

    let res = client.install("", &install_dir);
    assert!(res.is_err());
    assert!(matches!(res.unwrap_err(), RegistryError::ToolNotFound(_)));

    let res = client.install("tool@name", &install_dir);
    assert!(res.is_err());
    assert!(matches!(res.unwrap_err(), RegistryError::ToolNotFound(_)));
}

#[test]
fn test_registry_local_file_disclosure_via_download_url() {
    let temp_dir = tempfile::tempdir().unwrap();

    // 1. Create a "sensitive" local JSON file that shouldn't be accessible by the registry client
    let sensitive_content = r#"{"aws_access_key_id": "AKIAIOSFODNN7EXAMPLE"}"#;
    let sensitive_file_path = temp_dir.path().join("sensitive_secrets.json");
    fs::write(&sensitive_file_path, sensitive_content).unwrap();

    // 2. Create mock index file referencing the sensitive file path via file:// url
    let download_url = format!("file://{}", sensitive_file_path.to_str().unwrap());
    let mock_index = RegistryIndex {
        schemas: vec![RegistrySchemaMeta {
            tool: "malicious".to_string(),
            version: "0.1.0".to_string(),
            author: "hacker".to_string(),
            commands_count: 0,
            verified: true,
            download_url,
            description: None,
        }],
    };

    let index_file_path = temp_dir.path().join("index.json");
    let index_json = serde_json::to_string(&mock_index).unwrap();
    fs::write(&index_file_path, index_json).unwrap();

    // The repo registry url itself is a remote/mock registry
    let repo_url = format!("file://{}", index_file_path.to_str().unwrap());
    let client = RegistryClient::new(&repo_url);

    let install_dir = temp_dir.path().join("installed_schemas");

    // Install the "malicious" tool
    let res = client.install("malicious", &install_dir);
    assert!(
        res.is_ok(),
        "Client should not fail to install if it permits file:// URLs in metadata"
    );

    // Verify the sensitive content was copied into the schemas folder
    let installed_file = install_dir.join("malicious.json");
    assert!(installed_file.exists());
    let copied_content = fs::read_to_string(installed_file).unwrap();
    assert_eq!(
        copied_content, sensitive_content,
        "The sensitive local file was disclosed and copied!"
    );
}

#[test]
fn test_registry_install_absolute_path_traversal() {
    let temp_dir = tempfile::tempdir().unwrap();
    let client = RegistryClient::new("file:///mock/index.json");
    let install_dir = temp_dir.path().join("installed_schemas");

    let res = client.install("/absolute/path/to/tool", &install_dir);
    assert!(res.is_err());
    assert!(matches!(res.unwrap_err(), RegistryError::ToolNotFound(_)));
}

#[test]
fn test_registry_install_windows_path_traversal() {
    let temp_dir = tempfile::tempdir().unwrap();
    let client = RegistryClient::new("file:///mock/index.json");
    let install_dir = temp_dir.path().join("installed_schemas");

    let res = client.install(r#"..\..\tool"#, &install_dir);
    assert!(res.is_err());
    assert!(matches!(res.unwrap_err(), RegistryError::ToolNotFound(_)));
}

#[test]
fn test_registry_index_missing_schemas_field() {
    let temp_dir = tempfile::tempdir().unwrap();
    let index_path = temp_dir.path().join("index.json");
    fs::write(&index_path, "{}").unwrap();

    let repo_url = format!("file://{}", index_path.to_str().unwrap());
    let client = RegistryClient::new(&repo_url);

    let res = client.fetch_index();
    assert!(res.is_err());
    if let Err(RegistryError::Json(err)) = res {
        assert!(err.to_string().contains("missing field `schemas`"));
    } else {
        panic!("Expected JSON parsing error due to missing schemas field");
    }
}

#[test]
fn test_registry_local_file_disclosure_remote_repo_denied() {
    use std::io::Write;
    use std::net::TcpListener;
    use std::thread;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let temp_dir = tempfile::tempdir().unwrap();
    let sensitive_content = r#"{"aws_access_key_id": "AKIAIOSFODNN7EXAMPLE"}"#;
    let sensitive_file_path = temp_dir.path().join("sensitive_secrets.json");
    fs::write(&sensitive_file_path, sensitive_content).unwrap();

    let download_url = format!("file://{}", sensitive_file_path.to_str().unwrap());
    let mock_index = RegistryIndex {
        schemas: vec![RegistrySchemaMeta {
            tool: "malicious".to_string(),
            version: "0.1.0".to_string(),
            author: "hacker".to_string(),
            commands_count: 0,
            verified: true,
            download_url,
            description: None,
        }],
    };
    let index_json = serde_json::to_string(&mock_index).unwrap();

    // Run simple background server
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            use std::io::Read;
            let mut buffer = [0; 4096];
            let _ = stream.read(&mut buffer);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                index_json.len(),
                index_json
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });

    let repo_url = format!("http://127.0.0.1:{}/index.json", port);
    let client = RegistryClient::new(&repo_url);
    let install_dir = temp_dir.path().join("installed_schemas");

    let res = client.install("malicious", &install_dir);
    assert!(
        res.is_err(),
        "Should fail to install from remote registry with file:// download URL"
    );
    match res {
        Err(RegistryError::Http(err)) => {
            assert_eq!(err, "Local file schema not allowed for remote registry");
        }
        other => {
            panic!(
                "Expected RegistryError::Http with security warning message, but got: {:?}",
                other
            );
        }
    }
}
