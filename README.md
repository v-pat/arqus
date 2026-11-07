# pirs

A small terminal coding agent written in Rust, modeled on the ideas behind
**pi**: a tiny core, four tools (`read`, `write`, `edit`, `bash`), streaming
model calls, append-only JSONL sessions with branching, compaction, and a
ratatui TUI.

`pirs` is a learning project. It works and it is tested, but it is not a
production tool. Read [Limitations](#limitations) before pointing it at a real
repository.

- Version: `0.2.0` (`pirs --version` prints it)
- Status: stages 0 to 25 are done; stage 26 is the release gate
- Tests: `cargo test` runs 293 tests (249 unit, 44 end-to-end) with no network
- Crates: `serde`, `serde_json`, `anyhow`, `uuid`, `dirs`, `tokio`, `reqwest`,
  `futures-util`, `clap`, `ratatui`, `crossterm`, `unicode-width`

## What it does

- Streams from OpenAI-compatible servers, OpenRouter, and Anthropic.
- Runs a tool loop with `read`, `write`, `edit`, and `bash` (bounded rounds per
  prompt).
- Stores every session as an append-only JSONL file with an `id` / `parent_id`
  tree. Forking and moving the leaf are first-class.
- Rebuilds the model context from the session with a pure function, including
  sanitizing orphaned tool calls.
- Compacts old history into a summary when the context fills up. The full
  history stays on disk.
- Loads `AGENTS.md`, `SYSTEM.md`, `APPEND_SYSTEM.md`, skills, and prompt
  templates.
- Has a print mode and a ratatui TUI with approval prompts and a steering queue.

## Requirements

- A recent stable Rust toolchain (edition 2021). Built here with rustc 1.99.1.
- A model endpoint and an API key, unless you run a local server.
- On Linux, `reqwest` may need `sudo apt install pkg-config libssl-dev`.

## Build, test, install

```sh
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
```

Expected test result (no network, no real key):

```sh
$ cargo test
test result: ok. 249 passed; 0 failed; 0 ignored
test result: ok. 4 passed; 0 failed; 0 ignored      # e2e_basic
test result: ok. 9 passed; 0 failed; 0 ignored      # e2e_cost
test result: ok. 12 passed; 0 failed; 0 ignored     # e2e_errors
test result: ok. 4 passed; 0 failed; 0 ignored      # e2e_safety
test result: ok. 15 passed; 0 failed; 0 ignored     # e2e_stream
test result: ok. 0 passed; 0 failed; 6 ignored      # live_openrouter
```

Install and run:

```sh
cargo install --path . --force
pirs --version
pirs --help
```

`scripts/check.sh` runs everything CI runs: `cargo fmt --check`, clippy with
`-D warnings`, `cargo test`, `cargo build --release`, and a scan for committed
API keys.

## Quick start

### Print mode

One prompt, assistant text on stdout, short tool lines and one summary on
stderr.

```sh
export PIRS_API_KEY=sk-...
export PIRS_MODEL=deepseek/deepseek-chat
export PIRS_BASE_URL=https://openrouter.ai/api/v1
pirs --no-session -p "list the files in src and summarize them"
```

A local OpenAI-compatible server:

```sh
PIRS_BASE_URL=http://127.0.0.1:11434/v1 \
PIRS_API_KEY=ollama \
PIRS_MODEL=qwen3:8b \
pirs --no-session -p "Reply with exactly: hello from pirs"
```

Anthropic:

```sh
PIRS_API_KEY=sk-ant-... \
pirs -p "summarize this project" --model anthropic:claude-sonnet-4
```

The last stderr line of a run looks like
`[pirs] requests=1 in=900 out=40 cost=$0.0001 time=0.8s`.

### Interactive mode

Run with no prompt:

```sh
pirs
```

The TUI shows a transcript, a status bar with model, context usage, session file
and status, and a prompt box.

## CLI

```
pirs [OPTIONS]

  -p, --prompt <PROMPT>     Run one prompt in print mode
  -c, --continue            Continue the latest session for this directory
      --no-session          Use a temporary session instead of <PIRS_HOME>/sessions
      --debug               Append debug JSON lines to <PIRS_HOME>/debug.log
      --model <PROVIDER:MODEL>
                            Pick provider and model, for example
                            anthropic:claude-sonnet-4
      --max-cost <DOLLARS>  Stop when one prompt costs more than this
      --max-session-cost <DOLLARS>
                            Stop when the whole session costs more than this
      --max-rounds <N>      Maximum model calls in one prompt
      --max-output-tokens <N>
                            Maximum generated tokens per request (max_tokens)
      --max-tokens-per-run <N>
                            Token budget per prompt when no cost is reported
      --yolo                Skip tool approvals (same as approval = "auto")
      --allow-risky         Allow risky bash commands in auto mode
  -h, --help                Print help, including exit codes, environment
                            variables, settings keys and slash commands
  -V, --version             Print version
```

Exit codes:

| Code | Meaning |
| --- | --- |
| `0` | success (including a run stopped by the runaway guard) |
| `1` | the prompt or run failed: provider error, cost/token limit, I/O error |
| `2` | bad configuration or usage: bad flag, unknown provider, missing model |

- With no `-p`, `pirs` starts the TUI.
- Known providers: `openai`, `openrouter`, `anthropic`. Any other name uses the
  generic OpenAI-compatible adapter, which covers most local servers.
- The generic adapter's default base URL is OpenRouter's. Set `PIRS_BASE_URL`
  for another endpoint.

## Configuration

### Environment variables

| Variable | Meaning | Default |
| --- | --- | --- |
| `PIRS_API_KEY` | Key for every provider. The only accepted key source. | empty |
| `PIRS_MODEL` | Model name | empty |
| `PIRS_PROVIDER` | Provider name | `openai` |
| `PIRS_BASE_URL` | Endpoint | per provider |
| `PIRS_CONTEXT_WINDOW` | Context size in tokens | 128000, 200000 for Anthropic |
| `PIRS_HOME` | Global folder for settings, sessions, skills, history, log | `~/.pirs` |
| `PIRS_MAX_OUTPUT_TOKENS` | Cap for `max_tokens` | 8192 |
| `PIRS_MAX_RETRIES` | Retries before a transient request fails | 3 |
| `PIRS_RETRY_BASE_MS` | First retry delay in milliseconds | 1000 |
| `PIRS_IDLE_TIMEOUT_SECS` | Abort a stream that stays idle this long | 60 |
| `PIRS_DEBUG` | `1` or `true` turns the debug log on | off |
| `PIRS_APP_TITLE` | OpenRouter `X-Title` header | `pirs` |
| `PIRS_HTTP_REFERER` | OpenRouter `HTTP-Referer` header | unset |

The API key is never printed, stored in settings, or written to the debug log.
`ModelConfig` has a hand-written `Debug` that redacts it, and every log line
passes through a redactor that removes the configured key and `sk-...` tokens.

### settings.json

Two optional files:

- `<PIRS_HOME>/settings.json` (global, default `~/.pirs/settings.json`)
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
  "limits": {
    "max_cost_per_run": 0.25,
    "max_cost_per_session": 2.0,
    "max_rounds": 30,
    "max_output_tokens": 4096,
    "max_tokens_per_run": 200000
  },
  "skill_folders": ["~/shared-skills", "tools/skills"],
  "approval": "ask",
  "allow_paths": ["~/scratch", "../shared"]
}
```

Rules:

- `model` uses the same `provider:model` shape as `--model`.
- Precedence, highest first: CLI flag, environment variable, project settings,
  global settings, built-in default.
- `skill_folders` from both files accumulate. `~/x` is under the home folder; a
  relative path is relative to the project.
- `approval` is `"ask"` (default) or `"auto"`. `--yolo` also means auto.
- `allow_paths` from both files accumulate. `write` and `edit` outside the
  project are refused unless the target is under one of these folders. `~` is
  the home folder; a relative path is relative to the project.
- A missing file is ignored silently. A malformed file prints a warning and the
  other file is still used.
- Settings are read at startup. The TUI `/model` command does not re-read them.
- No settings file is created automatically.

## Interactive mode

### Keys

| Key | Action |
| --- | --- |
| `Enter` | Submit the prompt, queue it while a turn runs, or pick in a tree/resume list |
| `Ctrl+J`, `Alt+Enter` | Insert a newline instead of submitting |
| `Left` / `Right` | Move the cursor one character (`Alt+Left`/`Alt+Right`: one word) |
| `Home` / `End`, `Ctrl+A` / `Ctrl+E` | Start or end of the current line. With an empty box they scroll the transcript |
| `Backspace` / `Delete` | Delete the character before / under the cursor |
| `Ctrl+W` | Delete the previous word |
| `Ctrl+U` / `Ctrl+K` | Delete to the line start / line end |
| `Up` / `Down` | Previous / next prompt from the first / last line, or move in a picker |
| `Tab` | Complete a slash command when exactly one matches |
| `PageUp` / `PageDown` | Scroll the transcript |
| `Esc` | Abort a running turn (and drop the queue), close a picker, or clear an idle prompt |
| `Ctrl+C` | Abort a running turn. Idle: clear the box, or press twice to quit |
| `Ctrl+D` | Quit on an empty box; otherwise delete the character under the cursor |

When an approval modal is open it owns the keyboard:

| Key | Action |
| --- | --- |
| `y` | Allow this call once |
| `a` | Allow this tool for the rest of the session |
| `n` | Deny the call; the model gets `The user denied this tool call.` |
| `Esc` | Deny and abort the turn |

The box grows to 6 lines and then scrolls. Pasted text keeps its newlines and
never submits. Prompt history is saved to `<PIRS_HOME>/history.jsonl`: up to 500
entries, no consecutive duplicates.

Enter during a running turn queues the text: queued prompts show dim above the
box as `queued: ...` and are delivered at the next safe point. `Esc` aborts the
turn and drops the queue.

Mac note: in Terminal.app and iTerm2 set the Option key to "Esc+" (Terminal) or
"Use Option as Meta key" (iTerm2) for `Alt+Enter` and `Alt+arrows` to work.
`Ctrl+J` always works.

### Commands

| Command | Where | Action |
| --- | --- | --- |
| `/compact [focus]` | both | Summarize old history now. `focus` is an optional hint. |
| `/cost` | both | Show session totals |
| `/budget` | both | Ask OpenRouter for the key limit |
| `/tree` | both | List the session tree, `*` marks the current path |
| `/tree <id>` | print mode | Move the in-memory leaf to an older entry |
| `/fork` | both | Copy the current path into a new session file |
| `/model` | TUI | Show the current model |
| `/model provider:model` | TUI | Switch adapter and record a `model_change` entry |
| `/new` | TUI | Start a new session |
| `/resume` | TUI | Pick an older session from this directory |
| `/help` | TUI | List keys and commands |
| `/quit`, `/exit` | TUI | Quit |
| `/skill:name [text]` | both | Inline the full `SKILL.md`, then append `text` |
| `/name [text]` | both | Run the prompt template `.pirs/prompts/name.md` |

Unknown slash words pass through unchanged.

## Tools

The model gets four tools. Schemas are hand-written JSON.

| Tool | Arguments | Behavior |
| --- | --- | --- |
| `read` | `path`, `offset` (1-based), `limit` | At most 2000 lines and 50000 bytes. Long lines cut at 2000 characters. A directory returns a sorted listing. A NUL byte in the first 8000 bytes means `error: binary file`. |
| `write` | `path`, `content` | Creates missing parent folders. Writes a temp file and renames it, so a reader never sees half a file. Keeps the permissions of an existing file. |
| `edit` | `path`, `old_text`, `new_text` | Requires `old_text` to match exactly once. Matches CRLF files as LF and writes CRLF back. Keeps permissions. |
| `bash` | `command`, `timeout_secs` | Runs `sh -c` with stdin `/dev/null`, a non-interactive environment, and its own process group. ANSI escapes are stripped. Timeout default 120 s, capped at 600 s. Output keeps the last 50 KB and 2000 lines. |

Tool errors become text results, not crashes. `grep`, `find` and `ls` are not
tools; `bash` covers those.

### Safety

These are guardrails, not a sandbox. They stop accidents, not attacks. A
determined model can still use `bash` to write outside the project, so use a
container for real isolation (see [Docker](#docker) below).

- In the TUI (`approval: "ask"`), `bash`, `write` and `edit` open a modal.
  `read` never asks. `y` allows once, `a` allows that tool for the rest of the
  session, `n` denies, `Esc` denies and aborts.
- In print mode nobody can answer. With `ask`, `bash`, `write` and `edit` return
  `Approval required. Run pirs with --yolo to allow this.`; `read` still works.
  `--yolo` (or `approval: "auto"`) skips the prompts.
- `write` and `edit` outside the working folder are refused in every mode,
  including `--yolo`. Symlinks are resolved first, so a link that points outside
  counts as outside. Add folders to `allow_paths` to permit them.
- `read` may leave the project, except under `~/.ssh`, `~/.aws`, `~/.gnupg`,
  `~/.config/gcloud`, `~/.kube` and `<PIRS_HOME>` (settings and history).
- A small pattern list marks risky bash commands: `rm -rf /`, `rm -rf ~`,
  `rm -rf *` in the home folder, `sudo`, `mkfs`, `dd if=`, `> /dev/sd`,
  `chmod -R 777 /`, `curl ... | sh`, `wget ... | sh` and `git push --force`.
  The modal shows a red `RISKY` line. In auto mode they are refused unless
  `--allow-risky` is given.
- At startup pirs warns when the folder is not a git repository, or when it has
  uncommitted changes.

### Limits and runaway protection

Defaults, all overridable by flag, environment or settings:

| Limit | Default |
| --- | --- |
| `max_cost_per_run` | `$0.50` |
| `max_cost_per_session` | `$5.00` |
| `max_rounds` | 50 model calls per prompt |
| `max_output_tokens` | 8192 |
| `max_tokens_per_run` | 500000 (only when no request reports cost) |

A turn also stops when the model repeats the same tool call 3 times or sends
two empty replies in a row. Those stops exit `0`. A cost or token limit exits
`1` and names the key to raise.

## Sessions

### Location

`<PIRS_HOME>/sessions/<project-slug>/<timestamp>_<id>.jsonl`

The default home is `~/.pirs`. The slug is the working directory with `/` and
`.` replaced by `-`.

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

Loaded in this order: `<PIRS_HOME>/AGENTS.md`, then `AGENTS.md` from the
filesystem root down to the working directory. `CLAUDE.md` is used when a folder
has no `AGENTS.md`. Keep it short: it is sent on every request.

### SYSTEM.md and APPEND_SYSTEM.md

- Project `<cwd>/.pirs/SYSTEM.md` wins over global `<PIRS_HOME>/SYSTEM.md`.
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

Looked up in `<PIRS_HOME>/skills/`, `<cwd>/.pirs/skills/`, then `skill_folders`
from settings. A later folder wins on a name clash. Only name, description, and
path go into the system prompt (max 50 skills, 200 characters each). The model
reads the full file with `read` when the task matches, or you inline it with
`/skill:name`.

### Prompt templates

`<cwd>/.pirs/prompts/name.md` wins over `<PIRS_HOME>/prompts/name.md`.
`$ARGUMENTS` is replaced with the text after the command. Names are limited to
`[A-Za-z0-9_-]`.

## Providers

| Provider | Adapter | Notes |
| --- | --- | --- |
| `anthropic` | `src/ai/anthropic.rs` | `x-api-key`, `anthropic-version: 2023-06-01`, Anthropic SSE parser, 200k default window |
| `openrouter` | `src/ai/openrouter.rs` | OpenAI chat/completions format plus attribution headers and `/budget` |
| `openai` and unknown names | `src/ai/openai.rs` | OpenAI-compatible streaming; default endpoint is OpenRouter |

Neutral messages are converted to the provider wire format only inside `ai/`.
This is what lets one session mix models. Model switches append a `model_change`
entry instead of rewriting history.

## Test layers

| Layer | What | Command | Network | Cost |
| --- | --- | --- | --- | --- |
| L1 unit | pure functions, temp folders | `cargo test` | no | 0 |
| L2 e2e | the real binary against the fake LLM server (`tests/e2e_*.rs`) | `cargo test` | local only | 0 |
| L3 live | real OpenRouter, tiny requests (`tests/live_openrouter.rs`) | `scripts/live-smoke.sh` | yes | cents |
| L4 evals | thirteen coding tasks with pass/fail | `python3 evals/run.py` | yes | cents |

L3 needs `PIRS_API_KEY` and `PIRS_LIVE_MODEL`. Its tests are `#[ignore]`, so a
plain `cargo test` never runs them. L4 needs the same two variables plus
`PIRS_PROVIDER`; see `evals/README.md`.

## Architecture

```
src/
  main.rs           CLI, print mode, wiring
  cli.rs            flags, limit resolution, help text
  types.rs          provider-neutral Message, Block, Entry, Header
  session.rs        JSONL store, tree walking, session listing
  context.rs        build_context(): entries -> messages, sanitize
  system_prompt.rs  base prompt plus AGENTS/SYSTEM/skills/date/cwd
  resources.rs      AGENTS.md, SYSTEM.md, skills, prompt templates
  settings.rs       settings.json loading and precedence
  safety.rs         approvals, check_path, risky commands, git notice
  agent.rs          the agent loop and AgentEvent
  agent_safety.rs   approval step around one tool call
  runtime.rs        Runtime: run, compact, switch_model, context_tokens
  compaction.rs     token math, cut point, summary call
  cost.rs           totals, limits, summary lines
  tree.rs           tree rows, path text, fork
  debug.rs          redacted JSON-lines debug log
  ai/               provider-agnostic streaming
    mod.rs            ToolDef, LlmRequest, StreamEvent, Llm
    openai.rs         OpenAI-compatible adapter
    openrouter.rs     OpenRouter adapter (reuses OpenAI body and SSE)
    anthropic.rs      Anthropic messages adapter
    sse.rs            SSE parser
    mock.rs           scripted provider for tests
  tools/            read, write, edit, bash
  tui/              ratatui interface
```

Import direction: `types` is at the bottom. `session`, `context`, `compaction`
depend on it; `ai` and `tools` sit above; `agent` above those; `main` and `tui`
on top. `ai/` and `tools/` never import `session`. `tui/` never imports `ai/`
directly; it uses `Runtime`.

## Docker

The container is the real boundary. It sees the project folder mounted at
`/work` and nothing else from your machine, so `--yolo` is safe inside it.

```sh
docker build -t pirs:dev .                     # build once, rebuild after code changes
./scripts/docker-smoke.sh                      # prove the isolation, all checks PASS
mkdir -p ~/.local/bin
cp scripts/pirs-docker ~/.local/bin/pirs-docker

cd ~/code/my-project
export PIRS_API_KEY=sk-...
pirs-docker                                    # the TUI, sandboxed
pirs-docker --yolo -p "fix the failing test"   # print mode, no approvals
```

The wrapper mounts the current folder at `/work`, keeps sessions in
`~/.pirs/docker-state` (`PIRS_DOCKER_STATE` overrides it), and forwards only
`PIRS_API_KEY`, `PIRS_MODEL`, `PIRS_PROVIDER`, `PIRS_BASE_URL` and
`PIRS_CONTEXT_WINDOW`. It drops all capabilities, sets
`no-new-privileges`, and caps memory (`PIRS_DOCKER_MEMORY`, default `4g`),
CPUs (`PIRS_DOCKER_CPUS`, default `4`) and processes (512).

`scripts/docker-smoke.sh` builds the image and checks: the container user is
not root, `pirs --version` works, a mounted folder is writable from both
sides, `/home/agent/.ssh` does not exist, and no host folder is visible. It
prints PASS or FAIL per check and exits non-zero on the first failure.

| The container protects | The container does NOT protect |
| --- | --- |
| The rest of the machine's files | The mounted project folder (the agent can change or delete it) |
| Your SSH keys, cloud credentials and Git credentials | Network access (the agent can reach the internet) |
| Other repositories on the machine | The API key in the container's environment (the agent can read `$PIRS_API_KEY`) |

Use `--yolo` inside the container. Native runs should keep approvals on,
because on the host `bash` can still write anywhere your user can.

Variants (documented only, not tested here):

- Node projects: `docker build --build-arg EXTRA_PACKAGES="nodejs npm" -t
  pirs:node .`, then run with `PIRS_IMAGE=pirs:node pirs-docker`.
- Rust projects: use `rust:1-bookworm` as the final base and set
  `CARGO_HOME=/home/agent/.cargo`, so `cargo` works inside the container.
  Build output inside the mounted folder is slower than native; prefer a
  Docker volume for `target/`.

## Limitations

Honest list, matching `PROGRESS.md`:

- **Guardrails, not a sandbox.** Approvals, `allow_paths`, the `read` denylist
  and the risky-command matcher stop accidents. `bash` can still write anywhere
  your user can. Use the [Docker sandbox](#docker) when you need real isolation.
- **Anthropic is not live-tested.** The converter and SSE parser have unit
  tests, but no call was made with a real key in this environment.
- **Thinking blocks are dropped for Anthropic.** Extended-thinking signatures
  are not kept or sent back.
- **`/model` in the TUI uses the environment, not settings**, and the context
  window follows the new provider default unless `PIRS_CONTEXT_WINDOW` is set.
- **No session names.** The resume picker shows raw file names. The status bar
  shows tokens and percentage; `/cost` shows dollars.
- **No markdown rendering.** Code fences are dimmed and text wraps. Tool results
  show at most 10 lines.
- **Context tokens refresh after a turn**, not live during streaming.
- **No split-turn compaction.** When the cut lands mid-turn, the summary covers
  the whole removed range; there is no separate turn-prefix summary.
- **No settings auto-create and no `--settings` flag.**
- **No extensions.** The brief's Part C is out of scope.
- **Single crate, synchronous simplicity.** `Clone` is used freely; speed was
  not a goal. Files are kept under 300 lines.

## License

MIT. See `LICENSE`.
