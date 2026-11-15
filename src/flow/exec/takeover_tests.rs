use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

use super::*;
use crate::ai::{Llm, StreamEvent};
use crate::flow::config::defaults::default_flow_settings;
use crate::flow::config::types::{FlowLimits, OrchestratorConfig, SandboxConfig};
use crate::flow::schema::types::{
    Assignee, Category, FailureKind, FailureRecord, RunState, RunStatus, TaskContract, TaskState,
    TaskStatus,
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
                model: "orchestrator-model".to_string(),
                usage: Some(Usage {
                    input: 100,
                    output: 50,
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

fn test_orch_config() -> OrchestratorConfig {
    OrchestratorConfig {
        provider: "openrouter".to_string(),
        model: "deepseek/v3".to_string(),
        base_url: "https://openrouter.ai/api/v1".to_string(),
        api_key_env: "TEST_ORCH_KEY".to_string(),
        temperature: Some(0.2),
        max_output_tokens: Some(4096),
        price: None,
        extra_body: None,
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
            executor: Assignee::Orchestrator,
            worker_attempts: 2,
            redesigns: 1,
            takeover_attempts: 0,
            split_depth: 0,
            failures: vec![
                FailureRecord::new(
                    FailureKind::NoMatch,
                    "worker failed match",
                    1,
                    Assignee::Worker,
                ),
                FailureRecord::new(
                    FailureKind::VerifyFailed,
                    "worker failed verify",
                    2,
                    Assignee::Worker,
                ),
            ],
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

    async fn run(
        &mut self,
        llm: &Llm,
        cfg: &OrchestratorConfig,
        ledger: Option<&mut Ledger>,
    ) -> TakeoverRoundResult {
        let ctx = TakeoverContext {
            worktree: &self.repo,
            run_state: &self.run_state,
            limits: &self.limits,
            sandbox: &self.sandbox,
            run_id: "run-123",
        };
        run_takeover_rounds(llm, cfg, &self.contract, &mut self.ts, &ctx, ledger).await
    }
}

#[tokio::test]
async fn test_takeover_prompt_includes_failure_history() {
    let mut f = Fixture::new();
    let reply = "<<<<<<< FILE file_a.txt\nresolved by orchestrator\n>>>>>>> END\n".to_string();
    let (llm, seen) = mock_llm(vec![reply]);
    let res = f.run(&llm, &test_orch_config(), None).await;
    assert!(res.is_accepted());
    let req = &seen.lock().unwrap()[0];
    assert!(req.contains("PRIOR ATTEMPTS FAILURE HISTORY"));
    assert!(req.contains("worker failed match"));
    assert!(req.contains("worker failed verify"));
}

#[tokio::test]
async fn test_takeover_goes_to_orchestrator_model_never_worker() {
    let mut f = Fixture::new();
    let reply = "<<<<<<< FILE file_a.txt\norch\n>>>>>>> END\n".to_string();
    let (llm, _seen) = mock_llm(vec![reply]);
    let mut ledger = crate::flow::model::ledger::Ledger::new(None);
    let res = f.run(&llm, &test_orch_config(), Some(&mut ledger)).await;
    assert!(res.is_accepted());
    assert_eq!(f.ts.executor, Assignee::Orchestrator);
    let totals = ledger.totals();
    assert_eq!(
        totals
            .by_role
            .get(&crate::flow::model::ledger::Role::Takeover)
            .map(|r| r.requests),
        Some(1)
    );
    assert_eq!(
        totals
            .by_role
            .get(&crate::flow::model::ledger::Role::Worker)
            .map(|r| r.requests),
        None
    );
}

#[tokio::test]
async fn test_takeover_success_on_first_attempt() {
    let mut f = Fixture::new();
    let reply = "<<<<<<< FILE file_a.txt\nsuccess\n>>>>>>> END\n".to_string();
    let (llm, seen) = mock_llm(vec![reply]);
    let res = f.run(&llm, &test_orch_config(), None).await;
    assert!(res.is_accepted());
    assert_eq!(seen.lock().unwrap().len(), 1);
    assert_eq!(f.ts.takeover_attempts, 1);
    assert_eq!(
        std::fs::read_to_string(f.repo.join("file_a.txt")).unwrap(),
        "success\n"
    );
}

#[tokio::test]
async fn test_takeover_second_attempt_retry_feedback() {
    let mut f = Fixture::new();
    let bad = "<<<<<<< SEARCH file_a.txt\nmissing\n=======\nrep\n>>>>>>> REPLACE\n".to_string();
    let good = "<<<<<<< FILE file_a.txt\nrecovered\n>>>>>>> END\n".to_string();
    let (llm, seen) = mock_llm(vec![bad, good]);
    let res = f.run(&llm, &test_orch_config(), None).await;
    assert!(res.is_accepted());
    assert_eq!(seen.lock().unwrap().len(), 2);
    assert_eq!(f.ts.takeover_attempts, 2);
    let req2 = &seen.lock().unwrap()[1];
    assert!(req2.contains("PREVIOUS TAKEOVER ATTEMPT FAILED"));
    assert!(req2.contains("no_match"));
}

#[tokio::test]
async fn test_takeover_exhausted_attempts_returns_failure_record() {
    let mut f = Fixture::new();
    let bad1 = "<<<<<<< SEARCH file_a.txt\nmissing1\n=======\nrep\n>>>>>>> REPLACE\n".to_string();
    let bad2 = "<<<<<<< SEARCH file_a.txt\nmissing2\n=======\nrep\n>>>>>>> REPLACE\n".to_string();
    let (llm, seen) = mock_llm(vec![bad1, bad2]);
    let res = f.run(&llm, &test_orch_config(), None).await;
    match res {
        TakeoverRoundResult::Failed(record) => {
            assert_eq!(record.kind, FailureKind::NoMatch);
            assert_eq!(record.attempt, 2);
            assert_eq!(record.executor, Assignee::Orchestrator);
        }
        _ => panic!("Expected TakeoverRoundResult::Failed"),
    }
    assert_eq!(seen.lock().unwrap().len(), 2);
    assert_eq!(f.ts.takeover_attempts, 2);
}
