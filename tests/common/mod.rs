//! Common test doubles and helpers for flow tests.
//! Re-exports the shared FakeLlm server from `tests/support` as MockServer.

#![allow(dead_code)]

#[path = "../support/mod.rs"]
pub mod support;

pub use support::*;

use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

/// Alias `MockServer` to the shared `FakeLlm` test server.
pub type MockServer = support::FakeLlm;

pub struct FlowTestFixture {
    pub project: TempDir,
    pub home: TempDir,
    pub orch_url: String,
    pub worker_url: String,
}

impl FlowTestFixture {
    pub fn new(orch_url: &str, worker_url: &str) -> Self {
        Self::with_limits(orch_url, worker_url, json!({}))
    }

    pub fn with_limits(orch_url: &str, worker_url: &str, limits: Value) -> Self {
        let project = TempDir::new("flow_e2e_proj");
        let home = TempDir::new("flow_e2e_home");

        let git = |args: &[&str]| {
            let status = Command::new("git")
                .args(args)
                .current_dir(&project.path)
                .status()
                .expect("git failed");
            assert!(status.success(), "git {:?} failed", args);
        };

        git(&["init"]);
        git(&["config", "user.name", "Test User"]);
        git(&["config", "user.email", "test@example.com"]);

        fs::write(
            project.path.join("Cargo.toml"),
            "[package]\nname = \"flow_test\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .expect("write Cargo.toml");

        let src_dir = project.path.join("src");
        fs::create_dir_all(&src_dir).expect("create src dir");
        fs::write(src_dir.join("main.rs"), "fn main() {}\n").expect("write main.rs");
        fs::write(src_dir.join("lib.rs"), "// lib\n").expect("write lib.rs");

        let settings_dir = project.path.join(".arqus");
        fs::create_dir_all(&settings_dir).expect("create .arqus dir");

        let mut flow_val = json!({
            "orchestrator": {
                "provider": "openai",
                "model": "orch-model",
                "base_url": orch_url,
                "api_key_env": "ARQUS_ORCH_KEY"
            },
            "routing": {
                "default_worker": "worker-1"
            },
            "workers": [
                {
                    "name": "worker-1",
                    "provider": "openai",
                    "model": "worker-model",
                    "base_url": worker_url,
                    "api_key_env": "",
                    "max_prompt_tokens": 4096,
                    "max_output_tokens": 2048,
                    "limits": {
                        "max_context_files": 5,
                        "max_write_files": 5,
                        "max_est_changed_lines": 100
                    }
                }
            ],
            "sandbox": {
                "allowed_commands": ["cargo test", "cargo check", "true", "false", "exit 0", "exit 1"],
                "protected_paths": [".git/**", ".arqus/flow/**", ".github/**"],
                "env_passthrough": ["PATH", "HOME", "CARGO_HOME", "RUSTUP_HOME"]
            }
        });

        if let Value::Object(lims) = limits {
            if !lims.is_empty() {
                flow_val["limits"] = Value::Object(lims);
            }
        }

        let settings = json!({ "flow": flow_val });
        fs::write(
            settings_dir.join("settings.json"),
            serde_json::to_string_pretty(&settings).unwrap(),
        )
        .expect("write settings.json");

        git(&["add", "."]);
        git(&["commit", "-m", "Initial commit"]);

        Self {
            project,
            home,
            orch_url: orch_url.to_string(),
            worker_url: worker_url.to_string(),
        }
    }

    pub fn run_flow(&self, objective: &str) -> Output {
        self.run_flow_with_env(objective, &[])
    }

    pub fn run_flow_with_env(&self, objective: &str, extra_env: &[(&str, &str)]) -> Output {
        let mut env = vec![
            ("ARQUS_ORCH_KEY", "orch_secret"),
            ("ARQUS_WORKER_KEY", "worker_secret"),
            ("ARQUS_RETRY_BASE_MS", "5"),
            ("ARQUS_MAX_RETRIES", "1"),
        ];
        env.extend_from_slice(extra_env);
        run_arqus(
            &self.project.path,
            &self.home.path,
            &self.orch_url,
            &["flow", "run", objective],
            &env,
        )
    }

    pub fn run_flow_args(&self, args: &[&str], extra_env: &[(&str, &str)]) -> Output {
        let mut env = vec![
            ("ARQUS_ORCH_KEY", "orch_secret"),
            ("ARQUS_WORKER_KEY", "worker_secret"),
            ("ARQUS_RETRY_BASE_MS", "5"),
            ("ARQUS_MAX_RETRIES", "1"),
        ];
        env.extend_from_slice(extra_env);
        let mut full_args = vec!["flow"];
        full_args.extend_from_slice(args);
        run_arqus(
            &self.project.path,
            &self.home.path,
            &self.orch_url,
            &full_args,
            &env,
        )
    }

    pub fn run_id(&self) -> Option<String> {
        let branch = self.get_flow_branch()?;
        branch.strip_prefix("arqus/flow/").map(|s| s.to_string())
    }

    pub fn get_flow_branch(&self) -> Option<String> {
        let out = Command::new("git")
            .args(["branch", "--list", "arqus/flow/*"])
            .current_dir(&self.project.path)
            .output()
            .ok()?;
        let s = String::from_utf8_lossy(&out.stdout);
        s.lines()
            .map(|l| {
                l.trim()
                    .trim_start_matches(['*', '+', ' '])
                    .trim()
                    .to_string()
            })
            .find(|b| b.starts_with("arqus/flow/"))
    }

    pub fn get_commits_on_flow_branch(&self) -> Vec<String> {
        let branch = match self.get_flow_branch() {
            Some(b) => b,
            None => return vec![],
        };
        let out = Command::new("git")
            .args(["log", &format!("HEAD..{branch}"), "--oneline"])
            .current_dir(&self.project.path)
            .output()
            .expect("git log");
        let s = String::from_utf8_lossy(&out.stdout);
        s.lines().map(|l| l.to_string()).collect()
    }

    pub fn get_commit_bodies_on_flow_branch(&self) -> Vec<String> {
        let branch = match self.get_flow_branch() {
            Some(b) => b,
            None => return vec![],
        };
        let out = Command::new("git")
            .args(["log", &format!("HEAD..{branch}"), "--format=%B---END---"])
            .current_dir(&self.project.path)
            .output()
            .expect("git log");
        let s = String::from_utf8_lossy(&out.stdout);
        s.split("---END---")
            .map(|b| b.trim().to_string())
            .filter(|b| !b.is_empty())
            .collect()
    }

    pub fn flow_dir(&self) -> Option<PathBuf> {
        let branch = self.get_flow_branch()?;
        let run_id = branch.strip_prefix("arqus/flow/")?;
        Some(self.project.path.join(".arqus").join("flow").join(run_id))
    }
}

pub fn make_file_edit(path: &str, content: &str) -> String {
    format!("<<<<<<< FILE {path}\n{content}\n>>>>>>> END\n")
}

pub fn make_search_replace_edit(path: &str, search: &str, replace: &str) -> String {
    format!("<<<<<<< SEARCH {path}\n{search}\n=======\n{replace}\n>>>>>>> REPLACE\n")
}

pub fn plan_json(tasks: Vec<Value>, final_verify: Vec<&str>) -> String {
    json!({
        "type": "plan",
        "summary": "E2E plan",
        "tasks": tasks,
        "final_verify": final_verify
    })
    .to_string()
}

pub fn single_task(id: &str, assignee: &str, verify: &[&str], allowed_writes: &[&str]) -> Value {
    json!({
        "id": id,
        "title": format!("Task {id}"),
        "category": "single_function",
        "assignee": assignee,
        "objective": format!("Objective for {id}"),
        "context": [{ "path": "src/main.rs" }],
        "allowed_writes": allowed_writes,
        "depends_on": [],
        "constraints": [],
        "acceptance_criteria": [],
        "interface_notes": "",
        "verify": verify,
        "est_changed_lines": 10
    })
}
