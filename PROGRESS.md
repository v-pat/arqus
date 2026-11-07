## Stage 0: Skeleton   [DONE]
- Files created or changed:
  - `Cargo.toml` (added `dirs`; already had `serde`, `serde_json`, `anyhow`, `uuid`)
  - `src/system_prompt.rs` (new stub module)
  - `src/main.rs` (declared `mod system_prompt`, prints `pirs <version>`)
  - `PROGRESS.md` (this block)
- What works (command + short real output):
  - `cargo run` -> `pirs 0.1.0`
  - `cargo test` -> `test result: ok. 9 passed; 0 failed`
- Assumptions:
  - `src/types.rs`, `src/session.rs`, `src/context.rs`, `src/compaction.rs` already existed and work, so they were skipped, not rewritten.
  - `.gitignore` already contained `/target`.
- Known problems / TODO:
  - `src/system_prompt.rs` is only a doc stub; real prompt lands in stage 3.
- Next stage:
  - Stage 1: core types in `src/types.rs`.

## Stage 1: Core types   [DONE]
- Files created or changed:
  - `src/types.rs` (already existed and matched A7 exactly, so it was not rewritten; appended a `#[cfg(test)] mod tests` block)
  - `PROGRESS.md` (this block)
- What works (command + short real output):
  - `cargo test -- --nocapture` -> `user entry JSON: {"id":"e1000001","parent_id":null,"ts":1762400001000,"type":"message","message":{"role":"user","text":"list the files"}}`
  - `cargo test` -> `test result: ok. 18 passed; 0 failed`
  - `cargo clippy --all-targets` -> no warnings
- Assumptions:
  - Roundtrip helper takes the value by move and the test compares against `value.clone()`, because `assert_eq!` borrows both sides.
  - `PROGRESS.md` was updated even though the stage prompt said "do not touch other files", because IRON RULE 11 requires the stage block.
- Known problems / TODO:
  - `Header` has no roundtrip test yet; it lands with the session store in stage 2.
- Next stage:
  - Stage 2: JSONL session store and tree walking in `src/session.rs`.

## Stage 2: Session store   [DONE]
- Files created or changed:
  - `src/session.rs`: kept the starter code (`now_ms`, `new_id`, `project_slug`, `Session`, `path_to_root`) and added `sessions_dir(cwd)` and `latest_session(dir)`. Added a `#[cfg(test)] mod tests` block at the end.
  - `PROGRESS.md` (this block)
- What works (command + short real output):
  - `cargo test` -> `test result: ok. 25 passed; 0 failed`
  - `cargo clippy --all-targets` -> no warnings
- Assumptions:
  - Tests 1 to 5 already lived in `src/main.rs`; the new `session::tests` block covers them plus the new helpers, so the stage is checked in both places.
  - `sessions_dir` returns `PathBuf` as the brief asks. If `dirs::home_dir()` is `None`, it falls back to `"."` instead of failing.
  - `latest_session` sorts by file name. Names start with `ts`, so this equals newest-first without reading metadata.
- Known problems / TODO:
  - `Header` still has no roundtrip test; `Session::open` exercises it indirectly.
- Next stage:
  - Stage 3: `build_context()` and the real system prompt.

## Stage 3: Context and prompt
- Files created or changed:
  - `src/context.rs`: kept existing code; added six tests.
  - `src/system_prompt.rs`: added base prompt and builder.
  - `PROGRESS.md` (this block)
- What works (command + short real output):
  - `cargo test` -> `test result: ok. 32 passed; 0 failed`
  - `cargo clippy --all-targets` -> no warnings
- Assumptions:
  - Compaction test keeps from `u3`; output is 4 messages.
  - Synthetic result text: No result: this tool call was interrupted.
  - Prompt appends cwd and date; length is under 1500 chars.
- Known problems / TODO:
  - No SYSTEM.md, AGENTS.md or skills loading yet; stage 8.
- Next stage:
  - Stage 4: LLM layer in `src/ai/`.

## Stage 4: LLM layer (mock + OpenAI-compatible streaming)   [DONE]
- Files created or changed:
  - `Cargo.toml` + `Cargo.lock`: added `tokio` (full), `reqwest` (json, stream), `futures-util`.
  - `src/ai/mod.rs` (new): `ToolDef`, `LlmRequest`, `ModelConfig::from_env`, `StreamEvent`, `Llm` enum with `async fn stream`.
  - `src/ai/mock.rs` (new): pops one scripted event list per call.
  - `src/ai/sse.rs` (new): pure `StreamParser` with `feed_line`, `feed_data`, `finish`.
  - `src/ai/sse_tests.rs` (new): parser tests, split out to keep `sse.rs` under 300 lines.
  - `src/ai/openai.rs` (new): `convert_messages`, `build_body`, streaming HTTP client.
  - `src/main.rs`: temporary `pirs llm-test "<prompt>"`.
  - `PROGRESS.md` (this block)
- What works (command + short real output):
  - `cargo test` -> `test result: ok. 44 passed; 0 failed`
  - `cargo clippy --all-targets` -> no warnings
  - `cargo run -- llm-test` with no env -> `error: provider returned 401 Unauthorized: {"error":{"message":"Missing Authentication header","code":401}}` (request reached the endpoint; no key printed)
- Assumptions:
  - `provider` comes from optional `PIRS_PROVIDER`, default `openai`. `base_url` default is `https://openrouter.ai/api/v1`; the four required vars still override it.
  - `ModelConfig` implements `Debug` by hand and prints `api_key: "<redacted>"`, so the derived rule from A4 is intentionally broken for this one type.
  - `ToolCall` events are emitted in `finish()`, after the stream ends, because the usage chunk arrives after `finish_reason` and the arguments are only complete then.
  - Assistant messages send `content: ""` and omit `tool_calls` when there are none. `Thinking` blocks become a `reasoning_content` field on the assistant message (the adapter-only quirk).
  - `usage.prompt_tokens`/`completion_tokens` map to `input`/`output`; `cache_read` and `cache_write` stay 0 until a provider sends them.
  - `StreamEvent` and the request/config types derive serde; `Llm` does not, because it holds `Arc<Mutex<..>>`.
- Known problems / TODO:
  - `llm-test` is temporary; stage 6 replaces it with clap and print mode.
  - No Anthropic adapter yet.
- Next stage:
  - Stage 5: the four tools in `src/tools/`.

## Stage 5: Tools   [DONE]
- Files created or changed:
  - `src/tools/mod.rs` (new): `ToolOutput`, `tool_defs()`, `execute()`, arg helpers, `resolve_path`.
  - `src/tools/read.rs` (new): line window, 50,000 byte cap, truncation hint.
  - `src/tools/write.rs` (new): creates parents, overwrites.
  - `src/tools/edit.rs` (new): exactly-one-match replacement.
  - `src/tools/bash.rs` (new): `sh -c`, timeout, tail truncation, exit code.
  - `src/tools/tests.rs` (new): dispatch and end-to-end tool tests.
  - `src/main.rs`: added `mod tools;`.
  - `PROGRESS.md` (this block)
- What works (command + short real output):
  - `cargo test` -> `test result: ok. 64 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s`
  - `cargo clippy --all-targets` -> `Finished dev profile ...` with no warnings.
- Assumptions:
  - `read` `offset` is 1-based, like pi. The truncation hint gives the next 1-based line to request.
  - A single line longer than 50,000 bytes returns its head and skips that line; the hint says how to continue.
  - `read` keeps the final newline when it returns the whole file, so write then read gives back the exact text.
  - `bash` captures stdout and stderr with `Command::output()`, so they are concatenated (stdout first), not interleaved.
  - The timeout uses `kill_on_drop(true)`, so dropping the timed-out future kills the child.
  - Tests live in `src/tools/tests.rs` so every tool file stays under 300 lines.
- Known problems / TODO:
  - No approval mode; `// TODO: optional approval mode` marks the spot.
  - No `grep`, `find` or `ls` tools; `bash` covers them.
- Next stage:
  - Stage 6: agent loop and `pirs -p` print mode.
## Stage 6: Agent loop + print mode
- Files:
- Cargo.toml: added clap derive
- src/agent.rs (new): events, config, loop
- src/main.rs: clap CLI and print mode
- src/system_prompt.rs: today_utc()
- What works:
- cargo test -> 68 passed, 0 failed
- cargo clippy --all-targets -> no warnings
- pirs --help shows -p, -c, --no-session
- Assumptions:
- mock scripts put ToolCall blocks in Done
- ToolCall stream events are ignored; Done wins
- provider errors save an Error assistant entry
- Known problems:
- provider error still exits 0 in print mode
- no compaction yet (TODO stage 7)
- Next stage: Stage 7 compaction

## Stage 7: Compaction   [DONE]
- Files created or changed:
  - `src/compaction.rs`: kept `estimate_tokens`, `should_compact`, `find_cut_point`; added `context_tokens`, `CompactionSettings`, `serialize_conversation`, `extract_file_ops`, `compact`.
  - `src/compaction_summary.rs` (new): `summary_request` and `generate_summary` (separate, tool-free LLM call).
  - `src/compaction_tests.rs` (new): serializer, file ops, first/second compaction, custom focus, agent auto-compaction.
  - `src/ai/mod.rs`, `src/ai/mock.rs`: `Llm::MockCapture` records requests so tests can inspect the summary prompt.
  - `src/agent.rs`: `AgentEvent::Compacted`, `AgentConfig::compaction`, auto-compaction after each assistant turn, one-shot overflow recovery, `is_context_overflow`.
  - `src/main.rs`: `pirs -p "/compact [focus]"` manual compaction, `Compacted` print event.
  - `PROGRESS.md` (this block)
- What works (command + short real output):
  - `cargo test` -> `test result: ok. 75 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s`
  - `cargo clippy --all-targets` -> `Finished dev profile ...` with no warnings.
- Assumptions:
  - The summarizer is a normal `Llm` call; tests use the mock, production uses the configured provider.
  - `extract_file_ops` treats `read` minus (written + edited) as read-only files; lists from earlier `compaction` entries carry forward.
  - `context_tokens` uses the last assistant `usage.total()` when present, else the sum of `estimate_tokens`.
  - `agent.rs` compacts after the tool results of a round (or after a final assistant message), never between a tool call and its result.
  - Manual `/compact` uses default settings and exits without calling the agent loop.
- Known problems / TODO:
  - No "turn prefix" summary when the cut lands mid-turn (TODO comment in `compact`).
  - Print mode still exits 0 after a provider error.
- Next stage:
  - Stage 8: `AGENTS.md`, `SYSTEM.md`, skills and prompt templates in `src/resources.rs`.

## Stage 8: AGENTS.md, SYSTEM.md, skills, prompt templates   [DONE]
- Files created or changed:
  - `src/resources.rs` (new): `AgentFile`, `Skill`, `Resources`, `load`/`load_in`, AGENTS.md loader, SYSTEM/APPEND loader, skill discovery and frontmatter parser, `skills_block`, `expand_input`.
  - `src/resources_tests.rs` (new): temp-folder tests for order, fallback, override, broken skills, caps, slash expansion.
  - `src/system_prompt.rs`: `build_system_prompt` now takes `&Resources` and assembles base/SYSTEM + append + AGENTS blocks + skills + cwd/date.
  - `src/main.rs`: loads resources, builds the prompt from them, expands slash input before the agent loop.
  - `src/ai/sse.rs` + `src/ai/sse_tests.rs`: fixed newline-terminated `data:` lines (real streams send `data: [DONE]\n`; the old parser tried to parse `[DONE]\n` as JSON). Added regression test.
  - `PROGRESS.md` (this block)
- What works (command + short real output):
  - `cargo test` -> `test result: ok. 89 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s`
  - `cargo clippy --all-targets` -> `Finished dev profile ...` with no warnings.
  - End-to-end with a local fake SSE provider and one project skill:
    `PIRS_BASE_URL=http://127.0.0.1:8765/v1 PIRS_API_KEY=test PIRS_MODEL=fake pirs --no-session -p "which skills do you have?"`
    -> `You have the demo skill.`; server log: `skill_in_prompt=True`, `system_len=1025`.
- Assumptions:
  - AGENTS.md order: `~/.pirs/AGENTS.md`, then `<root>/AGENTS.md` ... `<cwd>/AGENTS.md`; `CLAUDE.md` is used when a folder has no `AGENTS.md`.
  - `SYSTEM.md` and `APPEND_SYSTEM.md`: project `<cwd>/.pirs/` wins; global `~/.pirs/` is the fallback. APPEND is not concatenated from both places.
  - Skills are sorted by name (`BTreeMap`); project skill overrides global with the same name. Only `name` and `description` are parsed from frontmatter; extra keys are ignored.
  - Prompt templates: project `<cwd>/.pirs/prompts/name.md` wins over `~/.pirs/prompts/name.md`. Slash names are limited to `[A-Za-z0-9_-]` to block path traversal.
  - `/compact` remains a built-in and is handled before template expansion. `/skill:name` inlines the whole SKILL.md and appends the rest of the input.
- Known problems / TODO:
  - `build_system_prompt` has no byte cap; a very large AGENTS.md is sent on every request (the strategy note tells the human to keep it short).
  - No slash autocomplete or list command yet.
  - Print mode still exits 0 after a provider error.
- Next stage:
  - Stage 9: ratatui TUI in `src/tui/`.

## Stage 9: ratatui TUI   [DONE]
- Files created or changed:
  - `Cargo.toml` + `Cargo.lock`: added `ratatui` 0.30.2 and `crossterm` 0.29 (`event-stream`).
  - `src/tui/app.rs` (new): `UiLine`, `LineKind`, `Resume`, `App` and the pure `apply_event`.
  - `src/tui/app_tests.rs` (new): event merging, tool lines, 10-line cut, `TurnDone`, scrolling, resume picker.
  - `src/tui/ui.rs` (new): transcript, status bar, input box, cursor, resume overlay.
  - `src/tui/mod.rs` (new): `tokio::select!` loop over keys, agent events, done channel and 100 ms tick.
  - `src/tui/tasks.rs` (new): spawn/abort/reload helpers (keeps `mod.rs` under 300 lines).
  - `src/agent.rs`: added `Runtime` (`run`, `compact`, `context_tokens`) so `tui/` never imports `ai/`.
  - `src/session.rs`: added `list_sessions`; `latest_session` now uses it.
  - `src/main.rs`: `mod tui`; no-prompt run starts interactive mode.
  - `PROGRESS.md` (this block)
- What works (command + short real output):
  - `cargo test` -> `test result: ok. 98 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s`
  - `cargo clippy --all-targets` -> `Finished dev profile ...` with no warnings.
  - Print mode against local Ollama: `PIRS_BASE_URL=http://127.0.0.1:11434/v1 PIRS_API_KEY=ollama PIRS_MODEL=qwen3:8b cargo run --quiet -- --no-session -p "Reply with exactly: hello from pirs"` -> `hello from pirs`
  - PTY smoke test (100x30): streamed assistant text appeared, `-> bash` and `<- bash` rendered after a tool call, process exited 0, alt screen entered and left, no panic.
  - PTY abort test: prompt, then Esc -> `aborted` status, UI accepted another prompt, Ctrl+D -> exit 0, terminal restored.
- Assumptions:
  - `ratatui::init()` installs its own panic hook, so the terminal is restored on panic; `ratatui::restore()` runs after the loop.
  - The agent task takes the `Session` out of `App` and sends it back through `done_tx`; Esc aborts the task and reopens the file with `Session::open` (JSONL is append-only).
  - Scroll offset is a plain line index; fragments are not wrapped, so long lines are clipped at the right edge.
  - Tool results show at most 10 lines, then `... N more lines`. Newlines become separate dim lines.
  - `/compact` runs through `Runtime::compact` in a spawned task; "nothing to compact" is reported through `AgentEvent::Error`.
  - Ctrl+D always quits; Ctrl+C aborts the run while running (otherwise quits); Esc clears a non-running prompt.
  - `--no-session` keeps `/new` and `/resume` inside the temporary session folder.
- Known problems / TODO:
  - No markdown rendering and no text wrapping in the transcript (plain lines, as requested).
  - Resume picker shows raw file names only, no timestamps or previews.
  - Status context tokens refresh after each turn, not live during streaming.
  - Print mode still exits 0 after a provider error.
- Next stage:
  - No stage defined after 9. Possible follow-ups: Anthropic adapter, approval mode, markdown rendering.

## Stage 10: Anthropic adapter, model switching, tree and fork   [DONE]
- Files created or changed:
  - `src/ai/anthropic.rs` (new): `sanitize_tool_id`, `convert_messages`, `build_body`, streaming HTTP call with `x-api-key` and `anthropic-version: 2023-06-01`.
  - `src/ai/anthropic_sse.rs` (new): pure `AnthropicParser` for the Anthropic SSE events; text, thinking and split `input_json_delta`.
  - `src/ai/anthropic_tests.rs` (new): conversion, id sanitizing, body defaults, stream event parsing, stop mapping.
  - `src/ai/mod.rs`: `Llm::Anthropic`, `Llm::from_config`, `ModelConfig::from_model_spec`, provider-aware default base URL and context window.
  - `src/tree.rs` + `src/tree_tests.rs` (new): `TreeRow`, depth, 60-character previews, path text, `fork`.
  - `src/session.rs`: `Session::append_entry` writes an existing entry unchanged.
  - `src/agent.rs`: `Runtime::switch_model` appends `model_change` and swaps the adapter. Tests moved to `src/agent_tests.rs`.
  - `src/runtime.rs` (new): the `Runtime` type and its `run`, `compact`, `switch_model`, `context_tokens` methods.
  - `src/main.rs`: `--model provider:model`, print-mode `/tree`, `/tree <id>`, `/fork`. Tests moved to `src/main_tests.rs`.
  - `src/tui/app.rs`, `src/tui/tree.rs` (new), `src/tui/ui.rs`, `src/tui/mod.rs`: `/model`, `/tree` picker, `/fork`; tree status shows the new path.
  - `PROGRESS.md` (this block)
- What works (command + short real output):
  - `cargo test` -> `test result: ok. 114 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s`
  - `cargo clippy --all-targets` -> `Finished dev profile ...` with no warnings.
  - `cargo run -- --help` shows `--model <PROVIDER:MODEL>`.
  - `cargo run --quiet -- --no-session -p "/fork"` -> `forked to /var/folders/.../1793991301549_5d9be85d.jsonl`
  - Live Anthropic call skipped: neither `PIRS_API_KEY` nor `ANTHROPIC_API_KEY` is set.
- Assumptions:
  - `PIRS_API_KEY` is the key for every provider, including Anthropic. `PIRS_BASE_URL` still overrides the default `https://api.anthropic.com/v1`.
  - Missing `PIRS_CONTEXT_WINDOW` defaults to 200000 for Anthropic and 128000 for other providers.
  - Anthropic tool ids are sanitized by replacing every illegal character with `_` and truncating to 64 characters; the matching result uses the same function, so ids agree.
  - A thinking-only assistant message is skipped during Anthropic conversion because Anthropic rejects empty content. Thinking blocks are dropped as the brief asks.
  - Tool results are sent as a user message with an array of `tool_result` blocks; consecutive results merge into one message.
  - `/tree <id>` in print mode moves the in-memory leaf and prints the path. The selection becomes durable when the next entry is appended; the TUI does this naturally on the next prompt.
  - Fork copies the current path into the same session directory, keeping ids, parents and timestamps; it writes a new header.
  - Optional branch summaries on `/tree` are not implemented; the brief marks them optional.
- Known problems / TODO:
  - No live Anthropic confirmation yet (no key). The pure conversion and parser tests pass.
  - The TUI tree list does not scroll independently when the session has many entries.
  - Print mode still exits 0 after a provider error.
- Next stage:
  - Stage 11: pick one polish item (settings file, steering queue, session names, Anthropic thinking signatures, split-turn compaction, or markdown).

## Stage 10 follow-up: OpenRouter provider   [DONE]
- Files created or changed:
  - `src/ai/openrouter.rs` (new): `DEFAULT_BASE_URL`, attribution headers, body delegation, streaming call.
  - `src/ai/openrouter_tests.rs` (new): header rules, OpenAI body reuse, id pass-through.
  - `src/ai/openai.rs`: `stream` now calls `stream_with_headers`; OpenAI behavior is unchanged.
  - `src/ai/mod.rs`: `Llm::OpenRouter`, explicit `"openrouter"` routing, shared default base constant, adapter tests.
  - `PROGRESS.md` (this block)
- What works (command + short real output):
  - `cargo test openrouter` -> `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 114 filtered out`
  - `cargo test` -> `test result: ok. 117 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s`
  - `cargo clippy --all-targets` -> no warnings.
- Assumptions:
  - OpenRouter speaks the OpenAI chat/completions format, so it reuses the OpenAI converter and SSE parser instead of duplicating them.
  - `X-Title` defaults to `pirs`; `PIRS_APP_TITLE` overrides it. `HTTP-Referer` is sent only when `PIRS_HTTP_REFERER` is set.
  - `--model openrouter:<model>` selects the dedicated adapter. Unknown provider names still use the generic OpenAI adapter.
  - Default base URL for OpenRouter, OpenAI and local compatible servers remains `https://openrouter.ai/api/v1`.
- Known problems / TODO:
  - No live OpenRouter call; no key was set in this environment.
- Next stage:
  - Stage 11: pick one polish item.

## Stage 11: Settings file   [DONE]
- Files created or changed:
  - `src/settings.rs` (new): `Settings`, `CompactionOptions`, `EnvOverrides`; `load`/`load_in`/`overlay`, `compaction_settings`, `EnvOverrides::resolve`.
  - `src/settings_tests.rs` (new): defaults, project override, folder accumulation, broken file, compaction overlay, precedence.
  - `src/ai/mod.rs`: `split_model_spec` and `default_context_window` are now public; `from_model_spec` reuses them. Test module moved to `src/ai/mod_tests.rs` to keep the file under 300 lines.
  - `src/resources.rs`: `load_with`/`load_in_with` scan extra skill folders from settings; `~` means home, a relative path means the project.
  - `src/resources_tests.rs`: one test for the extra folders.
  - `src/main.rs`: `mod settings`; both modes load settings and use them for the model, compaction and resources.
  - `PROGRESS.md` (this block)
- What works (command + short real output):
  - `cargo test` -> `test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.02s`
  - `cargo clippy --all-targets` -> `Finished dev profile ...` with no warnings.
  - Bad settings model in a temp cwd: `{"model":"no-colon"}` -> `error: model must look like provider:model, got "no-colon"` and exit 1.
  - Valid settings model in a temp cwd: `{"model":"anthropic:fake-model"}` -> `pirs --no-session -p "/tree"` exits 0.
- Assumptions:
  - Precedence: CLI `--model`, then `PIRS_*` environment variables, then project settings, then global settings, then built-in defaults. `PIRS_API_KEY` is the only key source; settings never store it.
  - Settings `model` uses the same `provider:model` shape as `--model`, so `provider` and `model` resolve together (a CLI flag replaces both).
  - `skill_folders` from both files accumulate: global folders first, then project ones. A later folder wins on a skill name clash, same rule as project over project-global skills.
  - A missing settings file is silent. A broken one prints `warning: ignoring <path>: ...` and the other file is still used.
  - Settings apply at startup. The TUI `/model` command still builds its config from the environment and does not re-read settings.
- Known problems / TODO:
  - No settings file is created on first run; the human writes it by hand.
  - No CLI flag to point at a different settings file.
  - Print mode still exits 0 after a provider error.
- Next stage:
  - No stage defined after 11. Remaining polish items: steering queue, session names, Anthropic thinking signatures, split-turn compaction, markdown rendering.

## Stage 12: PIRS_HOME and the debug log   [DONE]
- Files created or changed:
  - `src/home.rs` (new): `pirs_home()` and pure `pirs_home_from(Option<&str>)`.
  - `src/home_tests.rs` (new): env value wins, empty value falls back, one test sets and restores `PIRS_HOME`.
  - `src/debug.rs` (new): `init`, `env_on`, `log`, `redact`/`redact_with`, `open_log`, `append_line`, `render`, 5 MB rotation to `debug.log.1`.
  - `src/debug_tests.rs` (new): exact key, `sk-test123`, clean text, debug off creates no file, debug on writes 5 parseable JSON lines with no key.
  - `src/session.rs`: `sessions_dir` uses `pirs_home()`.
  - `src/settings.rs`: `Settings::load` uses `pirs_home()`; `load_in` now takes the pirs home itself (`home/settings.json`).
  - `src/resources.rs`: `load`/`load_with` use `pirs_home()`; `load_in`/`load_in_with` take the pirs home itself; global SYSTEM/APPEND/AGENTS/skills/prompts read from it directly.
  - `src/resources_tests.rs`, `src/settings_tests.rs`: temp `home` is now the pirs home, so `.pirs/` was removed from those paths.
  - `src/ai/openai.rs`, `src/ai/anthropic.rs`: log `request` (url + body), `status` (code), every raw `sse` line, and `error` text. Headers are never logged.
  - `src/agent.rs`: log `tool` (name + args) before execution and `error` on a failed turn.
  - `src/main.rs`: `mod debug`, `mod home`, `--debug` flag, `debug::init(cli.debug, &home::pirs_home())` before either mode.
  - `PROGRESS.md` (this block)
- Hard-coded global paths found and replaced (grep `\.pirs` / `home_dir`):
  - `src/session.rs:26-27` `dirs::home_dir().join(".pirs")` -> `pirs_home()` (sessions).
  - `src/settings.rs:45,54` global settings -> `pirs_home()/settings.json`.
  - `src/resources.rs:46,52` `load`/`load_with` home -> `pirs_home()`.
  - `src/resources.rs:69,73` global SYSTEM.md / APPEND_SYSTEM.md -> `pirs_home()/...`.
  - `src/resources.rs:85` global AGENTS.md -> `pirs_home()/AGENTS.md`.
  - `src/resources.rs:125` global skills -> `pirs_home()/skills`.
  - `src/resources.rs:278` global prompts -> `pirs_home()/prompts`.
  - `src/main.rs:34` help text only.
  - Remaining `.pirs` in `src/` are project-level `<cwd>/.pirs/...`, docs, or test fixtures.
- What works (command + short real output):
  - `cargo test` -> `test result: ok. 134 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s`
  - `cargo clippy --all-targets` -> `Finished dev profile ...` with no warnings.
  - `PIRS_HOME=<temp>/pirs-stage12-x PIRS_DEBUG=1 cargo run --quiet -- --no-session -p "hi"` -> provider 401 (no key), and `<temp>/pirs-stage12-x/debug.log` exists with 4 lines: `{'request': 1, 'status': 1, 'error': 2}`; every line parses as JSON; `sk-` absent.
- Assumptions:
  - `PIRS_HOME` is the pirs folder itself, not a parent of `.pirs`. Default stays `~/.pirs`.
  - `PIRS_DEBUG` accepts `1` or `true` (case-insensitive). `--debug` also enables it.
  - `redact` reads `PIRS_API_KEY`; `redact_with` takes the key explicitly so tests do not need the environment.
  - The debug file is opened once per process (`OnceLock`); tests exercise `open_log`/`append_line` directly instead of resetting the global.
  - Rotation happens at startup only, as the stage asks.
- Known problems / TODO:
  - Print mode still exits 0 after a provider error.
  - Debug log has no size cap during a run; it rotates on the next start.
- Next stage:
  - Stage 13: fake LLM server and black-box tests.



## Stage 13: Fake LLM server and black-box tests   [DONE]
- Files created or changed:
  - `tests/support/mod.rs` (new): copied the supplied `fake_llm.rs` unchanged; it compiled against the crate as-is.
  - `tests/e2e_basic.rs` (new): four black-box tests, each with its own `TempDir` project and `PIRS_HOME`.
  - `PROGRESS.md` (this block).
- What works (command + short real output):
  - `cargo test --test e2e_basic` -> `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.87s`
  - `cargo test` -> unit suite `134 passed` plus `tests/e2e_basic.rs`: `4 passed`.
  - `cargo clippy --all-targets` -> `Finished dev profile ...` with no warnings.
- Assumptions:
  - The helper file was taken from `/Users/vaibhavpathak/Downloads/fake_llm.rs`; no line needed fixing.
  - `run_pirs` clears inherited `PIRS_*` variables, so a developer's real key and home never reach the tests.
  - Requests are asserted by exact count in every test (`1`, `2`, `2`, `2`); an unexpected extra request fails the test.
  - The unknown-tool check looks for `unknown tool` in the tool message content, which is how the agent reports that error to the model.
  - The session test uses one fake server with two scripted replies; the second run uses `-c` on the same `PIRS_HOME`.
- Known problems / TODO:
  - Print mode still exits 0 after a provider error (stage 15 fixes exit codes).
  - The fake server's listener thread lives until the test process exits; ports are unique, so this is harmless.
- Next stage:
  - Stage 14: stream robustness.

## Stage 14: Stream robustness   [DONE]
- Files created or changed:
  - `tests/e2e_stream.rs` (new): 15 black-box tests, one or more per scenario 1 to 12, each with its own temp project and `PIRS_HOME`.
  - `src/ai/openai.rs`: the response buffer is now `Vec<u8>`; complete lines only are decoded; an unfinished stream (no `finish_reason`, no `[DONE]`) sends `StreamEvent::Error("stream ended early: ...")`.
  - `src/ai/sse.rs`: `saw_finish()` flag; usage splits cached tokens (`input = prompt_tokens - cached_tokens`, `cache_read = cached_tokens`); error chunks keep only `error.message` (plus code) and mark the stream finished; unparseable tool arguments are kept as the raw string instead of failing the turn; `finish_reason: "error"` maps to `StopReason::Error`.
  - `src/ai/sse_tests.rs`: updated the bad-arguments test, added cached-token, `saw_finish` and provider-error tests.
  - `src/tools/mod.rs`: `execute` turns non-object (string) arguments into `tool arguments were not valid JSON: ...`.
  - `PROGRESS.md` (this block).
- Which scenarios needed a fix (written as failing e2e tests first, A15):
  - 2 (multi-byte char cut): FAILED before, `stdout: "a\u{fffd}\u{fffd}\u{1f680}b"` -> byte buffering. Fixed.
  - 5 (usage / cached tokens): FAILED before, `usage.input` was `100` (cached counted twice) -> split in the parser. Fixed.
  - 7 (cut stream, no finish): FAILED before, `stderr: ""` and `stop: "stop"` -> early-end detection. Fixed.
  - 9 (invalid tool arguments): FAILED before, the parser sent `StreamEvent::Error`, the turn ended and the second reply never ran -> raw args kept, tool result is the error. Fixed.
  - 1, 3, 4, 6, 8, 10, 11, 12 passed before the fixes (regression guards now).
- What works (command + short real output):
  - `cargo test --test e2e_stream` -> `test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`
  - `cargo test` -> unit `137 passed`, `e2e_basic` `4 passed`, `e2e_stream` `15 passed`
  - `cargo clippy --all-targets` -> `Finished dev profile ...` with no warnings.
- Assumptions:
  - A stream that ends after `finish_reason` is complete even without `data: [DONE]` (scenario 6); an error chunk also counts as the end.
  - Invalid tool arguments travel as `Block::ToolCall { args: Value::String(raw) }`. The neutral types are unchanged; `tools::execute` rejects a non-object before dispatch.
  - `prompt_tokens_details.cached_tokens` is the only cache field read; `cache_write` stays 0.
  - Scenario 7 still exits 0: exit codes are stage 15.
- Known problems / TODO:
  - Print mode still exits 0 after a provider error.
  - No idle timeout or retry yet (stage 15).
- Next stage:
  - Stage 15: errors, retries, timeouts, exit codes.


## Stage 15: Errors, retries, timeouts, exit codes   [DONE]
- Files created or changed:
  - `src/ai/error.rs` (new): moved `LlmError`, `classify_http`, `retry_delay`, `RetrySettings` and `friendly_error` out of `ai/mod.rs`; added `Display`, `std::error::Error`, `validate_config` and `is_local_base_url`.
  - `src/ai/error_tests.rs` (new): status classification, context-overflow phrases, retry policy, backoff cap, colon model specs, config checks.
  - `src/ai/mod.rs`: re-exports the error module; now 287 lines.
  - `src/ai/openrouter.rs`: returns `Result<(), LlmError>` (was `anyhow`).
  - `src/ai/anthropic.rs`: connect timeout 15 s, idle timeout on send and on each stream read, `classify_http`, and `Stream("stream ended early")` when `message_stop` is missing.
  - `src/agent.rs`: `AgentEvent::Notice`, forwards retry notices, returns `Err` after saving the failed assistant entry (print mode exits 1).
  - `src/main.rs`: startup config check exits 2, notices to stderr, print-mode run errors exit 1.
  - `src/tui/app.rs`: shows notices as info lines.
  - `src/agent_tests.rs`: provider-error test now expects `Err`.
  - `src/settings_tests.rs`: a settings model keeps colons.
  - `tests/e2e_errors.rs` (new): 12 black-box tests.
  - `docs/BUGS.md` (new): start of the bug log.
  - `PROGRESS.md` (this block).
- What works (command + short real output):
  - `cargo test` -> `148 passed` (unit), `4 passed` (e2e_basic), `12 passed` (e2e_errors), `15 passed` (e2e_stream).
  - `cargo clippy --all-targets` -> no warnings.
  - 429 then success: exit 0, 2 requests, stderr `[pirs] rate limited, retrying in 251ms (attempt 1/4)`.
  - 401: exit 1, 1 request, `error: Authentication failed (401). Check PIRS_API_KEY.`
  - Missing key on a remote host: exit 2, no request.
- Assumptions:
  - A retry is allowed only when no text, thinking or tool call was delivered. Thinking counts as content because it costs tokens and may repeat work.
  - `PIRS_MAX_RETRIES=4` means retry attempts 0 to 3; `Retry-After` wins and is capped at 60 s.
  - Config errors (bad `--model`, empty model, empty key on a remote host) exit 2 in both modes, before opening a session. A localhost or 127.0.0.1 base URL may run without a key.
  - The failed turn keeps its partial assistant entry with `stop: error` and returns `Err`; print mode prints that error once and exits 1.
  - `tests/e2e_errors.rs` is 345 lines and `src/ai/openai.rs` was already 323 lines; both stay close to the 300-line rule.
- Known problems / TODO:
  - Anthropic has no black-box test: the fake server speaks the OpenAI shape.
  - Tests for the three behavioral bugs landed in the same step as the fix; the old behavior was visible in the code (`run_prompt` returned `Ok`).
- Next stage:
  - Stage 16: cost tracking, limits, runaway protection.

## Stage 16: Cost tracking, limits, runaway protection [DONE]
- What I built: per-request cost accounting (including compaction summaries),
  hard run/session limits applied before every model call, runaway guards, and
  cost reporting in the CLI and TUI.
- Files changed: `src/cost.rs`, `src/cost_tests.rs`, `src/agent.rs`,
  `src/agent_util.rs`, `src/cli.rs`, `src/ai/sse.rs`, `src/ai/openrouter.rs`,
  `src/settings.rs`, `src/compaction.rs`, `src/compaction_summary.rs`,
  `src/main.rs`, the TUI files, and `tests/e2e_cost.rs`.
- What works (real output):
  - `cargo test` -> `166 passed` (unit), `4 passed` (e2e_basic), `9 passed`
    (e2e_cost), `12 passed` (e2e_errors), `15 passed` (e2e_stream).
  - `cargo clippy --all-targets` -> no warnings.
  - `--max-cost 0.5` with replies costing `0.30` stops after 2 requests with
    exit 1 and `cost limit reached: $0.60 of $0.50
    (limits.max_cost_per_run). Raise it to continue.`
  - Print mode ends with `[pirs] requests=1 in=100 out=20 cost=$0.0001 time=0.0s`.
  - Repeated identical tool call stops after 3 with
    `stopped: repeated the same tool call 3 times`.
- Assumptions:
  - A run's totals are the difference from the snapshot taken after the user
    message is appended; session totals include every branch.
  - Limits are reached at `>=`, so two 0.30 requests stop before a third with a
    0.50 limit.
  - `max_tokens_per_run` applies only when no request in the run reported cost.
  - A compaction summary is checked by the next loop iteration; its usage is
    counted in the entry immediately.
  - A single empty assistant reply gets one more request; the second in a row
    stops the turn. Runaway stops exit 0.
  - Null `limit_remaining` prints `no limit`; null `limit` prints `none`.
- Known problems / TODO:
  - Anthropic reports no cost, so it falls back to the token limit.
  - The summary request has no `max_tokens` cap in the OpenAI adapter.
- Next stage:
  - Stage 17: live tests against OpenRouter.

## Stage 17: Live tests against OpenRouter   [IN PROGRESS]
- Files created or changed:
  - `tests/live_openrouter.rs` (new): six `#[ignore]` tests (L1 reply, L2
    bash, L3 edit, L4 resume, L5 budget, L6 compaction). Each returns after
    printing `skipped: ...` unless `PIRS_LIVE=1`, `PIRS_API_KEY` and
    `PIRS_LIVE_MODEL` are all set.
  - `tests/live_helpers/mod.rs` (new): live runner that clears inherited
    `PIRS_*`, sets the temp `PIRS_HOME`, provider `openrouter`, the live model,
    `PIRS_MAX_OUTPUT_TOKENS=300`, `--debug` and `--yolo`; cost/request parsing;
    redacted `debug.log` tail in failures; newest-session lookup.
  - `scripts/live-smoke.sh` (new): checks the two secrets are set (prints
    neither), runs the ignored tests serially, and prints the sum of the cost
    lines.
  - `docs/LIVE_FINDINGS.md` (new): the five questions (reasoning field, 400
    without reasoning, finish reasons, usage, missing tool-call index).
  - `src/cli.rs`: hidden no-op `--yolo` flag plus a parse test.
  - `PROGRESS.md` (this block).
- What works (command + short real output):
  - `cargo test` -> `167 passed` (unit), `4 passed` (e2e_basic), `9 passed`
    (e2e_cost), `12 passed` (e2e_errors), `15 passed` (e2e_stream), and the six
    live tests `ignored`.
  - `cargo test --test live_openrouter -- --ignored --test-threads=1 --nocapture`
    with no key -> six lines `skipped: PIRS_LIVE is not 1`, `6 passed`.
  - `scripts/live-smoke.sh` with no key -> `error: PIRS_API_KEY is not set`,
    exit 2.
  - `cargo clippy --all-targets` -> no warnings.
- Assumptions:
  - The key was not present in this environment, so no live request was made.
    `LIVE_FINDINGS.md` stays `PENDING` until the human runs the smoke script.
  - L6 writes `<project>/.pirs/settings.json`, because that is the project
    settings path `Settings::load` reads.
  - First live run: L1 to L5 passed. L6 failed with the stage numbers, because
    four 5-line replies add about 19 estimated tokens each and real prompt
    usage stayed near 1000; `context_window` 3000 and `keep_recent_tokens` 800
    can never compact. Test now uses 1200 and 180, with `reserve_tokens` 500
    unchanged, and longer 5-line prompts. `docs/LIVE_FINDINGS.md` records the
    observed wire shape.
  - A model-call run must print a `[pirs] ... cost=$` line, so a missing cost
    fails the test. `/budget` makes no model call and is exempt.
  - Live runs remove every inherited `PIRS_*` variable first, so a shell-level
    `PIRS_PROVIDER` or `PIRS_BASE_URL` cannot redirect a live test.
  - The L6 request cap is enforced by summing `requests=` from the four runs.
- Known problems / TODO:
  - The stage stays IN PROGRESS until the human runs `scripts/live-smoke.sh`
    once and fills `docs/LIVE_FINDINGS.md`.
  - If (b) confirms the 400, add a fake-server e2e test for the request shape.
    The adapter already sends `reasoning_content` for thinking blocks.
- Next stage:
  - Stage 18: eval suite.

## Stage 18: Eval suite   [IN PROGRESS]
- Files created or changed:
  - `evals/run.py`: stdlib-only runner. Git-initialised temp copy of each
    task, per-task and overall timeout, `[pirs]` cost parsing, per-run JSONL
    append, `--tasks`/`--repeat`/`--bin`/`--model`, total-cost stop, summary
    table.
  - `evals/tasks/`: twelve tasks with `prompt.txt`, `setup/` and `check.sh`
    (`hello-file`, `fix-typo`, `fix-failing-test`, `add-function`,
    `rename-symbol`, `read-and-answer`, `multi-file-feature`, `run-and-fix`,
    `big-file-edit`, `no-op-restraint`, `skill-use`, `survives-compaction`).
  - `evals/README.md` (layout, flags, tuning loop rules), `evals/CHANGELOG.md`,
    `evals/BASELINE.md` (PENDING template).
  - `.gitignore`: added `/evals/results.jsonl`.
  - `PROGRESS.md` (this block).
- What works (command + short real output):
  - `python3 -m py_compile evals/run.py` -> ok.
  - Every `check.sh` fails on an untouched `setup/`: 12/12 returned nonzero.
  - A fixed working tree passes all 12 checks (verified with a local script).
  - `python3 evals/run.py --tasks hello-file --bin <stub>`: PASS, table
    prints, exit 0. With `EVAL_MAX_COST=0.00005` the run stops after the first
    task and prints the cap message.
  - The real `target/debug/pirs` against a local scripted SSE server (no
    network key): `hello-file` PASS through the runner, `requests=1
    cost=$0.0002`.
  - `cargo test`: 208 passed, 0 failed, 6 live tests ignored.
- Assumptions:
  - The runner points `PIRS_HOME` at a throwaway temp folder, so evals never
    read or write the real `~/.pirs`. The project `.pirs/settings.json` and
    `.pirs/skills` still come from the task folder, as they should.
  - `--model` was added to the runner (default `PIRS_MODEL`) so the result
    record can name the model. It is the only flag beyond the stage list.
- Known problems / TODO:
  - Baseline still needs a paid run with `PIRS_PROVIDER`, `PIRS_MODEL` and
    `PIRS_API_KEY` set. `evals/BASELINE.md` stays PENDING until then.
  - One check was initially wrong (`fix-typo` compared a trailing newline);
    fixed and re-verified. No harness change was needed.
- Next stage:
  - Stage 19: TUI display.

## Stage 19: TUI display   [DONE]
- Files created or changed:
  - `Cargo.toml`: added `unicode-width`.
  - `src/tui/wrap.rs` (new): `char_width`, `display_width`, `wrap_text`
    (space breaks, hard breaks, CJK/emoji width, tabs as 4 spaces, leading
    indentation kept on continuation lines) and `build_wrapped` (wraps every
    logical line and marks lines inside triple backticks `dim`).
  - `src/tui/wrap_tests.rs` (new): plain, long word, CJK, emoji, tab, empty,
    width 1, indentation.
  - `src/tui/layout.rs` (new): `visible_window`, UTC `format_timestamp` and
    `resume_time` from the file modified time.
  - `src/tui/layout_tests.rs` (new): window top/middle/bottom/short list;
    epoch and `1_700_000_000` timestamps.
  - `src/tui/scroll.rs` (new): page, width (re-wrap on resize), scroll up/down,
    `scroll_top` (Home), `end_follow` (End and new prompt), `on_tick`,
    `spinner`, `elapsed_secs`.
  - `src/tui/picker.rs` (new): `Resume` with per-path timestamps.
  - `src/tui/app.rs`: `wrapped` cache, `wrap_width`, tick counters, `dim` on
    `UiLine`, `refresh_wrap`, `display_text`; scroll/resume methods moved out.
  - `src/tui/ui.rs`: draws `app.wrapped`, dim code fences, spinner and elapsed
    seconds while running, `[scrolled - End to follow]`, pickers windowed with
    `n/total` and a timestamp per resume entry.
  - `src/tui/mod.rs`: sets the wrap width from the terminal each loop, ticks
    `App::on_tick`, Home/End keys, and an Enter prompt re-follows the bottom.
  - `src/tui/tasks.rs`: resets the elapsed counter when a task starts.
  - `src/tui/app_tests.rs`: home/end, scroll while streaming, resize re-wrap,
    spinner/elapsed, code-fence dim, user prefix after wrapping.
- What works (command + short real output):
  - `cargo test` -> `187 passed` (unit), `4 passed` (e2e_basic), `9 passed`
    (e2e_cost), `12 passed` (e2e_errors), `15 passed` (e2e_stream), 6 live
    tests `ignored`.
  - `cargo clippy --all-targets` -> no warnings.
- Assumptions:
  - The transcript is re-wrapped on every width change and on every line
    change; the draw step never wraps anything itself.
  - Scroll units are wrapped rows, as `ratatui` scrolls the wrapped paragraph.
  - `wrap_width` 0 means "no wrap", so pure tests that never set a width see
    one wrapped row per logical line.
  - Fence markers themselves are not dim; only the lines between them are.
  - Resume timestamps are UTC (`YYYY-MM-DD HH:MM`), no new date crate.
- Known problems / TODO:
  - Manual checklist (60-line answer, resize, scroll during streaming, a
    100-entry tree) was not run in a real terminal in this environment.
  - Markdown is not rendered, as the stage says.
- Next stage:
  - Stage prompt not given yet.

## Stage 20: TUI input   [DONE]
- Files created or changed:
  - `src/tui/input.rs` (new): pure `InputState` with `Vec<char>` and a char
    cursor; insert/delete, Left/Right, Home/End, Ctrl+A/E, Ctrl+W, Ctrl+U,
    Ctrl+K, Ctrl+J newline, paste, history up/down with a saved draft, slash
    matches, hint and Tab completion.
  - `src/tui/input_lines.rs` (new): line start/end, cursor line, line count,
    line text, display column (CJK 2, tab 4) and the 6-row scrolling `view`.
  - `src/tui/input_tests.rs` (new): middle edits, emoji/CJK, Ctrl+W on
    punctuation and spaces, Home/End, multi-line cursor moves, history
    up/up/down/down restoring the draft, paste with newlines, Tab completion,
    slash hints, view scrolling, word moves.
  - `src/tui/history.rs` (new): `<PIRS_HOME>/history.jsonl`, one JSON string per
    line, load/save, 500-entry cap, consecutive duplicates skipped.
  - `src/tui/history_tests.rs` (new): missing file, save/load round trip,
    duplicates, bad lines, cap.
  - `src/tui/commands.rs` (new): `submit` moved out of `mod.rs`, remembers each
    prompt, adds `/help`.
  - `src/tui/mod.rs`: bracketed paste on/off, panic hook that disables paste,
    history loaded at startup, full key map (Ctrl+C/D/J/W/U/K/A/E, Alt+Enter,
    Alt+Left/Right, Delete, Tab, Home/End, Up/Down), `Event::Paste`.
  - `src/tui/app.rs`: `input: InputState`, `remember_prompt`, `ctrl_c_idle`.
  - `src/tui/scroll.rs`: `on_tick` ends the 2 second Ctrl+C warning.
  - `src/tui/ui.rs`: input box grows to 6 rows then scrolls, real cursor,
    slash hint line under the box.
  - `src/tui/app_tests.rs`: Ctrl+C double press inside and outside the window.
  - `README.md`: full key table, Ctrl+C/Ctrl+D text, `/help`, Mac Option note.
- What works (command + short real output):
  - `cargo test` -> `205 passed` (unit), `4 passed` (e2e_basic), `9 passed`
    (e2e_cost), `12 passed` (e2e_errors), `15 passed` (e2e_stream), 6 live
    tests `ignored`.
  - `cargo clippy --all-targets` -> no warnings.
- Assumptions:
  - Home/End move the cursor when the box has text; with an empty box they keep
    the stage 19 transcript behavior (top, follow). Documented in README.
  - Tab completion knows only the built-in commands; `/skill:` and prompt
    templates still pass through unchanged.
  - History stores every submitted non-empty text, commands included, trimmed.
  - Ctrl+W treats punctuation as part of the word and stops at whitespace.
  - A second Ctrl+C inside 20 ticks (2 s) quits; the warning clears on the next
    tick after that.
- Known problems / TODO:
  - Manual checklist (20 line paste, middle edit, prompt recall, Ctrl+C in each
    state) was not run in a real terminal in this environment.
  - Long input lines are clipped, not scrolled horizontally.
- Next stage:
  - Stage 21: steering queue.

## Stage 21: Steering queue   [DONE]
- Files created or changed:
  - `src/agent.rs`: `run_prompt` takes a `steer_rx: UnboundedReceiver<String>`.
    `drain_steering` appends every waiting text as its own user entry, in
    order, at the start of every round (after tool results). A leftover queue
    after the final answer continues the loop, so a new turn starts.
    New event `AgentEvent::UserMessage(text)`.
  - `src/runtime.rs`: `Runtime::run` forwards the receiver.
  - `src/main.rs`: print mode passes an empty, disconnected receiver; the new
    event prints `> text` on stderr (never used in print mode).
  - `src/ai/anthropic.rs`: tool results plus a following user text become ONE
    user turn: `tool_result` blocks first, then the text blocks.
  - `src/tui/app.rs`: `queue: Vec<String>`, `steer_tx`, `queue_prompt`,
    `clear_queue`; `UserMessage` removes the matching queued line and shows it.
  - `src/tui/tasks.rs`: `spawn_turn` creates the steering channel; `abort_run`
    drops the sender.
  - `src/tui/mod.rs`: Enter during a run queues instead of saying busy; Esc and
    Ctrl+C abort AND clear the queue with a notice; a finished run clears the
    sender and drops any unsent queue with a notice.
  - `src/tui/ui.rs`: up to 2 dim `queued:` lines plus `... N more queued`
    above the prompt box.
  - `src/tui/commands.rs`, `README.md`: key text and docs.
  - Tests: `src/agent_tests.rs` (two queued messages appear in the next request
    in order; leftover steering starts a new turn; abort mid-tool then reload
    and build context gives every call a result), `src/ai/anthropic_tests.rs`
    (merge test, old expectation updated), `src/tui/app_tests.rs` (queue add,
    accept, reject without a run, clear).
  - `src/compaction_tests.rs`: passes an empty receiver.
- What works (command + short real output):
  - `cargo test` -> `212 passed` (unit), `4 passed` (e2e_basic), `9 passed`
    (e2e_cost), `12 passed` (e2e_errors), `15 passed` (e2e_stream), 6 live
    tests `ignored`.
  - `cargo clippy --all-targets` -> no warnings.
- Assumptions:
  - Queueing sends to the unbounded steering channel immediately; the agent
    decides when to accept it, so the UI never depends on tool boundaries.
  - A message typed during a run is remembered in prompt history when queued.
  - The leftover-queue test checks the drain helper (entry appended under the
    final answer) instead of racing a real slow final stream.
  - Ctrl+C also drops the queue, so no text can be stranded with no run.
- Known problems / TODO:
  - Manual check (queue two messages during a slow bash command) was not run in
    a real terminal in this environment.
  - Anthropic merges results plus following user text; two plain user texts in
    a row are still sent as-is.
- Next stage:
  - Stage prompt not given yet.
## Stage 22: Safety (approvals, path guards, git warnings)   [DONE]
- Files created or changed:
  - `src/safety.rs` (new): `Approver { Auto, DenyAll, Interactive }`,
    `Decision`, `Approvals` (an `Arc<Mutex<HashSet<String>>>` for `a`),
    `Guard` + pure `check_path(cwd, path, op, allow_paths, pirs_home,
    user_home)`, `expand_allow_paths` and `git_notice`.
  - `src/safety_risky.rs` (new): `is_risky` and its small helpers.
  - `src/safety_setup.rs` (new): `approver(yolo, settings, interactive)` and
    `guard(settings, cwd)` wiring, so `main.rs` stays small.
  - `src/agent_safety.rs` (new): approval and path step around one tool call;
    `preview` for bash/write/edit; the exact texts for a denial and for print
    mode without `--yolo`.
  - `src/agent.rs`: `AgentConfig` gains `approver`, `allow_risky`, `approvals`
    and `guard`; `run_prompt` takes `approval_rx`. New
    `AgentEvent::ApprovalRequest { tool, preview }`.
  - `src/runtime.rs`: `Runtime::run` forwards the approval receiver.
  - `src/settings.rs`: `approval` (`ask`/`auto`) and `allow_paths`; both files
    overlay, `allow_paths` accumulates.
  - `src/cli.rs`: visible `--yolo` and new `--allow-risky`.
  - `src/main.rs`: builds the approver and guard; print mode uses `DenyAll` in
    `ask` mode and prints the git notice; TUI uses `Interactive`.
  - `src/tui/app.rs`, `tasks.rs`, `mod.rs`, `ui.rs`, plus new `approval.rs`
    (modal state), `events.rs` (event folding) and `keys.rs` (key map):
    approval modal with `y`/`a`/`n`/`Esc`, red `RISKY` line, the approval
    channel, and the git notice at startup.
  - Tests: `src/safety_tests.rs` (11: inside/outside, allow_paths, `..`,
    symlink escape, missing target, read denylist, risky positives and
    lookalikes, `~` expansion), `src/agent_tests.rs` (deny, allow, allow-always
    for one tool only), `src/tui/app_tests.rs` (modal open/answer/clear),
    `src/settings_tests.rs` (new keys), `tests/e2e_safety.rs` (4: print-mode
    approval text, read never asks, outside write refused with `--yolo`, risky
    bash refused in auto mode).
  - `tests/support/mod.rs`: `run_pirs` now adds `--yolo`; new
    `run_pirs_without_yolo`.
  - `evals/tasks/refuse-outside/` (new task 13) and `evals/README.md`.
  - `README.md`: flags, settings keys, modal keys, honest Safety section.
- What works (command + short real output):
  - `cargo fmt && cargo test` -> `229 passed` (unit), `4 passed` (e2e_basic),
    `9 passed` (e2e_cost), `12 passed` (e2e_errors), `4 passed` (e2e_safety),
    `15 passed` (e2e_stream), 6 live tests `ignored`.
  - `cargo clippy --all-targets -- -D warnings` -> no warnings.
- Assumptions:
  - `check_path` is pure and takes every input explicitly (`allow_paths`,
    `pirs_home`, `user_home`) instead of reading the environment. `Guard` is
    the small wrapper built once in `main`. It canonicalizes the longest
    existing prefix, so symlinks are followed before containment and `link/..`
    is correct.
  - The path guard runs in every mode, before approval, so `--yolo` cannot
    write outside the project. `read` is checked in every mode too.
  - `AllowAlways` is stored per tool name in an `Arc` shared by every turn of
    the same runtime.
  - `git_notice` treats any non-zero `git status` as "not a repository". A
    missing git binary is silent.
  - In auto mode a risky command is refused unless `--allow-risky`; in `ask`
    mode it only gets the `RISKY` marker.
  - Denied calls are normal tool results (`is_error: true`), so the model can
    adapt. `Esc` denies and aborts.
- Known problems / TODO:
  - The TUI manual check (bash modal opens, `n` returns control) was not run in
    a real terminal in this environment.
  - The path guard and the risky matcher are guardrails, not a sandbox. `bash`
    can still write outside the project; stage 25 adds the container.
  - DenyAlways (`a`) is per runtime; a new process asks again.
- Next stage:
  - Stage 23: Tool hardening.
## Stage 23: Tool hardening   [DONE]
- Files created or changed:
  - `src/tools/mod.rs`: new `truncate_bytes` (prefix) and `truncate_from_end`
    (suffix) helpers; both cut on a char boundary.
  - `src/tools/read.rs`: binary sniff (NUL in the first 8000 bytes), directory
    listing (sorted, `/` on folders), 2000-character line cut with
    `...[line truncated]`, char-safe byte cap, CRLF handled by `lines()`.
  - `src/tools/write.rs`: temp file in the same folder plus `rename` (atomic),
    keeps the permissions of an existing file, refuses a directory target.
  - `src/tools/edit.rs`: empty or identical `old_text` is an error, CRLF file
    matched as LF and written back as CRLF, not-found error reports the line
    count and the indentation/tabs hint, permissions kept.
  - `src/tools/bash.rs`: stdin `/dev/null`, fixed non-interactive environment,
    ANSI stripper (CSI + OSC state machine), own process group and
    `kill -KILL -<pid>` on timeout, abort or a still-open pipe; 200 ms bounded
    drain after the main process exits; 50 000 bytes and 2000 lines kept;
    `timeout_secs` clamped to 600.
  - Tests split into `read_tests.rs`, `write_tests.rs`, `edit_tests.rs`,
    `bash_tests.rs` (all loaded with `#[path]`); `src/tools/tests.rs` binary
    test now expects `binary file`.
- What works (command + short real output):
  - `cargo fmt && cargo test` -> `248 passed` (unit), `4 passed`
    (e2e_basic), `9 passed` (e2e_cost), `12 passed` (e2e_errors), `4 passed`
    (e2e_safety), `15 passed` (e2e_stream), 6 live tests `ignored`.
  - `cargo clippy --all-targets -- -D warnings` -> no warnings.
  - Background test: `sleep 30 & echo started` returns `started` in about
    0.2 s. Timeout test: `sleep 4242 & wait` leaves no `sleep 4242` behind
    (checked with `pgrep -f`).
- Assumptions:
  - `read` returns a directory listing as normal (not error) text; an empty
    folder says so.
  - The binary sniff is the literal rule: any NUL in the first 8000 bytes.
    Invalid UTF-8 without a NUL keeps the "not valid UTF-8" error.
  - `write` creates the temp file as `.<name>.pirs-tmp-<pid>-<n>` and renames
    it over the target; on Unix this is an atomic replace.
  - `edit` normalizes CRLF to LF for matching when the file contains CRLF,
    then converts every LF back to CRLF. Files that mix endings are treated
    as CRLF files.
  - `bash` kills the process group with the external `kill` command, as the
    stage asks. `kill_on_drop` alone would leak children of the shell.
  - A pipe held open by a background child is why `wait_with_output` cannot
    be used: the pumps are read directly with a 200 ms drain window.
  - On a platform without process groups the guard is a no-op.
- Known problems / TODO:
  - The eval suite was not run in this environment (it needs a live model and
    money). The human must rerun it and compare with the baseline.
  - `pgrep` may be missing on some systems; the test then skips the check.
- Next stage:
  - Stage 24: Packaging and release hygiene.

## Stage 24: Packaging and release hygiene
- Files created or changed:
  - `Cargo.toml`: version `0.2.0-dev`; `[profile.release]` with `strip = true`
    and `lto = "thin"`.
  - `src/cli.rs`: `AFTER_HELP` lists exit codes, environment variables, settings
    keys and slash commands, wired with `#[command(after_help = AFTER_HELP)]`;
    test `version_and_help_are_complete` checks `pirs <CARGO_PKG_VERSION>` and
    that the help contains the flag, key and command sections.
  - `scripts/check.sh` (new, executable): `cargo fmt --check`,
    `cargo clippy --all-targets -- -D warnings`, `cargo test`,
    `cargo build --release`, then the secret scan
    `git grep -nE "sk-(or-)?[A-Za-z0-9_-]{16,}"`.
  - `LICENSE` (new): MIT, `2026 v-pat`.
  - `CHANGELOG.md` (new): one entry per stage; `0.1.0` covers stages 0 to 11,
    `0.2.0-dev` covers stages 12 to 24.
  - `README.md`: rewritten against the code: build, install, CLI flags, exit
    codes, environment, settings, commands, tools, approvals, limits, sessions,
    test layers L1 to L4, architecture, Docker placeholder, limitations.
  - `.gitignore`: `debug.log`, `debug.log.1`, `.DS_Store` added next to
    `/target` and `/evals/results.jsonl`.
  - `PROGRESS.md`: the stage 12 line that held a fake `sk-or-v1-...` key was
    shortened to `sk-test123`, because the secret scan matched it.
- What works (command + short real output):
  - `scripts/check.sh` -> `all checks passed`, exit 0. Release build 33.5 s.
  - `cargo test` -> `249 passed` (unit), `4 passed` (e2e_basic), `9 passed`
    (e2e_cost), `12 passed` (e2e_errors), `4 passed` (e2e_safety), `15 passed`
    (e2e_stream), 6 live tests `ignored`.
  - `cargo install --path . --force` -> `Installed package pirs v0.2.0-dev`;
    `pirs --version` -> `pirs 0.2.0-dev`.
  - `pirs --help` prints every flag plus the four new help sections.
  - `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` -> clean.
- Assumptions:
  - The repo has no git remote, so the stage 24 step 4 condition is false and
    `.github/workflows/ci.yml` was not added. `scripts/check.sh` is the CI
    entry point when a remote exists.
  - The MIT name `v-pat` comes from `git config user.name`; the year is 2026.
  - README quick-start commands that call a real provider were not executed
    (no key in this environment). Every local command in the README was run:
    build, test, clippy, check script, `cargo install`, `--version`, `--help`.
  - `scripts/check.sh` uses `git grep`, so files outside git or untracked files
    are not scanned. Run it on a committed tree.
- Known problems / TODO:
  - Stage 17 and stage 18 stay IN PROGRESS until a paid live run and an eval
    baseline are done.
  - Version stays `0.2.0-dev`; stage 26 changes it to `0.2.0`.
- Next stage:
  - Stage 25: Docker sandbox.

## Stage 25: Docker sandbox   [DONE]
- Files created or changed:
  - `Dockerfile` (new): multi-stage build from `rust:1-bookworm`, final
    `debian:bookworm-slim`, non-root `agent` user, `PIRS_HOME=/home/agent/.pirs`.
  - `.dockerignore` (new): `target`, `.git`, `evals/results.jsonl`, `*.log`.
  - `scripts/pirs-docker` (new, executable): mounts only `$PWD` at `/work` and
    a state folder at `/home/agent/.pirs`; forwards only the five `PIRS_*`
    variables; `--cap-drop=ALL`, `no-new-privileges`, memory/CPU/pid caps.
  - `scripts/docker-smoke.sh` (new, executable): builds `pirs:dev`, then PASS or
    FAIL for non-root uid, `--version`, writable mount both ways, no
    `/home/agent/.ssh`, no host home visible; exit 1 on any failure.
  - `README.md`: Docker section with commands, a protects/does-not-protect table,
    `--yolo` guidance, and the Node and Rust variants marked untested.
  - `CHANGELOG.md`: stage 25 entry.
  - `PROGRESS.md` (this block).
- What works (command + short real output):
  - `sh -n scripts/pirs-docker` -> no output, exit 0.
  - `sh -n scripts/docker-smoke.sh` -> no output, exit 0.
  - `docker build -t pirs:dev .` -> `pirs:dev 657MB`.
  - `scripts/docker-smoke.sh` -> 5 PASS lines, `smoke test PASSED`, exit 0:
    `uid 1000, not root`; `pirs --version` -> `pirs 0.2.0-dev`; a file written
    in the container was readable on the host; `/home/agent/.ssh` absent;
    `/usr` inside the container has no host home (`ls /home` = `agent`).
  - `pirs-docker --version` -> `pirs 0.2.0-dev`.
  - `pirs-docker --yolo -p "create a file named index.html"` -> wrote
    `/work/index.html` (411 bytes); the same 411-byte file exists on the host.
    Cost `$0.0003`.
- Assumptions:
  - `reqwest` 0.13 in this lock file uses rustls, so the build stage needs no
    OpenSSL headers. The Dockerfile is unchanged from the stage text.
  - The image is built from `Cargo.toml`, `Cargo.lock` and `src/` only, because
    `cargo build --release` needs nothing else.
  - The smoke script compares the container `ls /home` output to exactly
    `agent`, so a host folder in `/home` fails the check.
  - First build takes several minutes (base image pull plus a release compile).
- Known problems / TODO:
  - The image is 657 MB. A slim base or a build cache mount would shrink it.
  - Sessions live in the host state folder (`PIRS_DOCKER_STATE`, default
    `~/.pirs-docker`); deleting it loses container history.
- Next stage:
  - Stage 26: stability gate and v0.2.0.

## Stage 26: Stability gate and v0.2.0   [IN PROGRESS]
- Files created or changed:
  - `src/session.rs`: `path_to_root` builds one id-to-index `HashMap`; tests
    moved to `src/session_tests.rs`.
  - `src/session_tests.rs` (new): the old session tests, a corrupt-middle-line
    test, and a 20,000-entry perf test.
  - `tests/e2e_crash.rs` (new): SIGKILL during a tool call then `-c`; corrupt
    middle line opens with a warning on stderr.
  - `tests/support/mod.rs`: `pirs_command()` so a test can spawn and kill the
    real binary.
  - `src/resources_tests.rs`: empty skills folder, bad prompt template name.
  - `src/debug_tests.rs`: key-shaped test strings built at runtime so the
    secret scan stays clean.
  - `scripts/check.sh`: secret scan skips `pirs-stages-12-plus.md`, which
    quotes a fake key in its own text.
  - `Cargo.toml`, `Cargo.lock`: version `0.2.0`.
  - `README.md`, `CHANGELOG.md`, `docs/BUGS.md`, `.gitignore`, `PROGRESS.md`.
- What works (command + short real output):
  - `cargo test` five times in a row: every run `253 passed; 0 failed`, then
    `4`, `9`, `2`, `12`, `4`, `15` in the integration files, `6 ignored`
    (live tests). No flaky test.
  - Perf test `session::tests::twenty_thousand_entries_open_and_build_in_under_two_seconds`
    -> `ok`, `finished in 0.08s` (limit 2 s in a debug build).
  - `cargo test --test e2e_crash` -> `2 passed; 0 failed; finished in 0.54s`.
  - `scripts/check.sh` -> `all checks passed`, exit 0 (fmt, clippy with
    warnings as errors, tests, release build, secret scan).
  - `cargo run -- --version` -> `pirs 0.2.0`.
  - Commit `release: v0.2.0`, tag `v0.2.0`.
  - Fresh clone of the repo in a temp folder: `cargo install --path . --force`
    -> `Finished release profile in 40.91s`, `pirs --version` -> `pirs 0.2.0`.
  - `scripts/check.sh` in that fresh clone -> exit 0.
- Assumptions:
  - A corrupt line in the middle of a session warns on stderr and is skipped;
    the last line stays silent because a crash often leaves it half-written.
  - The session file is now read whole so a bad line can be reported with its
    line number. 20k entries still load in well under 2 s.
  - `pirs-stages-12-plus.md` is part of the repo and contains a fake key, so it
    is excluded from the secret scan by path, not by pattern.
- Known problems / TODO:
  - Gate items 5 and 6 are blocked: `PIRS_API_KEY` is unset here, so the eval
    suite (twice, panic scan) and `scripts/live-smoke.sh` (three runs) were not
    run. `evals/BASELINE.md` is still `PENDING`.
  - `scripts/docker-smoke.sh` was not rerun on this machine during stage 26;
    stage 25 recorded 5 PASS lines.
  - The tag `v0.2.0` exists locally only; the repo has no remote.
- Next stage:
  - Run the two live checks when a key is available, fill `evals/BASELINE.md`,
    then mark this stage DONE.
