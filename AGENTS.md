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
## A13. Test layers

| Layer | What | Runs with | Network | Cost |
|---|---|---|---|---|
| L1 unit | pure functions, temp folders | `cargo test` | no | 0 |
| L2 e2e | the real `pirs` binary against the fake LLM server (`tests/e2e_*.rs`) | `cargo test` | local only | 0 |
| L3 live | real OpenRouter, tiny requests (`tests/live_*.rs`, all `#[ignore]`) | `cargo test --test live_openrouter -- --ignored --test-threads=1` | yes | cents |
| L4 evals | small coding tasks with pass/fail (`evals/`) | `python3 evals/run.py` | yes | cents |

Default `cargo test` runs L1 and L2 only. It must NEVER need a network, a real key, or the real `~/.pirs`.

## A14. Cost and secrets rules

1. Never write an API key to a file, a log line, a test, a commit or a message. Debug logs must redact it (stage 12).
2. Live tests send at most 3 requests each, with `max_tokens` of 300 or less, and fail if one test costs more than 0.05 dollars.
3. Never put a live call in a loop without a hard cap on iterations and on cost.
4. Environment variables that tests may set: `PIRS_HOME`, `PIRS_RETRY_BASE_MS`, `PIRS_MAX_RETRIES`, `PIRS_IDLE_TIMEOUT_SECS`, `PIRS_MAX_OUTPUT_TOKENS`, `PIRS_DEBUG`. Live-only: `PIRS_LIVE`, `PIRS_LIVE_MODEL`.

## A15. Bug protocol

When you find a bug (a human report or a live failure): 1) write a test that fails because of it (L1 or L2 first), 2) show it failing, 3) fix the code, 4) show it passing, 5) add one line to `docs/BUGS.md` with symptom, cause, test name. No fix without a test, unless it is impossible to test; then say why.

## A16. Facts about OpenRouter (checked in the OpenRouter docs on 7 Nov 2026)

- The stream may contain comment lines starting with `:` such as `: OPENROUTER PROCESSING`. Ignore them. Never pass them to a JSON parser.
- Every stream ends with a chunk carrying `usage`, sent just before `[DONE]`. It contains `prompt_tokens` (this INCLUDES cached tokens), `completion_tokens`, `total_tokens`, `cost` (credits, roughly dollars), `prompt_tokens_details.cached_tokens`, `completion_tokens_details.reasoning_tokens`. That chunk has one choice with an empty delta that repeats the `finish_reason`. It is an accounting frame, not a second ending. Be ready for the OpenAI-style variant too: an empty `choices` array plus `usage`.
- Errors before the first token: a normal HTTP status with JSON `{"error":{"message","code","metadata"}}`. 400 bad request, 401 bad key, 402 no credits or the key's limit is used up, 429 rate limited (a `Retry-After` header may be present), 502 or 503 provider trouble.
- Errors after the first token: HTTP stays 200. The stream carries a chunk with a top-level `error` object and a choice whose `finish_reason` is `"error"`. It can even be the first and only event. Treat it as a failure.
- OpenRouter estimates the worst-case cost of a paid request up front using `max_tokens` (or a fixed cap when unset) and can refuse with 402 if that estimate exceeds what the key's limit has left. Always send a modest `max_tokens`.
- Closing the connection cancels generation for supported providers.
- `GET {base_url}/key` with the key returns `data.limit`, `data.limit_remaining` (null means unlimited), `data.usage`, `data.usage_daily` and more.
- Not verified, check live in stage 17: whether the DeepSeek model streams its reasoning as `delta.reasoning` or `delta.reasoning_content`, and whether it rejects a tool-use turn sent back without its reasoning.

## A17. Extra approved crates

`unicode-width` (stage 19 only). Nothing else new. The fake server and test helpers use only the standard library.

## A18. PREFIX for this part

> Read AGENTS.md (including the A13 to A18 addendum) and PROGRESS.md first. Do ONLY the stage below. Follow the IRON RULES. If a file I mention exists, read it and extend it. Every bug fix needs a test first (A15).

---
