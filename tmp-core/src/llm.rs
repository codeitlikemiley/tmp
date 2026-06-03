use crate::config::{Config, ProviderConfig};
use crate::schema::Schema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct RotationState {
    pub provider_index: usize,
    pub key_indices: HashMap<String, usize>,
}

pub fn rotation_state_path() -> Option<PathBuf> {
    if let Ok(custom_dir) = std::env::var("TMP_CONFIG_DIR") {
        if !custom_dir.trim().is_empty() {
            let mut p = PathBuf::from(custom_dir);
            p.push("rotation_state.json");
            return Some(p);
        }
    }
    dirs::home_dir().map(|mut p| {
        p.push(".config");
        p.push("tmp");
        p.push("rotation_state.json");
        p
    })
}

pub struct LlmDispatcher {
    pub config: Config,
    pub state: RotationState,
}

impl LlmDispatcher {
    pub fn new(config: Config) -> Self {
        let state = Self::load_state();
        LlmDispatcher { config, state }
    }

    fn load_state() -> RotationState {
        if let Some(p) = rotation_state_path() {
            if p.exists() {
                if let Ok(content) = std::fs::read_to_string(&p) {
                    if let Ok(s) = serde_json::from_str(&content) {
                        return s;
                    }
                }
            }
        }
        RotationState::default()
    }

    pub fn save_state(&self) {
        if let Some(p) = rotation_state_path() {
            if let Some(parent) = p.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(content) = serde_json::to_string(&self.state) {
                let _ = std::fs::write(&p, content);
            }
        }
    }

    pub fn query_resolve(
        &mut self,
        prompt: &str,
        provider_override: Option<&str>,
        model_override: Option<&str>,
    ) -> Result<String, String> {
        let providers_len = self.config.llm.providers.len();
        if providers_len == 0 {
            return Err("No LLM providers configured".to_string());
        }

        if let Some(prov_name) = provider_override {
            let idx = self
                .config
                .llm
                .providers
                .iter()
                .position(|p| p.provider == prov_name)
                .ok_or_else(|| format!("Provider '{}' not found in configuration", prov_name))?;
            let res = self.try_provider_raw(idx, prompt, model_override)?;
            self.save_state();
            return Ok(res);
        }

        let strategy = self.config.llm.strategy.clone();
        match strategy.as_str() {
            "single" => {
                let idx = self.state.provider_index % providers_len;
                let res = self.try_provider_raw(idx, prompt, model_override)?;
                self.save_state();
                Ok(res)
            }
            "round-robin" => {
                let idx = self.state.provider_index % providers_len;
                let res = self.try_provider_raw(idx, prompt, model_override);
                self.state.provider_index = (idx + 1) % providers_len;
                self.save_state();
                res
            }
            _ => {
                let mut last_err = String::new();
                let start_idx = self.state.provider_index % providers_len;
                for i in 0..providers_len {
                    let idx = (start_idx + i) % providers_len;
                    match self.try_provider_raw(idx, prompt, model_override) {
                        Ok(res) => {
                            self.state.provider_index = idx;
                            self.save_state();
                            return Ok(res);
                        }
                        Err(e) => {
                            last_err = e;
                        }
                    }
                }
                Err(format!("All providers failed. Last error: {}", last_err))
            }
        }
    }

    fn try_provider_raw(
        &mut self,
        prov_idx: usize,
        prompt: &str,
        model_override: Option<&str>,
    ) -> Result<String, String> {
        let prov = &self.config.llm.providers[prov_idx];
        let keys = &prov.keys;
        if keys.is_empty() {
            return Err(format!(
                "No API keys configured for provider '{}'",
                prov.provider
            ));
        }
        let start_key_idx = *self
            .state
            .key_indices
            .entry(prov.provider.clone())
            .or_insert(0);
        let mut last_err = String::new();

        for i in 0..keys.len() {
            let current_key_idx = (start_key_idx + i) % keys.len();
            let key = &keys[current_key_idx];
            match call_provider_api_raw(prov, key, prompt, model_override) {
                Ok(raw_response) => {
                    self.state
                        .key_indices
                        .insert(prov.provider.clone(), current_key_idx);
                    return Ok(raw_response);
                }
                Err(e) => {
                    last_err = e;
                }
            }
        }
        Err(format!(
            "Provider '{}' failed with: {}",
            prov.provider, last_err
        ))
    }

    pub fn generate_schema(
        &mut self,
        tool: &str,
        help_text: &str,
        provider_override: Option<&str>,
        model_override: Option<&str>,
    ) -> Result<Schema, String> {
        let providers_len = self.config.llm.providers.len();
        if providers_len == 0 {
            return Err("No LLM providers configured".to_string());
        }

        if let Some(prov_name) = provider_override {
            let idx = self
                .config
                .llm
                .providers
                .iter()
                .position(|p| p.provider == prov_name)
                .ok_or_else(|| format!("Provider '{}' not found in configuration", prov_name))?;
            let schema = self.try_provider(idx, tool, help_text, model_override)?;
            self.save_state();
            return Ok(schema);
        }

        let strategy = self.config.llm.strategy.clone();
        match strategy.as_str() {
            "single" => {
                let idx = self.state.provider_index % providers_len;
                let schema = self.try_provider(idx, tool, help_text, model_override)?;
                self.save_state();
                Ok(schema)
            }
            "round-robin" => {
                let idx = self.state.provider_index % providers_len;
                let schema = self.try_provider(idx, tool, help_text, model_override);
                self.state.provider_index = (idx + 1) % providers_len;
                self.save_state();
                schema
            }
            _ => {
                let mut last_err = String::new();
                let start_idx = self.state.provider_index % providers_len;
                for i in 0..providers_len {
                    let idx = (start_idx + i) % providers_len;
                    match self.try_provider(idx, tool, help_text, model_override) {
                        Ok(schema) => {
                            self.state.provider_index = idx;
                            self.save_state();
                            return Ok(schema);
                        }
                        Err(e) => {
                            last_err = e;
                        }
                    }
                }
                Err(format!("All providers failed. Last error: {}", last_err))
            }
        }
    }

    fn try_provider(
        &mut self,
        prov_idx: usize,
        tool: &str,
        help_text: &str,
        model_override: Option<&str>,
    ) -> Result<Schema, String> {
        let prov = &self.config.llm.providers[prov_idx];
        let keys = &prov.keys;
        if keys.is_empty() {
            return Err(format!(
                "No API keys configured for provider '{}'",
                prov.provider
            ));
        }
        let start_key_idx = *self
            .state
            .key_indices
            .entry(prov.provider.clone())
            .or_insert(0);
        let mut last_err = String::new();

        for i in 0..keys.len() {
            let current_key_idx = (start_key_idx + i) % keys.len();
            let key = &keys[current_key_idx];
            match call_provider_api(prov, key, tool, help_text, model_override) {
                Ok(raw_json) => {
                    let cleaned = clean_json_markdown(&raw_json);
                    match Schema::from_json(&cleaned) {
                        Ok(schema) => {
                            self.state
                                .key_indices
                                .insert(prov.provider.clone(), current_key_idx);
                            return Ok(schema);
                        }
                        Err(e) => {
                            last_err = format!(
                                "Failed to parse schema JSON: {}. Raw response: {}",
                                e, raw_json
                            );
                        }
                    }
                }
                Err(e) => {
                    last_err = e;
                }
            }
        }
        Err(format!(
            "Provider '{}' failed with: {}",
            prov.provider, last_err
        ))
    }
}

pub fn clean_json_markdown(text: &str) -> String {
    let mut s = text.trim();
    if s.starts_with("```") {
        if let Some(first_line_end) = s.find('\n') {
            s = &s[first_line_end + 1..];
        } else {
            s = s.trim_start_matches('`').trim_start_matches("json");
        }
    }
    if s.ends_with("```") {
        s = &s[..s.len() - 3];
    }
    s.trim().to_string()
}

fn call_provider_api(
    prov: &ProviderConfig,
    key: &str,
    tool: &str,
    help_text: &str,
    model_override: Option<&str>,
) -> Result<String, String> {
    let prompt = format!(
        "You are an expert command schema generator. Generate a JSON schema matching the following Rust struct definition:\n\
        ```rust\n\
        pub struct Schema {{\n\
            pub meta: SchemaMeta,\n\
            pub commands: Vec<Command>,\n\
        }}\n\
        ```\n\
        Here is the tool name: \"{}\".\n\
        Here is the help text output from the tool:\n\
        \"\"\"\n\
        {}\n\
        \"\"\"\n\
        Generate the complete JSON schema for this tool. Ensure all commands and subcommands from the help text are parsed, with descriptions, groups, and arguments/options mapped to tokens. Return ONLY the JSON object, do not explain.",
        tool, help_text
    );

    call_provider_api_raw(prov, key, &prompt, model_override)
}

pub fn call_provider_api_raw(
    prov: &ProviderConfig,
    key: &str,
    prompt: &str,
    model_override: Option<&str>,
) -> Result<String, String> {
    match prov.provider.as_str() {
        "gemini" => {
            let model = model_override
                .or(prov.model.as_deref())
                .unwrap_or("gemini-1.5-flash");
            let base = prov
                .base_url
                .as_deref()
                .unwrap_or("https://generativelanguage.googleapis.com");
            let url = format!(
                "{}/v1beta/models/{}:generateContent?key={}",
                base.trim_end_matches('/'),
                model,
                key
            );

            let payload = serde_json::json!({
                "contents": [
                    {
                        "parts": [
                            {
                                "text": prompt
                            }
                        ]
                    }
                ]
            });

            let resp: serde_json::Value = ureq::post(&url)
                .set("Content-Type", "application/json")
                .send_json(payload)
                .map_err(|e| format!("Gemini API failed: {}", e))?
                .into_json()
                .map_err(|e| format!("Gemini JSON parsing failed: {}", e))?;

            let text = resp["candidates"][0]["content"]["parts"][0]["text"]
                .as_str()
                .ok_or_else(|| format!("Invalid Gemini response structure: {:?}", resp))?;
            Ok(text.to_string())
        }
        "openai" | "openai-compatible" => {
            let model = model_override.or(prov.model.as_deref()).unwrap_or("gpt-4o");
            let default_base = "https://api.openai.com";
            let base = prov.base_url.as_deref().unwrap_or(default_base);
            let url = format!("{}/v1/chat/completions", base.trim_end_matches('/'));

            let payload = serde_json::json!({
                "model": model,
                "messages": [
                    {
                        "role": "user",
                        "content": prompt
                    }
                ]
            });

            let resp: serde_json::Value = ureq::post(&url)
                .set("Content-Type", "application/json")
                .set("Authorization", &format!("Bearer {}", key))
                .send_json(payload)
                .map_err(|e| format!("OpenAI API failed: {}", e))?
                .into_json()
                .map_err(|e| format!("OpenAI JSON parsing failed: {}", e))?;

            let text = resp["choices"][0]["message"]["content"]
                .as_str()
                .ok_or_else(|| format!("Invalid OpenAI response structure: {:?}", resp))?;
            Ok(text.to_string())
        }
        "ollama" => {
            let model = model_override.or(prov.model.as_deref()).unwrap_or("llama3");
            let base = prov.base_url.as_deref().unwrap_or("http://localhost:11434");
            let url = format!("{}/api/chat", base.trim_end_matches('/'));

            let payload = serde_json::json!({
                "model": model,
                "messages": [
                    {
                        "role": "user",
                        "content": prompt
                    }
                ],
                "stream": false
            });

            let resp: serde_json::Value = ureq::post(&url)
                .set("Content-Type", "application/json")
                .send_json(payload)
                .map_err(|e| format!("Ollama API failed: {}", e))?
                .into_json()
                .map_err(|e| format!("Ollama JSON parsing failed: {}", e))?;

            let text = resp["message"]["content"]
                .as_str()
                .ok_or_else(|| format!("Invalid Ollama response structure: {:?}", resp))?;
            Ok(text.to_string())
        }
        _ => Err(format!("Unsupported provider: {}", prov.provider)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LlmConfig;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    fn spawn_mock_server<F>(listener: TcpListener, response_fn: F) -> thread::JoinHandle<()>
    where
        F: Fn(&str) -> (u16, String) + Send + 'static,
    {
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0; 4096];
                if let Ok(n) = stream.read(&mut buf) {
                    let req_str = String::from_utf8_lossy(&buf[..n]);
                    let (status, resp_body) = response_fn(&req_str);
                    let status_line = if status == 200 {
                        "200 OK"
                    } else {
                        "500 Internal Server Error"
                    };
                    let http_response = format!(
                        "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                        status_line,
                        resp_body.len(),
                        resp_body
                    );
                    let _ = stream.write_all(http_response.as_bytes());
                    let _ = stream.flush();
                    let _ = stream.shutdown(std::net::Shutdown::Both);
                }
            }
        })
    }

    fn valid_schema_json() -> String {
        r#"{
            "meta": {
                "tool": "git",
                "version": 1,
                "verified": true,
                "keywords": []
            },
            "commands": []
        }"#
        .to_string()
    }

    #[test]
    fn test_clean_json_markdown() {
        let input = "```json\n{\n  \"meta\": {}\n}\n```";
        let cleaned = clean_json_markdown(input);
        assert_eq!(cleaned, "{\n  \"meta\": {}\n}");

        let input_no_json = "```\n{\n  \"meta\": {}\n}\n```";
        let cleaned_no_json = clean_json_markdown(input_no_json);
        assert_eq!(cleaned_no_json, "{\n  \"meta\": {}\n}");
    }

    #[test]
    fn test_llm_dispatcher_fallback() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::env::set_var("TMP_CONFIG_DIR", temp_dir.path());

        // Spawn a failing server for gemini, and a successful server for openai
        let gemini_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let gemini_port = gemini_listener.local_addr().unwrap().port();
        let _h1 = spawn_mock_server(gemini_listener, |_| (500, "".to_string()));

        let openai_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let openai_port = openai_listener.local_addr().unwrap().port();
        let _h2 = spawn_mock_server(openai_listener, |_| {
            let body = serde_json::json!({
                "choices": [
                    {
                        "message": {
                            "role": "assistant",
                            "content": format!("```json\n{}\n```", valid_schema_json())
                        }
                    }
                ]
            });
            (200, body.to_string())
        });

        let config = Config {
            llm: LlmConfig {
                strategy: "fallback".to_string(),
                providers: vec![
                    ProviderConfig {
                        provider: "gemini".to_string(),
                        keys: vec!["gemini_key".to_string()],
                        base_url: Some(format!("http://127.0.0.1:{}", gemini_port)),
                        model: None,
                    },
                    ProviderConfig {
                        provider: "openai".to_string(),
                        keys: vec!["openai_key".to_string()],
                        base_url: Some(format!("http://127.0.0.1:{}", openai_port)),
                        model: None,
                    },
                ],
            },
        };

        let mut dispatcher = LlmDispatcher::new(config);
        assert_eq!(dispatcher.state.provider_index, 0);

        let schema = dispatcher
            .generate_schema("git", "help text", None, None)
            .unwrap();
        assert_eq!(schema.meta.tool, "git");

        // After fallback success on OpenAI (index 1), provider_index should update to 1
        assert_eq!(dispatcher.state.provider_index, 1);

        // Clean up env
        std::env::remove_var("TMP_CONFIG_DIR");
    }

    #[test]
    fn test_llm_dispatcher_round_robin() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::env::set_var("TMP_CONFIG_DIR", temp_dir.path());

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let _h = spawn_mock_server(listener, |_| {
            let body = serde_json::json!({
                "message": {
                    "role": "assistant",
                    "content": valid_schema_json()
                }
            });
            (200, body.to_string())
        });

        let config = Config {
            llm: LlmConfig {
                strategy: "round-robin".to_string(),
                providers: vec![
                    ProviderConfig {
                        provider: "ollama".to_string(),
                        keys: vec!["dummy_key".to_string()],
                        base_url: Some(format!("http://127.0.0.1:{}", port)),
                        model: None,
                    },
                    ProviderConfig {
                        provider: "openai".to_string(),
                        keys: vec!["dummy_key2".to_string()],
                        base_url: None,
                        model: None,
                    },
                ],
            },
        };

        let mut dispatcher = LlmDispatcher::new(config);
        assert_eq!(dispatcher.state.provider_index, 0);

        let _schema = dispatcher
            .generate_schema("git", "help text", None, None)
            .unwrap();
        // Index should increment unconditionally to 1
        assert_eq!(dispatcher.state.provider_index, 1);

        std::env::remove_var("TMP_CONFIG_DIR");
    }
}
