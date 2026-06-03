#![allow(dead_code, unused)]
// E2E Test Common Harness
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;
use tempfile::TempDir;

pub struct TestSandbox {
    pub temp_dir: TempDir,
    pub home_dir: PathBuf,
    pub config_dir: PathBuf,
    pub project_dir: PathBuf,
    pub bin_path: PathBuf,
}

impl TestSandbox {
    pub fn new() -> Self {
        let temp_dir = tempfile::tempdir().unwrap();
        let home_dir = temp_dir.path().join("home");
        let config_dir = home_dir.join(".config").join("tmp");
        let project_dir = temp_dir.path().join("project");

        fs::create_dir_all(&home_dir).unwrap();
        fs::create_dir_all(&config_dir).unwrap();
        fs::create_dir_all(&project_dir).unwrap();

        let bin_path = PathBuf::from(env!("CARGO_BIN_EXE_tmp"));

        TestSandbox {
            temp_dir,
            home_dir,
            config_dir,
            project_dir,
            bin_path,
        }
    }

    pub fn run(&self, args: &[&str]) -> Output {
        self.run_in_dir(args, &self.project_dir)
    }

    pub fn run_in_dir(&self, args: &[&str], cwd: &Path) -> Output {
        Command::new(&self.bin_path)
            .args(args)
            .current_dir(cwd)
            .env("HOME", &self.home_dir)
            .env("TMP_CONFIG_DIR", &self.config_dir)
            .env(
                "TMP_REGISTRY_REPO",
                format!(
                    "file://{}",
                    self.temp_dir.path().join("mock_registry").display()
                ),
            )
            .output()
            .expect("Failed to execute tmp binary")
    }

    pub fn write_config(&self, content: &str) {
        let config_path = self.config_dir.join("config.toml");
        fs::write(config_path, content).unwrap();
    }

    pub fn write_mock_registry(&self, index_json: &str, schemas: &[(&str, &str)]) {
        let registry_dir = self.temp_dir.path().join("mock_registry");
        fs::create_dir_all(&registry_dir).unwrap();

        fs::write(registry_dir.join("index.json"), index_json).unwrap();

        for (name, content) in schemas {
            fs::write(registry_dir.join(format!("{}.json", name)), content).unwrap();
        }
    }

    pub fn setup_cargo_project(&self, name: &str, members: &[&str]) {
        let cargo_toml = format!("[workspace]\nresolver = \"2\"\nmembers = {:?}\n", members);
        fs::write(self.project_dir.join("Cargo.toml"), cargo_toml).unwrap();
        fs::write(self.project_dir.join("README.md"), format!("# {}\n", name)).unwrap();

        for member in members {
            let member_dir = self.project_dir.join(member);
            fs::create_dir_all(member_dir.join("src")).unwrap();
            fs::write(
                member_dir.join("Cargo.toml"),
                format!(
                    "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
                    member
                ),
            )
            .unwrap();
            fs::write(member_dir.join("src").join("lib.rs"), "pub fn work() {}\n").unwrap();
        }
    }

    pub fn setup_npm_project(&self, package_json: &str) {
        fs::write(self.project_dir.join("package.json"), package_json).unwrap();
    }

    pub fn setup_python_project(&self) {
        fs::write(self.project_dir.join("requirements.txt"), "pytest==7.0.0\n").unwrap();
        fs::write(self.project_dir.join("main.py"), "print('hello')\n").unwrap();
    }

    pub fn setup_go_project(&self) {
        fs::write(
            self.project_dir.join("go.mod"),
            "module test-go\n\ngo 1.18\n",
        )
        .unwrap();
        fs::write(
            self.project_dir.join("main.go"),
            "package main\n\nfunc main() {}\n",
        )
        .unwrap();
    }
}

pub struct MockHttpServer {
    pub port: u16,
}

impl MockHttpServer {
    pub fn start<F>(handler: F) -> Self
    where
        F: Fn(&str) -> String + Send + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let listener_clone = listener.try_clone().unwrap();
        thread::spawn(move || {
            for mut stream in listener_clone.incoming().flatten() {
                let mut buffer = [0; 4096];
                if let Ok(n) = stream.read(&mut buffer) {
                    let request_str = String::from_utf8_lossy(&buffer[..n]);
                    let response_body = handler(&request_str);
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        response_body.len(),
                        response_body
                    );
                    let _ = stream.write_all(response.as_bytes());
                    let _ = stream.flush();
                    let _ = stream.shutdown(std::net::Shutdown::Both);
                }
            }
        });
        MockHttpServer { port }
    }
}
