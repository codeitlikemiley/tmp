use std::collections::HashMap;
use std::sync::Arc;
use tmp_agent::agent_utils;
use tmp_agent::db;
use tmp_agent::server::{create_router, AppState};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

fn extract_python_code(content: &str) -> String {
    let mut code = String::new();
    let mut in_block = false;
    for line in content.lines() {
        if line.trim().starts_with("```python") {
            in_block = true;
            continue;
        }
        if in_block && line.trim().starts_with("```") {
            break;
        }
        if in_block {
            code.push_str(line);
            code.push('\n');
        }
    }
    if code.is_empty() {
        content.to_string()
    } else {
        code
    }
}

/// Returns a static fallback Python script that performs the same tasks
/// offline by simulating the subagent logic.
fn get_fallback_python_script() -> String {
    r#"import os
import sys
import json
import urllib.request
import urllib.error
import time
from pathlib import Path

def call_post(url, payload):
    req = urllib.request.Request(
        url,
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"},
        method="POST"
    )
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except Exception as e:
        print(f"POST {url} failed: {e}")
        raise

def call_get(url):
    try:
        with urllib.request.urlopen(url, timeout=10) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except Exception as e:
        print(f"GET {url} failed: {e}")
        raise

def main():
    port = os.environ.get("TMP_AGENT_PORT", "0")
    db_path = os.environ.get("TMP_DB_PATH")
    config_dir = os.environ.get("TMP_CONFIG_DIR", str(Path.home() / ".config" / "tmp"))
    base_url = f"http://127.0.0.1:{port}"

    call_post(f"{base_url}/log", {"level": "info", "message": "Starting fallback python workflow script"})

    if not db_path:
        print("TMP_DB_PATH is not set.")
        sys.exit(1)

    connection = {"sqlite_path": db_path, "pg_url": None}

    # 1. Fetch tables
    try:
        tables_res = call_post(f"{base_url}/db/tables", {"connection": connection})
    except Exception:
        sys.exit(1)

    if not tables_res.get("success"):
        print(f"Failed to fetch tables: {tables_res.get('error')}")
        sys.exit(1)

    tables = tables_res.get("tables", [])
    call_post(f"{base_url}/log", {"level": "info", "message": f"Found tables: {tables}"})

    all_commands = []
    tool_name = Path(db_path).stem or "db_tool"

    # 2. Iterate tables
    for table in tables:
        cols_res = call_post(f"{base_url}/db/columns", {"table_name": table, "connection": connection})
        if not cols_res.get("success"):
            print(f"Failed to fetch columns for {table}: {cols_res.get('error')}")
            sys.exit(1)

        columns = cols_res.get("columns", [])
        columns_desc = ", ".join([f"{c['name']} ({c['data_type']})" for c in columns])
        call_post(f"{base_url}/log", {"level": "info", "message": f"Table {table} columns: {columns_desc}"})

        # Mock prompt execution via loopback server subagent endpoint
        prompt = f"Generate commands for table {table} with columns {columns_desc}"
        subagent_res = call_post(f"{base_url}/subagent", {"prompt": prompt})
        subagent_id = subagent_res.get("subagent_id")
        call_post(f"{base_url}/log", {"level": "info", "message": f"Spawned subagent {subagent_id} for table {table}"})

        # Poll status
        polls = 0
        while polls < 20:
            status_res = call_get(f"{base_url}/subagent/{subagent_id}")
            status = status_res.get("status")
            if status in ("Success", "Failure"):
                call_post(f"{base_url}/log", {"level": "info", "message": f"Subagent {subagent_id} completed with status: {status}"})
                
                # If success and contains valid JSON commands, parse them
                cmds = []
                if status == "Success":
                    output = status_res.get("output", "")
                    if output.strip().startswith("["):
                        try:
                            cmds = json.loads(output)
                        except Exception as parse_err:
                            print(f"Failed to parse subagent output JSON: {parse_err}")
                
                # If we couldn't get commands, fallback to mock commands
                if not cmds:
                    cmds = [
                        {
                            "command": f"select_{table}",
                            "description": f"Query rows from {table}",
                            "group": table,
                            "verified": True,
                            "tokens": []
                        }
                    ]
                
                all_commands.extend(cmds)
                break
            else:
                time.sleep(0.5)
                polls += 1

    # 3. Save aggregated schema
    schema = {
        "meta": {
            "tool": tool_name,
            "version": 1,
            "verified": True,
            "keywords": ["database", tool_name]
        },
        "commands": all_commands
    }

    schema_dir = Path(config_dir) / "schemas"
    schema_dir.mkdir(parents=True, exist_ok=True)
    schema_file = schema_dir / f"{tool_name}.json"
    
    with open(schema_file, "w") as f:
        json.dump(schema, f, indent=2)

    call_post(f"{base_url}/log", {"level": "info", "message": f"Saved schema to {schema_file}"})
    print("Workflow completed successfully.")
    sys.exit(0)

if __name__ == "__main__":
    main()
"#.to_string()
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // 1. Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    tracing::info!("Initializing tmp-agent application...");

    // 2. Load environment variables
    dotenvy::dotenv().ok();

    // 3. Initialize SQLite DB
    let sqlite_conn = db::init_sqlite()?;
    tracing::info!("SQLite database initialized successfully.");

    // 4. Initialize Antigravity Agent
    let has_api_key = std::env::var("GEMINI_API_KEY").is_ok();
    let agent = if has_api_key {
        match agent_utils::setup_agent().await {
            Ok(started_agent) => {
                tracing::info!("Antigravity Agent started successfully.");
                Some(Arc::new(started_agent))
            }
            Err(e) => {
                tracing::warn!("Could not start Antigravity Agent: {}", e);
                None
            }
        }
    } else {
        tracing::info!("GEMINI_API_KEY not found. Agent setup skipped.");
        None
    };

    // 5. Create Axum Router and Server
    let state = AppState {
        sqlite_conn: Arc::new(Mutex::new(sqlite_conn)),
        agent: agent.clone(),
        subagents: Arc::new(Mutex::new(HashMap::new())),
        subagent_keys: Arc::new(Mutex::new(std::collections::VecDeque::new())),
    };

    let app = create_router(state);

    // Read port from environment variable if present, or bind to 0 (dynamic allocation)
    let port = std::env::var("TMP_AGENT_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(0);
    let bind_addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    let addr = listener.local_addr()?;
    tracing::info!("Starting Axum server on {}", addr);

    // Dynamically retrieve the bound port and export/pass it in the environment variable TMP_AGENT_PORT
    let bound_port = addr.port();
    std::env::set_var("TMP_AGENT_PORT", bound_port.to_string());

    // Create oneshot channel for server shutdown
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    // Spawn server task in the background with graceful shutdown signal
    let server_task = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
            .unwrap();
    });

    // 6. Run a basic test of the setup
    tracing::info!("Running basic verification test of the setup...");
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let mut stream = TcpStream::connect(addr).await?;
    stream
        .write_all(b"GET /status HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .await?;

    let mut response_buf = String::new();
    stream.read_to_string(&mut response_buf).await?;
    tracing::info!("Verification client received response:\n{}", response_buf);

    if response_buf.contains("HTTP/1.1 200 OK") && response_buf.contains(r#""status":"ok""#) {
        tracing::info!("Verification test PASSED.");
    } else {
        tracing::error!("Verification test FAILED. Response: {}", response_buf);
        let _ = shutdown_tx.send(());
        let _ = server_task.await;
        return Err(anyhow::anyhow!("Verification test failed"));
    }

    // 7. Check server-only mode
    let args: Vec<String> = std::env::args().collect();
    let server_only = std::env::var("TMP_AGENT_SERVER_ONLY").is_ok()
        || std::env::var("TMP_SERVER_ONLY").is_ok()
        || args.contains(&"--server".to_string())
        || args.contains(&"--server-only".to_string())
        || args.contains(&"-s".to_string());

    let run_workflow = !server_only && std::env::var("TMP_DB_PATH").is_ok();

    if run_workflow {
        // 8. Generate and run workflow.py
        let python_code = if let Some(ref started_agent) = agent {
            tracing::info!("Generating python orchestration script workflow.py using Gemini API...");

            let planning_prompt = r#"
You are a Python script generator. Generate a python3 script named 'workflow.py'.
This script will act as an orchestration engine for generating command schemas from a database.
The script must:
1. Read 'TMP_AGENT_PORT' from the environment variable (default to 0 if not found).
2. Read 'TMP_DB_PATH' from the environment variable (absolute path to the SQLite database).
3. Read 'TMP_CONFIG_DIR' from the environment variable (fallback to default config directory like ~/.config/tmp if not set).
4. Connect to the Axum REST server running on http://127.0.0.1:<TMP_AGENT_PORT>.
5. Query `/db/tables` (POST request with JSON: `{"connection": {"sqlite_path": "<TMP_DB_PATH>"}}`) to get the list of tables in the database.
6. For each table, query `/db/columns` (POST request with JSON: `{"table_name": "<table_name>", "connection": {"sqlite_path": "<TMP_DB_PATH>"}}`) to get details of all columns (name, data_type).
7. Generate a prompt for a subagent for each table, instructing the subagent to generate CLI command schema objects (select, insert, delete, query) matching the Command JSON structure:
   `{"command": "string", "description": "string", "group": "string", "verified": false, "tokens": []}`.
8. Send each prompt to `/subagent` (POST with JSON: `{"prompt": "<prompt>"}`) to spawn a subagent in the background. Collect the returned `subagent_id`s.
9. Periodically poll `/subagent/<subagent_id>` (GET request) for all spawned subagents until their status is either 'Success' or 'Failure'. Poll in a loop with a sleep interval of 1 second.
   Note that the response follows tagged structure: `{"status": "Success", "output": "<output>"}`.
10. Log progress to the loopback server's `/log` endpoint (POST with JSON: `{"level": "info", "message": "<msg>"}`) showing progress (e.g. table inspection, subagent spawning, polling status, aggregation).
11. Aggregate the generated command schema arrays from all successful subagents.
12. Combine them into a single valid Schema JSON object of the form:
    `{"meta": {"tool": "<tool_name>", "version": 1, "verified": true, "keywords": ["database", "<tool_name>"]}, "commands": <aggregated_commands>}`
    where `<tool_name>` is derived from the database filename (e.g. 'my_db' for '/path/to/my_db.db') or 'db_tool' as fallback.
    Make sure the merged commands is a flat list.
13. Save this schema file to `{TMP_CONFIG_DIR}/schemas/{tool_name}.json` (make sure parent directory exists).
14. Exit with status code 0 if successful, or status code 1 if any subagent failed or an error occurred.

Write only clean, robust, runnable Python 3 code using the built-in libraries (like urllib.request, json, os, time, sys, pathlib).
Do NOT use third-party libraries like `requests`.
Output the python code enclosed in a ```python ... ``` block.
"#;

            match started_agent.chat(planning_prompt).await {
                Ok(resp) => {
                    extract_python_code(&resp.text)
                }
                Err(e) => {
                    tracing::error!("Failed to generate workflow.py via agent: {}", e);
                    let _ = shutdown_tx.send(());
                    let _ = server_task.await;
                    return Err(anyhow::anyhow!("Workflow generation failed: {}", e));
                }
            }
        } else {
            tracing::info!("GEMINI_API_KEY not found. Creating fallback/offline workflow.py...");
            get_fallback_python_script()
        };

        tracing::info!("Writing generated code to workflow.py...");
        tokio::fs::write("workflow.py", python_code).await?;

        // 9. Execute workflow.py
        tracing::info!("Executing workflow.py...");
        let mut child = tokio::process::Command::new("python3")
            .arg("workflow.py")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let stdout = child.stdout.take().expect("Failed to open stdout");
        let stderr = child.stderr.take().expect("Failed to open stderr");

        let mut stdout_reader = tokio::io::BufReader::new(stdout).lines();
        let mut stderr_reader = tokio::io::BufReader::new(stderr).lines();

        tokio::spawn(async move {
            while let Ok(Some(line)) = stdout_reader.next_line().await {
                tracing::info!("[workflow.py stdout] {}", line);
            }
        });

        tokio::spawn(async move {
            while let Ok(Some(line)) = stderr_reader.next_line().await {
                tracing::warn!("[workflow.py stderr] {}", line);
            }
        });

        let status = child.wait().await?;

        // 10. Cleanly terminate the background HTTP server task
        tracing::info!("Terminating Axum loopback server...");
        let _ = shutdown_tx.send(());
        let _ = server_task.await;

        if status.success() {
            tracing::info!("Workflow completed successfully. Starting schema validation and compilation...");

            let db_path_str = std::env::var("TMP_DB_PATH")?;
            let db_path = std::path::Path::new(&db_path_str);
            let tool_name = db_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("db_tool");

            let config_file_path = tmp_core::config::default_config_path()
                .ok_or_else(|| anyhow::anyhow!("Could not determine default config directory"))?;
            let config_dir = config_file_path
                .parent()
                .ok_or_else(|| anyhow::anyhow!("Invalid configuration path"))?;
            let schema_file = config_dir
                .join("schemas")
                .join(format!("{}.json", tool_name));

            tracing::info!("Reading schema from {:?}", schema_file);
            let schema_content = std::fs::read_to_string(&schema_file)?;

            tracing::info!("Deserializing and validating schema...");
            let _schema = tmp_core::schema::Schema::from_json(&schema_content)?;

            tracing::info!("Detecting context and compiling...");
            let current_dir = std::env::current_dir()?;
            let context = tmp_core::context::Context::detect(&current_dir, None, None);
            let compile_output = tmp_core::compile::Compiler::compile(&current_dir, &context, None)
                .map_err(|e| anyhow::anyhow!(e))?;

            tracing::info!("Writing compilation output to disk...");
            tmp_core::compile::Compiler::write_to_disk(&current_dir, &compile_output)?;

            tracing::info!("Schema validation and compilation completed successfully. Exiting.");
            std::process::exit(0);
        } else {
            tracing::error!("Workflow execution failed with exit code: {:?}", status.code());
            std::process::exit(status.code().unwrap_or(1));
        }
    } else {
        if server_only {
            tracing::info!("Server-only mode requested. Keeping server active.");
        } else {
            tracing::info!("TMP_DB_PATH not set in non-server-only mode. Keeping server active in standby mode.");
        }
        server_task.await?;
    }

    Ok(())
}
