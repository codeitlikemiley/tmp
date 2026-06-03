use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::Mutex;
use tmp_agent::server::{AppState, create_router};
use tmp_core::schema::Schema;

struct TestCleanup {
    temp_dir: std::path::PathBuf,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Drop for TestCleanup {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if self.temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&self.temp_dir);
        }
    }
}

const PYTHON_SCRIPT_CONTENT: &str = r#"import os
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
    with urllib.request.urlopen(req, timeout=5) as resp:
        return json.loads(resp.read().decode("utf-8"))

def call_get(url):
    with urllib.request.urlopen(url, timeout=5) as resp:
        return json.loads(resp.read().decode("utf-8"))

def main():
    port = os.environ.get("TMP_AGENT_PORT")
    db_path = os.environ.get("TMP_DB_PATH")
    config_dir = os.environ.get("TMP_CONFIG_DIR")
    base_url = f"http://127.0.0.1:{port}"

    # 1. Log starting
    call_post(f"{base_url}/log", {"level": "info", "message": "Python integration test starting"})

    # 2. Get tables
    connection = {"sqlite_path": db_path, "pg_url": None}
    tables_res = call_post(f"{base_url}/db/tables", {"connection": connection})
    if not tables_res.get("success"):
        print("Failed to fetch tables", file=sys.stderr)
        sys.exit(1)
    tables = tables_res.get("tables", [])
    if "test_users" not in tables:
        print(f"Expected test_users table, but got: {tables}", file=sys.stderr)
        sys.exit(1)
    
    # 3. Get columns for each table
    for table in tables:
        cols_res = call_post(f"{base_url}/db/columns", {"table_name": table, "connection": connection})
        if not cols_res.get("success"):
            print(f"Failed to fetch columns for {table}", file=sys.stderr)
            sys.exit(1)
            
    # 4. Call /subagent
    subagent_res = call_post(f"{base_url}/subagent", {"prompt": "Generate schema"})
    subagent_id = subagent_res.get("subagent_id")
    
    # Poll subagent status
    success = False
    status = "Unknown"
    for _ in range(100):
        time.sleep(0.1)
        try:
            status_res = call_get(f"{base_url}/subagent/{subagent_id}")
            status = status_res.get("status")
            if status in ("Success", "Failure"):
                success = True
                break
        except Exception as e:
            print(f"Failed to get subagent status: {e}", file=sys.stderr)
            
    if not success:
        print(f"Subagent polling timed out or failed to reach terminal state. Last status: {status}", file=sys.stderr)
        sys.exit(1)

    # 5. Call /execute
    exec_res = call_post(f"{base_url}/execute", {"command": "echo", "args": ["hello-integration-test"], "cwd": None})
    if not exec_res.get("success") or "hello-integration-test" not in exec_res.get("stdout", ""):
        print("Execute check failed", file=sys.stderr)
        sys.exit(1)

    # 6. Save schema
    schema = {
        "meta": {
            "tool": "test_db",
            "version": 1,
            "verified": True,
            "keywords": ["database", "test_db"]
        },
        "commands": [
            {
                "command": "select_test_users",
                "description": "Query rows from test_users",
                "group": "test_users",
                "verified": True,
                "tokens": []
            }
        ]
    }
    schema_dir = Path(config_dir) / "schemas"
    schema_dir.mkdir(parents=True, exist_ok=True)
    with open(schema_dir / "test_db.json", "w") as f:
        json.dump(schema, f)

    sys.exit(0)

if __name__ == "__main__":
    main()
"#;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_integration_workflow() {
    // Remove GEMINI_API_KEY to ensure subagent fails immediately and deterministically offline
    std::env::remove_var("GEMINI_API_KEY");

    // 1. Create unique temporary directory
    let test_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("tmp_agent_test_{}", test_id));
    std::fs::create_dir_all(&temp_dir).expect("Failed to create temporary directory");

    // 2. Setup SQLite DB inside it
    let db_path = temp_dir.join("test_db.db");
    {
        let conn = rusqlite::Connection::open(&db_path).expect("Failed to open sqlite db");
        conn.execute(
            "CREATE TABLE test_users (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL
            )",
            [],
        ).expect("Failed to create test_users table");
        conn.execute(
            "INSERT INTO test_users (name) VALUES ('Alice')",
            [],
        ).expect("Failed to insert Alice");
        conn.execute(
            "INSERT INTO test_users (name) VALUES ('Bob')",
            [],
        ).expect("Failed to insert Bob");
    }

    // 3. Start Axum server on dynamic port
    let sqlite_conn = rusqlite::Connection::open_in_memory().expect("Failed to init state sqlite");
    let state = AppState {
        sqlite_conn: Arc::new(Mutex::new(sqlite_conn)),
        agent: None,
        subagents: Arc::new(Mutex::new(HashMap::new())),
        subagent_keys: Arc::new(Mutex::new(VecDeque::new())),
    };
    let app = create_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind TcpListener");
    let addr = listener.local_addr().expect("Failed to get local address");
    let port = addr.port();

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    
    // Set up cleanup guard
    let mut cleanup = TestCleanup {
        temp_dir: temp_dir.clone(),
        shutdown_tx: Some(shutdown_tx),
    };

    let server_task = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
            .unwrap();
    });

    // 4. Create test_client.py
    let script_path = temp_dir.join("test_client.py");
    std::fs::write(&script_path, PYTHON_SCRIPT_CONTENT).expect("Failed to write python script");

    // 5. Run Python client script
    let script_path_clone = script_path.clone();
    let db_path_clone = db_path.clone();
    let temp_dir_clone = temp_dir.clone();
    let status = tokio::task::spawn_blocking(move || {
        std::process::Command::new("python3")
            .arg(&script_path_clone)
            .env("TMP_AGENT_PORT", port.to_string())
            .env("TMP_DB_PATH", db_path_clone.to_str().unwrap())
            .env("TMP_CONFIG_DIR", temp_dir_clone.to_str().unwrap())
            .status()
            .expect("Failed to run python client script")
    })
    .await
    .expect("Spawn blocking panicked");

    assert!(status.success(), "Python client script exited with non-zero code");

    // 6. Read generated schema and verify
    let schema_file = temp_dir.join("schemas/test_db.json");
    assert!(schema_file.exists(), "Expected schema file was not generated");
    
    let schema_content = std::fs::read_to_string(&schema_file).expect("Failed to read schema file");
    let parsed_schema = Schema::from_json(&schema_content).expect("Failed to parse schema JSON");

    assert_eq!(parsed_schema.meta.tool, "test_db");
    assert_eq!(parsed_schema.commands.len(), 1);
    assert_eq!(parsed_schema.commands[0].command, "select_test_users");
    assert_eq!(parsed_schema.commands[0].group, "test_users");

    // 7. Gracefully shutdown server
    if let Some(tx) = cleanup.shutdown_tx.take() {
        let _ = tx.send(());
    }
    server_task.await.expect("Server task panicked or failed to join");

    // Temp directory and database cleanup is handled by Drop of cleanup
}
