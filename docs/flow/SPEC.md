# Implementation Spec: arqus flow

*Rewritten for Rust and the arqus codebase on 15 Nov 2026. It is based on the arqus README (v0.2.0) only. I have not read the source, so every arqus internal named below is an assumption that Stage F0 must verify against the code. MUST means a test has to enforce it (see document 6).*

## 1. Goal and scope

**Primary goal:** minimise cloud tokens (orchestrator side) per solved task, with accuracy at least equal to the existing arqus agent running the same orchestrator model. Accuracy is the constraint and token saving is the objective. The eval suite measures both; nothing here assumes the saving exists.

**Baseline:** `arqus -p`, the current agent loop with read, write, edit and bash. It is a real agent, so the bar is higher than the pipeline baseline in my earlier drafts. The README reports 13 of 13 eval tasks passing for $0.0073 in total on a flash-tier model, so the existing eval suite cannot tell the two approaches apart (document 6 deals with this).

**v1 includes:** `arqus flow`, a print-mode subcommand with a planner that splits an objective into nano-task contracts aware of the configured local workers' capabilities; one-shot local workers; deterministic verification; a fallback ladder (retry, redesign, orchestrator takeover); orchestrator and workers configurable in `settings.json`; persistence and resume; per-role token and cost accounting; the eval suite.

**v1 excludes:** parallel execution, tool calls or loops for workers, per-category model selection (hook only), LLM code review, vector search, TUI integration, MCP use inside flow, non-git repositories, automatic repair of final-verification failures, a cloud-only pipeline mode.

## 2. Design decisions

1. **Workers are one-shot editors, not agents.** A worker gets one prompt and returns edit blocks. No tool calls, no loops, no shell. This removes tool-call reliability as a risk for small models and makes worker cost predictable. The harness runs verification.
2. **The orchestrator references code and never copies it.** Contracts name files and line ranges. The harness injects file contents into worker prompts locally, so orchestrator output stays small.
3. **Context is assembled deterministically** from the contract and checked against the worker's prompt budget. A contract that does not fit is rejected before any worker runs.
4. **Sequential execution in one git worktree on a new branch.** Accept means verify passes, then commit. Reject means hard reset. The user's working tree and branch are never touched.
5. **Success is decided only by deterministic checks:** scope check, verify commands, final verify. No LLM judge in v1.
6. **Structured model output is JSON in plain text**, extracted and validated with serde. On failure there is one bounded repair round with the validator's messages, then the ladder or an abort. (Provider-side JSON-schema modes are out of scope for v1.)
7. **Cloud prompts put stable material first** (rules, capability sheet, repo map) and varying material last, to maximise provider prompt caching.
8. **Everything is on disk** (state, events, usage), so any run can be inspected or resumed after a crash.
9. **Reuse before writing.** Anything arqus already does (streaming, retries, SSE, edit matching, process groups, path safety, cost totals, redaction) is reused, extracted into a shared function, or extended additively. flow never carries a second copy.

## 3. What arqus already gives us

Assumed from the README. Stage F0 confirms each row against the code and records the real names in `docs/flow/NOTES.md`.

| Need | Existing piece | Action |
| --- | --- | --- |
| Streaming calls to OpenAI-compatible servers, OpenRouter, Anthropic, with retries and idle timeout | `src/ai/` (`Llm`, `LlmRequest`, `StreamEvent`, adapters) | Reuse. Additive change: pass an optional `extra_body` JSON object into the OpenAI-compatible request body |
| Scripted provider for tests | `src/ai/mock.rs` | Reuse. Extend additively if it cannot script errors or record requests |
| Mock SSE HTTP server for black-box tests | helpers used by `tests/e2e_*` | Reuse. If each test file has its own copy, extract one into `tests/common/` first |
| Exact-once edit matching, CRLF handling, permissions, atomic write | `src/tools/edit.rs`, `write.rs` | Extract the pure matching and write functions so flow calls them |
| Process groups, timeouts, output caps, kill on cancel | `src/tools/bash.rs`, `src/mcp/` | Extract one shared helper if the two do not already share one |
| Path checks, risky-command patterns, git notice | `src/safety.rs` | Reuse for model-supplied paths and as a second line of defence for verify commands |
| Settings with global and project precedence | `src/settings.rs` | Add the `flow` section |
| Cost totals, limits, summary line | `src/cost.rs` | Reuse. flow adds per-role tallies on top |
| Redacted JSON-lines debug log | `src/debug.rs` | Reuse. The redactor MUST also cover every flow role key |
| clap CLI, exit codes 0, 1, 2 | `src/cli.rs`, `src/main.rs` | Add a `flow` subcommand without changing existing behaviour |
| Evals, baseline and results | `evals/run.py`, `evals/BASELINE.md`, `scripts/run-evals.sh` | The existing agent is condition D0. Add flow eval files beside them |
| Docker sandbox wrapper | `scripts/arqus-docker`, `Dockerfile` | Forward the new key variables |
| Gate script | `scripts/check.sh` | Add the flow audit step |

**Deliberately not reused:** the agent loop, sessions and `build_context`, compaction, the TUI and MCP. Workers must start from a fresh context, so there is nothing in a session worth carrying over.

## 4. Layout and conventions

```
src/flow/
  mod.rs  cli.rs  config.rs  schema.rs  errors.rs
  model/     mod.rs  complete.rs  json.rs  ledger.rs
  repo/      git.rs  map.rs  context.rs
  planning/  prompts.rs  planner.rs  validator.rs  redesign.rs  capability.rs
  exec/      edits.rs  worker.rs  takeover.rs  verify.rs  sandbox.rs
  engine/    state.rs  scheduler.rs  run_loop.rs  router.rs  report.rs
tests/       e2e_flow_plan.rs  e2e_flow_ladder.rs  e2e_flow_resume.rs  live_flow.rs  common/
evals/       flow_run.py  flow_metrics.py  flow_report.py  flow/  tests/
scripts/     audit-flow.sh  run-flow-evals.sh  build-flow-profile.sh
docs/flow/   SPEC.md  TESTS.md  STAGES.md  NOTES.md
```

Any file that would pass 300 lines is split into a submodule. Unit tests live in sibling `tests.rs` or `tests_<topic>.rs` files, never inline.

Conventions (they extend the README's contributor rules):

- Rust edition 2021. Prefer `String`, `Vec<T>`, `Option<T>` and `anyhow::Result<T>`. No lifetimes or generic abstractions unless forced.
- New crates: `shlex`; dev only: `proptest`, `tempfile`. Nothing else without asking.
- Sequential only. No `tokio::spawn`, `join!` or threads in `src/flow`. Calls are async only because `Llm` is.
- Typed error enums in `flow/errors.rs` with hand-written `Display`, converted to `anyhow` at the CLI boundary. No `thiserror`.
- No `unwrap`, `expect`, `panic!`, `todo!`, `unsafe` or `println!` outside tests and `cli.rs`.
- Import direction: flow may import `types`, `ai`, `cost`, `safety`, `settings`, `debug` and the extracted helpers. It MUST NOT import `session`, `tui`, `agent`, `runtime` or `compaction`. Only `main.rs` imports flow.
- Tests first. Every bug fix starts with a reproducing test.

## 4a. Configuration (settings.json)

```json
{
  "flow": {
    "orchestrator": {
      "provider": "openrouter",
      "model": "deepseek/deepseek-v4.1-flash-1017",
      "base_url": "https://openrouter.ai/api/v1",
      "api_key_env": "ARQUS_API_KEY",
      "temperature": 0.2,
      "max_output_tokens": 8192,
      "price": { "input": 0.15, "cached_input": 0.003, "output": 0.60 }
    },
    "workers": [
      {
        "name": "local-9b",
        "provider": "openai",
        "model": "qwen3.5:9b",
        "base_url": "http://127.0.0.1:11434/v1",
        "api_key_env": "",
        "temperature": 0.1,
        "max_prompt_tokens": 12000,
        "max_output_tokens": 4096,
        "extra_body": {},
        "strengths": ["boilerplate", "tests", "single_function", "docs"],
        "weaknesses": ["multi_file", "ambiguous_requirements"],
        "limits": { "max_context_files": 3, "max_write_files": 2, "max_est_changed_lines": 80 }
      }
    ],
    "routing": { "default_worker": "local-9b", "by_category": {} },
    "limits": {
      "max_worker_attempts": 2,
      "max_redesigns": 1,
      "max_takeover_attempts": 2,
      "max_split_depth": 1,
      "max_planning_repairs": 2,
      "max_file_read_rounds": 2,
      "max_tasks": 25,
      "max_cloud_tokens_per_run": 400000,
      "task_timeout_secs": 600,
      "verify_timeout_secs": 300,
      "max_changed_lines_hard": 400,
      "whole_file_max_lines": 150
    },
    "sandbox": {
      "allowed_commands": ["cargo test", "cargo build", "cargo clippy", "pytest", "python3 -m pytest", "go test", "go build", "go vet", "npm test", "npx tsc", "npx vitest"],
      "protected_paths": [".git/**", ".arqus/flow/**", ".github/**"],
      "env_passthrough": ["PATH", "HOME", "CARGO_HOME", "RUSTUP_HOME", "GOPATH", "GOCACHE", "GOMODCACHE", "npm_config_cache"]
    }
  }
}
```

The model names, base URLs and prices above are examples. The orchestrator entry reuses the model named in the README's baseline run. Check every value against the provider's own docs. `arqus flow models-check` exists to catch mistakes.

Rules:

- The `flow` object follows the existing precedence: CLI flag, environment variable, project settings, global settings, built-in default. Objects merge key by key. The `workers` array is replaced wholesale, never merged.
- Provider, model and base URL are separate fields. Do not use the `provider:model` string here, because model names such as `qwen3.5:9b` contain a colon (a test MUST cover this).
- Keys come only from the environment variable named by `api_key_env`. An empty name sends the literal placeholder `local` (for local servers). Keys are never stored in settings, printed or logged, and the redactor MUST cover every resolved role key, not only `ARQUS_API_KEY`.
- `price` is optional and used only when the provider does not report cost. OpenRouter does.
- Unknown keys under `flow` are errors. A malformed `flow` section makes `arqus flow` exit 2 and leaves every other mode unaffected.
- `routing.by_category` MUST parse but be ignored in v1, with one logged warning if it is non-empty. The `Router` trait (section 14) exists so a category router can be added later without touching the engine.
- `extra_body` is merged shallowly into the request JSON, so provider quirks (for example turning off a thinking mode) live in settings, not code.
- Cloud budget: `max_cloud_tokens_per_run` counts planner, redesign and takeover calls only. The existing `limits.max_cost_per_run` and `--max-cost` also apply to cloud spend. Whichever trips first stops the run.

## 5. Data models (schema.rs)

```rust
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Category { Boilerplate, SingleFunction, Tests, Docs, LocalRefactor, LocalBugfix, MultiFile, Other }

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Assignee { Worker, Orchestrator }

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct FileRef { pub path: String, pub start_line: Option<u32>, pub end_line: Option<u32> }

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct TaskContract {
    pub id: String,                  // T plus 2-3 digits, optional .N segments: T07, T07.1
    pub title: String,               // <= 80 chars
    pub category: Category,
    pub assignee: Assignee,
    pub objective: String,           // <= 600 chars, imperative, one outcome
    pub context: Vec<FileRef>,       // read-only inputs injected by the harness
    pub allowed_writes: Vec<String>, // exact repo-relative paths; new files must be listed
    pub depends_on: Vec<String>,
    pub constraints: Vec<String>,    // <= 6 items, each <= 200 chars
    pub acceptance_criteria: Vec<String>, // <= 6 items
    #[serde(default)]
    pub interface_notes: String,     // <= 800 chars: signatures to honour, no code bodies
    pub verify: Vec<String>,         // commands; allowlisted
    pub est_changed_lines: u32,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct Plan { pub summary: String, pub tasks: Vec<TaskContract>, pub final_verify: Vec<String> }

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlannerReply {
    Plan { summary: String, tasks: Vec<TaskContract>, final_verify: Vec<String> },
    NeedFiles { files: Vec<FileRef> },   // <= 8 per round
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    NoOutput, Declined, ParseError, NoMatch, AmbiguousMatch, ScopeViolation,
    VerifyFailed, Timeout, ProviderError, BudgetExceeded,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct FailureRecord { pub kind: FailureKind, pub detail: String, pub attempt: u32, pub executor: Assignee }

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum RedesignDecision {
    Revise { reason: String, revised: TaskContract },
    Split { reason: String, split: Vec<TaskContract> },   // ids are <old>.1, <old>.2, ...
    Takeover { reason: String },
}

pub enum Edit {
    Replace { path: String, search: String, replace: String },
    File { path: String, content: String },
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus { Pending, Ready, Running, Redesigning, Done, Failed, Skipped, Superseded }

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TaskState {
    pub contract: TaskContract, pub status: TaskStatus, pub executor: Assignee,
    pub worker_attempts: u32, pub redesigns: u32, pub takeover_attempts: u32, pub split_depth: u32,
    pub failures: Vec<FailureRecord>, pub commit: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RunState {
    pub run_id: String, pub objective: String, pub repo: String, pub base_commit: String, pub branch: String,
    pub status: RunStatus,            // Planning, Running, Finished, BudgetStopped, Failed
    pub tasks: std::collections::BTreeMap<String, TaskState>,
    pub final_verify: Vec<String>, pub plan_summary: String,
}
```

Task id syntax and every length limit are checked by a hand-written `validate_shape(&TaskContract) -> Vec<Issue>` (no regex crate). The model call helper is `complete_text(llm, request) -> Result<Completion>`, which consumes the existing stream, joins the text, and returns the provider's usage (input, output, cache read, cache write) plus latency. Follow the existing pattern for how `Runtime` holds and calls `Llm`; do not invent a new trait.

## 6. Run algorithm

```
run(objective):
  require clean git repo; create branch arqus/flow/<run_id> and worktree W at <repo>/.arqus/flow/<run_id>/worktree
  (append .arqus/flow/ to .git/info/exclude; never edit .gitignore)
  plan = planner.plan(objective)          # section 7; exit 1 if no valid plan; the repo is untouched
  while let Some(task) = scheduler.next_ready():   # lowest id among READY, deterministic
      execute_task(task)
  run final_verify in W; write report; exit 0 only if every task is DONE and final verify passed

execute_task(ts):
  ts.status = Running
  if ts.contract.assignee == Orchestrator: return run_takeover(ts)
  loop:
    res = run_worker_rounds(ts)            # up to max_worker_attempts calls; failure feedback between calls
    if res.accepted: return accept(ts)
    ts.failures.push(res.failure)
    if ts.redesigns >= max_redesigns: return run_takeover(ts)
    ts.status = Redesigning; ts.redesigns += 1
    dec = redesign(ts)                     # 1 orchestrator call; any failure or invalid output => takeover
    match dec:
      Revise:   validate revised contract (same rules as the validator); invalid => takeover
                ts.contract = revised; ts.worker_attempts = 0; ts.status = Running; continue
      Split:    if ts.split_depth >= max_split_depth or subtasks invalid => takeover
                else replace_with_subtasks(ts); return
      Takeover: return run_takeover(ts)

run_takeover(ts):
  ts.executor = Orchestrator
  for up to max_takeover_attempts: orchestrator one-shot edit attempt with feedback
  accepted => accept(ts); else mark FAILED and mark transitive dependents SKIPPED

one attempt (worker or takeover):
  prompt = build_prompt(contract, executor)      # deterministic; enforces the budget
  completion = complete_text(...); record usage
  edits = parse_edits(completion.text)           # a CANNOT_COMPLETE line => Declined
  apply all edits in memory; stop at the first problem   # atomic: nothing is written unless all apply
  scope check; write files; run contract.verify commands in W
  pass => accepted; any failure => FailureRecord, reset_hard_clean(W)

accept(ts): git add -A; commit 'arqus flow: <id> <title>' with executor and model in the body; ts.status = Done
```

Allowed status transitions (anything else is `FlowError::InvalidTransition`): Pending to Ready when all dependencies are Done; Pending to Skipped when any dependency is Failed or Skipped; Ready to Running; Running to Done, Redesigning or Failed; Redesigning to Running, Superseded or Failed. On a split, the new subtasks start Pending with the old task's dependencies. Tasks that depended on the old task are rewired to depend on every subtask that no other subtask depends on. The old task becomes Superseded.

Budget: before every cloud call, compare the running cloud-token total with `max_cloud_tokens_per_run` and the running cloud cost with the arqus cost limits. If either is exceeded, stop cleanly with status `BudgetStopped`, exit 1, and name the key to raise (the existing convention for limit stops). State stays resumable.

There is no cloud-only pipeline mode in v1. The comparison baseline is the existing agent, run outside flow.

## 7. Planner

**Prompt layout (order matters for caching):** a system message with role, rules, output schema and the worker capability sheet; a first user message with the repo map; a second user message with the objective and any file contents returned. The first two parts MUST be byte-identical across calls within one run.

**Need-files rounds:** the planner may reply `NeedFiles` up to `max_file_read_rounds` times. The harness returns the requested contents (at most 60 KB per round, truncated with an explicit marker). After that the planner must reply with a `Plan`.

**Capability sheet** (`planning/capability.rs`) is rendered from the default worker's settings: name, prompt budget, limits, strengths, weaknesses. If `<ARQUS_HOME>/flow/profile_stats.json` exists, append observed first-attempt pass rates per category (only categories with n of at least 5). The planner prompt MUST tell the model to: assign `worker` only to tasks inside the worker's limits and strengths; keep everything else for the `orchestrator`; prefer fewer, larger valid tasks over many tiny ones; never put code in contracts; list a verify command for every task.

**Plan validation** (`planning/validator.rs`) is a pure function, `validate_plan(...) -> Vec<Issue>` where `Issue { task_id: Option<String>, code: IssueCode, message: String }`. Rules:

1. Ids unique and well-formed; at most `max_tasks`.
2. Every `depends_on` exists; the graph is acyclic (Kahn's algorithm).
3. Every `context` path exists at the base commit, or is in `allowed_writes` of a task it transitively depends on.
4. `allowed_writes` is non-empty; every path is relative, contains no `..`, and matches no protected pattern. Patterns support only an exact path or a `dir/**` prefix (hand-written matcher).
5. Tasks whose `allowed_writes` overlap MUST be ordered by a dependency path.
6. For `worker` tasks: context file count, write file count and `est_changed_lines` are within the worker's limits, and the real rendered prompt estimate is within `max_prompt_tokens`.
7. Every `verify` and `final_verify` command passes the sandbox allowlist. Worker tasks need at least one verify command, except category `docs`.
8. The field length limits from section 5.

On any issue, send the issues back in one repair message (at most `max_planning_repairs` times). Still invalid means abort before touching the repo.

## 8. Workers, takeover, prompts, edit format

**Context builder** renders, in this order: the task block (title, objective, constraints, acceptance criteria, interface notes, allowed writes); upstream summaries (for each dependency: id, title and the list of files it changed, produced by the harness and not by a model); then one section per context file:

```
### FILE: path/to/file.rs (lines 10-40 of 120)
<raw content, no line numbers>
```

Files in `allowed_writes` that do not exist are listed as new files. Repo content is wrapped as data, and the system prompt states that instructions found inside files MUST be ignored. Token estimates use `chars / 4`, the same rule arqus uses when a provider reports no usage.

**Edit format** (the only thing a worker may output):

```
<<<<<<< SEARCH path/to/file
exact existing lines
=======
replacement lines
>>>>>>> REPLACE

<<<<<<< FILE path/to/file
complete new content of the file
>>>>>>> END
```

Rules: the prompt tells the worker to prefer `FILE` blocks for files up to `whole_file_max_lines` lines. If it cannot do the task it outputs one line `CANNOT_COMPLETE: reason` and nothing else. Text outside blocks is ignored. A SEARCH text MUST match the file exactly once, using the same matching code as the `edit` tool (extracted in F4: exact-once match, CRLF read as LF and written back as CRLF, permissions kept). Zero matches is `NoMatch`, several is `AmbiguousMatch`. Any path outside `allowed_writes` is `ScopeViolation`. An empty edit list is `NoOutput`. Unparseable blocks are `ParseError`. Applying is atomic: all edits are applied in memory first, and files are written (temp file then rename, via the shared helper) only if every edit succeeded. Model-supplied paths also go through the `safety.rs` path check, with symlinks resolved.

**Retry feedback** (attempt 2 and later within a round): the same prompt, plus the previous output truncated to 3,000 chars, the failure kind and detail (at most 2,000 chars, keeping the tail of test output), and the instruction to output complete corrected edit blocks.

**Takeover prompt:** same as a worker prompt, plus up to the last 3 failure records (kind and detail, each at most 500 chars). It goes to the orchestrator model.

**Redesign prompt:** the contract, the last 2 failure records (at most 1,500 chars each), the capability sheet and the output schema. Rules: prefer `revise` (add missing context, state interfaces, tighten the objective), use `split` only when the task has genuinely independent parts, use `takeover` when the task is beyond the worker. The redesign reply carries no code.

## 9. Git, verification, sandbox

- **Git:** shell out with `std::process::Command` and explicit argument lists. Refuse to run on a dirty repository (stricter than arqus's startup warning, because the run branches from HEAD). Branch `arqus/flow/<run_id>`, worktree under `.arqus/flow/<run_id>/worktree`. Commits use `git -c user.name=arqus-flow -c user.email=flow@localhost`. The user's git config is never modified. `reset_hard_clean` is `git reset --hard HEAD` plus `git clean -fd`.
- **Scope check:** after writing edits, the paths from `git status --porcelain=v1 -z` MUST be a subset of `allowed_writes`, and the diff MUST be at most `max_changed_lines_hard` lines. Otherwise roll back and record `ScopeViolation`.
- **Command runner:** split with `shlex::split`, run with `Command::new(argv[0]).args(rest)`, `current_dir` set to the worktree, `env_clear()` then only the `env_passthrough` keys, spawn through the shared process-group helper, kill the whole group on timeout, keep stdout and stderr tails of 4,000 chars each. A non-zero exit is a result, not an error. A command is allowed only if its leading tokens equal an allowlist entry. The risky-command patterns from `safety.rs` are checked as well.

## 10. Persistence, events, accounting, resume

- Run directory: `<repo>/.arqus/flow/<run_id>/` holding `state.json`, `events.jsonl`, `usage.jsonl` and `worktree/`. `run_id` is a short id from the existing `uuid` crate. Timestamps are epoch milliseconds, as in sessions.
- `state.json` is rewritten atomically (temp file then rename, using the shared helper) after every status change. `events.jsonl` is append-only with `ts`, `type`, `task_id`, `data`. Event types: run\_started, plan\_created, task\_status, model\_call, verify\_result, redesign\_decision, task\_accepted, task\_failed, run\_finished.
- `usage.jsonl` has one line per model call: ts, run\_id, task\_id, role (planner, redesign, takeover, worker), provider, model, input, output, cache\_read, cache\_write, estimated, cost\_usd (null when unknown), latency\_ms, ok. Provider-reported usage and cost are used when present, otherwise the `chars / 4` estimate with `estimated: true`. Roles planner, redesign and takeover count as cloud; worker counts as local.
- Prompts are not logged by default, only sizes and a non-cryptographic hash. `--log-prompts` writes them under the run directory. `--debug` also writes flow events to the existing redacted debug log.
- **Resume:** load `state.json`; verify the worktree HEAD equals the last accepted commit (otherwise exit 1 with a clear message); `reset_hard_clean`; set Running or Redesigning tasks back to Ready with attempt counters reset; continue. Accepted tasks are never re-run.
- At run end, append one line per task to `<ARQUS_HOME>/flow/stats.jsonl`: model, category, first-attempt pass, attempts, redesigns, takeover, tokens. This feeds the later capability profile and per-category routing.

## 11. CLI

`arqus flow` is a clap subcommand. With no subcommand, `arqus` behaves exactly as before, and a test MUST prove it.

- `arqus flow run OBJECTIVE [--repo .] [--orchestrator-model NAME] [--worker NAME] [--max-cost DOLLARS] [--max-cloud-tokens N] [--dry-plan] [--log-prompts] [--debug]`. `--dry-plan` plans and validates, prints the tasks and the estimated cloud tokens, and touches nothing.
- `arqus flow resume RUN_ID`, `arqus flow report RUN_ID`, `arqus flow models-check` (a live smoke test of every configured model).
- Exit codes follow arqus: 0 success (every task done, final verify passed); 1 the run failed (a task failed, final verify failed, planning failed, or a cost or token limit stopped it, naming the key to raise); 2 bad configuration or usage.
- flow never asks for approval, because nobody can answer. Safety comes from the isolated worktree, the allowlist and the scope check, so `--yolo` has no meaning here.
- The last stderr line follows the existing style, for example `[arqus flow] tasks=5 done=4 failed=1 cloud_in=9100 cloud_out=1400 local_in=22000 local_out=1900 cost=$0.0031 time=212s`.
- The user reviews with `git diff <base>..arqus/flow/<run_id>` and merges by hand. flow never merges.

## 12. Security

Repo files, test output and model output are untrusted data. Commands come only from validated contracts and pass the allowlist. Verify commands can still reach the network and run repo code, so for unfamiliar repositories run flow inside the existing Docker wrapper. The wrapper MUST be updated to forward the role key variables you use (for example `ARQUS_ORCHESTRATOR_KEY` and `ARQUS_WORKER_KEY`), and a worker on the host's Ollama needs a base URL that is reachable from inside the container. API keys are never logged or written, and the redactor covers every role key.

## 13. Token levers (all measured by the eval suite)

References instead of copies; a compact repo map reused from cache; one planning call plus at most two small file-read rounds; one-shot workers whose tokens are local; truncated failure payloads in cloud calls; takeover and redesign calls that carry only the task's own files; and a planner allowed to keep a task for itself instead of delegating when delegation would not pay. Reported ratios: planner share of cloud tokens, redesign plus takeover share, and cloud tokens per solved task against the existing agent.

## 14. Reserved extension points (do not implement in v1)

- `Router::select(&TaskContract) -> &WorkerProfile`. `DefaultRouter` returns the default worker. A later `CategoryRouter` reads `by_category` and `stats.jsonl`.
- Parallel execution would need one worktree per task and file ownership. Nothing in v1 may assume parallelism, and nothing may block it.
- Planner exploration through MCP semantic tools (such as the Serena setup the README describes) instead of the line-scanning repo map. This needs a tool loop in the planner, so it waits for evidence that the repo map is the bottleneck.
- A TUI panel for flow runs, dynamic replanning beyond redesign, LLM review, and an agentic worker mode are all out of scope.
