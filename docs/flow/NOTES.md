# arqus flow: Touch Point Audit and Codebase Reality (Stage F0)

*Generated on 15 Nov 2026 for Stage F0 orientation. This document audits each row of `docs/flow/SPEC.md` Section 3 against the actual arqus codebase, recording real file paths, type definitions, function signatures, whether changes are additive or extractions, and flagging all wrong assumptions.*

---

## Section 3 Touch Point Verification

### 1. Streaming calls with retries and timeout
- **Spec Claim:** `src/ai/` (`Llm`, `LlmRequest`, `StreamEvent`, adapters). Reuse. Additive change: pass optional `extra_body` into OpenAI-compatible request body.
- **Actual Codebase:**
  - Files: `src/ai/mod.rs`, `src/ai/openai.rs`, `src/ai/anthropic.rs`, `src/ai/openrouter.rs`, `src/ai/error.rs`
  - Types:
    ```rust
    pub struct LlmRequest {
        pub system: String,
        pub messages: Vec<Message>,
        pub tools: Vec<ToolDef>,
        pub max_tokens: Option<u32>,
    }
    pub enum StreamEvent {
        TextDelta(String),
        ThinkingDelta(String),
        ToolCall { id: String, name: String, args: Value },
        Done(Message),
        Error(String),
        Notice(String),
    }
    pub enum Llm {
        OpenAi(ModelConfig),
        OpenRouter(ModelConfig),
        Anthropic(ModelConfig),
        Mock(Arc<Mutex<VecDeque<Vec<StreamEvent>>>>),
        MockCapture(Arc<Mutex<VecDeque<Vec<StreamEvent>>>>, Arc<Mutex<Vec<String>>>),
    }
    pub struct ModelConfig {
        pub provider: String,
        pub model: String,
        pub base_url: String,
        pub api_key: String,
        pub context_window: u64,
    }
    ```
  - Functions:
    - `pub async fn Llm::stream(&self, req: LlmRequest, tx: Sender<StreamEvent>) -> anyhow::Result<()>`
    - `pub async fn openai::stream(cfg: &ModelConfig, req: LlmRequest, tx: Sender<StreamEvent>) -> std::result::Result<(), LlmError>`
- **Additive?:** Yes. Adding `pub extra_body: Option<serde_json::Value>` to `ModelConfig` (or request) and shallowly merging into `body` in `src/ai/openai.rs` is purely additive.
- **Flagged Assumptions:**
  - Existing `split_model_spec` parses `provider:model` by splitting on the first `:`. However, local models (e.g., `qwen3.5:9b`) contain colons. Flow configs must keep `provider` and `model` as separate fields (as required in spec 4a) rather than using `split_model_spec`.

---

### 2. Scripted provider for tests
- **Spec Claim:** `src/ai/mock.rs`. Reuse. Extend additively if it cannot script errors or record requests.
- **Actual Codebase:**
  - Files: `src/ai/mock.rs`, `src/ai/mod.rs`
  - Types & Functions:
    - `Llm::Mock` and `Llm::MockCapture`
    - `pub async fn mock::stream(queue: Arc<Mutex<VecDeque<Vec<StreamEvent>>>>, _req: LlmRequest, tx: Sender<StreamEvent>) -> Result<()>`
    - `pub async fn mock::stream_capture(queue: Arc<Mutex<VecDeque<Vec<StreamEvent>>>>, seen: Arc<Mutex<Vec<String>>>, req: LlmRequest, tx: Sender<StreamEvent>) -> Result<()>`
- **Additive?:** Yes.
- **Flagged Assumptions:**
  - Spec speculated that mock might not record requests: `mock::stream_capture` **already records** every request serialized as JSON into `seen: Arc<Mutex<Vec<String>>>`.
  - For errors, `StreamEvent::Error(String)` already streams an error event.

---

### 3. Mock SSE HTTP server for black-box tests
- **Spec Claim:** Helpers used by `tests/e2e_*`. Reuse. If each test file has its own copy, extract one into `tests/common/` first.
- **Actual Codebase:**
  - File: `tests/support/mod.rs` (shared across all existing e2e tests)
  - Types & Functions:
    - `pub struct FakeLlm { pub url: String, recorded: Arc<Mutex<Vec<Recorded>>> }`
    - `pub enum Reply { Sse { chunks: Vec<Vec<u8>>, delay_ms: u64 }, Http { status: u16, body: String, headers: Vec<(String, String)> }, Hang { secs: u64 }, Drop }`
    - `pub struct Recorded { pub method: String, pub path: String, pub headers: Vec<(String, String)>, pub body: String }`
    - `pub fn FakeLlm::start(replies: Vec<Reply>) -> FakeLlm`
    - `pub fn FakeLlm::requests(&self) -> Vec<Recorded>`
- **Additive?:** Yes. Added `tests/common/mod.rs` which aliases `pub type MockServer = support::FakeLlm;` and re-exports `support` helpers.
- **Flagged Assumptions:**
  - **Wrong assumption in spec**: The spec stated "If each test file has its own copy...". In reality, the existing e2e tests (`e2e_basic.rs`, `e2e_connect.rs`, `e2e_cost.rs`, `e2e_crash.rs`, `e2e_errors.rs`, `e2e_safety.rs`, `e2e_stream.rs`) **already share** `tests/support/mod.rs`. No duplication existed. Existing tests remain untouched while `tests/common/mod.rs` serves flow tests.

---

### 4. Exact-once edit matching, CRLF handling, permissions, atomic write
- **Spec Claim:** `src/tools/edit.rs`, `write.rs`. Extract the pure matching and write functions so flow calls them.
- **Actual Codebase:**
  - Files: `src/tools/edit.rs`, `src/tools/write.rs`
  - Functions:
    - `pub fn edit::run(args: &Value, cwd: &Path) -> ToolOutput`
    - `pub fn write::run(args: &Value, cwd: &Path) -> ToolOutput`
- **Additive / Extraction?:** Pure extraction in Stages F3 and F4.
- **Flagged Assumptions:**
  - **Wrong assumption in spec**: The spec assumed exact-once matching and atomic writes were already clean helper functions ready for reuse. In reality:
    - In `src/tools/edit.rs`, string matching is implemented directly inside `edit::run` and writes via non-atomic `std::fs::write(&full, updated)`.
    - In `src/tools/write.rs`, atomic write is implemented via a unique temp file (`.file.arqus-tmp-...`), copying metadata/permissions, and renaming.
    - In Stage F3, atomic write will be extracted into a shared helper; in Stage F4, exact matching and CRLF normalization will be extracted without altering the behavior or signatures of `edit::run` and `write::run`.

---

### 5. Process groups, timeouts, output caps, kill on cancel
- **Spec Claim:** `src/tools/bash.rs`, `src/mcp/`. Extract one shared helper if the two do not already share one.
- **Actual Codebase:**
  - Files: `src/tools/bash.rs`, `src/mcp/transport.rs`
  - Functions & Types:
    - `pub async fn bash::run(args: &Value, cwd: &Path) -> ToolOutput`
    - `struct GroupGuard { pid: Option<u32> }` (private to `bash.rs`, kills process group with `kill -KILL -<pid>`)
    - `fn tail(text: &str) -> String` (private output truncation in `bash.rs`)
    - `pub fn StdioTransport::spawn(...)` in `src/mcp/transport.rs` (spawns with `process_group(0)`)
- **Additive / Extraction?:** Pure extraction in Stage F3.
- **Flagged Assumptions:**
  - `bash.rs` and `mcp` do not currently share a helper. `GroupGuard` is private in `bash.rs`. In Stage F3, a shared process-group runner will be extracted.

---

### 6. Path checks, risky-command patterns, git notice
- **Spec Claim:** `src/safety.rs`. Reuse for model-supplied paths and as a second line of defence for verify commands.
- **Actual Codebase:**
  - Files: `src/safety.rs`, `src/safety_risky.rs`
  - Types & Functions:
    - `pub fn check_path(cwd: &Path, path: &str, op: PathOp, allow_paths: &[PathBuf], arqus_home: &Path, user_home: Option<&Path>) -> Result<PathBuf, String>`
    - `pub struct Guard { pub cwd: PathBuf, pub allow_paths: Vec<PathBuf>, pub arqus_home: PathBuf, pub user_home: Option<PathBuf> }`
    - `pub fn Guard::check(&self, path: &str, op: PathOp) -> Result<PathBuf, String>`
    - `pub fn is_risky(command: &str) -> Option<&'static str>`
    - `pub fn git_notice(cwd: &Path) -> Option<String>`
- **Additive?:** Pure reuse. Spec assumption is 100% accurate.

---

### 7. Settings with global and project precedence
- **Spec Claim:** `src/settings.rs`. Add the `flow` section.
- **Actual Codebase:**
  - File: `src/settings.rs`
  - Types & Functions:
    - `pub struct Settings { pub model: Option<String>, pub base_url: Option<String>, pub limits: LimitOptions, ... }`
    - `pub fn Settings::load_in(home: &Path, cwd: &Path) -> Settings`
    - `pub fn Settings::load(cwd: &Path) -> Settings`
- **Additive?:** Additive in Stage F1. `pub flow: Option<FlowSettings>` will be added to `Settings`.
- **Flagged Assumptions:**
  - Spec assumption is accurate.

---

### 8. Cost totals, limits, summary line
- **Spec Claim:** `src/cost.rs`. Reuse. flow adds per-role tallies on top.
- **Actual Codebase:**
  - File: `src/cost.rs`
  - Types & Functions:
    - `pub struct Totals { pub requests: u64, pub input: u64, pub output: u64, pub cache_read: u64, pub cost: Option<f64> }`
    - `pub fn session_totals(entries: &[Entry]) -> Totals`
    - `pub fn totals_line(totals: &Totals) -> String`
    - `pub fn summary_line(totals: &Totals, seconds: f64) -> String`
- **Additive?:** Pure reuse of `Totals`.
- **Flagged Assumptions:**
  - `session_totals` accepts `&[Entry]`, which comes from `src/session.rs`. Because `src/flow` is strictly forbidden from importing `session`, `flow/model/ledger.rs` will maintain its own tally records using `Totals` fields rather than calling `session_totals`.

---

### 9. Redacted JSON-lines debug log
- **Spec Claim:** `src/debug.rs`. Reuse. The redactor MUST also cover every flow role key.
- **Actual Codebase:**
  - File: `src/debug.rs`
  - Functions:
    - `pub fn init(flag: bool, home: &Path)`
    - `pub fn log(kind: &str, value: Value)`
    - `pub fn redact(s: &str) -> String`
    - `pub fn redact_with(s: &str, key: &str) -> String`
- **Additive?:** Additive in Stage F1.
- **Flagged Assumptions:**
  - `debug::redact` currently reads only `ARQUS_API_KEY`. In Stage F1, we must add multi-key redaction (e.g. `pub fn redact_keys(s: &str, keys: &[&str]) -> String`) covering all resolved role keys.

---

### 10. clap CLI, exit codes 0, 1, 2
- **Spec Claim:** `src/cli.rs`, `src/main.rs`. Add a `flow` subcommand without changing existing behaviour.
- **Actual Codebase:**
  - Files: `src/cli.rs`, `src/main.rs`
  - `Cli` is currently a flat struct with flags (`-p`, `-c`, `-r`, etc.) without clap subcommands.
- **Additive?:** Additive in Stage F1 & F9.
- **Flagged Assumptions:**
  - Adding a `flow` subcommand via `#[command(subcommand)] pub command: Option<Commands>` must preserve top-level execution when no subcommand is specified.

---

### 11. Evals, baseline and results
- **Spec Claim:** `evals/run.py`, `evals/BASELINE.md`, `scripts/run-evals.sh`. The existing agent is condition D0. Add flow eval files beside them.
- **Actual Codebase:**
  - Files `evals/run.py`, `evals/BASELINE.md`, `scripts/run-evals.sh` are present and operative.
- **Additive?:** Pure addition of flow eval scripts in Stage F10.
- **Flagged Assumptions:**
  - Spec assumption is accurate.

---

### 12. Docker sandbox wrapper
- **Spec Claim:** `scripts/arqus-docker`, `Dockerfile`. Forward the new key variables.
- **Actual Codebase:**
  - Files `scripts/arqus-docker` and `Dockerfile` are present.
- **Additive?:** Forwarding role keys in `scripts/arqus-docker` in Stage F9.
- **Flagged Assumptions:**
  - Spec assumption is accurate.

---

### 13. Gate script
- **Spec Claim:** `scripts/check.sh`. Add the flow audit step.
- **Actual Codebase:**
  - File `scripts/check.sh` is present.
- **Additive?:** Completed in Stage F0. Wired `./scripts/audit-flow.sh` and `./scripts/test-audit-flow.sh`.
- **Flagged Assumptions:**
  - Spec assumption is accurate.

