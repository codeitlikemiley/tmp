use command::Command;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Context {
    pub cwd: String,
    pub project_root: Option<String>,
    pub build_system: String,
    pub file_kind: String,
    pub script_engine: Option<String>,
    pub recommended_target: Option<String>,
    pub package_name: Option<String>,
    pub packages: Vec<String>,
    pub bins: Vec<String>,
    pub examples: Vec<String>,
    pub features: Vec<String>,
    pub profiles: Vec<String>,
    pub tests: Vec<String>,
    pub benches: Vec<String>,
    pub git_branches: Vec<String>,
    pub git_remotes: Vec<String>,
    pub npm_scripts: Vec<String>,
}

impl Context {
    pub fn detect(cwd: &Path, file_path: Option<&str>, line: Option<usize>) -> Self {
        let _ = line; // line number is stored/processed if needed but here we just accept it as part of API
        let cwd_str = cwd.to_string_lossy().to_string();

        let resolved_file_path = file_path.map(|fp| {
            let p = Path::new(fp);
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                cwd.join(p)
            }
        });

        let start_dir = resolved_file_path
            .as_deref()
            .and_then(|p| if p.is_file() { p.parent() } else { Some(p) })
            .unwrap_or(cwd);

        let project_root_path = find_project_root(start_dir);
        let project_root = project_root_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string());

        let script_engine = resolved_file_path.as_deref().and_then(detect_script_engine);

        let mut build_system = "none".to_string();
        if let Some(ref pr) = project_root_path {
            if pr.join("Cargo.toml").exists() {
                build_system = "cargo".to_string();
            } else if pr.join("package.json").exists() {
                build_system = "npm".to_string();
            } else if pr.join("go.mod").exists() {
                build_system = "go".to_string();
            } else if pr.join("pyproject.toml").exists() || pr.join("requirements.txt").exists() {
                build_system = "python".to_string();
            }
        }

        let file_kind = if script_engine.is_some() {
            "single_file_script".to_string()
        } else if build_system == "cargo" {
            "cargo_project".to_string()
        } else if build_system == "npm" {
            "npm_project".to_string()
        } else {
            "standalone".to_string()
        };

        let mut ctx = Self {
            cwd: cwd_str,
            project_root,
            build_system,
            file_kind,
            script_engine,
            recommended_target: resolved_file_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string()),
            package_name: None,
            packages: Vec::new(),
            bins: Vec::new(),
            examples: Vec::new(),
            features: Vec::new(),
            profiles: Vec::new(),
            tests: Vec::new(),
            benches: Vec::new(),
            git_branches: Vec::new(),
            git_remotes: Vec::new(),
            npm_scripts: Vec::new(),
        };

        ctx.refresh();

        ctx.recommended_target = detect_recommended_target(
            resolved_file_path.as_deref(),
            ctx.script_engine.as_deref(),
            ctx.package_name.as_deref(),
        );

        ctx
    }

    pub fn refresh(&mut self) {
        let cwd_path = Path::new(&self.cwd);

        // If there was a recommended target, use it to detect script engine.
        let mut script_engine = None;
        if let Some(ref target) = self.recommended_target {
            let target_path = Path::new(target);
            let resolved_target = if target_path.is_absolute() {
                target_path.to_path_buf()
            } else {
                cwd_path.join(target_path)
            };
            if resolved_target.is_file() {
                script_engine = detect_script_engine(&resolved_target);
            }
        }
        self.script_engine = script_engine;

        let project_root_path = find_project_root(cwd_path);
        self.project_root = project_root_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string());

        let mut build_system = "none".to_string();
        if let Some(ref pr) = project_root_path {
            if pr.join("Cargo.toml").exists() {
                build_system = "cargo".to_string();
            } else if pr.join("package.json").exists() {
                build_system = "npm".to_string();
            } else if pr.join("go.mod").exists() {
                build_system = "go".to_string();
            } else if pr.join("pyproject.toml").exists() || pr.join("requirements.txt").exists() {
                build_system = "python".to_string();
            }
        }
        self.build_system = build_system;

        self.file_kind = if self.script_engine.is_some() {
            "single_file_script".to_string()
        } else if self.build_system == "cargo" {
            "cargo_project".to_string()
        } else if self.build_system == "npm" {
            "npm_project".to_string()
        } else {
            "standalone".to_string()
        };

        self.packages.clear();
        self.bins.clear();
        self.examples.clear();
        self.features.clear();
        self.profiles.clear();
        self.tests.clear();
        self.benches.clear();
        self.git_branches.clear();
        self.git_remotes.clear();
        self.npm_scripts.clear();

        // Default static profiles
        self.profiles = vec![
            "dev".to_string(),
            "release".to_string(),
            "test".to_string(),
            "bench".to_string(),
        ];

        let project_root_path = self.project_root.as_ref().map(Path::new);

        if self.build_system == "cargo" {
            if let Some(root_dir) = project_root_path {
                let root_cargo = root_dir.join("Cargo.toml");
                if root_cargo.exists() {
                    let mut cargo_tomls = vec![root_cargo.clone()];
                    if let Ok(content) = std::fs::read_to_string(&root_cargo) {
                        if let Ok(toml_val) = content.parse::<toml::Value>() {
                            if let Some(members) = toml_val
                                .get("workspace")
                                .and_then(|w| w.get("members"))
                                .and_then(|m| m.as_array())
                            {
                                for m in members {
                                    if let Some(pattern) = m.as_str() {
                                        for p in resolve_member_paths(root_dir, pattern) {
                                            cargo_tomls.push(p.join("Cargo.toml"));
                                        }
                                    }
                                }
                            }
                        }
                    }

                    cargo_tomls.sort();
                    cargo_tomls.dedup();

                    for cargo_path in cargo_tomls {
                        if let Ok(content) = std::fs::read_to_string(&cargo_path) {
                            if let Ok(toml_val) = content.parse::<toml::Value>() {
                                let cargo_dir = cargo_path.parent().unwrap();

                                // package name
                                if let Some(name) = toml_val
                                    .get("package")
                                    .and_then(|p| p.get("name"))
                                    .and_then(|n| n.as_str())
                                {
                                    self.packages.push(name.to_string());
                                }

                                // bins
                                if let Some(bin_array) =
                                    toml_val.get("bin").and_then(|b| b.as_array())
                                {
                                    for entry in bin_array {
                                        if let Some(name) =
                                            entry.get("name").and_then(|n| n.as_str())
                                        {
                                            self.bins.push(name.to_string());
                                        }
                                    }
                                }
                                let bin_dir = cargo_dir.join("src").join("bin");
                                if bin_dir.is_dir() {
                                    if let Ok(entries) = std::fs::read_dir(&bin_dir) {
                                        for entry in entries.flatten() {
                                            let path = entry.path();
                                            if path.extension().and_then(|e| e.to_str())
                                                == Some("rs")
                                            {
                                                if let Some(stem) =
                                                    path.file_stem().and_then(|s| s.to_str())
                                                {
                                                    self.bins.push(stem.to_string());
                                                }
                                            } else if path.is_dir() && path.join("main.rs").exists()
                                            {
                                                if let Some(dir_name) =
                                                    path.file_name().and_then(|n| n.to_str())
                                                {
                                                    self.bins.push(dir_name.to_string());
                                                }
                                            }
                                        }
                                    }
                                }
                                if cargo_dir.join("src").join("main.rs").exists() {
                                    if let Some(pkg) = toml_val
                                        .get("package")
                                        .and_then(|p| p.get("name"))
                                        .and_then(|n| n.as_str())
                                    {
                                        self.bins.push(pkg.to_string());
                                    }
                                }

                                // examples
                                if let Some(ex_array) =
                                    toml_val.get("example").and_then(|e| e.as_array())
                                {
                                    for entry in ex_array {
                                        if let Some(name) =
                                            entry.get("name").and_then(|n| n.as_str())
                                        {
                                            self.examples.push(name.to_string());
                                        }
                                    }
                                }
                                let examples_dir = cargo_dir.join("examples");
                                if examples_dir.is_dir() {
                                    if let Ok(entries) = std::fs::read_dir(&examples_dir) {
                                        for entry in entries.flatten() {
                                            let path = entry.path();
                                            if path.extension().and_then(|e| e.to_str())
                                                == Some("rs")
                                            {
                                                if let Some(stem) =
                                                    path.file_stem().and_then(|s| s.to_str())
                                                {
                                                    self.examples.push(stem.to_string());
                                                }
                                            } else if path.is_dir() && path.join("main.rs").exists()
                                            {
                                                if let Some(dir_name) =
                                                    path.file_name().and_then(|n| n.to_str())
                                                {
                                                    self.examples.push(dir_name.to_string());
                                                }
                                            }
                                        }
                                    }
                                }

                                // features
                                if let Some(feat_table) =
                                    toml_val.get("features").and_then(|f| f.as_table())
                                {
                                    for key in feat_table.keys() {
                                        if key != "default" {
                                            self.features.push(key.clone());
                                        }
                                    }
                                }

                                // profiles
                                if let Some(profile_table) =
                                    toml_val.get("profile").and_then(|p| p.as_table())
                                {
                                    for key in profile_table.keys() {
                                        self.profiles.push(key.clone());
                                    }
                                }

                                // tests
                                if let Some(test_array) =
                                    toml_val.get("test").and_then(|t| t.as_array())
                                {
                                    for entry in test_array {
                                        if let Some(name) =
                                            entry.get("name").and_then(|n| n.as_str())
                                        {
                                            self.tests.push(name.to_string());
                                        }
                                    }
                                }
                                let tests_dir = cargo_dir.join("tests");
                                if tests_dir.is_dir() {
                                    if let Ok(entries) = std::fs::read_dir(&tests_dir) {
                                        for entry in entries.flatten() {
                                            let path = entry.path();
                                            if path.extension().and_then(|e| e.to_str())
                                                == Some("rs")
                                            {
                                                if let Some(stem) =
                                                    path.file_stem().and_then(|s| s.to_str())
                                                {
                                                    self.tests.push(stem.to_string());
                                                }
                                            }
                                        }
                                    }
                                }

                                // benches
                                if let Some(bench_array) =
                                    toml_val.get("bench").and_then(|b| b.as_array())
                                {
                                    for entry in bench_array {
                                        if let Some(name) =
                                            entry.get("name").and_then(|n| n.as_str())
                                        {
                                            self.benches.push(name.to_string());
                                        }
                                    }
                                }
                                let benches_dir = cargo_dir.join("benches");
                                if benches_dir.is_dir() {
                                    if let Ok(entries) = std::fs::read_dir(&benches_dir) {
                                        for entry in entries.flatten() {
                                            let path = entry.path();
                                            if path.extension().and_then(|e| e.to_str())
                                                == Some("rs")
                                            {
                                                if let Some(stem) =
                                                    path.file_stem().and_then(|s| s.to_str())
                                                {
                                                    self.benches.push(stem.to_string());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else if self.build_system == "npm" {
            if let Some(root_dir) = project_root_path {
                let package_json_path = root_dir.join("package.json");
                if package_json_path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&package_json_path) {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                            if self.package_name.is_none() {
                                self.package_name = json
                                    .get("name")
                                    .and_then(|n| n.as_str())
                                    .map(|s| s.to_string());
                            }
                            if let Some(scripts_obj) =
                                json.get("scripts").and_then(|s| s.as_object())
                            {
                                for key in scripts_obj.keys() {
                                    self.npm_scripts.push(key.clone());
                                }
                            }
                        }
                    }
                }
            }
        }

        // Deduplicate and sort caches
        self.packages.sort();
        self.packages.dedup();
        self.bins.sort();
        self.bins.dedup();
        self.examples.sort();
        self.examples.dedup();
        self.features.sort();
        self.features.dedup();
        self.profiles.sort();
        self.profiles.dedup();
        self.tests.sort();
        self.tests.dedup();
        self.benches.sort();
        self.benches.dedup();
        self.npm_scripts.sort();
        self.npm_scripts.dedup();

        if self.build_system == "cargo" {
            let local_cargo = Path::new(&self.cwd).join("Cargo.toml");
            let mut pkg_name = None;
            if local_cargo.exists() {
                if let Ok(content) = std::fs::read_to_string(&local_cargo) {
                    if let Ok(toml) = content.parse::<toml::Value>() {
                        pkg_name = toml
                            .get("package")
                            .and_then(|p| p.get("name"))
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string());
                    }
                }
            }
            if pkg_name.is_none() {
                pkg_name = self.packages.first().cloned();
            }
            self.package_name = pkg_name;
        }

        // Git resolution
        self.git_branches = {
            let mut cmd = Command::new("git");
            cmd.args(["branch", "--format=%(refname:short)"]);
            cmd.current_dir(&self.cwd);
            if let Ok(output) = cmd.output() {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let mut list = stdout
                        .lines()
                        .map(|l| l.trim().to_string())
                        .filter(|l| !l.is_empty())
                        .collect::<Vec<String>>();
                    list.sort();
                    list.dedup();
                    list
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        };

        self.git_remotes = {
            let mut cmd = Command::new("git");
            cmd.arg("remote");
            cmd.current_dir(&self.cwd);
            if let Ok(output) = cmd.output() {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let mut list = stdout
                        .lines()
                        .map(|l| l.trim().to_string())
                        .filter(|l| !l.is_empty())
                        .collect::<Vec<String>>();
                    list.sort();
                    list.dedup();
                    list
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        };
    }
}

fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        if current.join("Cargo.toml").exists()
            || current.join("package.json").exists()
            || current.join(".git").exists()
        {
            return Some(current);
        }
        if !current.pop() {
            break;
        }
    }
    None
}

fn detect_script_engine(file_path: &Path) -> Option<String> {
    if !file_path.is_file() {
        return None;
    }
    let content = std::fs::read_to_string(file_path).ok()?;
    let first_line = content.lines().next()?;
    if !(first_line.starts_with("#!") && content.contains("fn main(")) {
        return None;
    }
    if first_line.contains("rust-script") {
        Some("rust-script".to_string())
    } else if first_line.contains("cargo") && first_line.contains("-Zscript") {
        Some("cargo +nightly -Zscript".to_string())
    } else {
        None
    }
}

fn detect_recommended_target(
    file_path: Option<&Path>,
    script_engine: Option<&str>,
    package_name: Option<&str>,
) -> Option<String> {
    if script_engine.is_some() {
        return file_path.map(|path| path.to_string_lossy().to_string());
    }

    let Some(file_path) = file_path else {
        return package_name.map(|name| name.to_string());
    };
    let stem = file_path.file_stem().and_then(|s| s.to_str())?.to_string();
    let normalized = file_path.to_string_lossy().replace('\\', "/");

    if normalized.ends_with("/src/main.rs") || normalized.ends_with("src/main.rs") {
        return package_name.map(|name| name.to_string()).or(Some(stem));
    }
    if normalized.contains("/src/bin/") || normalized.starts_with("src/bin/") {
        return Some(stem);
    }
    if normalized.contains("/examples/") || normalized.starts_with("examples/") {
        return Some(stem);
    }
    if normalized.contains("/tests/") || normalized.starts_with("tests/") {
        return Some(stem);
    }
    if normalized.contains("/benches/") || normalized.starts_with("benches/") {
        return Some(stem);
    }
    if normalized.ends_with("build.rs") {
        return Some("build".to_string());
    }
    Some(stem)
}

fn resolve_member_paths(cwd: &Path, pattern: &str) -> Vec<PathBuf> {
    let mut results = Vec::new();
    if pattern.contains('*') {
        if let Some(prefix) = pattern.strip_suffix("/*") {
            let base = cwd.join(prefix);
            if base.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&base) {
                    for entry in entries.flatten() {
                        if entry.path().is_dir() && entry.path().join("Cargo.toml").exists() {
                            results.push(entry.path());
                        }
                    }
                }
            }
        }
    } else {
        let member_path = cwd.join(pattern);
        if member_path.join("Cargo.toml").exists() {
            results.push(member_path);
        }
    }
    results
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
