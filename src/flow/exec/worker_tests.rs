use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

use super::*;
use crate::ai::{Llm, StreamEvent};
use crate::flow::config::defaults::default_flow_settings;
use crate::flow::config::types::{FlowLimits, SandboxConfig, WorkerConfig, WorkerLimits};
use crate::flow::schema::types::{
    Assignee, Category, FailureKind, RunState, RunStatus, TaskContract, TaskState, TaskStatus,
};
use crate::types::{Block, Message, StopReason, Usage};

fn run_git(cwd: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("git");
    assert!(
        out.status.success(),
        "git failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn init_git_worktree() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let repo = dir.path().to_path_buf();
    run_git(&repo, &["init"]);
    run_git(&repo, &["config", "user.name", "Test User"]);
    run_git(&repo, &["config", "user.email", "test@example.com"]);
    std::fs::write(repo.join("file_a.txt"), "hello world\n").unwrap();
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-m", "Initial commit"]);
    (dir, repo)
}

fn mock_llm(responses: Vec<String>) -> (Llm, Arc<Mutex<Vec<String>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let queue = Arc::new(Mutex::new(VecDeque::new()));
    for resp in responses {
        queue.lock().unwrap().push_back(vec![
            StreamEvent::TextDelta(resp.clone()),
            StreamEvent::Done(Message::Assistant {
                blocks: vec![Block::Text { text: resp }],
                provider: "mock".to_string(),
                model: "mock-worker".to_string(),
                usage: Some(Usage {
                    input: 50,
                    output: 25,
                    cache_read: 0,
                    cache_write: 0,
                    cost: None,
                }),
                stop: StopReason::Stop,
            }),
        ]);
    }
    (Llm::MockCapture(queue, Arc::clone(&seen)), seen)
}

fn test_worker_config() -> WorkerConfig {
    WorkerConfig {
        name: "local-worker".to_string(),
        provider: "openai".to_string(),
        model: "test-model".to_string(),
        base_url: "http://127.0.0.1:11434/v1".to_string(),
        api_key_env: "TEST_KEY".to_string(),
        temperature: Some(0.1),
        max_prompt_tokens: 4000,
        max_output_tokens: 1000,
        extra_body: None,
        strengths: vec![],
        weaknesses: vec![],
        limits: WorkerLimits {
            max_context_files: 5,
            max_write_files: 3,
            max_est_changed_lines: 100,
        },
    }
}

fn test_contract() -> TaskContract {
    TaskContract {
        id: "T01".to_string(),
        title: "Update file_a".to_string(),
        category: Category::Boilerplate,
        assignee: Assignee::Worker,
        objective: "Add greeting to file_a".to_string(),
        context: vec![],
        allowed_writes: vec!["file_a.txt".to_string()],
        depends_on: vec![],
        constraints: vec![],
        acceptance_criteria: vec![],
        interface_notes: String::new(),
        verify: vec!["git status".to_string()],
        est_changed_lines: 5,
    }
}

fn test_run_state() -> RunState {
    RunState {
        run_id: "run-123".to_string(),
        objective: "Test objective".to_string(),
        repo: "/tmp/repo".to_string(),
        base_commit: "abc1234".to_string(),
        branch: "arqus/flow/run-123".to_string(),
        status: RunStatus::Running,
        tasks: BTreeMap::new(),
        final_verify: vec![],
        plan_summary: "summary".to_string(),
    }
}

struct Fixture {
    _dir: TempDir,
    repo: PathBuf,
    contract: TaskContract,
    ts: TaskState,
    limits: FlowLimits,
    sandbox: SandboxConfig,
    run_state: RunState,
}

impl Fixture {
    fn new() -> Self {
        let (_dir, repo) = init_git_worktree();
        let contract = test_contract();
        let ts = TaskState {
            contract: contract.clone(),
            status: TaskStatus::Running,
            executor: Assignee::Worker,
            worker_attempts: 0,
            redesigns: 0,
            takeover_attempts: 0,
            split_depth: 0,
            failures: vec![],
            commit: None,
            changed_files: vec![],
        };
        let s = default_flow_settings();
        let mut sandbox = s.sandbox.unwrap();
        sandbox.allowed_commands.push("git status".to_string());
        Self {
            _dir,
            repo,
            contract,
            ts,
            limits: s.limits.unwrap(),
            sandbox,
            run_state: test_run_state(),
        }
    }

    async fn run(&mut self, llm: &Llm, cfg: &WorkerConfig) -> WorkerRoundResult {
        let ctx = WorkerContext {
            worktree: &self.repo,
            run_state: &self.run_state,
            limits: &self.limits,
            sandbox: &self.sandbox,
            run_id: "run-123",
        };
        run_worker_rounds(llm, cfg, &self.contract, &mut self.ts, &ctx, None).await
    }
}

#[tokio::test]
async fn test_worker_success_on_first_call() {
    let mut f = Fixture::new();
    let reply = "<<<<<<< FILE file_a.txt\nhello world\nnew line\n>>>>>>> END\n".to_string();
    let (llm, seen) = mock_llm(vec![reply]);
    let res = f.run(&llm, &test_worker_config()).await;
    assert!(res.is_accepted());
    assert_eq!(seen.lock().unwrap().len(), 1);
    assert_eq!(f.ts.worker_attempts, 1);
    assert_eq!(
        std::fs::read_to_string(f.repo.join("file_a.txt")).unwrap(),
        "hello world\nnew line\n"
    );
}

#[tokio::test]
async fn test_worker_second_call_carries_failure_kind_and_truncated_previous_output() {
    let mut f = Fixture::new();
    let bad =
        "<<<<<<< SEARCH file_a.txt\nnon_existent\n=======\nrep\n>>>>>>> REPLACE\n".to_string();
    let good = "<<<<<<< FILE file_a.txt\nrecovered\n>>>>>>> END\n".to_string();
    let (llm, seen) = mock_llm(vec![bad, good]);
    let res = f.run(&llm, &test_worker_config()).await;
    assert!(res.is_accepted());
    assert_eq!(seen.lock().unwrap().len(), 2);
    assert_eq!(f.ts.worker_attempts, 2);
    let req2 = &seen.lock().unwrap()[1];
    assert!(req2.contains("no_match"));
    assert!(req2.contains("non_existent"));
}

#[tokio::test]
async fn test_worker_failure_after_max_attempts_returns_failure_record() {
    let mut f = Fixture::new();
    let bad1 = "<<<<<<< SEARCH file_a.txt\nmissing1\n=======\nrep\n>>>>>>> REPLACE\n".to_string();
    let bad2 = "<<<<<<< SEARCH file_a.txt\nmissing2\n=======\nrep\n>>>>>>> REPLACE\n".to_string();
    let (llm, seen) = mock_llm(vec![bad1, bad2]);
    let res = f.run(&llm, &test_worker_config()).await;
    match res {
        WorkerRoundResult::Failed(record) => {
            assert_eq!(record.kind, FailureKind::NoMatch);
            assert_eq!(record.attempt, 2);
            assert_eq!(record.executor, Assignee::Worker);
        }
        _ => panic!("Expected WorkerRoundResult::Failed"),
    }
    assert_eq!(seen.lock().unwrap().len(), 2);
    assert_eq!(f.ts.worker_attempts, 2);
}

#[tokio::test]
async fn test_worker_declined_recorded_as_such() {
    let mut f = Fixture::new();
    let (llm, _seen) = mock_llm(vec![
        "CANNOT_COMPLETE: task too complex\n".into(),
        "CANNOT_COMPLETE: task too complex\n".into(),
    ]);
    let res = f.run(&llm, &test_worker_config()).await;
    match res {
        WorkerRoundResult::Failed(record) => {
            assert_eq!(record.kind, FailureKind::Declined);
            assert!(record.detail.contains("too complex"));
        }
        _ => panic!("Expected WorkerRoundResult::Failed"),
    }
}

#[tokio::test]
async fn test_worker_scope_violation_triggers_reset() {
    let mut f = Fixture::new();
    let bad1 = "<<<<<<< FILE file_b.txt\nforbidden write\n>>>>>>> END\n".to_string();
    let bad2 = "<<<<<<< FILE file_b.txt\nforbidden again\n>>>>>>> END\n".to_string();
    let (llm, _seen) = mock_llm(vec![bad1, bad2]);
    let res = f.run(&llm, &test_worker_config()).await;
    match res {
        WorkerRoundResult::Failed(record) => assert_eq!(record.kind, FailureKind::ScopeViolation),
        _ => panic!("Expected WorkerRoundResult::Failed"),
    }
    assert!(!f.repo.join("file_b.txt").exists());
}

#[tokio::test]
async fn test_worker_verify_failure_resets_worktree() {
    let mut f = Fixture::new();
    f.contract.verify = vec!["false".to_string()];
    f.sandbox.allowed_commands.push("false".to_string());
    let bad1 = "<<<<<<< FILE file_a.txt\ncorrupted\n>>>>>>> END\n".to_string();
    let bad2 = "<<<<<<< FILE file_a.txt\ncorrupted2\n>>>>>>> END\n".to_string();
    let (llm, _seen) = mock_llm(vec![bad1, bad2]);
    let res = f.run(&llm, &test_worker_config()).await;
    match res {
        WorkerRoundResult::Failed(record) => assert_eq!(record.kind, FailureKind::VerifyFailed),
        _ => panic!("Expected WorkerRoundResult::Failed"),
    }
    assert_eq!(
        std::fs::read_to_string(f.repo.join("file_a.txt")).unwrap(),
        "hello world\n"
    );
}
