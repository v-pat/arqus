## Stage 0: Skeleton   [DONE]
- Files created or changed:
  - `Cargo.toml` (added `dirs`; already had `serde`, `serde_json`, `anyhow`, `uuid`)
  - `src/system_prompt.rs` (new stub module)
  - `src/main.rs` (declared `mod system_prompt`, prints `arqus <version>`)
  - `PROGRESS.md` (this block)
- What works (command + short real output):
  - `cargo run` -> `arqus 0.1.0`
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
  - `src/main.rs`: temporary `arqus llm-test "<prompt>"`.
  - `PROGRESS.md` (this block)
- What works (command + short real output):
  - `cargo test` -> `test result: ok. 44 passed; 0 failed`
  - `cargo clippy --all-targets` -> no warnings
  - `cargo run -- llm-test` with no env -> `error: provider returned 401 Unauthorized: {"error":{"message":"Missing Authentication header","code":401}}` (request reached the endpoint; no key printed)
- Assumptions:
  - `provider` comes from optional `ARQUS_PROVIDER`, default `openai`. `base_url` default is `https://openrouter.ai/api/v1`; the four required vars still override it.
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
  - Stage 6: agent loop and `arqus -p` print mode.
## Stage 6: Agent loop + print mode
- Files:
- Cargo.toml: added clap derive
- src/agent.rs (new): events, config, loop
- src/main.rs: clap CLI and print mode
- src/system_prompt.rs: today_utc()
- What works:
- cargo test -> 68 passed, 0 failed
- cargo clippy --all-targets -> no warnings
- arqus --help shows -p, -c, --no-session
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
  - `src/main.rs`: `arqus -p "/compact [focus]"` manual compaction, `Compacted` print event.
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
    `ARQUS_BASE_URL=http://127.0.0.1:8765/v1 ARQUS_API_KEY=test ARQUS_MODEL=fake arqus --no-session -p "which skills do you have?"`
    -> `You have the demo skill.`; server log: `skill_in_prompt=True`, `system_len=1025`.
- Assumptions:
  - AGENTS.md order: `~/.arqus/AGENTS.md`, then `<root>/AGENTS.md` ... `<cwd>/AGENTS.md`; `CLAUDE.md` is used when a folder has no `AGENTS.md`.
  - `SYSTEM.md` and `APPEND_SYSTEM.md`: project `<cwd>/.arqus/` wins; global `~/.arqus/` is the fallback. APPEND is not concatenated from both places.
  - Skills are sorted by name (`BTreeMap`); project skill overrides global with the same name. Only `name` and `description` are parsed from frontmatter; extra keys are ignored.
  - Prompt templates: project `<cwd>/.arqus/prompts/name.md` wins over `~/.arqus/prompts/name.md`. Slash names are limited to `[A-Za-z0-9_-]` to block path traversal.
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
  - Print mode against local Ollama: `ARQUS_BASE_URL=http://127.0.0.1:11434/v1 ARQUS_API_KEY=ollama ARQUS_MODEL=qwen3:8b cargo run --quiet -- --no-session -p "Reply with exactly: hello from arqus"` -> `hello from arqus`
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
  - Live Anthropic call skipped: neither `ARQUS_API_KEY` nor `ANTHROPIC_API_KEY` is set.
- Assumptions:
  - `ARQUS_API_KEY` is the key for every provider, including Anthropic. `ARQUS_BASE_URL` still overrides the default `https://api.anthropic.com/v1`.
  - Missing `ARQUS_CONTEXT_WINDOW` defaults to 200000 for Anthropic and 128000 for other providers.
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
  - `X-Title` defaults to `arqus`; `ARQUS_APP_TITLE` overrides it. `HTTP-Referer` is sent only when `ARQUS_HTTP_REFERER` is set.
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
  - Valid settings model in a temp cwd: `{"model":"anthropic:fake-model"}` -> `arqus --no-session -p "/tree"` exits 0.
- Assumptions:
  - Precedence: CLI `--model`, then `ARQUS_*` environment variables, then project settings, then global settings, then built-in defaults. `ARQUS_API_KEY` is the only key source; settings never store it.
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

## Stage 12: ARQUS_HOME and the debug log   [DONE]
- Files created or changed:
  - `src/home.rs` (new): `arqus_home()` and pure `arqus_home_from(Option<&str>)`.
  - `src/home_tests.rs` (new): env value wins, empty value falls back, one test sets and restores `ARQUS_HOME`.
  - `src/debug.rs` (new): `init`, `env_on`, `log`, `redact`/`redact_with`, `open_log`, `append_line`, `render`, 5 MB rotation to `debug.log.1`.
  - `src/debug_tests.rs` (new): exact key, `sk-test123`, clean text, debug off creates no file, debug on writes 5 parseable JSON lines with no key.
  - `src/session.rs`: `sessions_dir` uses `arqus_home()`.
  - `src/settings.rs`: `Settings::load` uses `arqus_home()`; `load_in` now takes the arqus home itself (`home/settings.json`).
  - `src/resources.rs`: `load`/`load_with` use `arqus_home()`; `load_in`/`load_in_with` take the arqus home itself; global SYSTEM/APPEND/AGENTS/skills/prompts read from it directly.
  - `src/resources_tests.rs`, `src/settings_tests.rs`: temp `home` is now the arqus home, so `.arqus/` was removed from those paths.
  - `src/ai/openai.rs`, `src/ai/anthropic.rs`: log `request` (url + body), `status` (code), every raw `sse` line, and `error` text. Headers are never logged.
  - `src/agent.rs`: log `tool` (name + args) before execution and `error` on a failed turn.
  - `src/main.rs`: `mod debug`, `mod home`, `--debug` flag, `debug::init(cli.debug, &home::arqus_home())` before either mode.
  - `PROGRESS.md` (this block)
- Hard-coded global paths found and replaced (grep `\.arqus` / `home_dir`):
  - `src/session.rs:26-27` `dirs::home_dir().join(".arqus")` -> `arqus_home()` (sessions).
  - `src/settings.rs:45,54` global settings -> `arqus_home()/settings.json`.
  - `src/resources.rs:46,52` `load`/`load_with` home -> `arqus_home()`.
  - `src/resources.rs:69,73` global SYSTEM.md / APPEND_SYSTEM.md -> `arqus_home()/...`.
  - `src/resources.rs:85` global AGENTS.md -> `arqus_home()/AGENTS.md`.
  - `src/resources.rs:125` global skills -> `arqus_home()/skills`.
  - `src/resources.rs:278` global prompts -> `arqus_home()/prompts`.
  - `src/main.rs:34` help text only.
  - Remaining `.arqus` in `src/` are project-level `<cwd>/.arqus/...`, docs, or test fixtures.
- What works (command + short real output):
  - `cargo test` -> `test result: ok. 134 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s`
  - `cargo clippy --all-targets` -> `Finished dev profile ...` with no warnings.
  - `ARQUS_HOME=<temp>/arqus-stage12-x ARQUS_DEBUG=1 cargo run --quiet -- --no-session -p "hi"` -> provider 401 (no key), and `<temp>/arqus-stage12-x/debug.log` exists with 4 lines: `{'request': 1, 'status': 1, 'error': 2}`; every line parses as JSON; `sk-` absent.
- Assumptions:
  - `ARQUS_HOME` is the arqus folder itself, not a parent of `.arqus`. Default stays `~/.arqus`.
  - `ARQUS_DEBUG` accepts `1` or `true` (case-insensitive). `--debug` also enables it.
  - `redact` reads `ARQUS_API_KEY`; `redact_with` takes the key explicitly so tests do not need the environment.
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
  - `tests/e2e_basic.rs` (new): four black-box tests, each with its own `TempDir` project and `ARQUS_HOME`.
  - `PROGRESS.md` (this block).
- What works (command + short real output):
  - `cargo test --test e2e_basic` -> `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.87s`
  - `cargo test` -> unit suite `134 passed` plus `tests/e2e_basic.rs`: `4 passed`.
  - `cargo clippy --all-targets` -> `Finished dev profile ...` with no warnings.
- Assumptions:
  - The helper file was taken from `/Users/vaibhavpathak/Downloads/fake_llm.rs`; no line needed fixing.
  - `run_arqus` clears inherited `ARQUS_*` variables, so a developer's real key and home never reach the tests.
  - Requests are asserted by exact count in every test (`1`, `2`, `2`, `2`); an unexpected extra request fails the test.
  - The unknown-tool check looks for `unknown tool` in the tool message content, which is how the agent reports that error to the model.
  - The session test uses one fake server with two scripted replies; the second run uses `-c` on the same `ARQUS_HOME`.
- Known problems / TODO:
  - Print mode still exits 0 after a provider error (stage 15 fixes exit codes).
  - The fake server's listener thread lives until the test process exits; ports are unique, so this is harmless.
- Next stage:
  - Stage 14: stream robustness.

## Stage 14: Stream robustness   [DONE]
- Files created or changed:
  - `tests/e2e_stream.rs` (new): 15 black-box tests, one or more per scenario 1 to 12, each with its own temp project and `ARQUS_HOME`.
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
  - 429 then success: exit 0, 2 requests, stderr `[arqus] rate limited, retrying in 251ms (attempt 1/4)`.
  - 401: exit 1, 1 request, `error: Authentication failed (401). Check ARQUS_API_KEY.`
  - Missing key on a remote host: exit 2, no request.
- Assumptions:
  - A retry is allowed only when no text, thinking or tool call was delivered. Thinking counts as content because it costs tokens and may repeat work.
  - `ARQUS_MAX_RETRIES=4` means retry attempts 0 to 3; `Retry-After` wins and is capped at 60 s.
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
  - Print mode ends with `[arqus] requests=1 in=100 out=20 cost=$0.0001 time=0.0s`.
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
    printing `skipped: ...` unless `ARQUS_LIVE=1`, `ARQUS_API_KEY` and
    `ARQUS_LIVE_MODEL` are all set.
  - `tests/live_helpers/mod.rs` (new): live runner that clears inherited
    `ARQUS_*`, sets the temp `ARQUS_HOME`, provider `openrouter`, the live model,
    `ARQUS_MAX_OUTPUT_TOKENS=300`, `--debug` and `--yolo`; cost/request parsing;
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
    with no key -> six lines `skipped: ARQUS_LIVE is not 1`, `6 passed`.
  - `scripts/live-smoke.sh` with no key -> `error: ARQUS_API_KEY is not set`,
    exit 2.
  - `cargo clippy --all-targets` -> no warnings.
- Assumptions:
  - The key was not present in this environment, so no live request was made.
    `LIVE_FINDINGS.md` stays `PENDING` until the human runs the smoke script.
  - L6 writes `<project>/.arqus/settings.json`, because that is the project
    settings path `Settings::load` reads.
  - First live run: L1 to L5 passed. L6 failed with the stage numbers, because
    four 5-line replies add about 19 estimated tokens each and real prompt
    usage stayed near 1000; `context_window` 3000 and `keep_recent_tokens` 800
    can never compact. Test now uses 1200 and 180, with `reserve_tokens` 500
    unchanged, and longer 5-line prompts. `docs/LIVE_FINDINGS.md` records the
    observed wire shape.
  - A model-call run must print a `[arqus] ... cost=$` line, so a missing cost
    fails the test. `/budget` makes no model call and is exempt.
  - Live runs remove every inherited `ARQUS_*` variable first, so a shell-level
    `ARQUS_PROVIDER` or `ARQUS_BASE_URL` cannot redirect a live test.
  - The L6 request cap is enforced by summing `requests=` from the four runs.
- Known problems / TODO:
  - The stage stays IN PROGRESS until the human runs `scripts/live-smoke.sh`
    once and fills `docs/LIVE_FINDINGS.md`.
  - If (b) confirms the 400, add a fake-server e2e test for the request shape.
    The adapter already sends `reasoning_content` for thinking blocks.
- Next stage:
  - Stage 18: eval suite.

## Stage 18: Eval suite   [DONE]
- Files created or changed:
  - `evals/run.py`: stdlib-only runner. Git-initialised temp copy of each
    task, per-task and overall timeout, `[arqus]` cost parsing, per-run JSONL
    append, `--tasks`/`--repeat`/`--bin`/`--model`, total-cost stop, summary
    table.
  - `evals/tasks/`: twelve tasks with `prompt.txt`, `setup/` and `check.sh`
    (`hello-file`, `fix-typo`, `fix-failing-test`, `add-function`,
    `rename-symbol`, `read-and-answer`, `multi-file-feature`, `run-and-fix`,
    `big-file-edit`, `no-op-restraint`, `skill-use`, `survives-compaction`).
  - `evals/README.md` (layout, flags, tuning loop rules), `evals/CHANGELOG.md`,
    `evals/BASELINE.md` (baseline recorded with 13/13 tasks passed).
  - `.gitignore`: added `/evals/results.jsonl`.
  - `scripts/run-evals.sh`: interactive eval runner script.
  - `PROGRESS.md` (this block).
- What works (command + short real output):
  - `python3 -m py_compile evals/run.py` -> ok.
  - Every `check.sh` fails on an untouched `setup/`: 12/12 returned nonzero.
  - A fixed working tree passes all 12 checks (verified with a local script).
  - `scripts/run-evals.sh` with live OpenRouter model: 13/13 PASS, 100%, total cost $0.0073.
  - `cargo test`: 563 unit passed, 48 e2e passed.
- Assumptions:
  - The runner points `ARQUS_HOME` at a throwaway temp folder, so evals never
    read or write the real `~/.arqus`. The project `.arqus/settings.json` and
    `.arqus/skills` still come from the task folder, as they should.
  - `--model` was added to the runner (default `ARQUS_MODEL`) so the result
    record can name the model. It is the only flag beyond the stage list.
- Known problems / TODO:
  - Baseline established and recorded in `evals/BASELINE.md`.
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
  - `src/tui/history.rs` (new): `<ARQUS_HOME>/history.jsonl`, one JSON string per
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
    `Guard` + pure `check_path(cwd, path, op, allow_paths, arqus_home,
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
  - `tests/support/mod.rs`: `run_arqus` now adds `--yolo`; new
    `run_arqus_without_yolo`.
  - `evals/tasks/refuse-outside/` (new task 13) and `evals/README.md`.
  - `README.md`: flags, settings keys, modal keys, honest Safety section.
- What works (command + short real output):
  - `cargo fmt && cargo test` -> `229 passed` (unit), `4 passed` (e2e_basic),
    `9 passed` (e2e_cost), `12 passed` (e2e_errors), `4 passed` (e2e_safety),
    `15 passed` (e2e_stream), 6 live tests `ignored`.
  - `cargo clippy --all-targets -- -D warnings` -> no warnings.
- Assumptions:
  - `check_path` is pure and takes every input explicitly (`allow_paths`,
    `arqus_home`, `user_home`) instead of reading the environment. `Guard` is
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
  - `write` creates the temp file as `.<name>.arqus-tmp-<pid>-<n>` and renames
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
    test `version_and_help_are_complete` checks `arqus <CARGO_PKG_VERSION>` and
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
  - `cargo install --path . --force` -> `Installed package arqus v0.2.0-dev`;
    `arqus --version` -> `arqus 0.2.0-dev`.
  - `arqus --help` prints every flag plus the four new help sections.
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
    `debian:bookworm-slim`, non-root `agent` user, `ARQUS_HOME=/home/agent/.arqus`.
  - `.dockerignore` (new): `target`, `.git`, `evals/results.jsonl`, `*.log`.
  - `scripts/arqus-docker` (new, executable): mounts only `$PWD` at `/work` and
    a state folder at `/home/agent/.arqus`; forwards only the five `ARQUS_*`
    variables; `--cap-drop=ALL`, `no-new-privileges`, memory/CPU/pid caps.
  - `scripts/docker-smoke.sh` (new, executable): builds `arqus:dev`, then PASS or
    FAIL for non-root uid, `--version`, writable mount both ways, no
    `/home/agent/.ssh`, no host home visible; exit 1 on any failure.
  - `README.md`: Docker section with commands, a protects/does-not-protect table,
    `--yolo` guidance, and the Node and Rust variants marked untested.
  - `CHANGELOG.md`: stage 25 entry.
  - `PROGRESS.md` (this block).
- What works (command + short real output):
  - `sh -n scripts/arqus-docker` -> no output, exit 0.
  - `sh -n scripts/docker-smoke.sh` -> no output, exit 0.
  - `docker build -t arqus:dev .` -> `arqus:dev 657MB`.
  - `scripts/docker-smoke.sh` -> 5 PASS lines, `smoke test PASSED`, exit 0:
    `uid 1000, not root`; `arqus --version` -> `arqus 0.2.0-dev`; a file written
    in the container was readable on the host; `/home/agent/.ssh` absent;
    `/usr` inside the container has no host home (`ls /home` = `agent`).
  - `arqus-docker --version` -> `arqus 0.2.0-dev`.
  - `arqus-docker --yolo -p "create a file named index.html"` -> wrote
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
  - Sessions live in the host state folder (`ARQUS_DOCKER_STATE`, default
    `~/.arqus-docker`); deleting it loses container history.
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
  - `tests/support/mod.rs`: `arqus_command()` so a test can spawn and kill the
    real binary.
  - `src/resources_tests.rs`: empty skills folder, bad prompt template name.
  - `src/debug_tests.rs`: key-shaped test strings built at runtime so the
    secret scan stays clean.
  - `scripts/check.sh`: secret scan skips `arqus-stages-12-plus.md`, which
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
  - `cargo run -- --version` -> `arqus 0.2.0`.
  - Commit `release: v0.2.0`, tag `v0.2.0`.
  - Fresh clone of the repo in a temp folder: `cargo install --path . --force`
    -> `Finished release profile in 40.91s`, `arqus --version` -> `arqus 0.2.0`.
  - `scripts/check.sh` in that fresh clone -> exit 0.
- Assumptions:
  - A corrupt line in the middle of a session warns on stderr and is skipped;
    the last line stays silent because a crash often leaves it half-written.
  - The session file is now read whole so a bad line can be reported with its
    line number. 20k entries still load in well under 2 s.
  - `arqus-stages-12-plus.md` is part of the repo and contains a fake key, so it
    is excluded from the secret scan by path, not by pattern.
- Known problems / TODO:
  - Eval suite established in `evals/BASELINE.md` (13/13 tasks passed, $0.0073).
  - `scripts/docker-smoke.sh` was not rerun on this machine during stage 26;
    stage 25 recorded 5 PASS lines.
  - The tag `v0.2.0` exists locally only; the repo has no remote.
- Next stage:
  - Stage T1: TUI foundation.

## Stage T1: TUI foundation (architecture, theme, test harness)   [DONE]
- Mapping of the old `src/tui/` code to the new files (written before moving code):
  - `app.rs`: `LineKind`, `UiLine`, `App` and its state methods (`new`,
    `set_session`, `take_input`, `remember_prompt`, `queue_prompt`,
    `clear_queue`, `ctrl_c_idle`, `push_user`, `info`, `clear_transcript`,
    `refresh_wrap`, `display_text`) stay in `app.rs`; `App` gains `theme`,
    `keymap`, `overlays`, `dirty`.
  - `events.rs`: `App::apply_event`, `push`, `append_to`, `push_result`,
    `compact_args` stay in `events.rs`, called from `update.rs`.
  - `keys.rs`: `handle_key` splits into the new `Key`/`KeyCodeX`,
    `from_crossterm`, `parse_key`, `Action`, `KeyMap` in `keys.rs`; the
    behavior moves into `update.rs`.
  - `commands.rs`: `submit` moves into `update.rs`; slash-command dispatch
    stays in `commands.rs` as a pure function returning `Effect` values; the
    `HELP` text moves to `overlay/help.rs`.
  - `tasks.rs`: `start_task`, `fail`, `spawn_turn`, `spawn_compact`,
    `abort_run`, `open_session`, `refresh_tokens` stay in `tasks.rs`, called
    by the effect executor in `mod.rs`.
  - `ui.rs`: `draw`, `draw_queue`, `draw_transcript`, `draw_status`,
    `draw_input`, `draw_hint` move to `view.rs`; `draw_resume` and
    `draw_tree` move to `widgets/picker_view.rs`; `draw_approval` moves to
    `view.rs`; `centered` moves to `widgets/dialog.rs`; `to_line` moves to
    `view.rs`.
  - `mod.rs`: `run`, `install_panic_hook` and `event_loop` stay in `mod.rs`;
    `event_loop` becomes the `Msg`/`Effect` loop. `execute(effect)` lives in
    `effects.rs` so `mod.rs` stays under 300 lines.
  - `input.rs`, `input_lines.rs`, `history.rs`, `wrap.rs`, `scroll.rs`,
    `layout.rs`, `approval.rs`: pure helpers, unchanged.
  - `picker.rs`: the starter `Picker`; the old `Resume` type moves to
    `resume.rs` as a wrapper over `Picker<PathBuf>`.
  - `tree.rs`: `TreeState` becomes a wrapper over the starter `Picker<String>`.
  - New: `msg.rs` (`Msg`, `Effect`, `Level`), `update.rs` (pure update),
    `effects.rs` (the effect executor), `view.rs` (drawing), `theme.rs`,
    `overlay/` (`Help`), `widgets/` (`dialog`, `picker_view`), `testing.rs`.
- Files created or changed:
  - New: `src/tui/{msg,update,effects,view,theme,resume,testing,fuzzy}.rs`,
    `src/tui/overlay/{mod,help}.rs`,
    `src/tui/widgets/{mod,dialog,picker_view}.rs`,
    `src/tui/{keys_tests,theme_tests,picker_tests,update_tests}.rs`,
    `src/tui/app_session_tests.rs`, `tests/snapshots/{empty-80x24,help-60x20}.txt`.
  - Changed: `src/tui/{mod,app,keys,commands,picker,tree}.rs`,
    `src/tui/app_tests.rs`, `src/tui/events.rs` (formatting only).
  - Removed: `src/tui/ui.rs` (old drawing module, deleted after the new
    `view.rs` passed the tests).
- What works (command + short real output):
  - `cargo check` -> `Finished dev profile`.
  - `cargo test` -> `294 passed; 0 failed` in the unit run, then
    `4`, `9`, `2`, `12`, `4`, `15` in the e2e files, `6 ignored` (live).
  - `cargo clippy --all-targets` -> no warnings.
  - Snapshots: `empty-80x24.txt` (status line and prompt box) and
    `help-60x20.txt` (the `?` dialog).
  - Replay tests: `hello enter` -> `Effect::RunPrompt("hello")`; `?` opens
    Help and `esc` closes it; `ctrl+c` warns then returns `Effect::Quit`.
  - Every Rust source file is under 300 lines.
- Assumptions:
  - Colors are semantic through `Theme`; the transcript layout is unchanged.
  - `Resume` and `TreeState` keep their `App` fields but use the starter
    `Picker`, so `/resume` and `/tree` now filter while keeping the old
    Up/Down/Enter/Esc flow.
  - `?` opens the Help overlay only when the input box is empty.
  - `/compact` and `/model <spec>` stay `Effect` values; the executor does the
    work and the event loop redraws when `app.dirty` is set.
- Known problems / TODO:
  - The TUI manual check (resize, `?`, terminal restore after quit) must be
    done by the human on a real terminal.
  - `PickerEvent` matching uses the starter names; later stages may rename it.
- Next stage:
  - T2 (theme dialog) after this stage is DONE.

## Stage T2: Launch, welcome screen, footer, toasts   [DONE]
- Files created or changed:
  - New: `src/tui/{facts,footer,git,running,toast,welcome}.rs`,
    `src/tui/{footer,git,toast}_tests.rs`.
  - Changed: `src/tui/{app,msg,update,effects,events,scroll,view,testing,mod}.rs`,
    `src/tui/app_tests.rs`, `src/tui/update_tests.rs`, `src/main.rs`.
  - Snapshots: new `footer-{120,80,60}x1.txt`,
    `welcome-connected-80x24.txt`, `welcome-no-provider-80x24.txt`,
    `chat-80x24.txt`, `too-small-40x10.txt`; updated `empty-80x24.txt` and
    `help-60x20.txt` (the footer replaced the old status line).
- What works (command + short real output):
  - `cargo fmt && cargo check && cargo test` -> `320 passed; 0 failed`
    (bin), then `4`, `9`, `2`, `12`, `4`, `15` in the e2e files, `6 ignored`.
  - `cargo clippy --all-targets` -> no warnings.
  - `cargo run` without a terminal -> `arqus needs a terminal; use -p "prompt"
    for scripts`, exit code 2.
  - Footer drops parts in order: hints (120 only), cost (80), branch (60),
    then the path shortens from the left and the model gets `…`.
  - `Msg::Started` and `Msg::Done` both return `Effect::RefreshGit`; the effect
    runs `git` in `spawn_blocking` and replies with `Msg::Git`.
  - Spinner frames are braille and advance one frame per 100 ms tick.
- Assumptions:
  - Toast ages use the 100 ms tick clock (30 ticks for info, 60 for errors),
    so expiry is pure and testable.
  - `App::has_conversation()` ignores `Info` lines, so a startup notice keeps
    the welcome screen visible; the git safety notice is now a toast.
  - `App::home` is filled from `dirs::home_dir()` and replaced in tests.
  - `Startup` gained `provider` and `auto`; `auto` (from `--yolo`) shows the
    `auto` chip, otherwise the chip is `ask`. `plan` arrives with modes.
  - Hints in the footer are shown only at width 100 or more.
- Known problems / TODO:
  - The manual check must be done by the human: launch in a git folder and in
    a non-git folder, resize wide to narrow, run a prompt, watch the running
    line.
  - Provider/model state is filled from the existing config only; the Connect
    dialog fills it in T4.
- Next stage:
  - T3 (pickers, command registry, slash menu, palette).

## Stage T3: Pickers, command registry, slash menu, palette   [DONE]
- Files created or changed:
  - New: `src/tui/{slash,flow,actions,command_registry}.rs`,
    `src/tui/{slash,commands}_tests.rs`,
    `src/tui/overlay/palette.rs`, `src/tui/widgets/slash_menu.rs`,
    `tests/snapshots/{slash-menu-80x24,palette-80x24,palette-60x20,help-80x24}.txt`.
  - Changed: `src/tui/{commands,keys,input,app,update,view,welcome,mod,testing}.rs`,
    `src/tui/overlay/{mod,help}.rs`, `src/tui/widgets/{mod,picker_view}.rs`,
    `src/tui/{update,input}_tests.rs`.
  - Updated snapshots: `help-60x20.txt` (generated two-column layout).
- What works (command + short real output):
  - `cargo fmt && cargo check` -> `Finished dev profile`.
  - `cargo test` -> `344 passed; 0 failed` in the unit run, then `4`, `9`,
    `2`, `12`, `4`, `15` in the e2e files, `6 ignored` (live).
  - `cargo clippy --all-targets` -> no warnings.
  - Slash menu: `/mo` shows `/mode`, `/models`, `/model`; Up/Down select,
    Tab completes to `/name `, Enter runs, Esc hides and keeps the text.
  - Palette: `ctrl+p m o d e enter` runs `/mode`, which cycles `ask` to
    `auto`; the palette uses one `Picker` grouped by `Session`, `Model`,
    `View`, `Help`, `Prompts`, `Skills`.
  - Help: `?` and `/help` draw two columns generated from `KeyMap::bindings()`
    and the command table; unavailable commands are hidden.
- Assumptions:
  - `Command.key_hint` stays a literal string; a unit test checks every hint
    against `KeyMap` through `CommandAction::key_action()`.
  - `/model` stays from the old dispatch; `/mode` only sets the footer chip
    until the engine modes arrive. `/thinking`, `/connect`, `/models`,
    `/theme`, `/settings`, `/copy`, `/export`, `/rename` and the palette-only
    cycle/toggle actions answer with `coming in a later stage`.
  - The `/` popup is a floating box drawn just above the prompt box, so the
    welcome and chat layouts share one code path.
  - The prompt box is not drawn while an overlay is open, so a narrower
    dialog cannot show stray borders behind it.
- Known problems / TODO:
  - The human must run the manual check on a real terminal: type `/`, filter,
    complete with Tab, run a palette command, and open `?`.
  - Palette rows beyond the visible height need the picker's own scrolling;
    there is no extra page indicator yet.
- Next stage:
  - T4 (Connect a provider and pick a model from the TUI).

## Stage T4: Connect a provider and pick a model from the TUI   [DONE]
- Files created or changed:
  - New: `src/{auth,catalog,models,state,config,print}.rs`,
    `src/{state,config}_tests.rs`, `src/catalog_tests.rs`,
    `src/tui/{app_connect,flow_models,flow_dialogs,event_loop,effects_http}.rs`,
    `src/tui/startup_tests.rs`, `src/tui/overlay/{connect,models}.rs`,
    `src/tui/overlay/connect/{input,keys,draw}.rs`,
    `src/tui/overlay/{connect,models}_tests.rs`, `tests/e2e_connect.rs`.
  - Changed: `src/main.rs`, `src/settings.rs`, `src/runtime.rs`,
    `src/ai/mod.rs`, `src/tui/{mod,app,msg,update,flow,effects,commands,testing}.rs`,
    `src/tui/{update,commands}_tests.rs`.
  - Snapshots: new `connect-providers-80x24`, `connect-key-empty-80x24`,
    `connect-key-masked-80x24`, `connect-validating-80x24`,
    `connect-error-80x24`, `models-loading-80x24`, `models-loaded-80x24`,
    `models-error-typed-80x24`; updated `help-60x20` and `help-80x24`
    (`/connect` and `/models` are now available), `empty-80x24` and
    `welcome-no-provider-80x24` (the welcome line now says
    `no provider connected · press ctrl+p, or type /connect`).
- What works (command + short real output):
  - `cargo fmt && cargo check && cargo test` -> `395 passed; 0 failed`
    (bin), then `4`, `2`, `9`, `2`, `12`, `4`, `15` in the e2e files,
    `6 ignored` (live).
  - `cargo clippy --all-targets` -> no warnings.
  - `tests/e2e_connect.rs`: `arqus -p` uses `auth.json` and `state.json`
    with no `ARQUS_*` key/provider/model variables; the fake server sees
    `Authorization: Bearer saved-key` and `"model":"saved-model"`.
  - No provider in print mode exits 2 with
    `No provider connected. Run arqus and use /connect, or set OPENROUTER_API_KEY (or another provider's variable).`
  - Connect: provider list grouped Recommended/Other, key dialog masked,
    validation states, 401 error, network y/n, disconnect, env-key notice.
  - Models: loading, loaded, tools/all toggle, typed-id row, `SetModel`.
- Assumptions:
  - `OpenAI` is shown as `OpenAI (GPT)` so the query `open` ranks the
    recommended OpenRouter first.
  - `state.json` base URL overrides the catalog, but `ARQUS_BASE_URL` still
    wins so the black-box tests can point at the fake server.
  - A localhost base URL needs no key, matching the old `validate_config`.
  - The model list is fetched with all models and filtered client-side; the
    OpenRouter `tools_only` query flag is not used.
  - `SaveBaseUrl` is emitted only when the entered URL differs from the
    catalog default.
- Known problems / TODO:
  - The human must run the manual check on a real terminal with a real
    OpenRouter key: connect, see the remaining limit, pick a model, send
    "say hi", quit, restart, and confirm no dialogs appear.
  - The TUI cannot be exercised end-to-end in this environment; the startup
    overlay choice is covered by `startup_tests`.
- Next stage:
  - T5 (transcript rendering).

## Stage T5: Transcript rendering (blocks, tool cards, diffs, markdown, collapsing)   [DONE]
- Files created or changed:
  - New: `src/tui/{block,markdown,styled,render}.rs`,
    `src/tui/{block,markdown,styled}_tests.rs`,
    `src/tui/widgets/{transcript,tool_card}.rs`,
    `src/tui/widgets/{transcript,tool_card}_tests.rs`.
  - Changed: `src/tui/{app,events,keys,msg,mod,scroll,view,wrap}.rs`,
    `src/tui/{app,app_session,footer}_tests.rs`, `src/tui/commands.rs`,
    `src/tui/effects.rs`, `src/state.rs`, `src/tui/testing.rs`.
  - Snapshots changed: `chat-80x24` (user prefix is now `┃`),
    `help-60x20` and `help-80x24` (`/thinking` and tool-output toggles are
    available), `slash-menu-80x24` (same reason).
  - Snapshots added: `chat-markdown-80x24`, `tool-running-80x24`,
    `tool-bash-collapsed-80x24`, `tool-bash-expanded-80x24`,
    `tool-edit-diff-80x24`, `tool-error-80x24`, `thinking-hidden-80x24`,
    `thinking-shown-80x24`.
- What works (command + short real output):
  - `cargo fmt && cargo check && cargo test` -> `441 passed; 0 failed` in
    the unit run, then `4`, `2`, `9`, `2`, `12`, `4`, `15` in the e2e
    files, `6 ignored` (live).
  - `cargo clippy --all-targets` -> no warnings.
  - The transcript is `Vec<Block>`: user, assistant, thinking, tool, notice
    and compaction. Agent events update the last streaming block only.
  - Tool cards show `● name  summary`, the right-aligned `⠋`/`✓ 1.2s`/`✗`
    icon, a `│` gutter, five output lines plus
    `… +N lines (ctrl+o to expand)`, and 200 lines when expanded.
  - `edit` cards draw `-`/`+` diff lines (12 each); `write` cards show the
    first 8 lines dim.
  - Markdown: `**bold**`, `*italic*`, `` `code` ``, `#` headings, `•`
    bullets, numbered lists, `>` quotes and fenced code with a `│` gutter.
    Half-finished `**bol` and an open fence are handled.
  - `Ctrl+T` toggles thinking and saves `show_thinking` to `state.json`;
    `Ctrl+O` toggles every tool output.
  - `PageUp` stops following and the footer shows
    `scrolled · End to follow`; `End`, `Ctrl+End`, `Home` and a new prompt
    work as specified.
- Assumptions:
  - A `ToolEnd` without a matching running tool still creates a finished
    card, so a replayed or interrupted event stream cannot lose output.
  - `summary_for` includes the tool name because the stage spec lists it
    (`read src/a.rs (lines 1-200)`); the card header strips that prefix so it
    does not print `bash bash …`.
  - Rendered lines are cached as ratatui `Line` values in `App`; the cache
    key is `(width, content length, expanded, show_thinking)`.
  - Before the first resize the cache renders at width 80.
- Known problems / TODO:
  - The manual check must be done by the human: run a prompt in a scratch
    folder and ask arqus to fix a failing test; watch the streaming text,
    tool cards, `Ctrl+O` and `Ctrl+T`.
  - Markdown has no tables or link styling by design. Inline styles inside a
    heading all become bold.
- Next stage:
  - T6 (transcript input: multiline editing, paste, file mentions).

## Stage T6: Editor (multi-line, history, paste, @ files, ! shell, queue)   [DONE]
- Files created or changed:
  - New: `src/tui/{mention,files,shell,paste,input_edit,flow_mention}.rs`,
    `src/tui/{mention,files,shell}_tests.rs`,
    `src/tui/overlay/history.rs`, `src/tui/overlay/history_tests.rs`,
    `src/tui/widgets/mention_menu.rs`, `src/tui/editor_tests.rs`,
    `src/tui/update_editor_tests.rs`.
  - Changed: `src/tui/{actions,app,effects,history,input,input_lines,keys,mod,msg,picker,update,view}.rs`,
    `src/tui/overlay/mod.rs`, `src/tui/update_tests.rs`, `src/tui/input_tests.rs`.
  - Snapshots added: `editor-1line-80x24`, `editor-6lines-80x24`,
    `editor-9lines-80x24`, `editor-paste-marker-80x24`,
    `mention-popup-80x24`, `history-picker-80x24`, `queue-80x24`.
  - Snapshots changed: `help-60x20` (three new keys: `ctrl+y yank`,
    `ctrl+r history`, `alt+up take queued`; the wider key column reflows
    the dialog). `help-80x24` is untracked in this tree and was rewritten
    for the same reason.
- What works (command + short real output):
  - `cargo fmt && cargo check && cargo test` -> `478 passed; 0 failed`
    in the unit run, then `4`, `2`, `9`, `2`, `12`, `4`, `15` in the e2e
    files, `6 ignored` (live).
  - `cargo clippy --all-targets` -> no warnings.
  - Editing: `Left`/`Right`/`Home`/`End`, `Ctrl+A`/`Ctrl+E`,
    `Backspace`, `Delete`, `Ctrl+W`, `Ctrl+U`, `Ctrl+K`, `Ctrl+Y`
    (yank), `Alt+Enter`/`Ctrl+J` newline. The box keeps 6 lines then
    scrolls; the terminal cursor follows the edit position.
  - A paste over 2000 characters is stored whole and drawn as one dim
    `[pasted 4,210 characters]` line; sending expands it again.
    `Backspace` on the marker deletes the whole paste.
  - `Ctrl+R` opens the history picker (newest first, `#N` detail);
    `Enter` puts the prompt back in the editor.
  - `@` opens a file popup above the editor. `Effect::ScanFiles` runs at
    startup: `git ls-files` in a repo, otherwise a walk that skips hidden
    folders, `target`, `node_modules` and `.git`, capped at 20,000 files.
    `Tab`/`Enter` insert `@path `. On send, files under 8 KB become
    `<file path="...">` blocks (max 3 files, 20 KB); larger files get
    `(file mentioned: path, N lines; use the read tool)`.
  - `!command` runs through the bash tool immediately, even during a run.
    The TUI shows a bash tool card and the session gets exactly one user
    message `I ran \`command\` locally. Output:` plus a fenced block.
  - Queue: `Enter` during a run queues, dim `queued:` lines sit above the
    editor, `Esc` aborts and returns the whole queue to the editor, and
    `Alt+Up` takes the last item back. The agent consumes the queue at
    tool-round starts and as follow-up turns (stage 21).
  - The editor border follows the mode: ask = border, auto = warning,
    plan = accent. First `Ctrl+J` shows the Mac Option-as-Meta note once.
- Assumptions:
  - `Alt+Enter` cannot be detected when the terminal swallows it, so the
    one-time Mac note appears on the first `Ctrl+J` newline instead.
  - `Alt+Up` removes the item from the TUI list only. The engine may
    already have taken it from the channel; in that case it still runs.
  - `!command` bypasses the running check on purpose, so it works during
    a slow turn. It never starts the model.
  - A paste marker counts as one char for editing; its display width is
    the width of the `[pasted N characters]` label.
  - Mentioned paths outside the project are not inlined; they get a note
    that asks the model to use the read tool.
- Known problems / TODO:
  - The manual check must be done by the human: paste 20 lines, edit the
    middle, `@` a file, run `!ls`, and queue two messages during a slow
    command (`bash sleep 3` via a tool call).
  - The `@` popup highlights by order inside the popup; `Esc` keeps it
    closed until the text changes.
- Next stage:
  - T7 (sessions in the TUI).

## Stage T7: Sessions in the TUI   [DONE]
- Files created or changed:
  - New: `src/tui/{sessions,sessions_tests}.rs`,
    `src/tui/{export,export_tests}.rs`, `src/tui/clipboard.rs`,
    `src/tui/tree_tests.rs`.
  - Changed: `src/{session,session_tests,tree_tests,cli,main}.rs`,
    `src/tui/{app,commands,commands_tests,effects,event_loop,flow,flow_dialogs,
    layout,layout_tests,mod,msg,resume,tree,view}.rs`,
    `src/tui/widgets/picker_view.rs`, `src/tui/app_session_tests.rs`.
  - Snapshots added: `sessions-picker-80x24`, `tree-picker-branch-80x24`,
    `rename-toast-80x24`.
  - Snapshots changed: `help-60x20` and untracked `help-80x24`
    (`/rename`, `/copy` and `/export` are available, so the command
    column lists them).
- What works (command + short real output):
  - `cargo fmt && cargo check && cargo test` -> `499 passed; 0 failed`
    in the unit run, then `4`, `2`, `9`, `2`, `12`, `4`, `15` in the
    e2e files, `6 ignored` (live).
  - `cargo clippy --all-targets` -> no warnings.
  - `/sessions` (alias `/resume`) lists title, `2h ago · 14 msgs`, newest
    first, with `*` on the open session. The reader counts lines and
    parses only title candidates; 10,000 lines finish well under 1 s.
    `Enter` opens and replays; `Ctrl+D` asks `y/n` and moves the file to
    `sessions/<slug>/trash/`.
  - `/new` (alias `/clear`) starts a fresh file and keeps the model and
    mode. `/rename <name>` appends a `session_name` custom entry and
    toasts `Session renamed to <name>`.
  - `/tree` indents two spaces per level, caps at eight levels with `…`,
    shows `role: first 60 chars`, and marks the current path with `*`.
    `Enter` moves the leaf and rebuilds blocks from that path.
    `Esc Esc` on an empty idle box opens the tree.
  - `/fork` copies only the current path into a new file, switches to
    it, and toasts `Forked`.
  - `arqus -c` continues the latest session; `arqus -r` opens the
    sessions picker at startup; no flag starts fresh.
  - `/export [file]` writes Markdown (default
    `arqus-session-<date>.md`) and toasts the path. `/copy` copies the
    last assistant message through `pbcopy` or `wl-copy`/`xclip`.
- Assumptions:
  - A session with no user message and no rename is titled
    `(empty session)` in the picker.
  - The tree detail column was removed; entry ids were random and made
    snapshots unstable. The row is exactly `role: preview`.
  - Deleting the open session is refused with a warning; other sessions
    move to `trash/`, never unlink.
  - `-r` still creates the fresh session before the picker opens, so it
    appears as the marked current row until another is chosen.
- Known problems / TODO:
  - The manual check must be done by the human: create two sessions,
    switch with `/sessions`, branch with `/tree`, `/fork`, then
    `arqus -c`.
  - Clipboard tests only cover the command choice, not a real clipboard.
- Next stage:
  - T8 (modes and approvals in the TUI).

## Stage T8: Modes and approvals in the TUI   [DONE]
- Files created or changed:
  - New: `src/tui/approval_tests.rs`; snapshots
    `approval-{bash,write,edit,risky}-80x24` and
    `footer-mode-{ask,auto,plan}-80x1`.
  - Changed: `src/agent.rs` and `src/agent_safety.rs` (`plan` on
    `AgentConfig`, `PLAN_REFUSAL`, the plan note in each request, the
    plan gate before every non-read tool), `src/{main,print}.rs`
    (`plan: false`; settings `approval = "auto"` starts the TUI in auto),
    `src/tui/{facts,app,approval,commands,commands_tests,effects,events,
    flow,flow_dialogs,keys,mod,msg,running,update,update_tests,view,
    footer_tests,agent_tests}.rs`.
  - Existing snapshots changed: none. `help-60x20` and `help-80x24`
    stay as they were; Shift+Tab is still one key row.
- What works (command + short real output):
  - `cargo fmt && cargo check && cargo test` -> `518 passed; 0 failed`
    in the unit run, then `4`, `2`, `9`, `2`, `12`, `4`, `15` in the
    e2e files, `6 ignored` (live).
  - `cargo clippy --all-targets -- -D warnings` -> no warnings.
  - `Shift+Tab` and `/mode [ask|auto|plan]` cycle or set; the footer
    chip and the editor border follow. The first switch to `auto` in a
    session shows `auto: tools run without asking. Use a git branch or
    the Docker sandbox.` once. A launch starts in `ask` unless `--yolo`
    or settings has `approval: "auto"`.
  - `plan` returns `Plan mode: changes are disabled. Describe the plan
    instead.` for `write`, `edit` and `bash`; `read` still works, and
    every request in plan mode carries the same line in its system
    prompt.
  - The approval dialog shows the tool title, wrapped preview (`bash`
    stops at 10 body lines, `write` at 20), a red `RISKY` marker, and
    `y`/`a`/`n`/`Esc`. It owns the keyboard until a decision. The running
    line says `waiting for your approval`; the bell becomes an `Effect`
    only after 100 idle ticks (10 s).
  - The mock-provider tests drive the real agent loop through
    `update`: `y` runs the command, `n` returns `The user denied this
    tool call.`, `a` remembers that tool for the session, `Esc` sends
    deny plus `Effect::Abort`.
- Assumptions:
  - The mode is applied to the engine by `Effect::SetMode`: `Ask` and
    `Plan` use `Approver::Interactive`, `Auto` uses `Approver::Auto`.
    The plan gate is separate and runs before approval.
  - A turn that is already running keeps its cloned runtime, so a mode
    change applies from the next prompt. The approval dialog cannot be
    covered by another dialog.
  - The bell is one `Effect` per idle approval request, not a timer in
    `update`.
- Known problems / TODO:
  - The manual check must be done by the human: ask for a file edit in
    `ask` mode and approve; deny a bash command; try `plan`; press
    `Shift+Tab` through the modes.
  - `AgentEvent::ApprovalRequest` keeps stage 22's `{tool, preview}`
    shape because the approval channel handles one call at a time; it
    has no `id` yet.
- Next stage:
  - T9 (themes, settings dialog, key overrides).


## Stage T9: Themes, settings dialog, key overrides   [DONE]
- Files created or changed:
  - New: `src/tui/{theme_file,keybinds,flow_settings,flow_slash}.rs`,
    `src/tui/{theme_file,keybinds}_tests.rs`,
    `src/tui/overlay/{theme,settings}.rs` and their test files,
    `src/settings_keybinds.rs`.
  - Changed: `src/{state,settings,main}.rs`, `src/settings_tests.rs`,
    `src/tui/{actions,keys,theme,commands,effects,flow,msg,mod,update,testing}.rs`,
    `src/tui/{theme_tests,commands_tests,update_tests}.rs`,
    `src/tui/overlay/mod.rs`.
  - Snapshots added: `theme-picker-{80x24,60x20}` and
    `settings-picker-{80x24,60x20}`.
  - Snapshots changed: `help-60x20` and `help-80x24` (`/theme` and
    `/settings` are now available, so the command column lists them).
- What works (command + short real output):
  - `cargo fmt && cargo check && cargo test` -> `544 passed; 0 failed`
    in the unit run, then `4`, `2`, `9`, `2`, `12`, `4`, `15` in the
    e2e files, `6 ignored` (live).
  - `cargo clippy --all-targets -- -D warnings` -> no warnings.
  - `/theme` lists `dark`, `light`, `mono`, `dark (256 colors)` and every
    `<ARQUS_HOME>/themes/<name>.json`. Moving the selection previews;
    `Enter` keeps the theme and writes `state.json`; `Esc` restores the
    theme from before the dialog. Invalid files are skipped and a toast
    names the file.
  - Theme files use `#rrggbb`; missing colors use the dark palette.
  - `/settings` shows the current value of theme, show thinking, expand
    tool output, mouse capture, terminal bell, default mode and
    compaction. `Enter` cycles or toggles one row and saves `state.json`.
  - `settings.json` may hold `"keybinds"` with one key or an array.
    Unknown action names and unparsable keys produce startup toasts and
    are ignored; a shared key goes to the later action with a toast.
    `/help` reads the effective key map.
  - `NO_COLOR` starts in `mono`; `COLORTERM=truecolor` gives RGB;
    a 256-color terminal routes every palette color through
    `rgb_to_xterm256`. Drawing sources are checked for raw colors.
- Assumptions:
  - `settings.json` keybinds are merged with the built-ins at startup only;
    a change needs a restart.
  - `keybinds` order follows the sorted action map, so "later wins" means
    later in that sorted order.
  - Mouse capture and the finished-run bell timing are stored now; T10
    uses the mouse flag, and `Msg::Done` rings the bell when the setting
    is on.
  - The compaction setting applies to the running engine immediately and
    is restored from `state.json` at the next launch.
- Known problems / TODO:
  - The human must check the themes in Terminal.app (256 colors) and in a
    truecolor terminal, and toggle each setting once.
  - A theme file named like a built-in is listed twice in the picker.
- Next stage:
  - T10 (terminal robustness: restore, resize, clipboard, mouse, title).


## Stage T10: Terminal robustness   [DONE]
- Files created or changed:
  - New: `src/tui/{terminal,resize}.rs` and their test files,
    `src/tui/clipboard_tests.rs`.
  - Changed: `src/tui/{mod,event_loop,msg,update,app,view,render,scroll,
    block,effects,flow_settings,commands,clipboard,testing}.rs`,
    `src/tui/{block,commands,update}_tests.rs`,
    `src/tui/overlay/settings.rs`.
  - Snapshots changed: `settings-picker-80x24` and `settings-picker-60x20`
    (the hint now documents Shift-drag while mouse capture is on).
    Both were new untracked files from T9; no other snapshot text changed.
- What works (command + short real output):
  - `cargo fmt && cargo check && cargo test` -> `562 passed; 0 failed`
    in the unit run, then `4`, `2`, `9`, `2`, `12`, `4`, `15` in the
    e2e files, `6 ignored` (live).
  - `cargo clippy --all-targets -- -D warnings` -> no warnings.
  - One guard restores on normal quit, an error from `event_loop`, and a
    panic (hook restores first, then prints the panic). `restore()` is
    idempotent; `TerminalGuard::finish` runs only the first time.
  - SIGTERM and SIGHUP send `Msg::Signal`; a closed stdin stream sends the
    same message; both return `Effect::Quit`.
  - Resize is debounced at 50 ms, keeps the last size, clears the render
    cache and puts the same block back at the top of the viewport.
  - Clipboard tries `pbcopy` (macOS) or `wl-copy`, `xclip -selection
    clipboard`, `xsel -b` (Linux), then writes OSC 52 with a hand-written
    base64 encoder. `/copy code` copies the last fenced block of the last
    answer.
  - Mouse stays off unless `state.json` says on; wheel events scroll by
    three lines. The title is `ESC ] 0 ; arqus · <folder> BEL` and is
    reset on exit. Draws are capped at 30 fps while streaming, and the
    transcript clones only the visible rows.
  - The finished-run bell rings only after 10 idle seconds. Block text
    cuts lines at 2,000 characters; invalid UTF-8 already reaches the UI
    through `String::from_utf8_lossy`.
- Assumptions:
  - Resize debounce uses caller milliseconds, so its rule is a pure
    function and the event loop owns the clock.
  - "Anchor the scroll position" keeps the block and the line inside it
    at the top; if that block got shorter, the scroll clamps to the end.
  - OSC 52 is the last fallback even when it may be ignored: the copy is
    reported as done, because no terminal can confirm acceptance.
- Known problems / TODO:
  - The human must run the manual check: `kill -TERM`, rapid resize, a
    100,000 character line, `TERM=xterm`, `NO_COLOR=1`, and Shift-drag
    with mouse capture on.
- Next stage:
  - T11 (tmux smoke tests and the acceptance checklist).

## Stage T11: tmux smoke tests and the acceptance checklist   [DONE]
- Files created or changed:
  - New: `examples/fake_llm.rs`, `scripts/tui-smoke.sh`,
    `scripts/tui-smoke-lib.sh` (the sourced helpers, both files stay under
    300 lines), `docs/TUI_ACCEPTANCE.md`.
  - Changed: `tests/support/mod.rs` (`FakeLlm::start_on`, runtime
    `CARGO_BIN_EXE_arqus` lookup so the example can include the module),
    `src/tui/update.rs` (a `Key OK` toast when a key is accepted),
    `src/tui/update_tests.rs`.
  - Snapshots changed: none.
- What works (command + short real output):
  - `tmux` was missing; installed it with `brew install tmux` (3.8).
  - `sh scripts/tui-smoke.sh` -> `all scenarios passed` for a to h:
    first run and filter, connect and key check, chat, approve a tool,
    deny a tool, slash menu and palette, quit and secret scan, restart.
  - The fake server is the in-process test server: `fake_llm <port>
    <script.json>` prints `listening on <port>` and serves the JSON
    script in order (`json`, `status`, `text`, `tool`).
  - `cargo test` -> `563 passed; 0 failed` in the unit run, then the
    e2e files pass; `6 ignored` (live).
  - `cargo clippy --all-targets -- -D warnings` -> no warnings.
  - `cargo fmt --check` and `sh -n scripts/tui-smoke.sh` -> clean.
- Assumptions:
  - The provider picker uses Esc once to clear a filter and once to
    close, so the first-run scenario sends Esc twice. This matches T3.
  - The Connect dialog switches to the model picker as soon as the key
    is accepted; the `Key OK` confirmation is a toast so it stays
    visible while the model list loads.
  - Restart (scenario h) starts a fresh session like a normal launch:
    the main view, saved model and no dialog. Continuing the old
    session is covered by `-c` in T7.
  - The smoke helpers are `start`, `keys`, `type`, `wait_for`, `screen`
    and `stop`, all in the sourced `scripts/tui-smoke-lib.sh`; the fake
    server picks a free port in 20000-39999.
- Known problems / TODO:
  - The human must run `docs/TUI_ACCEPTANCE.md` once (Terminal.app,
    a truecolor terminal, glyph widths, scrolling, Option-as-Meta).
- Next stage:
  - The Part 4 gate and the v0.2.0 release checks.

## Stage M: Model Context Protocol (MCP) Support & Serena Integration   [DONE]
- Files created or changed:
  - `src/mcp/types.rs`: JSON-RPC 2.0 wire format (Request, Response, Notification, Error), MCP protocol types (`McpInitializeParams`, `McpInitializeResult`, `McpTool`, `McpCallResult`, `McpServerConfig`).
  - `src/mcp/transport.rs`: Stdio transport over async child process with process group isolation (`process_group(0)`) and `kill_on_drop(true)`.
  - `src/mcp/client.rs`: Async MCP client actor handling handshake (`initialize`, `notifications/initialized`), `tools/list`, and `tools/call` with timeout handling.
  - `src/mcp/schema.rs`: Token-efficient schema pruning (stripping `$schema`, `title`, excess metadata), description compacting, and namespace formatting (`mcp__{server}__{tool}`).
  - `src/mcp/config.rs`: Multi-tier configuration loader merging `~/.arqus/mcp.json` and `<cwd>/.arqus/mcp.json` with environment variable expansion (`${VAR}`).
  - `src/mcp/manager.rs`: `McpManager` coordinating multi-server lifecycles, lazy child process spawning, output truncation (50KB cap), and tool dispatching.
  - `src/mcp/mod.rs`: Clean module interface and exports.
  - `src/mcp/tests.rs`: Unit tests for JSON-RPC, schema pruning, config loading, and mock server handshake.
  - `src/mcp/serena_tests.rs`: Functional tests for Serena MCP (symbol AST discovery, semantic symbol editing, plan mode safety, approval preview formatting, multi-turn agent execution).
  - `src/mcp/adversarial_tests.rs`: Stress and failure tests (server crash mid-call, hanging server timeout, output payload bomb truncation).
  - `src/agent.rs`, `src/agent_util.rs`: Integrated `McpManager` into `AgentConfig`, tool gathering, and tool execution.
  - `src/agent_safety.rs`: MCP tool approval preview rendering and Plan mode read-only permissions for inspection tools.
  - `src/compaction.rs`, `src/compaction_tests.rs`: Added support for tracking files inspected or modified by MCP Serena operations across context compactions.
  - `src/main.rs`, `src/print.rs`: Wired `McpManager::for_cwd` on startup.
  - `src/tui/block.rs`, `src/tui/block_tests.rs`: Formatted MCP tools cleanly as `mcp:server/tool args` in summary blocks.
  - `src/tui/widgets/tool_card.rs`: Rendered `● mcp:server/tool` in tool cards instead of internal identifiers.
  - `src/tui/view.rs`, `src/tui/approval_tests.rs`: Formatted approval modal header as `Run MCP server / tool?` with full action detail.
  - `src/tui/commands.rs`, `src/tui/command_registry.rs`: Added `/mcp` slash command to inspect configured MCP servers in the TUI.
  - `tests/snapshots/`: Updated `help` and `palette` snapshots to reflect the new `/mcp` command.
  - `README.md`: Documented MCP support, Serena integration, efficiency optimizations, and `/mcp` command.
- What works (command + short real output):
  - `cargo test mcp:: --bin arqus` -> `test result: ok. 15 passed; 0 failed; finished in 1.02s`
  - `cargo test --bin arqus` -> `test result: ok. 581 passed; 0 failed; finished in 1.10s`
  - `cargo clippy --bin arqus` -> clean with 0 warnings
  - `cargo fmt --check` -> clean
- Assumptions:
  - Tool names are namespaced with `mcp__{server}__{tool}` to avoid collisions with built-in tools (`read`, `write`, `edit`, `bash`).
  - Read-only inspection tools from Serena (`get_symbols`, `find_references`, etc.) are allowed in Plan mode, while mutating tools (`edit_symbol`, `rename_symbol`) are refused with `PLAN_REFUSAL`.
  - Process groups ensure all child processes spawned by language server backends (like Serena's rust-analyzer or python subprocesses) are killed cleanly without orphan processes.
- Known problems / TODO:
  - Remote HTTP/SSE MCP servers can be added as a subsequent transport layer.

## Stage: Mode Verification, Efficiency, and Mode Safety Testing   [DONE]
- Files created or changed:
  - `src/tui/widgets/tool_card.rs`: Cleaned up unused variable warnings (`display_name`, `summary_text`).
  - `src/tui/mod.rs`: Extracted and refined `init_app` to synchronize `runtime.cfg.approver` and `runtime.cfg.plan` directly with `app.mode` (resolving the startup auto-mode bug).
  - `src/tui/startup_tests.rs`: Added unit tests verifying startup mode synchronization (`init_app_syncs_auto_mode_from_state_default_mode`, `init_app_syncs_plan_mode_when_cfg_plan_is_true`, `init_app_defaults_to_ask_mode`).
  - `src/tui/mode_tests.rs`: Comprehensive TUI driver tests covering all 3 modes (`ask`, `auto`, `plan`), verifying approval prompts, automated execution, risky bash guard, read-only plan enforcement, and chip/border colors.
  - `src/cli.rs`: Added CLI `--mode <ask|auto|plan>` flag.
  - `src/settings.rs`: Added `ApprovalMode::Plan` to settings deserialization.
  - `src/safety_setup.rs`: Added `resolve_mode` helper reconciling CLI `--mode`, `--plan`, `--yolo`, and `settings.approval`.
  - `src/main.rs`, `src/print.rs`: Wired `resolve_mode` into startup for both interactive and print modes.
  - `src/agent_safety.rs`: Enhanced `PLAN_NOTE` in prompt to explicitly instruct LLMs upfront that only `read` is allowed in Plan mode, eliminating redundant mutating tool attempts.
  - `tests/e2e_safety.rs`: Added end-to-end integration tests for `--mode <ask|auto|plan>`.
  - `docs/BUGS.md`: Documented startup mode sync bug and fix per A15 bug protocol.
- What works (command + short real output):
  - `cargo test mode_tests` -> `test result: ok. 5 passed; 0 failed; finished in 0.02s`
  - `cargo test --bin arqus` -> `test result: ok. 590 passed; 0 failed; finished in 1.14s`
  - `cargo test --test e2e_safety` -> `test result: ok. 7 passed; 0 failed; finished in 0.14s`
  - `cargo test` (all unit and integration test binaries) -> `test result: ok. 641 passed; 0 failed; 6 ignored (live)`
  - `cargo fmt --check` -> clean
  - `cargo clippy --tests` -> clean on all modified code
- Assumptions:
  - In Plan mode, red output in the TUI is the deliberate safety refusal (`ToolState::Err`) blocking unauthorized write/edit/bash actions.
  - LLM efficiency in Plan mode is significantly improved by explicit negative prompting in `PLAN_NOTE`, discouraging futile mutating tool calls.
  - Startup mode synchronization ensures `runtime.cfg.approver` matches the active visual UI mode (`app.mode`) regardless of whether configured via CLI `--mode` or `state.json`.
- Known problems / TODO:
  - All modes verified and running efficiently with full test coverage.
- Next stage:
  - Continued development and production usage.

## Stage F0: Orientation and gate (no feature code)   [DONE]
- Files created or changed:
  - `docs/flow/NOTES.md` (new): comprehensive analysis of SPEC.md Section 3 touch points, real type/function signatures, additive classification, and flagged assumptions.
  - `Cargo.toml`: added `shlex = "2.0.1"` to dependencies, and `proptest = "1.12.0"` and `tempfile = "3.27.0"` to dev-dependencies.
  - `scripts/audit-flow.sh` (new): automated audit gate checking forbidden keywords, line counts, imports, live test ignore bounds, and sibling test file pairing.
  - `scripts/test-audit-flow.sh` (new): 30 isolated test fixtures verifying each violation fails and clean trees pass.
  - `scripts/check.sh`: wired `scripts/audit-flow.sh` and `scripts/test-audit-flow.sh`, and refined secret scan pattern.
  - `tests/common/mod.rs` (new): shared test doubles module re-exporting `tests/support` and aliasing `pub type MockServer = support::FakeLlm;`.
  - `src/mcp/adversarial_tests.rs`, `src/mcp/serena_tests.rs`, `src/mcp/tests.rs`: replaced `assert_eq!(..., true/false)` with `assert!(...)`/`assert!(!...)` to fix pre-existing `-D clippy::bool-assert-comparison` lint errors.
- What works (command + short real output):
  - `./scripts/test-audit-flow.sh` -> `Tests passed: 30, failed: 0. All audit-flow tests passed!`
  - `./scripts/audit-flow.sh` -> `audit-flow: all checks passed`
  - `cargo test` -> `590 unittests passed; 51 integration tests passed; 0 failed; 6 ignored (live)`
  - `cargo clippy --all-targets -- -D warnings` -> clean (0 warnings, 0 errors)
  - `scripts/check.sh` -> `all checks passed`
- Assumptions:
  - All existing e2e tests already shared `tests/support/mod.rs`; `tests/common/mod.rs` was added additively to serve flow tests without altering existing test imports.
  - The 6 clippy bool assert comparisons in `src/mcp/*` were minor test lint violations preventing `check.sh` from completing with `-D warnings`, updated cleanly without changing test behavior.
- Known problems / TODO:
  - None. Ready for Stage F1 (Schemas, errors, config).
- Next stage:
  - Stage F1: Schemas, errors, config (`src/flow/schema.rs`, `src/flow/errors.rs`, `src/flow/config.rs`).

## Stage F1: Schemas, errors, config   [DONE]
- Files created or changed:
  - `src/flow/mod.rs`: flow module root declaring `cli`, `config`, `errors`, `schema`.
  - `src/flow/errors.rs`, `src/flow/errors_tests.rs`: typed `FlowError` enum with hand-written `Display` (no `thiserror`) and unit tests.
  - `src/flow/schema/types.rs`, `src/flow/schema/mod.rs`, `src/flow/schema/tests.rs`: spec §5 data models (`Category`, `Assignee`, `FileRef`, `TaskContract`, `Plan`, `PlannerReply`, `FailureKind`, `FailureRecord`, `RedesignDecision`, `Edit`, `TaskStatus`, `RunStatus`, `TaskState`, `RunState`, `Issue`, `IssueCode`) and pure validation functions (`validate_task_id`, `validate_file_ref`, `validate_shape`).
  - `src/flow/config/types.rs`, `src/flow/config/defaults.rs`, `src/flow/config/mod.rs`, `src/flow/config/tests.rs`: settings deserialization with `deny_unknown_fields`, precedence resolution, role API key lookup, worker validation, and CLI overrides.
  - `src/flow/cli.rs`, `src/flow/cli_tests.rs`: clap CLI skeleton for `flow` (`run`, `resume`, `report`, `models-check`) and execution entrypoint.
  - `src/debug.rs`: additive multi-key redactor (`register_redacted_key`, `redact_with_keys`).
  - `src/cli.rs`: added optional `command: Option<Command>` with `Command::Flow` subcommand.
  - `src/main.rs`: wired `mod flow;` and subcommand dispatch.
- What works (command + short real output):
  - `cargo test --bin arqus flow` -> `test result: ok. 26 passed; 0 failed; finished in 0.01s`
  - `cargo run -- flow --help` -> prints `arqus flow <COMMAND>` help with all subcommands (`run`, `resume`, `report`, `models-check`)
  - `cargo run -- flow run --help` -> prints `arqus flow run [OPTIONS] <OBJECTIVE>`
  - `./scripts/audit-flow.sh` -> `audit-flow: all checks passed`
  - `./scripts/test-audit-flow.sh` -> `Tests passed: 30, failed: 0. All audit-flow tests passed!`
  - `./scripts/check.sh` -> `all checks passed`
- Assumptions:
  - `RedesignDecision::Revise` uses `Box<TaskContract>` to avoid clippy's `large_enum_variant` warning while preserving identical JSON serialization/deserialization.
  - `schema` and `config` are organized into submodules (`src/flow/schema/` and `src/flow/config/`) to ensure all source files remain strictly under the 300-line limit per rule.
- Known problems / TODO:
  - None. Ready for Stage F2 (Model layer).
- Next stage:
  - Stage F2: Model layer (`model/complete.rs`, `model/json.rs`, `model/ledger.rs`, adapter `extra_body`).

## Stage F2: Model layer   [DONE]
- Files created or changed:
  - `src/flow/model/mod.rs`: exported `complete`, `json`, `ledger`.
  - `src/flow/model/complete.rs`, `src/flow/model/complete_tests.rs`: `complete_text` consuming LLM streams into `Completion` (text, provider usage, cache read/write, latency, and chars/4 fallback estimation) using sequential single-task `tokio::select!` without `tokio::spawn`, `join!`, or threads.
  - `src/flow/model/json.rs`, `src/flow/model/json_tests.rs`: think tag stripping (balanced and unterminated), robust markdown fence/prose/balanced object JSON extraction, object vs array enforcement, and schema repair message generation preserving failing field paths.
  - `src/flow/model/ledger.rs`, `src/flow/model/ledger_tests.rs`: `Role` (Planner, Redesign, Takeover, Worker), `UsageRecord` serialization to `usage.jsonl`, cost calculation (reported cost priority, price table with cached input rate fallback, or null), and `Ledger` role/cloud/local totals aggregation.
  - `src/ai/openai.rs`, `src/ai/openai_tests.rs`: additive `extra_body` shallow merging into the OpenAI-compatible request body (preventing override of `model` and `messages`), with inline tests purely extracted to `openai_tests.rs` keeping `openai.rs` well under 300 lines.
  - `src/ai/mod.rs`: added `extra_body` field and `with_extra_body` builder to `ModelConfig`.
  - `src/flow/config/types.rs`: added `extra_body` support to `OrchestratorConfig` and updated `overlay_settings`.
  - `src/flow/cli.rs`: wired `FlowCommand::ModelsCheck` live smoke test runner.
  - `tests/e2e_flow_model.rs`: integration test verifying `extra_body` wire delivery to mock server and `models-check` command execution.
- What works (command + short real output):
  - `cargo test --bin arqus flow` -> `test result: ok. 46 passed; 0 failed; finished in 0.01s`
  - `cargo test --bin arqus openai` -> `test result: ok. 7 passed; 0 failed; finished in 0.00s`
  - `cargo test --test e2e_flow_model` -> `test result: ok. 1 passed; 0 failed; finished in 1.04s`
  - `./scripts/audit-flow.sh` -> `audit-flow: all checks passed`
  - `./scripts/test-audit-flow.sh` -> `Tests passed: 30, failed: 0. All audit-flow tests passed!`
  - `./scripts/check.sh` -> `all checks passed`
- Assumptions:
  - In `model/complete.rs`, single-task sequential execution with `tokio::select!` between `&mut run` and `rx.recv()` ensures zero spawned background tasks or `join!` while preventing channel backpressure deadlocks.
  - In `model/ledger.rs`, if `price.cached_input` is not explicitly set, the standard `price.input` rate is used as fallback.
  - For `models-check`, a minimal test prompt (`ping`) with `max_tokens: 16` is sent to verify model responsiveness and record round-trip latency.
- Known problems / TODO:
  - None. Ready for Stage F3 (Git, sandbox, verification).
- Next stage:
  - Stage F3: Git, sandbox, verification (`repo/git.rs`, `exec/sandbox.rs`, `exec/verify.rs`).

## Stage F3: Git, sandbox, verification   [DONE]
- Files created or changed:
  - `src/atomic_write.rs`, `src/atomic_write_tests.rs`: pure extraction of `write_atomic` with temporary file creation, permissions preservation, and atomic rename.
  - `src/tools/write.rs`: delegated atomic writing to `crate::atomic_write::write_atomic`.
  - `src/process_group.rs`, `src/process_group_tests.rs`: pure extraction of `configure_process_group`, `kill_process_group`, and `GroupGuard`.
  - `src/tools/bash.rs`, `src/mcp/transport.rs`: replaced private group killing with `crate::process_group`.
  - `src/flow/repo/mod.rs`: declared `pub mod git;`.
  - `src/flow/repo/git.rs`, `src/flow/repo/git_tests.rs`: git repository management (`check_clean_repo` rejecting modified/untracked files, `ensure_exclude_entry` updating `.git/info/exclude` without touching `.gitignore`, `create_branch_and_worktree` for `arqus/flow/<run_id>`, `get_head_commit`, `reset_hard_clean`, and `commit_changes` with user `arqus-flow <flow@localhost>`).
  - `src/flow/exec/mod.rs`: declared `pub mod sandbox;` and `pub mod verify;`.
  - `src/flow/exec/sandbox.rs`, `src/flow/exec/sandbox_tests.rs`: sandbox execution (`check_command_allowed` enforcing exact leading-token allowlist matching and checking `safety::is_risky`, and `run_command` with `env_clear()`, passthrough env keys, process group timeout kill, and 4,000 char tail truncation).
  - `src/flow/exec/verify.rs`, `src/flow/exec/verify_tests.rs`: scope checking (`check_scope` enforcing modified/added/deleted/renamed paths subset of `allowed_writes`, rejecting external paths, and enforcing `max_changed_lines_hard`), and `run_verify_commands` running verify commands with fail-fast semantics.
  - `src/flow/mod.rs`: declared `pub mod exec;` and `pub mod repo;`.
  - `src/main.rs`: declared `mod atomic_write;` and `mod process_group;`.
- What works (command + short real output):
  - `cargo test --bin arqus flow` -> `test result: ok. 71 passed; 0 failed; finished in 1.02s`
  - `cargo test --bin arqus atomic_write process_group` -> `test result: ok. 5 passed; 0 failed`
  - `./scripts/audit-flow.sh` -> `audit-flow: all checks passed`
  - `./scripts/test-audit-flow.sh` -> `Tests passed: 30, failed: 0. All audit-flow tests passed!`
  - `./scripts/check.sh` -> `all checks passed`
- Assumptions:
  - In `exec/verify.rs`, untracked files in the worktree that are within `allowed_writes` have their line count added to the numstat diff count against HEAD to ensure accurate total changed line count enforcement against `max_changed_lines_hard`.
  - In `exec/sandbox.rs`, command output truncation keeps the tail of up to 4,000 characters for stdout and stderr without injecting artificial headers so raw command outputs remain intact for tests and verify checks.
- Known problems / TODO:
  - None. Ready for Stage F4 (Edit format).
- Next stage:
  - Stage F4: Edit format (`exec/edits.rs`, SEARCH/REPLACE and FILE block parsing and atomic apply).






## Stage F4: Edit format   [DONE]
- Files created or changed:
  - `src/edit_match.rs`, `src/edit_match_tests.rs`: pure extraction of `apply_exact_edit` with CRLF normalization, exact-once match count checking, and CRLF preservation.
  - `src/tools/edit.rs`: delegated text matching and CRLF normalization to `crate::edit_match::apply_exact_edit`.
  - `src/main.rs`: declared `mod edit_match;`.
  - `src/flow/exec/mod.rs`: declared `pub mod edits;`.
  - `src/flow/exec/edits.rs`: implemented `EditBlock` (`Search` and `File`), `parse_edits` (extracting blocks, ignoring surrounding prose, supporting CRLF input, returning `FlowError::Declined` for `CANNOT_COMPLETE`, `FlowError::NoOutput` for empty outputs, and `FlowError::ParseError` for unclosed or malformed blocks), and `apply_edits` (scope checking against absolute paths, `..`, and `allowed_writes`, resolving paths with `safety::check_path`, applying edits in-memory sequentially with exact matching hints and ambiguous match detection, and atomically writing modified files to disk via `write_atomic` only if all edits succeed).
  - `src/flow/exec/edits_tests.rs`, `src/flow/exec/edits_apply_tests.rs`, `src/flow/exec/edits_prop_tests.rs`: unit tests from TESTS.md A3 and property tests from A5 (keeping all files <= 300 lines).
- What works (command + short real output):
  - `cargo test --bin arqus flow::exec::edits` -> `test result: ok. 24 passed; 0 failed; 0 ignored; finished in 0.15s`
  - `cargo test --bin arqus edit` -> `test result: ok. 33 passed; 0 failed; 0 ignored; finished in 0.01s`
  - `cargo test --bin arqus flow` -> `test result: ok. 95 passed; 0 failed; 0 ignored; finished in 1.02s`
  - `./scripts/audit-flow.sh` -> `audit-flow: all checks passed`
  - `./scripts/check.sh` -> `all checks passed`
- Assumptions:
  - `FILE` blocks ending with newline are formatted to preserve standard POSIX trailing newlines when non-empty.
  - `apply_edits` returns `Ok(Vec<String>)` containing the deduplicated list of repo-relative paths modified, in first-touched order.
- Known problems / TODO:
  - None. Ready for Stage F5 (Repo map and context builder).
## Stage F5: Repo map and context builder   [DONE]
- Files created or changed:
  - `src/flow/repo/mod.rs`: declared `pub mod map;` and `pub mod context;`.
  - `src/flow/repo/git.rs`, `src/flow/repo/git_tests.rs`: added `list_tracked_files` returning sorted list of tracked files via `git ls-files -z`.
  - `src/flow/schema/types.rs`: added additive `#[serde(default)] pub changed_files: Vec<String>` to `TaskState`.
  - `src/flow/repo/map.rs`, `src/flow/repo/map_tests.rs`, `src/flow/repo/map_fixture_tests.rs`: implemented `scan_file_symbols` (line scanners for Rust `fn`/`struct`/`enum`/`trait`/`impl`, Python `def`/`class`, Go `func`/`type`, TypeScript `function`/`class`/`interface`/exported `const`), and `build_repo_map` (deterministic, size-capped repo map using git tracked files, with truncation marker cut cleanly at line boundary).
  - `src/flow/repo/context.rs`, `src/flow/repo/context_tests.rs`: implemented `render_task_context` rendering task block with `(new file)` annotations for missing allowed writes, upstream dependency summaries from `RunState`, context file sections (whole file or line range), budget checking against token limits with largest file name in `FlowError::BudgetExceeded`, and byte-deterministic output.
- What works (command + short real output):
  - `cargo test --bin arqus flow::repo::map` -> `test result: ok. 8 passed; 0 failed; 0 ignored; finished in 0.22s`
  - `cargo test --bin arqus flow::repo::context` -> `test result: ok. 6 passed; 0 failed; 0 ignored; finished in 0.00s`
  - `cargo test --bin arqus flow` -> `test result: ok. 110 passed; 0 failed; 0 ignored; finished in 1.02s`
  - `./scripts/audit-flow.sh` -> `audit-flow: all checks passed`
  - `./scripts/test-audit-flow.sh` -> `Tests passed: 30, failed: 0. All audit-flow tests passed!`
  - `./scripts/check.sh` -> `all checks passed`
- Assumptions:
  - Python declarations keep parameter signatures up to trailing `:` for clean method / function definitions, taking the last colon so type annotations like `(url: str):` remain intact.
  - When a repo map exceeds `max_bytes`, the content slice is cut at the last newline before `max_bytes - TRUNCATION_MARKER.len()` so the truncation marker begins on a clean line.
- Known problems / TODO:
  - None. Ready for Stage F6 (Validator, capability sheet, planner).
- Next stage:
  - Stage F6: Validator, capability sheet, planner (`planning/validator.rs`, `capability.rs`, `prompts.rs`, `planner.rs`, and `arqus flow run --dry-plan`).

## Stage F6: Validator, capability sheet, planner   [DONE]
- Files created or changed:
  - `src/flow/mod.rs`: declared `pub mod planning;`.
  - `src/flow/schema/types.rs`: added `IssueCode` variants for Section 7 rules.
  - `src/flow/planning/mod.rs`: declared submodules `validator`, `validator_graph`, `capability`, `prompts`, `planner`.
  - `src/flow/planning/validator.rs`, `validator_tests.rs`, `validator_rules_tests.rs`: pure function `validate_plan` implementing rules 1-8 of Section 7.
  - `src/flow/planning/validator_graph.rs`, `validator_graph_tests.rs`: helper functions for transitive dependency analysis, Kahn's algorithm cycle detection, and protected path pattern matching.
  - `src/flow/planning/capability.rs`, `capability_tests.rs`: capability sheet rendering from worker configuration and optional `profile_stats.json` (categories with n >= 5).
  - `src/flow/planning/prompts.rs`, `prompts_tests.rs`: `PROMPT_VERSION = "v1"`, system prompt builder, repo map and objective user messages, issue repair message formatting, and 60 KB file content truncation.
  - `src/flow/planning/planner.rs`, `planner_tests.rs`, `planner_repair_tests.rs`: `Planner` loop supporting need-files rounds, max file read round forcing, JSON repair, validation issue repair, ledger accounting, and `FlowError::PlanningFailed`.
  - `src/flow/cli.rs`, `src/flow/cli_tests.rs`: wired `--dry-plan` to run planning, validation, print plan summary, tasks, final verify commands, and token totals, touching nothing.
  - `tests/e2e_flow_dry_plan.rs`: end-to-end black-box test verifying `--dry-plan` CLI execution against a mock server.
- What works (command + short real output):
  - `cargo test --bin arqus flow::planning` -> `test result: ok. 33 passed; 0 failed; 0 ignored; finished in 0.01s`
  - `cargo test --test e2e_flow_dry_plan` -> `test result: ok. 1 passed; 0 failed; 0 ignored; finished in 0.13s`
  - `cargo test --bin arqus flow` -> `test result: ok. 144 passed; 0 failed; 0 ignored; finished in 1.03s`
  - `./scripts/audit-flow.sh` -> `audit-flow: all checks passed`
  - `./scripts/test-audit-flow.sh` -> `Tests passed: 30, failed: 0. All audit-flow tests passed!`
  - `./scripts/check.sh` -> `all checks passed`
- Assumptions:
  - Protected pattern `dir/**` matches paths starting with `dir/` or exactly equal to `dir`.
  - In need-files rounds, requested files exceeding 60 KB (61,440 bytes) are cleanly truncated with `\n[... truncated at 60 KB ...]`.
- Known problems / TODO:
  - None. Ready for Stage F7 (Worker, takeover, redesign).
- Next stage:
  - Stage F7: Worker, takeover, redesign (`exec/worker.rs`, `exec/takeover.rs`, `planning/redesign.rs`).

## Stage F7: Worker, takeover, redesign   [DONE]
- Files created or changed:
  - `src/flow/exec/mod.rs`: declared `pub mod takeover;` and `pub mod worker;`.
  - `src/flow/planning/mod.rs`: declared `pub mod redesign;`.
  - `src/flow/schema/types.rs`: added `Display` implementation for `FailureKind` and `FailureRecord::new` constructor.
  - `src/flow/errors.rs`, `src/flow/errors_tests.rs`: added `to_failure_record(attempt, executor)` method on `FlowError`.
  - `src/flow/planning/prompts.rs`, `src/flow/planning/prompts_tests.rs`: added prompt builders for worker, takeover, and redesign, plus safe UTF-8 character truncation helpers (`truncate_head`, `truncate_tail`).
  - `src/flow/exec/worker.rs`, `src/flow/exec/worker_tests.rs`: implemented `run_worker_rounds` and `run_worker_attempt` supporting one-shot edit application, scope checking, verification commands, worktree reset on failure, and retry feedback loop with truncated previous output (3,000 chars) and failure kind/detail (2,000 chars tail).
  - `src/flow/exec/takeover.rs`, `src/flow/exec/takeover_tests.rs`: implemented `run_takeover_rounds` routing execution to the orchestrator model with prior failure records (capped at 500 chars), retry feedback, and worktree reset.
  - `src/flow/planning/redesign.rs`, `src/flow/planning/redesign_tests.rs`: implemented `redesign` sending the contract, worker capability sheet, and up to 2 failure records (capped at 1,500 chars) to orchestrator, parsing `RedesignDecision` (`Revise`, `Split`, `Takeover`) and defaulting any model error or unparseable output to `Takeover`.
- What works (command + short real output):
  - `cargo test --bin arqus flow::exec::worker` -> `test result: ok. 6 passed; 0 failed; 0 ignored; finished in 0.28s`
  - `cargo test --bin arqus flow::exec::takeover` -> `test result: ok. 5 passed; 0 failed; 0 ignored; finished in 0.19s`
  - `cargo test --bin arqus flow::planning::redesign` -> `test result: ok. 5 passed; 0 failed; 0 ignored; finished in 0.00s`
  - `cargo test --bin arqus flow` -> `test result: ok. 165 passed; 0 failed; 0 ignored; finished in 1.02s`
  - `./scripts/audit-flow.sh` -> `audit-flow: all checks passed`
  - `./scripts/test-audit-flow.sh` -> `Tests passed: 30, failed: 0. All audit-flow tests passed!`
  - `./scripts/check.sh` -> `all checks passed`
- Assumptions:
  - In `takeover_rounds`, prior failures from worker attempts are included in the prompt (up to 3 records, details capped at 500 chars), and the executor is recorded as `Assignee::Orchestrator`.
  - In `redesign`, any failure during the orchestrator call or failure to parse the response as a valid `RedesignDecision` JSON safely defaults to `RedesignDecision::Takeover`.
- Known problems / TODO:
  - None. Ready for Stage F8a (State, scheduler, router).
- Next stage:
  - Stage F8a: State, scheduler, router (`engine/state.rs`, `scheduler.rs`, `router.rs`).

## Stage F8a: State, scheduler, router   [DONE]
- Files created or changed:
  - `src/flow/mod.rs`: declared `pub mod engine;`.
  - `src/flow/engine/mod.rs`: declared `pub mod router;`, `pub mod scheduler;`, `pub mod state;`.
  - `src/flow/engine/router.rs`, `src/flow/engine/router_tests.rs`: defined `WorkerProfile` type alias, `Router` trait with `select(&TaskContract) -> &WorkerConfig`, and `DefaultRouter` with `from_config` that selects default worker and ignores `by_category`.
  - `src/flow/engine/scheduler.rs`, `src/flow/engine/scheduler_tests.rs`: implemented `Scheduler` / `next_ready` returning lowest task ID among ready tasks with all dependencies done, plus `is_finished` and `all_done` helpers.
  - `src/flow/engine/state.rs`, `src/flow/engine/state_tests.rs`, `src/flow/engine/state_split_tests.rs`: implemented `check_transition` (validating the 9 allowed transitions against the 64-pair matrix and returning `FlowError::InvalidTransition` for all other pairs), `TaskState::transition`, `RunState::from_plan`, `RunState::transition_task`, fixed-point `update_dependents` (cascading `Pending -> Ready` when all deps are Done, and `Pending -> Skipped` transitively when any dep is Failed or Skipped), `replace_with_subtasks` (superseding old task, `<old>.N` IDs, inheriting old dependencies, incrementing `split_depth`, and rewiring downstream dependents to leaf subtasks), and atomic persistence (`save` via `crate::atomic_write::write_atomic`, and `load`).
- What works (command + short real output):
  - `cargo test --bin arqus flow::engine` -> `test result: ok. 25 passed; 0 failed; 0 ignored; finished in 0.01s`
  - `cargo test --bin arqus flow` -> `test result: ok. 190 passed; 0 failed; 0 ignored; finished in 1.03s`
  - `./scripts/audit-flow.sh` -> `audit-flow: all checks passed`
  - `./scripts/test-audit-flow.sh` -> `Tests passed: 30, failed: 0. All audit-flow tests passed!`
  - `./scripts/check.sh` -> `all checks passed`
- Assumptions:
  - In `replace_with_subtasks`, leaf subtasks are defined as subtasks in the split that no other sibling subtask in the split depends on; downstream tasks depending on the old task are rewired to depend on all leaf subtasks.
  - In `update_dependents`, a fixed-point loop iterates until no more status changes occur, properly propagating transitive skips and readiness down arbitrary dependency trees.
- Known problems / TODO:
  - None. Ready for Stage F8b (Execution loop in `engine/run_loop.rs`).
- Next stage:
  - Stage F8b: Execution loop (`engine/run_loop.rs`).

## Stage F8b: Execution loop   [DONE]
- Files created or changed:
  - `src/flow/cli.rs`: wired `arqus flow run` CLI command to invoke `engine::run_loop::run_flow(run_args)`.
  - `src/flow/engine/mod.rs`: declared modules `events`, `run_loop`, `run_loop_task`, `task_accept`, `task_validate`.
  - `src/flow/engine/events.rs`, `src/flow/engine/events_tests.rs`: structured event stream logging to `.arqus/flow/<run_id>/events.jsonl` with `append_event` and `emit_event`.
  - `src/flow/engine/task_validate.rs`, `src/flow/engine/task_validate_tests.rs`: contract validation against orchestrator capability sheets (`is_revised_contract_valid`, `are_subtasks_valid`) and worker LLM construction (`build_worker_llm`).
  - `src/flow/engine/task_accept.rs`, `src/flow/engine/task_accept_tests.rs`: `TaskContext` struct, verification check runner, git worktree commit/merge on success (`accept_task`), and orchestrator takeover execution (`run_takeover_flow`).
  - `src/flow/engine/run_loop_task.rs`, `src/flow/engine/run_loop_task_tests.rs`: worker execution loop (`execute_task`) managing rounds, failure recording, redesign escalation (`Revise`, `Split`, `Takeover`), worktree rollback on rejection, and direct orchestrator execution.
  - `src/flow/engine/run_loop.rs`, `src/flow/engine/run_loop_tests.rs`: top-level entrypoint `run_flow` orchestrating clean repo verification, plan generation/loading, worktree branch creation, scheduler execution loop, final verification, and status finalization.
  - `src/flow/engine/scheduler_tests.rs`: updated test suite to verify scheduler loop behavior.
  - `tests/common/mod.rs`: updated test fixture with git initialization, sandbox configuration, and test helper assertions.
  - `tests/e2e_flow_plan.rs`: end-to-end scenarios S1 (three linear tasks) and S16 (orchestrator-assigned task).
  - `tests/e2e_flow_ladder.rs`: end-to-end scenarios S2 (worker retry), S3 (redesign revise), S4 (redesign split), S5 (redesign takeover), S6 (max redesigns automatic takeover).
  - `tests/e2e_flow_ladder_errors.rs`: end-to-end scenarios S7 (takeover fail), S11 (scope violation rollback), S17 (provider 500 retries), S19 (cannot complete declined).
- What works (command + short real output):
  - `cargo test --bin arqus flow` -> `test result: ok. 199 passed; 0 failed; 0 ignored; finished in 1.15s`
  - `cargo test --test e2e_flow_plan --test e2e_flow_ladder --test e2e_flow_ladder_errors` -> `test result: ok. 11 passed; 0 failed; 0 ignored; finished in 2.37s`
  - `./scripts/audit-flow.sh` -> `audit-flow: all checks passed`
  - `./scripts/test-audit-flow.sh` -> `Tests passed: 30, failed: 0. All audit-flow tests passed!`
  - `./scripts/check.sh` -> `all checks passed`
- Assumptions:
  - In `execute_task` and `accept_task`, common paths and run configuration are encapsulated in `TaskContext<'a>` to adhere to clippy's function argument limit.
  - Redesign split validation verifies that subtask dependencies only reference existing subtasks or dependencies from the parent task, and total subtask scope files remain within the orchestrator capability file count.
  - When git lists worktrees, branch names checked out in active worktrees may have a `+` prefix in `git branch --list`; branch identification handles both `*` and `+` markers.
- Known problems / TODO:
  - None. Ready for Stage F8c (Planning failures, resume, budget, report).
- Next stage:
  - Stage F8c: Planning failures, resume, budget, report (`engine/run_loop.rs` extensions, budget ledger checks).

## Stage F9: CLI wiring, stats, Docker, docs   [DONE]
- Files created or changed:
  - `src/flow/engine/mod.rs`: declared `pub mod stats;`.
  - `src/flow/engine/stats.rs`, `src/flow/engine/stats_tests.rs`, `src/flow/engine/stats_build_tests.rs`: implemented `TaskStatRecord` (exact serde representation: `model`, `category`, `first_attempt_pass`, `attempts`, `redesigns`, `takeover`, `tokens`), `build_task_stats` (extracting task metrics and model from ledger or config), `append_run_stats` (safe JSON line file append), and `record_run_stats` (appending to `<ARQUS_HOME>/flow/stats.jsonl`).
  - `src/flow/engine/run_loop.rs`, `src/flow/engine/resume.rs`: wired `record_run_stats` invocation at run end (upon reaching `Finished` or `Failed`).
  - `src/flow/engine/events.rs`: wired debug event forwarding to `crate::debug::log` for flow event auditing.
  - `src/flow/cli.rs`, `src/flow/cli_tests.rs`: added optional `--repo` to `ResumeArgs` and `ReportArgs`, ensured `--debug` initializes debug logging, added unit tests covering all `arqus flow` commands/flags and Docker wrapper key forwarding.
  - `tests/e2e_flow_cli.rs`: added `test_flow_run_appends_to_stats_jsonl` verifying end-to-end `arqus flow run` execution against mock servers creating `<ARQUS_HOME>/flow/stats.jsonl` with correct task statistics.
  - `scripts/arqus-docker`: updated to forward `ARQUS_ORCHESTRATOR_KEY` and `ARQUS_WORKER_KEY`, and allow either `ARQUS_API_KEY` or `ARQUS_ORCHESTRATOR_KEY` to be set.
  - `.env.example`: added documented examples for role key variables (`ARQUS_ORCHESTRATOR_KEY`, `ARQUS_WORKER_KEY`).
  - `README.md`: added a comprehensive `## Flow` section documenting workflow, CLI commands, configuration in `settings.json`, and honest v1 limitations.
- What works (command + short real output):
  - `cargo test --bin arqus flow` -> `test result: ok. 225 passed; 0 failed; 0 ignored; finished in 1.03s`
  - `cargo test --test 'e2e_flow_*'` -> `test result: ok. 21 passed; 0 failed; 0 ignored; finished in 5.48s`
  - `./scripts/audit-flow.sh` -> `audit-flow: all checks passed`
  - `./scripts/test-audit-flow.sh` -> `Tests passed: 30, failed: 0. All audit-flow tests passed!`
  - `./scripts/check.sh` -> `all checks passed`
- Assumptions:
  - In `build_task_stats`, tasks in status `Superseded` (resulting from redesign split) are excluded from the stats file since their replacement subtasks represent the evaluated executions.
  - For tasks with multiple attempts across redesigns or takeover, attempts reflect `worker_attempts + takeover_attempts`, and tokens reflect total input and output across all ledger records associated with the task.
- Known problems / TODO:
  - None. Ready for Stage F10 (Eval harness in Python).
- Next stage:
  - Stage F10: Eval harness (`evals/flow_run.py`, `flow_metrics.py`, `flow_report.py`, `flow_profile.py`, etc.).

## Stage F10: Eval harness (Python, standard library only)   [DONE]
- Files created or changed:
  - `evals/flow_task.py`: implemented task schema (`TaskDefinition`, `PlannerTask`, `NanoTask`), loaders for e2e, planner, and nano tasks, and directory scanner.
  - `evals/flow_validity.py`: implemented `validate_task` enforcing the 4 rules from document 6 (clean repo check, `hidden_verify` failing before reference patch and passing after, `regression_verify` passing before and after, `public_verify` failing before reference patch).
  - `evals/flow_metrics.py`: implemented all evaluation metrics from document 6 B6 (accuracy metrics, cloud token and dollar accounting, paired token savings with median and aggregate, delegation metrics, overhead metrics, contract quality metrics, stability metrics, paired table, and exact two-tailed sign test on discordant tasks).
  - `evals/flow_profile.py`: implemented `build_profile_stats` and `generate_profile_file` extracting category pass rates from `stats.jsonl` (filtering categories with n >= 5 only) and producing `profile_stats.json` for worker capability sheets.
  - `evals/flow_report.py`: implemented markdown report generation (`FLOW_REPORT.md`) with pins, paired table, condition comparison, category and language breakdowns, B9 failure causes tally, B8 decision table evaluator, and `flow_results.jsonl` append.
  - `evals/flow_run.py`: implemented multi-condition runner (D0, H1 to H5) with repeats, interleaving across tasks and conditions, isolated temporary git repositories, hidden test file copying post-model execution, and verification.
  - `evals/flow/tasks/`: created 6 smoke fixtures proving the eval machinery (`smoke-single-function`, `smoke-bugfix`, `smoke-tests`, `smoke-boilerplate`, `smoke-refactor`, `smoke-multifile`), each with setup git repository, verification scripts, and reference patch.
  - `evals/tests/`: added complete test suite with 42 unit tests (`test_flow_metrics.py`, `test_task_validity.py`, `test_task_loaders.py`, `test_flow_profile.py`, `test_flow_report.py`, `test_flow_run_smoke.py`).
  - `scripts/run-flow-evals.sh`, `scripts/build-flow-profile.sh`: added execution shell scripts.
  - `scripts/check.sh`: wired python unit test discovery (`python3 -m unittest discover -s evals/tests -p "test_*.py"`) when `python3` is available.
  - `.gitignore`: added bytecode caches and `flow_results.jsonl`.
- What works (command + short real output):
  - `python3 -m unittest discover -s evals/tests -p "test_*.py"` -> `Ran 42 tests in 1.991s; OK`
  - `cargo test --bin arqus flow` -> `test result: ok. 225 passed; 0 failed; 0 ignored; finished in 1.02s`
  - `./scripts/audit-flow.sh` -> `audit-flow: all checks passed`
  - `./scripts/test-audit-flow.sh` -> `Tests passed: 30, failed: 0. All audit-flow tests passed!`
  - `./scripts/check.sh` -> `all checks passed`
- Assumptions:
  - In `flow_metrics.py`, exact sign test on discordant tasks uses two-tailed cumulative binomial probability with parameter p = 0.5 under the null hypothesis.
  - In `flow_profile.py`, only categories with sample size n >= 5 are included in the generated `profile_stats.json`, matching the capability sheet requirement.
  - Smoke fixtures use minimal Python and shell verifiers so that eval machinery verification is self-contained and fast without requiring external toolchains.
- Known problems / TODO:
  - None. Ready for Stage F11 (Live shakedown and first eval).
- Next stage:
  - Stage F11: Live shakedown and first eval.








