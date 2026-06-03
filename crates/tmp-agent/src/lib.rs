pub mod db {
    use anyhow::Result;
    use rusqlite::Connection as SqliteConnection;
    use tokio_postgres::{Client, NoTls};

    /// Initializes a SQLite connection and sets up tables.
    pub fn init_sqlite() -> Result<SqliteConnection> {
        let conn = SqliteConnection::open_in_memory()?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS agents (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                status TEXT NOT NULL
            )",
            [],
        )?;
        Ok(conn)
    }

    /// Initializes a PostgreSQL client connection.
    pub async fn init_postgres(config_str: &str) -> Result<Client> {
        let (client, connection) = tokio_postgres::connect(config_str, NoTls).await?;
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("PostgreSQL connection error: {}", e);
            }
        });
        Ok(client)
    }
}

pub mod agent_utils {
    use antigravity_sdk_rust::agent::{Agent, Started};
    use anyhow::{Context, Result};

    /// Sets up and starts the Antigravity Agent.
    pub async fn setup_agent() -> Result<Agent<Started>> {
        let harness_path = std::env::var("ANTIGRAVITY_HARNESS_PATH").ok().or_else(|| {
            let default_path = "/Volumes/goldcoders/antigravity-sdk-rust/bin/localharness";
            if std::path::Path::new(default_path).exists() {
                Some(default_path.to_string())
            } else {
                None
            }
        });

        let api_key = std::env::var("GEMINI_API_KEY").ok();

        let mut builder = Agent::builder();
        if let Some(path) = harness_path {
            builder = builder.binary_path(path);
        }
        if let Some(key) = api_key {
            builder = builder.api_key(key);
        }

        let agent = builder
            .default_model("gemini-3.5-flash")
            .allow_all()
            .build();

        let started_agent = agent.start().await.context("Failed to start Agent")?;
        Ok(started_agent)
    }
}

pub mod server {
    use antigravity_sdk_rust::agent::{Agent, Started};
    use axum::{
        extract::{Path, State},
        http::StatusCode,
        response::IntoResponse,
        routing::{get, post},
        Json, Router,
    };
    use rusqlite::Connection as SqliteConnection;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use std::collections::VecDeque;
    use std::path::{Path as StdPath, PathBuf};
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(tag = "status", content = "output")]
    pub enum SubagentStatus {
        Running,
        Success(String),
        Failure(String),
    }

    #[derive(Clone)]
    pub struct AppState {
        pub sqlite_conn: Arc<Mutex<SqliteConnection>>,
        pub agent: Option<Arc<Agent<Started>>>,
        pub subagents: Arc<Mutex<HashMap<String, SubagentStatus>>>,
        pub subagent_keys: Arc<Mutex<VecDeque<String>>>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct StatusResponse {
        pub status: String,
        pub agent_active: bool,
    }

    /// Creates the Axum router and binds the state.
    pub fn create_router(state: AppState) -> Router {
        Router::new()
            .route("/status", get(status_handler))
            .route("/chat", post(chat_handler))
            .route("/execute", post(execute_handler))
            .route("/read_file", post(read_file_handler))
            .route("/write_file", post(write_file_handler))
            .route("/subagent", post(subagent_handler))
            .route("/subagent/:id", get(get_subagent_handler))
            .route("/log", post(log_handler))
            .route("/db/tables", post(tables_handler))
            .route("/db/columns", post(columns_handler))
            .route("/db/query", post(query_handler))
            .with_state(state)
    }

    pub async fn status_handler(State(state): State<AppState>) -> Json<StatusResponse> {
        Json(StatusResponse {
            status: "ok".to_string(),
            agent_active: state.agent.is_some(),
        })
    }

    #[derive(Deserialize, Debug, Clone)]
    pub struct ChatRequest {
        pub message: String,
    }

    #[derive(Serialize, Debug, Clone)]
    pub struct ChatResponse {
        pub reply: String,
    }

    pub async fn chat_handler(
        State(state): State<AppState>,
        Json(payload): Json<ChatRequest>,
    ) -> Json<ChatResponse> {
        let reply = if let Some(ref agent) = state.agent {
            match agent.chat(&payload.message).await {
                Ok(resp) => resp.text,
                Err(e) => format!("Agent chat error: {}", e),
            }
        } else {
            format!("Agent not initialized. Echo: {}", payload.message)
        };

        Json(ChatResponse { reply })
    }

    // 1. POST /execute
    #[derive(Deserialize, Debug, Clone)]
    pub struct ExecuteRequest {
        pub command: String,
        pub args: Option<Vec<String>>,
        pub cwd: Option<String>,
    }

    #[derive(Serialize, Debug, Clone)]
    pub struct ExecuteResponse {
        pub success: bool,
        pub exit_code: Option<i32>,
        pub stdout: String,
        pub stderr: String,
    }

    pub(crate) fn validate_and_canonicalize_path(input_path: &str) -> Result<PathBuf, anyhow::Error> {
        let path = StdPath::new(input_path);
        
        let mut existing_ancestor = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()?.join(path)
        };
        
        let mut remaining = Vec::new();
        
        while !existing_ancestor.exists() {
            if let Some(component) = existing_ancestor.components().next_back() {
                remaining.push(component.as_os_str().to_os_string());
                existing_ancestor.pop();
            } else {
                break;
            }
        }
        
        remaining.reverse();
        
        let canonical_ancestor = if existing_ancestor.exists() {
            existing_ancestor.canonicalize()?
        } else {
            std::env::current_dir()?.canonicalize()?
        };
        
        let full_path = remaining.iter().fold(canonical_ancestor, |acc, component| {
            acc.join(component)
        });
        
        // Normalize redundant/relative components (like ., ..)
        let mut normalized = PathBuf::new();
        for component in full_path.components() {
            match component {
                std::path::Component::Prefix(_) => normalized.push(component.as_os_str()),
                std::path::Component::RootDir => normalized.push(component.as_os_str()),
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    normalized.pop();
                }
                std::path::Component::Normal(c) => normalized.push(c),
            }
        }
        
        let workspace_dir = StdPath::new("/Volumes/goldcoders/tmp").canonicalize()
            .unwrap_or_else(|_| PathBuf::from("/Volumes/goldcoders/tmp"));
        let temp_dir = std::env::temp_dir().canonicalize()
            .unwrap_or_else(|_| std::env::temp_dir());
            
        if normalized.starts_with(&workspace_dir) || normalized.starts_with(&temp_dir) {
            Ok(normalized)
        } else {
            Err(anyhow::anyhow!("Path traversal detected: target path is outside allowed sandbox directories"))
        }
    }

    pub async fn execute_handler(
        Json(payload): Json<ExecuteRequest>,
    ) -> Json<ExecuteResponse> {
        let mut cmd = tokio::process::Command::new(&payload.command);
        if let Some(ref args) = payload.args {
            cmd.args(args);
        }
        if let Some(ref cwd) = payload.cwd {
            cmd.current_dir(cwd);
        }
        match cmd.output().await {
            Ok(output) => {
                Json(ExecuteResponse {
                    success: output.status.success(),
                    exit_code: output.status.code(),
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                })
            }
            Err(e) => {
                Json(ExecuteResponse {
                    success: false,
                    exit_code: None,
                    stdout: String::new(),
                    stderr: format!("Failed to execute command: {}", e),
                })
            }
        }
    }

    // 2. POST /read_file
    #[derive(Deserialize, Debug, Clone)]
    pub struct ReadFileRequest {
        pub path: String,
    }

    #[derive(Serialize, Debug, Clone)]
    pub struct ReadFileResponse {
        pub success: bool,
        pub content: Option<String>,
        pub error: Option<String>,
    }

    pub async fn read_file_handler(
        Json(payload): Json<ReadFileRequest>,
    ) -> Json<ReadFileResponse> {
        let validated_path = match validate_and_canonicalize_path(&payload.path) {
            Ok(p) => p,
            Err(e) => {
                return Json(ReadFileResponse {
                    success: false,
                    content: None,
                    error: Some(e.to_string()),
                });
            }
        };
        match tokio::fs::read_to_string(validated_path).await {
            Ok(content) => Json(ReadFileResponse {
                success: true,
                content: Some(content),
                error: None,
            }),
            Err(e) => Json(ReadFileResponse {
                success: false,
                content: None,
                error: Some(e.to_string()),
            }),
        }
    }

    // 3. POST /write_file
    #[derive(Deserialize, Debug, Clone)]
    pub struct WriteFileRequest {
        pub path: String,
        pub content: String,
    }

    #[derive(Serialize, Debug, Clone)]
    pub struct WriteFileResponse {
        pub success: bool,
        pub error: Option<String>,
    }

    pub async fn write_file_handler(
        Json(payload): Json<WriteFileRequest>,
    ) -> Json<WriteFileResponse> {
        let validated_path = match validate_and_canonicalize_path(&payload.path) {
            Ok(p) => p,
            Err(e) => {
                return Json(WriteFileResponse {
                    success: false,
                    error: Some(e.to_string()),
                });
            }
        };
        if let Some(parent) = validated_path.parent() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                return Json(WriteFileResponse {
                    success: false,
                    error: Some(format!("Failed to create parent directories: {}", e)),
                });
            }
        }
        match tokio::fs::write(validated_path, &payload.content).await {
            Ok(_) => Json(WriteFileResponse {
                success: true,
                error: None,
            }),
            Err(e) => Json(WriteFileResponse {
                success: false,
                error: Some(e.to_string()),
            }),
        }
    }

    // 4. POST /subagent
    #[derive(Deserialize, Debug, Clone)]
    pub struct SubagentRequest {
        pub prompt: String,
    }

    #[derive(Serialize, Debug, Clone)]
    pub struct SubagentResponse {
        pub subagent_id: String,
    }

    pub async fn subagent_handler(
        State(state): State<AppState>,
        Json(payload): Json<SubagentRequest>,
    ) -> Json<SubagentResponse> {
        let subagent_id = uuid::Uuid::new_v4().to_string();
        
        {
            let mut keys = state.subagent_keys.lock().await;
            let mut map = state.subagents.lock().await;
            map.insert(subagent_id.clone(), SubagentStatus::Running);
            keys.push_back(subagent_id.clone());
            if keys.len() > 100 {
                if let Some(oldest_key) = keys.pop_front() {
                    map.remove(&oldest_key);
                }
            }
        }

        let subagents_map = state.subagents.clone();
        let subagent_id_clone = subagent_id.clone();
        let prompt = payload.prompt.clone();

        tokio::spawn(async move {
            match crate::agent_utils::setup_agent().await {
                Ok(agent) => {
                    match agent.chat(&prompt).await {
                        Ok(response) => {
                            let mut map = subagents_map.lock().await;
                            if let Some(status) = map.get_mut(&subagent_id_clone) {
                                *status = SubagentStatus::Success(response.text);
                            }
                        }
                        Err(e) => {
                            let mut map = subagents_map.lock().await;
                            if let Some(status) = map.get_mut(&subagent_id_clone) {
                                *status = SubagentStatus::Failure(format!("Agent chat error: {}", e));
                            }
                        }
                    }
                    let _ = agent.stop().await;
                }
                Err(e) => {
                    let mut map = subagents_map.lock().await;
                    if let Some(status) = map.get_mut(&subagent_id_clone) {
                        *status = SubagentStatus::Failure(format!("Agent setup failed: {}", e));
                    }
                }
            }
        });

        Json(SubagentResponse { subagent_id })
    }

    // 5. GET /subagent/:id
    pub async fn get_subagent_handler(
        State(state): State<AppState>,
        Path(id): Path<String>,
    ) -> impl IntoResponse {
        let subagents = state.subagents.lock().await;
        if let Some(status) = subagents.get(&id) {
            (StatusCode::OK, Json(status.clone())).into_response()
        } else {
            (StatusCode::NOT_FOUND, Json(serde_json::json!({
                "error": "Subagent ID not found"
            }))).into_response()
        }
    }

    // 6. POST /log
    #[derive(Deserialize, Debug, Clone)]
    pub struct LogRequest {
        pub level: Option<String>,
        pub message: String,
    }

    #[derive(Serialize, Debug, Clone)]
    pub struct LogResponse {
        pub success: bool,
    }

    pub async fn log_handler(
        Json(payload): Json<LogRequest>,
    ) -> Json<LogResponse> {
        let level = payload.level.as_deref().unwrap_or("info");
        match level.to_lowercase().as_str() {
            "error" => tracing::error!("{}", payload.message),
            "warn" => tracing::warn!("{}", payload.message),
            "debug" => tracing::debug!("{}", payload.message),
            _ => tracing::info!("{}", payload.message),
        }
        Json(LogResponse { success: true })
    }

    // DB payloads
    #[derive(Deserialize, Debug, Clone)]
    pub struct DbConnectionPayload {
        pub sqlite_path: Option<String>,
        pub pg_url: Option<String>,
    }

    // 7. POST /db/tables
    #[derive(Deserialize, Debug, Clone)]
    pub struct TablesRequest {
        pub connection: DbConnectionPayload,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct TablesResponse {
        pub success: bool,
        pub tables: Option<Vec<String>>,
        pub error: Option<String>,
    }

    pub async fn tables_handler(
        Json(payload): Json<TablesRequest>,
    ) -> impl IntoResponse {
        match get_tables(&payload.connection).await {
            Ok(tables) => (StatusCode::OK, Json(TablesResponse {
                success: true,
                tables: Some(tables),
                error: None,
            })).into_response(),
            Err(e) => (StatusCode::BAD_REQUEST, Json(TablesResponse {
                success: false,
                tables: None,
                error: Some(e.to_string()),
            })).into_response(),
        }
    }

    pub(crate) fn strip_sql_comments(sql: &str) -> String {
        let mut result = String::new();
        let mut chars = sql.chars().peekable();
        
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum State {
            Normal,
            SingleQuote,
            DoubleQuote,
            SingleLineComment,
            MultiLineComment,
        }
        
        let mut state = State::Normal;
        
        while let Some(c) = chars.next() {
            match state {
                State::Normal => {
                    if c == '\'' {
                        state = State::SingleQuote;
                        result.push(c);
                    } else if c == '"' {
                        state = State::DoubleQuote;
                        result.push(c);
                    } else if c == '-' && chars.peek() == Some(&'-') {
                        chars.next();
                        state = State::SingleLineComment;
                    } else if c == '/' && chars.peek() == Some(&'*') {
                        chars.next();
                        state = State::MultiLineComment;
                    } else {
                        result.push(c);
                    }
                }
                State::SingleQuote => {
                    result.push(c);
                    if c == '\'' {
                        state = State::Normal;
                    }
                }
                State::DoubleQuote => {
                    result.push(c);
                    if c == '"' {
                        state = State::Normal;
                    }
                }
                State::SingleLineComment => {
                    if c == '\n' || c == '\r' {
                        state = State::Normal;
                        result.push(c);
                    }
                }
                State::MultiLineComment => {
                    if c == '*' && chars.peek() == Some(&'/') {
                        chars.next();
                        state = State::Normal;
                    }
                }
            }
        }
        result
    }

    pub(crate) fn tokenize_excluding_strings(sql: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut current_token = String::new();
        let mut chars = sql.chars().peekable();
        
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum State {
            Normal,
            SingleQuote,
            DoubleQuote,
            SingleLineComment,
            MultiLineComment,
        }
        
        let mut state = State::Normal;
        
        while let Some(c) = chars.next() {
            match state {
                State::Normal => {
                    if c == '\'' {
                        state = State::SingleQuote;
                        if !current_token.is_empty() {
                            tokens.push(current_token.clone());
                            current_token.clear();
                        }
                    } else if c == '"' {
                        state = State::DoubleQuote;
                        if !current_token.is_empty() {
                            tokens.push(current_token.clone());
                            current_token.clear();
                        }
                    } else if c == '-' && chars.peek() == Some(&'-') {
                        chars.next();
                        state = State::SingleLineComment;
                        if !current_token.is_empty() {
                            tokens.push(current_token.clone());
                            current_token.clear();
                        }
                    } else if c == '/' && chars.peek() == Some(&'*') {
                        chars.next();
                        state = State::MultiLineComment;
                        if !current_token.is_empty() {
                            tokens.push(current_token.clone());
                            current_token.clear();
                        }
                    } else if c.is_alphanumeric() || c == '_' {
                        current_token.push(c);
                    } else {
                        if !current_token.is_empty() {
                            tokens.push(current_token.clone());
                            current_token.clear();
                        }
                    }
                }
                State::SingleQuote => {
                    if c == '\'' {
                        state = State::Normal;
                    }
                }
                State::DoubleQuote => {
                    if c == '"' {
                        state = State::Normal;
                    }
                }
                State::SingleLineComment => {
                    if c == '\n' || c == '\r' {
                        state = State::Normal;
                    }
                }
                State::MultiLineComment => {
                    if c == '*' && chars.peek() == Some(&'/') {
                        chars.next();
                        state = State::Normal;
                    }
                }
            }
        }
        
        if !current_token.is_empty() {
            tokens.push(current_token);
        }
        
        tokens
    }

    // Helper for table retrieval
    async fn get_tables(connection: &DbConnectionPayload) -> Result<Vec<String>, anyhow::Error> {
        if let Some(ref path) = connection.sqlite_path {
            if !std::path::Path::new(path).exists() {
                return Err(anyhow::anyhow!("SQLite database file does not exist: {}", path));
            }
            let path = path.clone();
            tokio::task::spawn_blocking(move || {
                let conn = rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
                let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")?;
                let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
                let mut tables = Vec::new();
                for r in rows {
                    tables.push(r?);
                }
                Ok(tables)
            }).await?
        } else if let Some(ref url) = connection.pg_url {
            let client = crate::db::init_postgres(url).await?;
            let rows = client.query("SELECT table_name FROM information_schema.tables WHERE table_schema='public'", &[]).await?;
            let tables = rows.iter().map(|row| row.get::<_, String>(0)).collect();
            Ok(tables)
        } else {
            Err(anyhow::anyhow!("Either sqlite_path or pg_url must be provided"))
        }
    }

    // 8. POST /db/columns
    #[derive(Deserialize, Debug, Clone)]
    pub struct ColumnsRequest {
        pub table_name: String,
        pub connection: DbConnectionPayload,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct ColumnInfo {
        pub name: String,
        pub data_type: String,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct ColumnsResponse {
        pub success: bool,
        pub columns: Option<Vec<ColumnInfo>>,
        pub error: Option<String>,
    }

    pub async fn columns_handler(
        Json(payload): Json<ColumnsRequest>,
    ) -> impl IntoResponse {
        match get_columns(&payload.table_name, &payload.connection).await {
            Ok(columns) => (StatusCode::OK, Json(ColumnsResponse {
                success: true,
                columns: Some(columns),
                error: None,
            })).into_response(),
            Err(e) => (StatusCode::BAD_REQUEST, Json(ColumnsResponse {
                success: false,
                columns: None,
                error: Some(e.to_string()),
            })).into_response(),
        }
    }

    // Helper for columns retrieval
    async fn get_columns(
        table_name: &str,
        connection: &DbConnectionPayload,
    ) -> Result<Vec<ColumnInfo>, anyhow::Error> {
        if !table_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(anyhow::anyhow!("Invalid table name format"));
        }

        if let Some(ref path) = connection.sqlite_path {
            if !std::path::Path::new(path).exists() {
                return Err(anyhow::anyhow!("SQLite database file does not exist: {}", path));
            }
            let path = path.clone();
            let table = table_name.to_string();
            tokio::task::spawn_blocking(move || {
                let conn = rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
                let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
                let rows = stmt.query_map([], |row| {
                    Ok(ColumnInfo {
                        name: row.get::<_, String>(1)?,
                        data_type: row.get::<_, String>(2)?,
                    })
                })?;
                let mut columns = Vec::new();
                for r in rows {
                    columns.push(r?);
                }
                Ok(columns)
            }).await?
        } else if let Some(ref url) = connection.pg_url {
            let client = crate::db::init_postgres(url).await?;
            let rows = client.query(
                "SELECT column_name, data_type FROM information_schema.columns WHERE table_name = $1 AND table_schema = 'public'",
                &[&table_name],
            ).await?;
            let columns = rows.iter().map(|row| ColumnInfo {
                name: row.get::<_, String>(0),
                data_type: row.get::<_, String>(1),
            }).collect();
            Ok(columns)
        } else {
            Err(anyhow::anyhow!("Either sqlite_path or pg_url must be provided"))
        }
    }

    // 9. POST /db/query
    #[derive(Deserialize, Debug, Clone)]
    pub struct QueryRequest {
        pub query: String,
        pub connection: DbConnectionPayload,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct QueryResponse {
        pub success: bool,
        pub rows: Option<Vec<serde_json::Value>>,
        pub error: Option<String>,
    }

    pub async fn query_handler(
        Json(payload): Json<QueryRequest>,
    ) -> impl IntoResponse {
        match run_query(&payload.query, &payload.connection).await {
            Ok(rows) => (StatusCode::OK, Json(QueryResponse {
                success: true,
                rows: Some(rows),
                error: None,
            })).into_response(),
            Err(e) => (StatusCode::BAD_REQUEST, Json(QueryResponse {
                success: false,
                rows: None,
                error: Some(e.to_string()),
            })).into_response(),
        }
    }

    // Helper for query execution
    async fn run_query(
        query: &str,
        connection: &DbConnectionPayload,
    ) -> Result<Vec<serde_json::Value>, anyhow::Error> {
        let stripped = strip_sql_comments(query);
        let trimmed = stripped.trim();
        let upper = trimmed.to_uppercase();
        if !upper.starts_with("SELECT") && !upper.starts_with("WITH") {
            return Err(anyhow::anyhow!("Only SELECT or WITH queries are allowed"));
        }
        
        let mutating_keywords = ["INSERT", "UPDATE", "DELETE", "DROP", "ALTER", "CREATE", "TRUNCATE", "REPLACE"];
        let tokens = tokenize_excluding_strings(query);
        for token in tokens {
            let token_upper = token.to_uppercase();
            if mutating_keywords.contains(&token_upper.as_str()) {
                return Err(anyhow::anyhow!("Query contains mutating keyword: {}", token));
            }
        }

        if let Some(ref path) = connection.sqlite_path {
            if !std::path::Path::new(path).exists() {
                return Err(anyhow::anyhow!("SQLite database file does not exist: {}", path));
            }
            let path = path.clone();
            let query = query.to_string();
            tokio::task::spawn_blocking(move || {
                let conn = rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
                let mut stmt = conn.prepare(&query)?;
                let column_names: Vec<String> = stmt.column_names().into_iter().map(|s| s.to_string()).collect();
                let mut rows = stmt.query([])?;
                let mut result = Vec::new();
                while let Some(row) = rows.next()? {
                    let mut map = serde_json::Map::new();
                    for (i, name) in column_names.iter().enumerate() {
                        let val: rusqlite::types::Value = row.get(i)?;
                        let json_val = match val {
                            rusqlite::types::Value::Null => serde_json::Value::Null,
                            rusqlite::types::Value::Integer(v) => serde_json::Value::Number(v.into()),
                            rusqlite::types::Value::Real(v) => {
                                if let Some(n) = serde_json::Number::from_f64(v) {
                                    serde_json::Value::Number(n)
                                } else {
                                    serde_json::Value::Null
                                }
                            }
                            rusqlite::types::Value::Text(v) => serde_json::Value::String(v),
                            rusqlite::types::Value::Blob(v) => {
                                serde_json::Value::String(String::from_utf8(v).unwrap_or_else(|b| {
                                    b.into_bytes().iter().map(|byte| format!("{:02x}", byte)).collect()
                                }))
                            }
                        };
                        map.insert(name.clone(), json_val);
                    }
                    result.push(serde_json::Value::Object(map));
                }
                Ok(result)
            }).await?
        } else if let Some(ref url) = connection.pg_url {
            let client = crate::db::init_postgres(url).await?;
            let rows = client.query(query, &[]).await?;
            let mut result = Vec::new();
            if !rows.is_empty() {
                for row in rows {
                    let mut map = serde_json::Map::new();
                    let columns = row.columns();
                    for (i, col) in columns.iter().enumerate() {
                        let name = col.name().to_string();
                        let pg_type = col.type_();
                        let json_val = match pg_type.name() {
                            "bool" => row.get::<_, Option<bool>>(i).map_or(serde_json::Value::Null, serde_json::Value::Bool),
                            "int2" => row.get::<_, Option<i16>>(i).map_or(serde_json::Value::Null, |v| serde_json::Value::Number(v.into())),
                            "int4" => row.get::<_, Option<i32>>(i).map_or(serde_json::Value::Null, |v| serde_json::Value::Number(v.into())),
                            "int8" => row.get::<_, Option<i64>>(i).map_or(serde_json::Value::Null, |v| serde_json::Value::Number(v.into())),
                            "float4" => row.get::<_, Option<f32>>(i).map_or(serde_json::Value::Null, |v| {
                                serde_json::Number::from_f64(v as f64).map_or(serde_json::Value::Null, serde_json::Value::Number)
                            }),
                            "float8" => row.get::<_, Option<f64>>(i).map_or(serde_json::Value::Null, |v| {
                                serde_json::Number::from_f64(v).map_or(serde_json::Value::Null, serde_json::Value::Number)
                            }),
                            "text" | "varchar" | "bpchar" | "name" => {
                                row.get::<_, Option<String>>(i).map_or(serde_json::Value::Null, serde_json::Value::String)
                            }
                            "bytea" => {
                                row.get::<_, Option<Vec<u8>>>(i).map_or(serde_json::Value::Null, |bytes| {
                                    serde_json::Value::String(bytes.iter().map(|b| format!("{:02x}", b)).collect())
                                })
                            }
                            _ => {
                                if let Ok(Some(s)) = row.try_get::<_, Option<String>>(i) {
                                    serde_json::Value::String(s)
                                } else {
                                    serde_json::Value::String(format!("<unsupported type: {}>", pg_type.name()))
                                }
                            }
                        };
                        map.insert(name, json_val);
                    }
                    result.push(serde_json::Value::Object(map));
                }
            }
            Ok(result)
        } else {
            Err(anyhow::anyhow!("Either sqlite_path or pg_url must be provided"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::db::*;
    use super::server::*;
    use axum::response::IntoResponse;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[test]
    fn test_sqlite_db_initialization() {
        let conn = init_sqlite().expect("Failed to initialize SQLite");
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='agents'")
            .expect("Failed to prepare statement");
        let exists = stmt.exists([]).expect("Failed to execute query");
        assert!(exists, "The agents table should exist");
    }

    #[tokio::test]
    async fn test_status_handler_directly() {
        let conn = init_sqlite().expect("Failed to initialize SQLite");
        let state = AppState {
            sqlite_conn: Arc::new(Mutex::new(conn)),
            agent: None,
            subagents: Arc::new(Mutex::new(HashMap::new())),
            subagent_keys: Arc::new(Mutex::new(std::collections::VecDeque::new())),
        };
        let response = status_handler(axum::extract::State(state)).await;
        assert_eq!(response.status, "ok");
        assert!(!response.agent_active);
    }

    #[tokio::test]
    async fn test_execute_handler() {
        // Run echo command
        let payload = ExecuteRequest {
            command: "echo".to_string(),
            args: Some(vec!["hello-from-test".to_string()]),
            cwd: None,
        };
        let response = execute_handler(axum::Json(payload)).await;
        assert!(response.success);
        assert_eq!(response.exit_code, Some(0));
        assert!(response.stdout.contains("hello-from-test"));
    }

    #[tokio::test]
    async fn test_read_write_file_handlers() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("test_file_tmp_agent.txt");
        let path_str = file_path.to_string_lossy().to_string();

        // 1. Write file
        let write_payload = WriteFileRequest {
            path: path_str.clone(),
            content: "hello world from unit test".to_string(),
        };
        let write_resp = write_file_handler(axum::Json(write_payload)).await;
        assert!(write_resp.success);
        assert!(write_resp.error.is_none());

        // 2. Read file
        let read_payload = ReadFileRequest {
            path: path_str.clone(),
        };
        let read_resp = read_file_handler(axum::Json(read_payload)).await;
        assert!(read_resp.success);
        assert_eq!(read_resp.content.as_deref(), Some("hello world from unit test"));
        assert!(read_resp.error.is_none());

        // 3. Clean up
        let _ = std::fs::remove_file(&file_path);

        // 4. Read non-existent file
        let read_fail_payload = ReadFileRequest {
            path: "/nonexistent/path/to/file/that/does/not/exist".to_string(),
        };
        let read_fail_resp = read_file_handler(axum::Json(read_fail_payload)).await;
        assert!(!read_fail_resp.success);
        assert!(read_fail_resp.content.is_none());
        assert!(read_fail_resp.error.is_some());
    }

    #[tokio::test]
    async fn test_log_handler() {
        let payload = LogRequest {
            level: Some("info".to_string()),
            message: "test log output".to_string(),
        };
        let response = log_handler(axum::Json(payload)).await;
        assert!(response.success);
    }

    #[tokio::test]
    async fn test_db_endpoints() {
        use axum::response::IntoResponse;
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join("test_tmp_agent_db.db");
        let db_path_str = db_path.to_string_lossy().to_string();

        // Ensure clean state
        let _ = std::fs::remove_file(&db_path);

        // Initialize SQLite DB table using standard rusqlite
        {
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            conn.execute(
                "CREATE TABLE test_users (
                    id INTEGER PRIMARY KEY,
                    username TEXT NOT NULL,
                    score REAL
                )",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO test_users (username, score) VALUES ('alice', 95.5)",
                [],
            ).unwrap();
        }

        let connection_payload = DbConnectionPayload {
            sqlite_path: Some(db_path_str.clone()),
            pg_url: None,
        };

        // 1. Test tables endpoint
        let tables_payload = TablesRequest {
            connection: connection_payload.clone(),
        };
        let tables_resp = tables_handler(axum::Json(tables_payload)).await.into_response();
        assert_eq!(tables_resp.status(), axum::http::StatusCode::OK);
        
        let body_bytes = axum::body::to_bytes(tables_resp.into_body(), usize::MAX).await.unwrap();
        let tables_data: TablesResponse = serde_json::from_slice(&body_bytes).unwrap();
        assert!(tables_data.success);
        let tables = tables_data.tables.expect("Expected tables list");
        assert!(tables.contains(&"test_users".to_string()));

        // 2. Test columns endpoint
        let columns_payload = ColumnsRequest {
            table_name: "test_users".to_string(),
            connection: connection_payload.clone(),
        };
        let columns_resp = columns_handler(axum::Json(columns_payload)).await.into_response();
        assert_eq!(columns_resp.status(), axum::http::StatusCode::OK);

        let body_bytes = axum::body::to_bytes(columns_resp.into_body(), usize::MAX).await.unwrap();
        let columns_data: ColumnsResponse = serde_json::from_slice(&body_bytes).unwrap();
        assert!(columns_data.success);
        let columns = columns_data.columns.expect("Expected columns list");
        assert_eq!(columns.len(), 3);
        assert_eq!(columns[0].name, "id");
        assert_eq!(columns[1].name, "username");
        assert_eq!(columns[2].name, "score");

        // 3. Test query endpoint (valid SELECT)
        let query_payload = QueryRequest {
            query: "SELECT username, score FROM test_users".to_string(),
            connection: connection_payload.clone(),
        };
        let query_resp = query_handler(axum::Json(query_payload)).await.into_response();
        assert_eq!(query_resp.status(), axum::http::StatusCode::OK);

        let body_bytes = axum::body::to_bytes(query_resp.into_body(), usize::MAX).await.unwrap();
        let query_data: QueryResponse = serde_json::from_slice(&body_bytes).unwrap();
        assert!(query_data.success);
        let rows = query_data.rows.expect("Expected rows");
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row["username"], serde_json::Value::String("alice".to_string()));
        assert_eq!(row["score"], serde_json::Value::Number(serde_json::Number::from_f64(95.5).unwrap()));

        // 4. Test query endpoint (invalid SELECT / mutation statement error)
        let invalid_query_payload = QueryRequest {
            query: "INSERT INTO test_users (username, score) VALUES ('bob', 88.0)".to_string(),
            connection: connection_payload.clone(),
        };
        let invalid_query_resp = query_handler(axum::Json(invalid_query_payload)).await.into_response();
        assert_eq!(invalid_query_resp.status(), axum::http::StatusCode::BAD_REQUEST);

        let body_bytes = axum::body::to_bytes(invalid_query_resp.into_body(), usize::MAX).await.unwrap();
        let invalid_query_data: QueryResponse = serde_json::from_slice(&body_bytes).unwrap();
        assert!(!invalid_query_data.success);
        assert!(invalid_query_data.error.unwrap().contains("Only SELECT or WITH queries are allowed"));

        // Clean up DB file
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn test_path_traversal_validation() {
        // Valid paths:
        // 1. A file in the temp directory
        let temp_dir = std::env::temp_dir();
        let valid_temp_path = temp_dir.join("test.txt").to_string_lossy().to_string();
        let res = validate_and_canonicalize_path(&valid_temp_path);
        assert!(res.is_ok());

        // 2. A file in the workspace directory
        let workspace_path = "/Volumes/goldcoders/tmp/test.txt";
        let res2 = validate_and_canonicalize_path(workspace_path);
        assert!(res2.is_ok());

        // Invalid paths:
        // 3. A path escaping to /etc/passwd
        let invalid_path = "/Volumes/goldcoders/tmp/../../etc/passwd";
        let res3 = validate_and_canonicalize_path(invalid_path);
        assert!(res3.is_err());
        assert!(res3.unwrap_err().to_string().contains("Path traversal detected"));
    }

    #[test]
    fn test_strip_sql_comments_and_tokenize() {
        let sql = "SELECT * -- comment\nFROM users /* multiline \n comment */ WHERE name = 'bob -- not a comment'";
        let stripped = strip_sql_comments(sql);
        assert!(stripped.contains("SELECT *"));
        assert!(stripped.contains("FROM users"));
        assert!(stripped.contains("WHERE name = 'bob -- not a comment'"));
        assert!(!stripped.contains("multiline"));

        let tokens = tokenize_excluding_strings(sql);
        assert!(tokens.contains(&"SELECT".to_string()));
        assert!(tokens.contains(&"FROM".to_string()));
        assert!(tokens.contains(&"users".to_string()));
        assert!(tokens.contains(&"WHERE".to_string()));
        // 'bob -- not a comment' should not be a token itself since strings are excluded
        assert!(!tokens.contains(&"bob".to_string()));
    }

    #[tokio::test]
    async fn test_cte_and_mutating_keyword_validation() {
        let connection_payload = DbConnectionPayload {
            sqlite_path: Some("/nonexistent/file.db".to_string()),
            pg_url: None,
        };

        // 1. Should fail because path doesn't exist
        let query_payload = QueryRequest {
            query: "WITH cte AS (SELECT 1) SELECT * FROM cte".to_string(),
            connection: connection_payload.clone(),
        };
        let resp = query_handler(axum::Json(query_payload)).await.into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::BAD_REQUEST);
        let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let data: QueryResponse = serde_json::from_slice(&body_bytes).unwrap();
        assert!(!data.success);
        assert!(data.error.unwrap().contains("does not exist"));

        // 2. Query containing mutating keyword in string should not fail validation
        // (will still fail due to database file not existing, but that confirms it passed the SQL checks)
        let query_payload2 = QueryRequest {
            query: "SELECT 'INSERT' AS val".to_string(),
            connection: connection_payload.clone(),
        };
        let resp2 = query_handler(axum::Json(query_payload2)).await.into_response();
        let body_bytes2 = axum::body::to_bytes(resp2.into_body(), usize::MAX).await.unwrap();
        let data2: QueryResponse = serde_json::from_slice(&body_bytes2).unwrap();
        assert!(data2.error.unwrap().contains("does not exist")); // implies SQL validation passed!

        // 3. Query containing actual mutating keyword should fail SQL validation
        let query_payload3 = QueryRequest {
            query: "SELECT 1; DROP TABLE users;".to_string(),
            connection: connection_payload.clone(),
        };
        let resp3 = query_handler(axum::Json(query_payload3)).await.into_response();
        let body_bytes3 = axum::body::to_bytes(resp3.into_body(), usize::MAX).await.unwrap();
        let data3: QueryResponse = serde_json::from_slice(&body_bytes3).unwrap();
        assert!(data3.error.unwrap().contains("Query contains mutating keyword"));
    }

    #[tokio::test]
    async fn test_subagent_keys_bounding() {
        let conn = init_sqlite().expect("Failed to initialize SQLite");
        let state = AppState {
            sqlite_conn: Arc::new(Mutex::new(conn)),
            agent: None,
            subagents: Arc::new(Mutex::new(HashMap::new())),
            subagent_keys: Arc::new(Mutex::new(std::collections::VecDeque::new())),
        };

        // Spawn 105 subagents
        for i in 0..105 {
            let req = SubagentRequest {
                prompt: format!("Subagent prompt {}", i),
            };
            let _ = subagent_handler(axum::extract::State(state.clone()), axum::Json(req)).await;
        }

        // Check map and queue sizes
        let map = state.subagents.lock().await;
        let keys = state.subagent_keys.lock().await;
        
        assert_eq!(keys.len(), 100);
        assert_eq!(map.len(), 100);
    }

    #[tokio::test]
    async fn test_subagent_handlers() {
        use axum::response::IntoResponse;
        let conn = init_sqlite().expect("Failed to initialize SQLite");
        let subagents = Arc::new(Mutex::new(HashMap::new()));
        let state = AppState {
            sqlite_conn: Arc::new(Mutex::new(conn)),
            agent: None,
            subagents: subagents.clone(),
            subagent_keys: Arc::new(Mutex::new(std::collections::VecDeque::new())),
        };

        // 1. Start subagent
        let subagent_req = SubagentRequest {
            prompt: "Help me analyze this code".to_string(),
        };
        let subagent_resp = subagent_handler(
            axum::extract::State(state.clone()),
            axum::Json(subagent_req),
        ).await;
        
        let subagent_id = subagent_resp.subagent_id.clone();
        assert!(!subagent_id.is_empty());

        // Check state map immediately to see if it is running
        {
            let map = subagents.lock().await;
            assert!(map.contains_key(&subagent_id));
            match map.get(&subagent_id).unwrap() {
                SubagentStatus::Running => {}
                _ => panic!("Expected subagent to be running initially"),
            }
        }

        // 2. Query status of subagent
        let get_resp = get_subagent_handler(
            axum::extract::State(state.clone()),
            axum::extract::Path(subagent_id.clone()),
        ).await.into_response();
        assert_eq!(get_resp.status(), axum::http::StatusCode::OK);

        let body_bytes = axum::body::to_bytes(get_resp.into_body(), usize::MAX).await.unwrap();
        let status: SubagentStatus = serde_json::from_slice(&body_bytes).unwrap();
        match status {
            SubagentStatus::Running => {}
            _ => panic!("Expected status to be Running"),
        }

        // 3. Query status of non-existent subagent
        let get_fail_resp = get_subagent_handler(
            axum::extract::State(state.clone()),
            axum::extract::Path("non-existent-uuid".to_string()),
        ).await.into_response();
        assert_eq!(get_fail_resp.status(), axum::http::StatusCode::NOT_FOUND);
    }
}
