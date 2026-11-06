# pirs

A small terminal coding agent written in Rust, modeled on the ideas behind
**pi**: a tiny core, four tools, streaming model calls, JSONL sessions with
branching, compaction, and a terminal UI.

`pirs` is a **learning project**. It works and it is tested, but it is not a
production tool. Read [Limitations](#limitations) before you point it at a real
repository.

- Version: `0.1.0` (unreleased)
- Status: stages 0 to 11 of the build plan are complete
- Tests: 127 unit and integration tests, `cargo test` green
- Crates: `serde`, `serde_json`, `anyhow`, `uuid`, `dirs`, `tokio`, `reqwest`,
  `futures-util`, `clap`, `ratatui`, `crossterm`

## What it does

- Streams from OpenAI-compatible servers, OpenRouter, and Anthropic.
- Runs a tool loop with `read`, `write`, `edit`, and `bash` (up to 50 rounds per
  prompt).
- Stores every session as an append-only JSONL file with an `id` / `parent_id`
  tree. Forking and moving the leaf are first-class.
- Rebuilds the model context from the session with a pure function, including
  sanitizing orphaned tool calls.
- Compacts old history into a summary when the context fills up. The full
  history stays on disk.
- Loads `AGENTS.md`, `SYSTEM.md`, `APPEND_SYSTEM.md`, skills, and prompt
  templates.
- Has a print mode and a ratatui TUI.

## Requirements

- Rust 1.75 or newer (edition 2021). A current stable toolchain is recommended.
- A model endpoint and an API key, unless you run a local server.
- On Linux, `reqwest` may need `sudo apt install pkg-config libssl-dev`.

## Build and test

```sh
cargo build
cargo test
cargo clippy --all-targets
```

Expected test result:

```
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Quick start

### Print mode

One prompt, output on stdout, short tool lines on stderr.

```sh
export PIRS_API_KEY=sk-...
export PIRS_MODEL=deepseek/deepseek-chat
export PIRS_BASE_URL=https://openrouter.ai/api/v1
cargo run --quiet -- --no-session -p "list the files in src and summarize them"
```

A local OpenAI-compatible server:

```sh
PIRS_BASE_URL=http://127.0.0.1:11434/v1 \
PIRS_API_KEY=ollama \
PIRS_MODEL=qwen3:8b \
cargo run --quiet -- --no-session -p "Reply with exactly: hello from pirs"
```

Anthropic:

```sh
PIRS_API_KEY=sk-ant-... \
cargo run --quiet -- -p "summarize this project" --model anthropic:claude-sonnet-4
```

### Interactive mode

Run with no prompt:

```sh
cargo run --quiet --
```

The TUI shows a transcript, a status bar with model, context usage, session file
and status, and a prompt box.

## CLI

```
pirs [OPTIONS]

  -p, --prompt <PROMPT>     Run one prompt in print mode
  -c, --continue            Continue the latest session for this directory
      --no-session          Use a temporary session instead of ~/.pirs/sessions
      --model <PROVIDER:MODEL>
                            Pick provider and model, for example
                            anthropic:claude-sonnet-4
```

- With no `-p`, `pirs` starts the TUI.
- Known providers: `openai`, `openrouter`, `anthropic`. Any other name uses the
  generic OpenAI-compatible adapter, which covers most local servers.
- The generic adapter's default base URL is OpenRouter's. Always set
  `PIRS_BASE_URL` for another endpoint.

## Configuration

### Environment variables

| Variable | Meaning | Default |
| --- | --- | --- |
| `PIRS_API_KEY` | Key for every provider. The only accepted key source. | empty |
| `PIRS_MODEL` | Model name | empty |
| `PIRS_PROVIDER` | Provider name | `openai` |
| `PIRS_BASE_URL` | Endpoint | per provider |
| `PIRS_CONTEXT_WINDOW` | Context size in tokens | 128000, 200000 for Anthropic |
| `PIRS_APP_TITLE` | OpenRouter `X-Title` header | `pirs` |
| `PIRS_HTTP_REFERER` | OpenRouter `HTTP-Referer` header | unset |

The API key is never printed. `ModelConfig` has a hand-written `Debug` that
redacts it.

### settings.json

Two optional files:

- `~/.pirs/settings.json` (global)
- `<cwd>/.pirs/settings.json` (project, overrides global)

Every key is optional. A project file can override one key and inherit the rest.

```json
{
  "model": "openrouter:deepseek/deepseek-chat",
  "base_url": "https://openrouter.ai/api/v1",
  "context_window": 64000,
  "compaction": {
    "enabled": true,
    "reserve_tokens": 8192,
    "keep_recent_tokens": 16000
  },
  "skill_folders": ["~/shared-skills", "tools/skills"]
}
```

Rules:

- `model` uses the same `provider:model` shape as `--model`.
- Precedence: CLI `--model`, then environment variables, then project settings,
  then global settings, then built-in defaults.
- `skill_folders` from both files accumulate. `~/x` is under the home folder; a
  relative path is relative to the project.
- A missing file is ignored silently. A malformed file prints a warning and the
  other file is still used.
- Settings are read at startup. The TUI `/model` command does not re-read them.
- No settings file is created automatically.

## Interactive mode

### Keys

| Key | Action |
| --- | --- |
| `Enter` | Submit the prompt, or pick in a tree/resume list |
| `Backspace` | Delete one character |
| `PageUp` / `PageDown` | Scroll the transcript |
| `Up` / `Down` | Move in the tree or resume picker |
| `Esc` | Abort a running turn, close a picker, or clear an idle prompt |
| `Ctrl+D` | Quit |

`Ctrl+C` is not bound. Typing during a running turn is not queued; the UI says
`busy: wait, or press Esc to abort`.

### Commands

| Command | Where | Action |
| --- | --- | --- |
| `/compact [focus]` | both | Summarize old history now. `focus` is an optional hint. |
| `/tree` | both | List the session tree, `*` marks the current path |
| `/tree <id>` | print mode | Move the in-memory leaf to an older entry |
| `/fork` | both | Copy the current path into a new session file |
| `/model` | TUI | Show the current model |
| `/model provider:model` | TUI | Switch adapter and record a `model_change` entry |
| `/new` | TUI | Start a new session |
| `/resume` | TUI | Pick an older session from this directory |
| `/quit`, `/exit` | TUI | Quit |
| `/skill:name [text]` | both | Inline the full `SKILL.md`, then append `text` |
| `/name [text]` | both | Run the prompt template `.pirs/prompts/name.md` |

Unknown slash words pass through unchanged.

## Tools

The model gets four tools. Schemas are hand-written JSON.

| Tool | Arguments | Behavior |
| --- | --- | --- |
| `read` | `path`, `offset` (1-based), `limit` | At most 2000 lines and 50000 bytes. Says how to continue when cut. |
| `write` | `path`, `content` | Creates missing parent folders, overwrites the file. |
| `edit` | `path`, `old_text`, `new_text` | Requires `old_text` to match exactly once. |
| `bash` | `command`, `timeout_secs` | Runs `sh -c` in the working directory. Default timeout 120 s. Output capped at 50000 bytes (tail kept). |

Tool errors become text results, not crashes. There is no approval prompt and no
sandbox: `bash` and the file tools run with your user's permissions. There is no
`grep`, `find`, or `ls` tool; `bash` covers those.

## Sessions

### Location

`~/.pirs/sessions/<project-slug>/<timestamp>_<id>.jsonl`

The slug is the working directory with `/` and `.` replaced by `-`.

`--no-session` writes into a temporary folder instead. `-c` reopens the newest
session in that project folder. The TUI `/resume` picker lists sessions from the
current session folder by raw file name.

### Format

Line 1 is a header, then one entry per line:

```json
{"type":"session","version":1,"id":"a1b2c3d4","cwd":"/home/me/proj","ts":1762400000000}
{"id":"e1000001","parent_id":null,"ts":1762400001000,"type":"message","message":{"role":"user","text":"list the files"}}
{"id":"e1000002","parent_id":"e1000001","ts":1762400002000,"type":"message","message":{"role":"assistant","blocks":[{"type":"text","text":"Checking."},{"type":"tool_call","id":"call_1","name":"bash","args":{"command":"ls"}}],"provider":"openai","model":"some-model","usage":{"input":900,"output":40,"cache_read":0,"cache_write":0},"stop":"tool_use"}}
{"id":"e1000003","parent_id":"e1000002","ts":1762400003000,"type":"message","message":{"role":"tool_result","call_id":"call_1","tool_name":"bash","text":"Cargo.toml\nsrc","is_error":false}}
```

Entry kinds: `message`, `model_change`, `compaction`, `branch_summary`, `custom`.

Properties:

- Append only. History is never deleted, even after compaction.
- The last line is the leaf. Moving the leaf and appending creates a branch.
- `/tree <id>` in print mode moves the in-memory leaf but does not write until
  the next entry is appended.
- `/fork` keeps ids, parents, and timestamps, writes a new header, and copies
  only the current path.

## Context and compaction

`build_context(entries, leaf)` is the only code that decides what the model
sees. It walks leaf to root, reverses, applies the latest compaction (summary
plus kept messages), turns `branch_summary` entries into user messages, skips
`model_change` and `custom`, then sanitizes:

- Every tool call gets a result. A missing one becomes an "interrupted" error.
- A result without a call is dropped.
- An empty failed or aborted assistant message is dropped.

Compaction:

- Trigger: `context_tokens > context_window - reserve_tokens`.
- Defaults: reserve 16384 tokens, keep the last 20000 tokens.
- Uses the last assistant `usage` when the provider reported one, otherwise
  `chars / 4`.
- Never cuts on a tool result.
- A later compaction updates the previous summary.
- Read and modified file lists are computed from tool calls by our code, not by
  the model.
- Settings can change `enabled`, `reserve_tokens`, and `keep_recent_tokens`.

## Customization

### AGENTS.md

Loaded in this order: `~/.pirs/AGENTS.md`, then `AGENTS.md` from the filesystem
root down to the working directory. `CLAUDE.md` is used when a folder has no
`AGENTS.md`. Keep it short: it is sent on every request.

### SYSTEM.md and APPEND_SYSTEM.md

- Project `<cwd>/.pirs/SYSTEM.md` wins over global `~/.pirs/SYSTEM.md`.
- `SYSTEM.md` replaces the base prompt; `APPEND_SYSTEM.md` appends to it.
- Only one `APPEND_SYSTEM.md` is used, not both.

### Skills

A skill is a folder with `SKILL.md`:

```
---
name: release
description: cut a release and publish the changelog
---

# Release
...
```

Looked up in `~/.pirs/skills/`, `<cwd>/.pirs/skills/`, then `skill_folders` from
settings. A later folder wins on a name clash. Only name, description, and path
go into the system prompt (max 50 skills, 200 characters each). The model reads
the full file with `read` when the task matches, or you inline it with
`/skill:name`.

### Prompt templates

`<cwd>/.pirs/prompts/name.md` wins over `~/.pirs/prompts/name.md`.
`$ARGUMENTS` is replaced with the text after the command. Names are limited to
`[A-Za-z0-9_-]`.

## Providers

| Provider | Adapter | Notes |
| --- | --- | --- |
| `anthropic` | `src/ai/anthropic.rs` | `x-api-key`, `anthropic-version: 2023-06-01`, Anthropic SSE parser, 200k default window |
| `openrouter` | `src/ai/openrouter.rs` | OpenAI chat/completions format plus attribution headers |
| `openai` and unknown names | `src/ai/openai.rs` | OpenAI-compatible streaming; default endpoint is OpenRouter |

Neutral messages are converted to the provider wire format only inside `ai/`.
This is what lets one session mix models. Model switches append a `model_change`
entry instead of rewriting history.

## Architecture

```
src/
  main.rs           CLI, print mode, wiring
  types.rs          provider-neutral Message, Block, Entry, Header
  session.rs        JSONL store, tree walking, session listing
  context.rs        build_context(): entries -> messages, sanitize
  system_prompt.rs  base prompt plus AGENTS/SYSTEM/skills/date/cwd
  resources.rs      AGENTS.md, SYSTEM.md, skills, prompt templates
  settings.rs       settings.json loading and precedence
  ai/               provider-agnostic streaming
    mod.rs            ToolDef, LlmRequest, StreamEvent, Llm
    openai.rs         OpenAI-compatible adapter
    openrouter.rs     OpenRouter adapter (reuses OpenAI body and SSE)
    anthropic.rs      Anthropic messages adapter
    sse.rs            SSE parser
    mock.rs           scripted provider for tests
  tools/            read, write, edit, bash
  agent.rs          the agent loop and AgentEvent
  runtime.rs        Runtime: run, compact, switch_model, context_tokens
  compaction.rs     token math, cut point, summary call
  tree.rs           tree rows, path text, fork
  tui/              ratatui interface
```

Import direction: `types` is at the bottom. `session`, `context`, `compaction`
depend on it; `ai` and `tools` sit above; `agent` above those; `main` and `tui`
on top. `ai/` and `tools/` never import `session`. `tui/` never imports `ai/`
directly; it uses `Runtime`.

## Limitations and known gaps

Honest list, matching `PROGRESS.md`:

- **No approval mode and no sandbox.** The model can run any `bash` command and
  write any file you can. Use it in a scratch repository, not in production.
- **Print mode exits 0 after a provider error.** The error is recorded in the
  session and printed to stderr, but the process still succeeds.
- **Anthropic is not live-tested.** The converter and SSE parser have unit
  tests, but no call was made with a real key in this environment.
- **Thinking blocks are dropped for Anthropic.** Extended-thinking signatures
  are not kept or sent back.
- **`/model` in the TUI uses the environment, not settings**, and the context
  window follows the new provider default unless `PIRS_CONTEXT_WINDOW` is set.
- **No steering queue.** You cannot type the next instruction while a turn runs.
- **No session names and no cost display.** The resume picker shows raw file
  names. The status bar shows tokens and percentage, not dollars.
- **No markdown rendering and no wrapping** in the TUI. Long lines clip at the
  right edge. Tool results show at most 10 lines.
- **Context tokens refresh after a turn**, not live during streaming.
- **No split-turn compaction.** When the cut lands mid-turn, the summary covers
  the whole removed range; there is no separate turn-prefix summary.
- **No settings auto-create and no `--settings` flag.**
- **No extensions.** The brief's Part C is out of scope.
- **Single crate, synchronous simplicity.** `Clone` is used freely; speed was
  not a goal. Files are kept under 300 lines.
- **No LICENSE file** is included. Add one before publishing.

Remaining polish items, not implemented:

- steering queue
- session names, better `/resume` picker, cost display
- Anthropic thinking signatures
- split-turn compaction
- markdown rendering
- approval mode
