# pirs: build a pi-style coding agent in Rust (prompt kit for a small model)

This file drives opencode (DeepSeek V4 Flash Free, or any similar small model) through building `pirs`, a minimal terminal coding agent modeled on pi (pi.dev, github.com/badlogic/pi-mono).

- **Part A** is the permanent brief. Copy it into `AGENTS.md`.
- **Part B** has one prompt per stage. Paste them one at a time, each in a fresh opencode session.
- **Part C** is the roadmap: what "like pi" means and what to skip.
- **Part D** is rescue tips for when the small model gets lost.

`pirs-starter.zip` (delivered with this file) holds tested Rust code for the core of stages 1 to 3 and part of stage 7. Using it is optional, but it gives you a green `cargo test` on day one.

---

## How to use this (for you, the human)

1. Recommended: unzip `pirs-starter.zip` into a new empty folder named `pirs`, then `cd pirs && git init && cargo test` and expect 9 passed. (Alternative for a blank start: `cargo new pirs`.)
2. Copy everything in **Part A** (from "# PART A" to the line before "# PART B") into a file called `AGENTS.md` in the project root. opencode reads it at the start of every session. Create an empty `PROGRESS.md`.
3. Open a NEW opencode session for each stage. Paste the **PREFIX** line, then the stage text. A fresh session keeps the small model's context clean.
4. A stage is finished only when its DONE WHEN commands really pass. Then `git add -A && git commit -m "stage N"`.
5. If the model starts looping or breaks working code: `git checkout .`, then rerun the stage and paste the exact error at the top.
6. To learn Rust while building, after any stage say: `Explain src/<file>.rs to me line by line, like I am new to Rust.`

---

# PART A: MASTER BRIEF (copy into AGENTS.md)

## A1. Mission

We are building `pirs`: a minimal terminal coding agent in Rust, modeled on **pi**.

What pi is (facts to copy as ideas, never copy its source or its prompts):
- Tiny core. System prompt under about 1000 tokens. Four tools: `read`, `write`, `edit`, `bash`. Everything else is optional.
- Four layers: an LLM layer that streams from many providers, an agent loop that runs tools, a coding-agent layer (CLI, sessions, compaction, AGENTS.md, skills), and a terminal UI.
- Sessions are JSONL files. Each line has an `id` and a `parent_id`, so the history is a tree. Branching is a first-class feature.
- When the context fills up, old messages are summarized (compaction). The full history stays in the file.
- Skills are folders with a `SKILL.md`. Custom instructions come from `AGENTS.md` and `SYSTEM.md`.

We copy those four layers as Rust modules, in one crate (one `Cargo.toml`).

## A2. The human

The human is new to Rust. They understand pi only at overview level (agent loop, JSONL trees, interactive mode). Therefore:
- The first time you use a Rust concept, add a short **"Rust note:"** (2 to 3 lines, plain words).
- At the end of every stage, give the report described in A12.

## A3. IRON RULES (you are a small model; follow these exactly)

1. Do ONE stage at a time. Never start the next stage, even if it looks easy.
2. Start every stage by reading `AGENTS.md` and `PROGRESS.md`, then list `src/`. Then print a plan of at most 8 lines, then begin.
3. Work in small steps: change ONE file, run `cargo check`, fix, then the next file.
4. Compile loop: if `cargo check` fails, read only the FIRST error, fix it, run again. If the same error survives 3 attempts, STOP. Show the error and what you tried, and ask the human.
5. Never guess a crate's API. If unsure, open the crate source in `~/.cargo/registry/src/*/<crate>-<version>/` or read its docs.rs page. This matters most for `ratatui`, `crossterm` and `reqwest`, which change between versions.
6. Use only the crates in A5. Ask before adding any other crate.
7. Do not rewrite or delete files that already work. Make small edits.
8. Keep every file under 300 lines. Split it if it grows.
9. Write tests in the same stage as the code. A stage is not done until `cargo test` passes.
10. Never claim success without pasting the real command output.
11. At the end of the stage, update `PROGRESS.md` (template in A11).
12. If something is unclear, pick the simplest option, write it under "Assumptions" in `PROGRESS.md`, and continue. Ask only when blocked.
13. Save context: do not read huge files whole. Use `grep -n` and view line ranges.
14. Never print or store secrets. API keys come only from environment variables.

## A4. Rust simplicity rules

- Use `String`, `Vec<T>`, `Option<T>`, `anyhow::Result<T>`. Put NO lifetimes (`&'a`) in structs. Own the data and use `.clone()` freely. Speed does not matter yet.
- No `unsafe`. No custom macros. No generic-heavy traits. No `dyn Trait` or `async_trait` yet. Use `enum` plus `match` for "one of several" things (providers, tools).
- Errors: `anyhow::Result`, the `?` operator, and `.context("what failed")`.
- Async (`tokio`) only where needed: HTTP streaming, running `bash`, the TUI loop. Pure logic (context building, compaction math, parsing) is NOT async, so it is easy to test.
- Every data type gets `#[derive(Debug, Clone, Serialize, Deserialize)]`.
- Tool JSON schemas are written by hand with `serde_json::json!`.
- Tests go in a `#[cfg(test)] mod tests` block. File tests use a temp folder (`std::env::temp_dir()`), never the real home folder.
- Run `cargo fmt` after each file. Run `cargo clippy` at the end of each stage and fix simple warnings.

## A5. Approved crates (add with `cargo add`, only when a stage needs them)

`serde` (feature `derive`), `serde_json`, `anyhow`, `uuid` (feature `v4`), `dirs`, `tokio` (feature `full`), `reqwest` (features `json`, `stream`), `futures-util`, `clap` (feature `derive`), `ratatui`, `crossterm` (feature `event-stream`).

If `reqwest` fails to build with an OpenSSL error on Linux, tell the human to run `sudo apt install pkg-config libssl-dev`.

## A6. Files and dependency direction

```
src/
  main.rs           CLI entry. Picks print mode or interactive mode.
  types.rs          Message, Block, Entry ... provider-neutral        (stage 1)
  session.rs        JSONL store + tree walking                        (stage 2)
  context.rs        build_context(): entries -> messages              (stage 3)
  system_prompt.rs  builds the system prompt string                   (stage 3, grows in 8)
  ai/               LLM layer: provider-agnostic streaming            (stage 4, 10)
    mod.rs            ToolDef, LlmRequest, StreamEvent, Llm enum
    openai.rs         OpenAI-compatible chat/completions adapter
    anthropic.rs      Anthropic messages adapter (later)
    mock.rs           scripted fake provider for tests
  tools/            read.rs write.rs edit.rs bash.rs + mod.rs         (stage 5)
  agent.rs          the agent loop                                    (stage 6)
  compaction.rs     token math, cut point, summary call               (stage 7)
  resources.rs      AGENTS.md, SYSTEM.md, skills, prompt templates    (stage 8)
  tui/              ratatui interface                                 (stage 9)
```

Allowed imports (never reverse an arrow):
`types` <- `session`, `context`, `compaction` <- `ai`, `tools` <- `agent` <- `main`, `tui`.
`ai/` and `tools/` must NEVER import `session`. `tui/` must NEVER import `ai/` directly.

## A7. Core types (decided; keep them exactly)

Rules for these types: they are provider-neutral. NEVER add fields that belong to one provider. Provider quirks are handled inside `ai/` adapters only.

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One piece of an assistant message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    Text { text: String },
    Thinking { text: String },
    ToolCall { id: String, name: String, args: Value },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl Usage {
    pub fn total(&self) -> u64 {
        self.input + self.output + self.cache_read + self.cache_write
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    Stop,
    ToolUse,
    Length,
    Error,
    Aborted,
}

/// Provider-neutral message. Never put provider-specific fields here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum Message {
    User {
        text: String,
    },
    Assistant {
        blocks: Vec<Block>,
        provider: String,
        model: String,
        usage: Option<Usage>,
        stop: StopReason,
    },
    ToolResult {
        call_id: String,
        tool_name: String,
        text: String,
        is_error: bool,
    },
}

/// One line of the session file (after the header line).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub parent_id: Option<String>,
    pub ts: u64, // milliseconds since 1970
    #[serde(flatten)]
    pub kind: EntryKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EntryKind {
    Message { message: Message },
    ModelChange { provider: String, model: String },
    Compaction {
        summary: String,
        first_kept_id: String,
        tokens_before: u64,
        read_files: Vec<String>,
        modified_files: Vec<String>,
    },
    BranchSummary { summary: String, from_id: String },
    Custom { custom_type: String, data: Value },
}

/// First line of every session file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Header {
    #[serde(rename = "type")]
    pub kind: String, // always "session"
    pub version: u32,
    pub id: String,
    pub cwd: String,
    pub ts: u64,
}
```

Sample session file (every line is one JSON object; the first line is the header):

```
{"type":"session","version":1,"id":"a1b2c3d4","cwd":"/home/me/proj","ts":1762400000000}
{"id":"e1000001","parent_id":null,"ts":1762400001000,"type":"message","message":{"role":"user","text":"list the files"}}
{"id":"e1000002","parent_id":"e1000001","ts":1762400002000,"type":"message","message":{"role":"assistant","blocks":[{"type":"text","text":"Checking."},{"type":"tool_call","id":"call_1","name":"bash","args":{"command":"ls"}}],"provider":"openai","model":"some-model","usage":{"input":900,"output":40,"cache_read":0,"cache_write":0},"stop":"tool_use"}}
{"id":"e1000003","parent_id":"e1000002","ts":1762400003000,"type":"message","message":{"role":"tool_result","call_id":"call_1","tool_name":"bash","text":"Cargo.toml\nsrc","is_error":false}}
{"id":"e1000009","parent_id":"e1000003","ts":1762400009000,"type":"compaction","summary":"## Goal\n...","first_kept_id":"e1000003","tokens_before":185000,"read_files":["src/a.rs"],"modified_files":[]}
```

This format is inspired by pi's but is our own and is not byte-compatible with it.

## A8. The pi ideas we implement (design decisions)

1. **Session = JSONL, append only.** One header line, then one entry per line. Each entry has `id` and `parent_id`. The leaf is the LAST line of the file. To branch, move the in-memory leaf to an older id and append: the new entry becomes the last line, so the branch persists. History is never deleted, even after compaction. Session files live in `~/.pirs/sessions/<project-slug>/<ts>_<id>.jsonl`, where the slug is the cwd with `/` and `.` replaced by `-`.
2. **Context is a pure function.** `build_context(entries, leaf) -> Vec<Message>`: walk from the leaf to the root, reverse, apply the latest compaction (summary message plus kept messages), turn `branch_summary` entries into user messages, skip `model_change` and `custom` entries, then `sanitize`. Nothing else is allowed to decide what the model sees.
3. **Sanitize rules.** Every tool call must be followed by its tool result. A missing result gets a synthetic error result ("interrupted"). A result without a call is dropped. An assistant message that failed or was aborted and has no content is dropped.
4. **Adapters convert at the edge.** The neutral `Message` is converted to a provider's wire format only inside `ai/`. Adapters own provider quirks: tool-call id rules, thinking blocks, merging tool results. This is what lets one session contain messages from many providers.
5. **Compaction.** Trigger when `context_tokens > context_window - reserve_tokens` (defaults: reserve 16384, keep recent 20000). Prefer the real token count from the last assistant message's usage; if none, estimate `chars / 4`. Never cut on a tool result. The summary is stored as a `compaction` entry and sent as a user-role message. Later compactions UPDATE the previous summary instead of starting over. The list of read and modified files is produced by our code from the tool calls, not by the LLM.
6. **Small system prompt.** Layers: base prompt (under 1000 tokens), then AGENTS.md files (global, then parent folders, then cwd), then the skills list (name, description, path only), then date and cwd. `SYSTEM.md` replaces the base prompt. `APPEND_SYSTEM.md` appends to it.
7. **Skills use progressive disclosure.** Only name + description + path go into the system prompt. When a task matches, the model reads the full `SKILL.md` with the `read` tool.
8. **The core knows nothing about the UI.** The agent sends `AgentEvent` values through a channel. Print mode and the TUI are two consumers of that channel.

## A9. Agent loop (pseudocode; implement in stage 6)

```
run_prompt(session, llm, user_text):
    append user message entry
    loop (at most 50 rounds):
        messages = build_context(session.entries, session.leaf_id)
        request  = system_prompt + messages + tool definitions
        assistant = read the stream, sending TextDelta / ThinkingDelta events to the UI
        append assistant entry (with usage)
        if assistant has no tool calls: break
        for each tool call, in order:
            result = tools::execute(call)          // errors become text, never a crash
            append tool_result entry, send ToolEnd event
        if should_compact(...): compact(session)   // stage 7
    send TurnDone
```

## A10. Verification commands

`cargo fmt && cargo check && cargo test`. A stage prompt lists extra commands when needed.

## A11. PROGRESS.md template (append one block per stage)

```
## Stage N: <name>   [DONE | IN PROGRESS]
- Files created or changed:
- What works (command + short real output):
- Assumptions:
- Known problems / TODO:
- Next stage:
```

## A12. Final report format for every stage

1. What I built (3 lines).
2. Files created or changed.
3. How to run it (exact commands).
4. Rust notes (the concepts used).
5. Last lines of `cargo test` output.

---

# PART B: STAGE PROMPTS (paste one per fresh session)

**PREFIX (paste before every stage):**
> Read AGENTS.md and PROGRESS.md first. Do ONLY the stage below. Follow the IRON RULES. If a file I mention already exists, read it and extend it; do not rewrite it.

Copy each stage from its "PASTE" line down to the next `---`.

---

## STAGE 0: Setup

PASTE:
Goal: the project compiles with empty modules.
Do:
1. If `Cargo.toml` does not exist, run `cargo init --name pirs` in the current folder.
2. Add crates for stages 1 to 3 only: `cargo add serde --features derive`, `cargo add serde_json anyhow dirs`, `cargo add uuid --features v4`.
3. Create empty files `src/types.rs`, `src/session.rs`, `src/context.rs`, `src/compaction.rs`, `src/system_prompt.rs` and declare them with `mod` in `src/main.rs`. Skip any that already exist.
4. Make sure `.gitignore` contains `/target`. Create `PROGRESS.md` if missing.
Done when: `cargo check` succeeds and `cargo run` prints a line containing "pirs".

---

## STAGE 1: Types

PASTE:
Goal: create `src/types.rs` exactly as written in AGENTS.md section A7 (skip if the file already exists and matches).
Do:
1. Write the file. Do not rename anything and do not add provider-specific fields.
2. Add tests that serialize and deserialize: a `Message::User`, an `Assistant` message that contains a `ToolCall` block, a `ToolResult`, and each `EntryKind` variant inside an `Entry`. Each test must check `value == deserialize(serialize(value))`.
3. Add one test that prints the JSON line for a user entry, so the human can see the format. Run it with `cargo test -- --nocapture`.
4. Rust note to give: `enum` with data, `derive`, serde `tag` and `flatten`.
Done when: `cargo test` passes and the printed JSON looks like the sample lines in A7.
Don't: touch other files.

---

## STAGE 2: Session store (JSONL)

PASTE:
Goal: write `src/session.rs`, the JSONL store with a parent_id tree. If it already exists (from the starter), read it, explain it, and only add what is missing.
Required items:
- `now_ms()`, `new_id()` (8 characters), `project_slug(cwd)` (replace `/` and `.` with `-`).
- `sessions_dir(cwd) -> PathBuf` = `~/.pirs/sessions/<slug>/` using the `dirs` crate.
- `latest_session(dir) -> Option<PathBuf>`: newest `.jsonl` file by name or modified time (for "continue last session").
- `struct Session { path, header, entries, leaf_id }` with `create(dir, cwd)`, `open(path)`, `append(kind) -> id`, `set_leaf(id)`.
- `path_to_root(entries, leaf) -> Vec<&Entry>` ordered root to leaf.
- `open` must skip a half-written last line (a crash while writing) instead of failing.
Tests (all with temp folders):
1. create, append 2 entries, reopen: entries and leaf are equal.
2. a broken last line is skipped.
3. branching: append a, b; `set_leaf(a)`; append c; `path_to_root` gives a then c.
4. after reopening, the leaf is the last line.
5. `project_slug("/home/me/my.proj")` is `-home-me-my-proj`.
6. `latest_session` returns the newest of two files.
Done when: all session tests pass.
Don't: write to the real `~/.pirs` in tests.

---

## STAGE 3: Context builder + base system prompt

PASTE:
Goal: `src/context.rs` with `build_context` and `sanitize`, plus `src/system_prompt.rs`. If `context.rs` exists from the starter, read it, explain it, and extend the tests instead of rewriting.
`build_context(entries: &[Entry], leaf: Option<&str>) -> Vec<Message>` must follow AGENTS.md A8 items 2 and 3. Compaction handling: find the LAST `compaction` entry on the path. Output = one user message with the summary (plus `<read-files>` and `<modified-files>` blocks if those lists are not empty) + the messages from the entry whose id is `first_kept_id` up to just before the compaction entry + all messages after the compaction entry.
Tests (each builds entries in memory or with a temp session):
1. plain chat gives the same messages in order.
2. a branch: only the messages on the leaf's path appear.
3. a tool call without a result gets a synthetic `is_error: true` result.
4. a tool result without a call is dropped.
5. with a compaction entry: summary message first, old messages gone, kept and later messages present.
6. an aborted assistant message with no blocks is dropped.
System prompt: `build_system_prompt(cwd: &str, date: &str) -> String`. For now, a constant base prompt under 250 words, then the cwd and date. Use this text, with the last two lines filled in:
   You are pirs, a coding assistant running in the user's terminal. You help by reading files, running commands, editing code and writing files.
   Tools: read, write, edit, bash (see each tool's description).
   - Look before you change: read a file before you edit it.
   - Prefer edit (small exact replacements) over write for existing files.
   - Use bash for searching (grep, find, ls), running tests, and git.
   - Keep answers short and concrete. Show file paths clearly.
   - When you finish, say what you changed and how to check it.
   Current directory: {cwd}
   Current date: {date}
Add one test that the prompt contains the cwd and is under 1500 characters.
Done when: all context tests pass.
Don't: call any LLM, read files, or use async in this stage. Everything here is pure.

---

## STAGE 4: LLM layer (mock + OpenAI-compatible streaming)

PASTE:
Goal: provider-agnostic LLM calls with streaming. Start with TWO providers: a mock (for tests) and an OpenAI-compatible one (works with OpenRouter, DeepSeek's own API, Ollama, many gateways).
First run: `cargo add tokio --features full`, `cargo add reqwest --features json,stream`, `cargo add futures-util`.
In `src/ai/mod.rs` define:
- `ToolDef { name: String, description: String, parameters: serde_json::Value }` (parameters is a JSON schema).
- `LlmRequest { system: String, messages: Vec<Message>, tools: Vec<ToolDef>, max_tokens: Option<u32> }`.
- `ModelConfig { provider: String, model: String, base_url: String, api_key: String, context_window: u64 }`. Fill it from env vars `PIRS_BASE_URL`, `PIRS_API_KEY`, `PIRS_MODEL`, `PIRS_CONTEXT_WINDOW` (default 128000). Never log the key.
- `StreamEvent { TextDelta(String), ThinkingDelta(String), ToolCall { id, name, args }, Done(Message), Error(String) }`. A `ToolCall` event is sent only when that call is complete (collect argument fragments inside the adapter). `Done` carries the final `Message::Assistant` with blocks, usage and stop reason.
- `enum Llm { OpenAi(ModelConfig), Mock(Arc<Mutex<VecDeque<Vec<StreamEvent>>>>) }` with `async fn stream(&self, req: LlmRequest, tx: tokio::sync::mpsc::Sender<StreamEvent>) -> anyhow::Result<()>`. Rust note: why `Arc<Mutex<..>>` (shared mutable state), explained in 3 lines.
`ai/mock.rs`: each call to `stream` pops the next scripted list of events and sends them.
`ai/openai.rs`:
- Convert neutral messages: system prompt becomes `{"role":"system"}`; `User` becomes role `user`; `Assistant` becomes role `assistant` with `content` = joined text blocks and `tool_calls` = `[{"id","type":"function","function":{"name","arguments": <args as a JSON STRING>}}]`; drop `Thinking` blocks; `ToolResult` becomes `{"role":"tool","tool_call_id","content"}`.
- POST `{base_url}/chat/completions` with `Authorization: Bearer <key>` and body `{model, messages, tools:[{"type":"function","function":{name,description,parameters}}], stream:true, stream_options:{"include_usage":true}}`. Omit `tools` when the list is empty.
- Parse the SSE stream: read `bytes_stream()`, split into lines, handle lines starting with `data: `, stop at `data: [DONE]`. Put the line parsing in a PURE function that is unit tested.
- Map `delta.content` to `TextDelta`. Map `delta.reasoning_content` (or `delta.reasoning`) to `ThinkingDelta`. Collect `delta.tool_calls[i]` fragments by `index` (the id and name arrive first, the arguments arrive in pieces; parse the joined string as JSON at the end; if parsing fails, send an `Error`). Map `finish_reason`: `stop` is `Stop`, `tool_calls` is `ToolUse`, `length` is `Length`. Read `usage.prompt_tokens` as input and `usage.completion_tokens` as output.
- Known quirk: some reasoning models return a 400 error about `reasoning_content` when you send an assistant turn back without it. If that happens, keep the thinking text and send it as a `reasoning_content` field on that assistant message. Do this ONLY inside this adapter.
Add a temporary CLI command `pirs llm-test "your prompt"` that streams the answer to stdout (use a plain `std::env::args`; clap comes in stage 6).
Tests: (a) the SSE line parser, with canned chunks for text, for a tool call split across 3 chunks, and for the usage chunk; (b) the message conversion function (neutral to OpenAI JSON) compared against expected JSON; (c) the mock provider returns its scripted events in order.
Done when: `cargo test` passes. Manual check by the human (not by you): set the 3 env vars and run `cargo run -- llm-test "say hi"`.
Don't: implement Anthropic yet. Don't import `session` anywhere under `ai/`.

---

## STAGE 5: Tools (read, write, edit, bash)

PASTE:
Goal: the four pi tools in `src/tools/`.
`tools/mod.rs`:
- `fn tool_defs() -> Vec<ToolDef>`: the four definitions with hand-written JSON schemas.
- `async fn execute(name: &str, args: &serde_json::Value, cwd: &Path) -> ToolOutput` where `ToolOutput { text: String, is_error: bool }`.
- An unknown tool name, missing arguments, or any error returns `is_error: true` with a clear message. NOTHING in this module may panic or stop the agent. The message goes back to the model, so write it helpfully, for example "old_text was found 3 times; add more surrounding lines to make it unique".
Tool specs (relative paths resolve against `cwd`):
- `read { path, offset?, limit? }`: returns the file as text. Defaults: at most 2000 lines and 50,000 bytes. If cut, append the line `[truncated: call read again with offset=N to continue]`. A missing file or non-UTF-8 content returns an error.
- `write { path, content }`: creates parent folders, overwrites the file, returns "Wrote N bytes to path".
- `edit { path, old_text, new_text }`: `old_text` must occur EXACTLY ONCE in the file. 0 matches or more than 1 match is an error that says which. On success returns "Edited path".
- `bash { command, timeout_secs? }`: runs `sh -c <command>` in `cwd` with `tokio::process::Command`. Captures stdout and stderr together. Default timeout 120 seconds; on timeout kill the process and return an error. Output over 50,000 bytes keeps the LAST 50,000 bytes (errors are at the end) and starts with `[output truncated]`. Always include the exit code in the text, for example `exit code: 0`. `is_error` is true when the exit code is not 0.
Design note: like pi, there are no permission popups. Add a `// TODO: optional approval mode` comment and nothing more.
Tests with temp folders: read a file; read with offset and limit; read a missing file is an error; write then read back; edit with 0 matches, 2 matches, 1 match; bash `echo hi` gives exit code 0; bash `exit 3` is an error; bash `sleep 5` with `timeout_secs` 1 times out.
Done when: all tool tests pass.
Don't: import `session` or `agent`. Don't add grep, find or ls tools (bash covers them).

---

## STAGE 6: Agent loop + print mode (first full end-to-end)

PASTE:
Goal: a working agent: `pirs -p "do something"`. This is the first big milestone.
First run: `cargo add clap --features derive`.
`src/agent.rs`:
- `enum AgentEvent { TextDelta(String), ThinkingDelta(String), ToolStart { name: String, args: serde_json::Value }, ToolEnd { name: String, text: String, is_error: bool }, TurnDone, Error(String) }`.
- `struct AgentConfig { cwd: PathBuf, system_prompt: String, context_window: u64 }`.
- `async fn run_prompt(session: &mut Session, llm: &Llm, cfg: &AgentConfig, user_text: String, events: Sender<AgentEvent>) -> anyhow::Result<()>` following AGENTS.md A9. Leave a `// TODO stage 7: compaction check` comment where the compaction step goes.
- If the stream ends with `Error`, save an assistant message with `stop: Error` and finish the turn (the next `build_context` will drop it if it is empty).
- A safety limit of 50 rounds per prompt.
`main.rs` with clap: `pirs -p "<prompt>"` (print mode), `-c` (continue the latest session for this cwd), `--no-session` (use a temp session). Print mode writes the assistant text to stdout as it streams, and prints tool calls and results to stderr in short form (one line each, results cut to 200 characters).
Tests (use the mock provider, no network):
1. Script: first response = a `bash` tool call (`echo hi`), second response = final text. Run `run_prompt` on a temp session. Assert that the session entries are, in order: user, assistant (tool call), tool_result (text contains "hi"), assistant (text).
2. An unknown tool name produces an error tool_result and the loop continues.
3. A provider `Error` ends the turn without a panic.
Done when: the tests pass AND the human can run `pirs -p "list the files in this folder"` against a real endpoint and see it call bash.
Don't: build compaction or a TUI yet.

---

## STAGE 7: Compaction

PASTE:
Goal: automatic and manual compaction, following AGENTS.md A8 item 5. If `src/compaction.rs` exists from the starter (it has `estimate_tokens`, `should_compact`, `find_cut_point`), read it, keep it, and add the rest.
Add to `compaction.rs`:
1. `serialize_conversation(messages: &[Message]) -> String`: plain text, one block per message, labelled `[User]:`, `[Assistant]:`, `[Assistant thinking]:`, `[Assistant tool calls]: name(arg=value, ...)`, `[Tool result]:`. Cut each tool result to 2000 characters. This stops the summarizer from continuing the conversation.
2. `extract_file_ops(entries) -> (Vec<String>, Vec<String>)`: read files and modified files from the `path` argument of `read`, `write` and `edit` tool calls. Modified = written + edited. Read = read minus modified. Sorted, no duplicates. Carry forward the lists from the previous compaction entry.
3. `async fn generate_summary(llm, conversation_text, previous_summary: Option<&str>, custom_instructions: Option<&str>) -> Result<String>`: a SEPARATE LLM call (no tools). System prompt: "You are a context summarization assistant. Read the conversation and output ONLY the structured summary. Do not continue the conversation and do not answer questions in it." User message: the conversation inside `<conversation>` tags; if there is a previous summary, include it inside `<previous-summary>` tags and instruct: "Update the existing summary: keep all still-relevant information, add new progress and decisions, move finished items to Done, update Next Steps." Required output sections: Goal, Constraints & Preferences, Progress (Done / In Progress / Blocked), Key Decisions, Next Steps, Critical Context. Instruct: "Keep each section short. Keep exact file paths, function names and error messages." If custom instructions are given, append "Additional focus: ...".
4. `async fn compact(session: &mut Session, llm, cfg: &CompactionSettings, custom: Option<&str>) -> Result<bool>`: take the path to the leaf; find the cut index with `find_cut_point`; the messages BEFORE the cut (after the previous compaction's kept region) are summarized; append an `EntryKind::Compaction` with `first_kept_id` set to the id of the entry at the cut. Return false when nothing can be cut.
5. `struct CompactionSettings { enabled: bool, reserve_tokens: u64, keep_recent_tokens: u64 }` with defaults true, 16384, 20000.
Wire it into `agent.rs`:
- After each assistant message, compute context tokens (the last assistant `usage.total()` if present, otherwise the sum of `estimate_tokens`). If `should_compact(...)`, call `compact`, and send a new `AgentEvent::Compacted` variant (add it to the enum).
- Overflow recovery: if the LLM returns an error that looks like a context-length error (message contains "context" and "length" or "token", or HTTP 400/413), compact once and retry the request once. Use a flag so it can only happen once per prompt.
- Manual: `pirs -p "/compact optional focus text"` runs `compact` with the text as custom instructions.
Known limit (write it as a TODO): if the cut lands in the middle of a very long turn, pi writes a separate "turn prefix" summary. We skip that for now.
Tests (mock provider, tiny settings such as keep_recent 100 and reserve near the window): a long fake history triggers compaction; the new entry exists; `build_context` after it is shorter and its first message contains the mock summary text; running twice sends the previous summary into the second summary call; `extract_file_ops` gives correct lists.
Done when: all compaction tests pass.

---

## STAGE 8: AGENTS.md, SYSTEM.md, skills, prompt templates

PASTE:
Goal: `src/resources.rs`, the customization layer, and extend `system_prompt.rs`.
Rules:
1. **AGENTS.md.** Load, in this order: `~/.pirs/AGENTS.md`, then each parent folder from the filesystem root down to `cwd`. In each folder use `AGENTS.md`, or `CLAUDE.md` if there is no AGENTS.md. Put them in the system prompt as `<project-context path="...">...</project-context>` blocks, outermost first.
2. **SYSTEM.md.** If `<cwd>/.pirs/SYSTEM.md` exists, it REPLACES the base prompt. Else if `~/.pirs/SYSTEM.md` exists, it replaces it. `APPEND_SYSTEM.md` (same two places) is added after the base prompt.
3. **Skills.** A skill is a folder containing `SKILL.md`. Search `~/.pirs/skills/*/SKILL.md` and `<cwd>/.pirs/skills/*/SKILL.md` (the project one wins when names collide). `SKILL.md` starts with frontmatter:
```
---
name: my-skill
description: one or two sentences saying WHEN to use it
---
```
   Parse the frontmatter with a simple line parser (no YAML crate). Skip skills that lack a name or description. Render for the system prompt (max 50 skills, descriptions cut to 200 characters):
```
<available_skills>
- my-skill: description text (file: /abs/path/to/SKILL.md)
</available_skills>
When a task matches a skill's description, first read that SKILL.md with the read tool, then follow it. Paths inside a skill are relative to the skill's folder.
```
4. **Slash commands in user input.** `/skill:name rest of text` becomes a user message containing the full text of that skill's SKILL.md, then the rest of the text. `/name rest` where `~/.pirs/prompts/name.md` or `<cwd>/.pirs/prompts/name.md` exists becomes the file content with `$ARGUMENTS` replaced by `rest`. Unknown slash words are passed through unchanged.
5. `build_system_prompt` now takes the loaded resources and assembles: base (or replacement) + append + AGENTS blocks + skills block + cwd and date.
Tests with temp folder trees: nested AGENTS.md order; CLAUDE.md fallback; SYSTEM.md replace and APPEND; skill parsing and project override; broken skill is skipped; both slash expansions; unknown slash word unchanged.
Done when: tests pass, and `pirs -p "which skills do you have?"` run in a folder with one skill lists it.
Strategy note to include at the top of `resources.rs` as a comment (for the human):
- AGENTS.md: short, stable project facts: how to build, test and run; code style; folder map. Under about 100 lines, because it is sent on EVERY request.
- SYSTEM.md: only when you want a different persona or rules. It REPLACES the base prompt, so use APPEND_SYSTEM.md for small additions.
- Skills: procedures you need sometimes (release process, DB migration, writing a blog post). They cost almost nothing until used.
- Prompt templates: prompts you type often (/review, /commit-msg).

---

## STAGE 9: TUI (ratatui)

PASTE:
Goal: an interactive terminal UI. Do it in three sub-steps and run the checks after each.
First run: `cargo add ratatui`, `cargo add crossterm --features event-stream`. Read the docs of the INSTALLED ratatui version before writing code (rule 5). Use the crate's `ratatui::init()` and `ratatui::restore()` helpers if the installed version has them; the terminal MUST be restored on exit and on panic.
Architecture (important):
- `src/tui/app.rs`: `struct App { lines: Vec<UiLine>, input: String, scroll: usize, running: bool, status: String }` and a PURE method `apply_event(&mut self, ev: AgentEvent)` that updates the lines (append text deltas to the last assistant line, add tool lines, and so on). No drawing here, so it is unit-testable.
- `src/tui/ui.rs`: `fn draw(frame, app)`: transcript on top (scrollable), a one-line status bar (model name, context tokens / window as a percentage, session file name), and the input box at the bottom.
- `src/tui/mod.rs`: the event loop with `tokio::select!` over three sources: keyboard events (`crossterm::event::EventStream`), the `AgentEvent` receiver, and a 100 ms tick.
- Running the agent: when the user presses Enter, take the session out of `App` (`Option<Session>`) and `tokio::spawn` a task that runs `run_prompt`. When the task ends it sends the session back through a second channel (`done_tx: mpsc::Sender<Session>`), and the event loop puts it back into `App`. Rust note: this is "moving ownership into a task and back", and it avoids locks.
- Abort: Esc while running calls `handle.abort()` on the task, then reopens the session from its file (`Session::open`). This is safe because JSONL is append-only.
Sub-steps:
- 9a: static layout, typing in the input box, Enter adds a user line, Ctrl+D quits, PageUp and PageDown scroll.
- 9b: connect the agent: streaming text appears live; tool calls show as a yellow line with the tool name; tool results are dim and cut to 10 lines; errors are red; the status bar updates.
- 9c: slash commands: `/new`, `/compact [focus]`, `/quit`, `/resume` (list sessions for this folder, pick with the arrow keys). Plain text rendering only: no markdown rendering yet.
Tests: unit tests for `App::apply_event` (text deltas merge into one line; a tool start and end make two lines; `TurnDone` sets `running = false`).
Done when: unit tests pass and the human runs the manual checklist: type a prompt, see streaming, see a tool call, press Esc during a run and the UI stays alive, quit with Ctrl+D and the terminal is normal afterwards.
Don't: write your own differential renderer. ratatui already does that.

---

## STAGE 10: Anthropic adapter, model switching, tree and fork

PASTE:
Goal: prove the layer is truly provider-agnostic, and add the pi-style tree features.
Part 1, `ai/anthropic.rs` (add `"anthropic"` as a provider):
- POST `https://api.anthropic.com/v1/messages` (the base URL can be overridden) with headers `x-api-key`, `anthropic-version: 2023-06-01`, `content-type: application/json`. Body: `model`, `max_tokens` (REQUIRED; default 8192), `system`, `messages`, `tools` as `[{name, description, input_schema}]`, `stream: true`.
- Conversion: `User` becomes a user message. `Assistant` becomes role assistant with content blocks `text` and `tool_use {id, name, input}`. `Thinking` blocks are dropped. `ToolResult` becomes a USER message with content blocks `tool_result {tool_use_id, content, is_error}`; consecutive tool results MUST be merged into ONE user message. Tool-call ids must match `^[a-zA-Z0-9_-]+$` and be at most 64 characters: sanitize them here and keep the same id in the matching result.
- Stream events: `message_start` (input tokens), `content_block_start` (type text, thinking, or tool_use with id and name), `content_block_delta` (`text_delta`, `thinking_delta`, `input_json_delta` with `partial_json`), `content_block_stop`, `message_delta` (`stop_reason`, output tokens), `message_stop`. Map `end_turn` to `Stop`, `tool_use` to `ToolUse`, `max_tokens` to `Length`. Collect the `partial_json` pieces per block and parse them when the block stops.
- Same structure as the OpenAI adapter: a pure SSE parser and a pure conversion function, both unit tested.
Part 2, model switching: the `/model` command (TUI and print mode flag `--model provider:model`) appends a `model_change` entry and swaps the `Llm`. A session that holds messages from several providers must still work, because adapters only read neutral messages. Add a test: a history with an OpenAI-style tool call id like `call_abc|123` converts into legal Anthropic JSON.
Part 3, tree and fork:
- `/tree`: show entries as an indented list (depth = number of ancestors; show 60 characters of text per entry, with the role). Selecting an entry calls `set_leaf` and shows the new path. Optional: when leaving a branch, a `branch_summary` entry may summarize it (reuse `generate_summary`).
- `/fork`: copy the entries on the current path into a NEW session file (new header, same ids) and switch to it.
Tests: both Anthropic pure functions; fork keeps the path and drops other branches.
Done when: tests pass and the human confirms a live Anthropic call (they need a key; skip if they have none).

---

## STAGE 11: Polish (pick items one by one)

PASTE:
Goal: only ONE item per session; the human will say which.
- `~/.pirs/settings.json` and `<cwd>/.pirs/settings.json` (project overrides global): default model, base url, compaction numbers, skill folders. Environment variables still win.
- Steering queue: while the agent is running, text typed by the user is queued and delivered after the current tool results, before the next LLM call.
- Session names (`/name`), a better `/resume` picker, token and cost display in the status bar.
- Keep the thinking-block `signature` for Anthropic extended thinking: add `signature: Option<String>` to `Block::Thinking` and send it back on the same provider only.
- Split-turn compaction (the turn-prefix summary).
- Markdown rendering in the TUI.
Do not start "extensions" here. See Part C.

---

# PART C: HOW TO MOVE TOWARD "LIKE PI"

**Milestones**

| Milestone | Stages | You can say |
|---|---|---|
| M1: the brain | 0 to 3 | "I can load a JSONL tree and build correct LLM context from any leaf." (tests only, no AI yet) |
| M2: the first agent | 4 to 6 | "`pirs -p` edits files by itself." Real end-to-end. |
| M3: long sessions | 7 to 8 | "It survives long work and reads my AGENTS.md and skills." |
| M4: daily driver | 9 to 10 | "TUI, multiple providers, branching." |
| M5: pi-like extras | 11 and beyond | Settings, steering, extensions. |

**Why this order.** The types and context builder do not need a network or a UI, so the small model can finish them with simple unit tests. Everything after them depends on those types, so fixing them first prevents rewrites. The TUI comes late because print mode proves the core without terminal complexity.

**Use pirs to build pirs.** After M2, try giving small tasks to `pirs` itself with the same model. It will find the weak spots in your tools and your prompts quickly.

**Deliberately NOT copied from pi (yet).** Pi's extension system (TypeScript loaded at runtime), packages, themes, OAuth logins, HTML export, RPC mode. When you reach this, the simplest Rust-friendly design is: an extension is an external program that talks JSON over stdin/stdout, which pirs starts and calls (tools, slash commands, hooks like "before compaction"). Embedded scripting or WASM exist but are much harder.

**Facts to remember about pi's compaction numbers.** The defaults I used (reserve 16384, keep 20000, summary sections, file lists appended by code) come from a third-party analysis of pi's source from March 2026. Pi may have changed them since. They are good starting values, not a spec.

---

# PART D: WHEN THE SMALL MODEL GETS LOST

- **It invents an API** (a function or a ratatui widget that does not exist): paste the compiler error, and add "Open the installed crate source and use only functions that exist there."
- **It loops on one compile error**: stop it, `git checkout .` if needed, and tell it to make the change in a NEW smaller function in a separate file.
- **It rewrites working files**: say "Do not touch files other than X. Show me a diff before saving." Commit more often.
- **It forgets earlier decisions**: that is what `AGENTS.md` and `PROGRESS.md` are for. Make it write down every decision.
- **It writes tests that cannot fail** (asserting nothing): ask "Which bug would this test catch? Break the code on purpose and show the test failing."
- **A stage is too big for it**: split it yourself. For example stage 4 becomes 4a (types, mock, conversion function), 4b (SSE parser), 4c (the HTTP call).
- **The model's context fills up**: end the session, start a fresh one, and use the PREFIX again. Everything important is in the repo.
- **Free-tier limits**: the sessions are short on purpose. If you hit rate limits, wait, or switch the model for a stage that is only text editing.
