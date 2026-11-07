# Changelog

All notable changes to `pirs`. The version in `Cargo.toml` is the source of
truth; `pirs --version` prints it. One section per build stage.

## 0.2.0 — 2026-11-07

First stable release. Stages 12 to 26 are complete except the paid live and
eval checks, which need `PIRS_API_KEY` (see `evals/BASELINE.md`).

### Stage 26: Stability gate

- `path_to_root` builds one id-to-index `HashMap` per call, so a 20,000-entry
  chain opens and builds context in well under two seconds in a debug build.
- A corrupt line in the middle of a session file is skipped with a warning on
  stderr; the rest of the session still opens and `build_context` works. A
  half-written last line after a crash is still skipped silently.
- New black-box tests: SIGKILL during a `bash sleep 30` tool call, then `-c`
  resumes and sends the synthetic "interrupted" tool result; a corrupt middle
  line still opens with a warning.
- Empty skills folders, a `SKILL.md` without frontmatter, a prompt template with
  a bad name and a malformed `settings.json` are covered by tests and never
  crash.
- `cargo test` passed five times in a row with no key and no network.

### Stage 25: Docker sandbox

- `Dockerfile` and `.dockerignore` build a Linux image that runs `pirs` as the
  non-root `agent` user.
- `scripts/pirs-docker` mounts only the current folder and forwards only the
  `PIRS_*` variables, with dropped capabilities, no-new-privileges and memory,
  CPU and process caps.
- `scripts/docker-smoke.sh` proves the isolation: non-root user, working
  `--version`, a writable mount, no `.ssh`, and no host folders visible.
- README Docker section, including what the container does and does not protect.

### Stage 24: Packaging and release hygiene

- Version is `0.2.0-dev`; `--help` documents every flag, exit code, environment
  variable, settings key and slash command.
- `[profile.release] strip = true`, `lto = "thin"`.
- `scripts/check.sh` runs fmt, clippy, tests, a release build and a secret scan.
- MIT `LICENSE`, this changelog, a README that matches the code, and git ignores
  for `evals/results.jsonl` and `debug.log`.

### Stage 23: Tool hardening

- `read`: char-safe truncation, binary sniff, long-line cut, directory listing,
  2000-line/50000-byte caps.
- `write`: temp file plus atomic rename, permissions kept, refuses a directory.
- `edit`: CRLF-aware matching, clearer not-found error, permissions kept.
- `bash`: stdin `/dev/null`, fixed non-interactive environment, ANSI stripping,
  own process group killed on timeout/abort, 200 ms drain, 50 KB/2000-line tail,
  600 s timeout cap.

### Stage 22: Safety

- Approval modes (`ask`, `auto`, `--yolo`), TUI approval modal (`y`/`a`/`n`/`Esc`).
- Path guard for `write`/`edit`, `allow_paths`, `read` denylist, risky-command
  matcher, `--allow-risky`, git warnings at startup.

### Stage 21: Steering queue

- Enter during a running turn queues the prompt; it is delivered at the next
  safe point. `Esc` aborts and drops the queue.

### Stage 20: TUI input

- Multi-line prompt box, cursor movement, word delete, history, paste, Tab
  completion, queue display.

### Stage 19: TUI display

- Width-aware wrapping, code-fence dimming, scrolling, resize re-wrap, spinner
  and elapsed time, windowed pickers.

### Stage 18: Eval suite

- `evals/run.py` with thirteen tasks, per-task temp git copies, cost parsing and
  a JSONL result log. Baseline still pending a paid run.

### Stage 17: Live tests against OpenRouter

- Six `#[ignore]` live tests, `scripts/live-smoke.sh`, `docs/LIVE_FINDINGS.md`.

### Stage 16: Cost tracking, limits, runaway protection

- Per-run and per-session cost limits, token fallback, `--max-cost`,
  `--max-session-cost`, `--max-rounds`, `/cost`, `/budget`, repeated-call and
  empty-reply stops.

### Stage 15: Errors, retries, timeouts, exit codes

- Friendly provider errors, retries with backoff, idle-stream timeout, exit code
  1 for a failed run and 2 for bad configuration.

### Stage 14: Stream robustness

- OpenAI/OpenRouter/Anthropic SSE edges: comment lines, usage-only frames,
  in-stream errors, missing tool-call indexes, split deltas.

### Stage 13: Fake LLM server and black-box tests

- Local scripted SSE server and `tests/e2e_*.rs` black-box suites.

### Stage 12: PIRS_HOME and the debug log

- `PIRS_HOME` relocates all state; `--debug` / `PIRS_DEBUG` write redacted JSON
  lines to `debug.log` with rotation.

### Stage 11: Settings file

- `settings.json` at `<PIRS_HOME>` and `<cwd>/.pirs`, with precedence and
  project overlay.

## 0.1.0 — stages 0 to 11

Baseline before release work.

### Stage 10: Anthropic adapter, model switching, tree and fork

- Anthropic messages adapter and SSE parser, `/model`, `/tree`, `/fork`.
- Follow-up: OpenRouter provider with attribution headers and `/budget`.

### Stage 9: ratatui TUI

- Transcript, status bar, prompt box, pickers, key map.

### Stage 8: AGENTS.md, SYSTEM.md, skills, prompt templates

- Resource loading for `AGENTS.md`, `SYSTEM.md`, `APPEND_SYSTEM.md`, skills and
  `/name` prompt templates.

### Stage 7: Compaction

- Token math, cut point that never splits a tool result, summary entry, update
  of the previous summary, read/modified file lists.

### Stage 6: Agent loop and print mode

- `run_prompt` loop, `AgentEvent` channel, `-p` print mode.

### Stage 5: Tools

- `read`, `write`, `edit`, `bash` with hand-written JSON schemas.

### Stage 4: LLM layer

- `Llm` enum, OpenAI-compatible streaming adapter, scripted mock provider.

### Stage 3: Context and prompt

- `build_context` and sanitize rules, layer-based system prompt.

### Stage 2: Session store

- Append-only JSONL files, `id`/`parent_id` tree, leaf movement, session lookup.

### Stage 1: Core types

- Provider-neutral `Message`, `Block`, `Usage`, `StopReason`, `Entry`, `Header`.

### Stage 0: Skeleton

- One crate, CLI entry, module layout and import direction.
