# pirs, stages 12 to 26: from "it works" to "stable and usable" (plus the MacBook guide)

This continues `pirs-agent-prompt.md`. Same working style: one stage per fresh opencode session, the PREFIX line, the IRON RULES, `PROGRESS.md` after every stage, commit after every stage.

**What changes now.** Until stage 11 everything was tested with a mock. From here on the goal is to find and kill the bugs that only appear with a real provider, a real terminal and long sessions, and to do it cheaply and repeatably:

- Stages 12 to 16 make `pirs` testable and robust **offline** (a fake LLM server, retries, cost limits). No API key, no cost.
- Stage 17 is the first **live** testing against OpenRouter, with tiny, capped requests.
- Stage 18 builds an **eval suite** (small coding tasks with automatic pass/fail) so every prompt or code change is measured instead of guessed.
- Stages 19 to 23 fix the terminal UI, add the steering queue, add safety (approvals, path guards) and harden the tools.
- Stages 24 to 26 are packaging, the Docker sandbox, and a stability gate.
- **Part 4** is the guide for building and using it on your MacBook, with the Docker option.

Files delivered with this one: `fake_llm.rs` (a tested fake server for stage 13).

---

# PART 0: BEFORE YOU START (for you, the human)

**1. OpenRouter key with a spending cap.**
- Create the key at openrouter.ai (Keys page) and set a **credit limit** on it, for example 2 dollars. The limit belongs to the key, so a leaked or runaway key cannot spend more.
- Add only a few dollars of credit. Check that auto top-up is off.
- Never paste the key into a chat, a file in the repo, or a prompt. Put it in your shell for the current session: `export PIRS_API_KEY=...`. Part 4 shows the Keychain way.

**2. Which model.**
- The free variants (model id ends with `:free`) are limited to 20 requests per minute, and 50 requests per day unless you have bought at least 10 credits (then 1000 per day). One `pirs` prompt makes one request per tool round, so a few prompts can use up 50. Use free variants only for toy checks.
- For real testing use the paid id of the same model. For DeepSeek V4 Flash that is most likely `deepseek/deepseek-v4-flash` (the free one is the same id plus `:free`). Confirm the exact id and the price on the model's page at openrouter.ai/models. I did not find a reliable price, so stage 16 makes cost visible before you spend much.
- Model ids can contain a colon, so `openrouter:deepseek/deepseek-v4-flash:free` must split on the FIRST colon only. Stage 15 tests this.

**3. Tag where you are.** `git add -A && git commit -m "v0.1.0 stages 0-11" && git tag v0.1.0`.

**4. Live calls happen only in stages 17, 18 and 26.** Every other stage must pass offline.

---

# PART 1: AGENTS.md ADDENDUM (append to AGENTS.md, keep Part A as it is)

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

# PART 2: STAGE PROMPTS

Copy each stage from its "PASTE" line down to the next `---`.

---

## STAGE 12: Test foundations (PIRS_HOME and the debug log)

PASTE:
Goal: make the app relocatable and debuggable. Two small features.
1. `PIRS_HOME`: add `fn pirs_home() -> PathBuf` (env `PIRS_HOME` if set and not empty, else `~/.pirs`). Replace EVERY hard-coded use of `~/.pirs` or `dirs::home_dir().join(".pirs")` (sessions, settings, global AGENTS.md, SYSTEM.md, skills, prompts). Use `grep -rn "\.pirs" src` and `grep -rn "home_dir" src` to find them all and list them in PROGRESS.md. Project-level `<cwd>/.pirs/` stays as it is. Rust note: environment variables are process-wide, so unit tests that set them can disturb each other; in unit tests pass paths into functions instead, and use the env var only in e2e tests (separate processes).
2. Debug log: with `--debug` or `PIRS_DEBUG=1`, append one JSON object per line to `<PIRS_HOME>/debug.log`: `{"ts":<ms>,"kind":"request","url":"...","body":<the request JSON>}`, `{"kind":"status","code":200}`, `{"kind":"sse","line":"<raw line>"}` for every raw stream line, `{"kind":"tool","name":"...","args":{...}}`, `{"kind":"error","text":"..."}`. NEVER log headers. Make `fn redact(s: &str) -> String` that replaces the configured API key value (when it is at least 8 characters) and anything that looks like `sk-` followed by 10 or more letters, digits, `-` or `_` with `[REDACTED]`; apply it to every line before writing. At startup, if `debug.log` is larger than 5 MB, rename it to `debug.log.1` (replacing the old one). Use a small global: `static DEBUG: OnceLock<Option<Mutex<File>>>` in `src/debug.rs` and `pub fn log(kind: &str, value: serde_json::Value)`. Rust note: `OnceLock` is a global that is set once.
Tests: `redact` (exact key, `sk-or-v1-abcdef123456`, text without secrets unchanged); `pirs_home` with and without the env var (in one test, restore the variable afterwards); debug off means no file is created; debug on writes parseable JSON lines and the key never appears in the file.
Done when: tests pass, and `PIRS_HOME=/tmp/x PIRS_DEBUG=1 cargo run -- --no-session -p "hi"` creates `/tmp/x/debug.log` (it may fail to call the model; that is fine here).

---

## STAGE 13: Fake LLM server and black-box tests

PASTE:
Goal: test the real `pirs` binary end to end without a network. The human supplies a finished helper file named `fake_llm.rs`.
Do:
1. Copy `fake_llm.rs` to `tests/support/mod.rs` (create the folders). It must be `tests/support/mod.rs`, not `tests/support.rs`, otherwise Cargo treats it as a test of its own. In each test file start with `mod support; use support::*;`. Rust note: integration tests in `tests/` run the compiled binary as a separate program, so they cannot import our modules; they talk to it through its command line, environment and files.
2. Read the file once and explain in 6 lines what `FakeLlm`, `Reply`, `sse_text`, `sse_tool_call_json`, `run_pirs` and `TempDir` do. Do not rewrite it. If it does not compile against our crate, fix only the failing line.
3. Write `tests/e2e_basic.rs` with these tests (each uses its own `TempDir` for the project and for `PIRS_HOME`):
   a. Plain reply: script one `Reply::sse(sse_text(&["Hel","lo"]))`. Run `pirs --no-session -p "hi"`. Expect exit code 0 and stdout containing `Hello`. Expect exactly 1 recorded request, path ends with `/chat/completions`, header `authorization` is `Bearer test-key-123`, body has `stream: true`, `messages[0].role == "system"`, and `tools` has 4 entries.
   b. Tool loop: script a `bash` tool call with `{"command":"echo hi > out.txt"}`, then a text reply "done". Expect `out.txt` in the project folder with `hi`, exit code 0, exactly 2 requests, and the second request body contains a message with `role: "tool"`, the same `tool_call_id`, and content containing `exit code: 0`.
   c. Unknown tool: script a call to `nosuchtool`, then a text reply. Expect exit 0 and the second request contains a tool message that is an error.
   d. Session file: run WITHOUT `--no-session` and with `PIRS_HOME` set. Expect one `.jsonl` file under `<home>/sessions/`, a first line with `"type":"session"`, and at least a user and an assistant entry. Run again with `-c` and a new scripted reply; the second request must contain the first prompt's text in its messages.
4. In the temporary test setup, make unwanted extra requests visible: assert the exact number of recorded requests in every test.
Done when: `cargo test --test e2e_basic` passes with no network and no key set. Show the output.
Don't: add async to the tests; don't use real `~/.pirs`; don't start the real network.

---

## STAGE 14: Stream robustness

PASTE:
Goal: make the stream parser survive everything real servers do. Write each scenario as an e2e test in `tests/e2e_stream.rs` FIRST, watch it fail if it fails, then fix the parser (A15). Use the helpers `split_chunk`, `sse_comment`, `to_crlf`, `usage_chunk`. Facts to follow are in A16.
Scenarios (each with the final assertion on stdout and exit code 0 unless stated):
1. An event cut in the middle of the word `data:` across two writes (`split_chunk` at byte 3, delay 30 ms).
2. A multi-byte character cut in the middle across two writes (for example `é` or an emoji inside the text). Expected: the character appears intact, no U+FFFD. Rule for the fix: buffer BYTES, split on `\n`, and decode only complete lines. Never decode a network chunk on its own.
3. Comment lines `: OPENROUTER PROCESSING` before the first event and between events.
4. CRLF line endings (`to_crlf`).
5. The OpenRouter usage chunk (empty delta repeating `finish_reason`) and also the OpenAI variant with `"choices": []` and `usage`. After the run, the session file's assistant entry must have `usage.input` equal to `prompt_tokens` minus `cached_tokens`, `usage.cache_read` equal to `cached_tokens`, and `usage.output` equal to `completion_tokens`. This stops cached tokens being counted twice in `Usage::total()`. Fix the adapter if it does something else. Run these scenarios WITHOUT `--no-session`.
6. The stream ends after `finish_reason` but without `[DONE]`: success.
7. The stream is cut with no `finish_reason` and no `[DONE]` after some text (build the chunks yourself with `sse_event(&content_chunk("par"))` and send no finish chunk): expected: the partial text is saved in an assistant entry with `stop: error`, stderr mentions that the stream ended early. (Exit code 1 is added in stage 15; for now only check the session and stderr.)
8. Mid-stream error: a chunk `{"error":{"message":"Provider disconnected","code":502},"choices":[{"index":0,"delta":{"content":""},"finish_reason":"error"}]}` after some text, and also as the very first event. Expected: stderr shows `Provider disconnected`, the assistant entry has `stop: error`.
9. Tool call arguments that are an empty string: treated as `{}`. Arguments that are not valid JSON (`{"command": "ec`): the tool result is an error saying the arguments were not valid JSON; the loop continues to the next scripted reply and finishes.
10. Two tool calls in one response (indexes 0 and 1) with fragments interleaved: both run, in index order. Check that both output files exist.
11. A chunk with no `choices` key at all (like `{"id":"x"}`) is ignored.
12. `delta.reasoning_content` and `delta.reasoning` fields are accepted and stored as thinking blocks, and are not printed to stdout.
Done when: all scenarios pass and PROGRESS.md lists which ones needed a fix.
Don't: change the neutral types; add a crate for SSE parsing.

---

## STAGE 15: Errors, retries, timeouts, exit codes

PASTE:
Goal: behave well when the network or the provider misbehaves. Tests first, in `tests/e2e_errors.rs`, using the fake server.
Do:
1. In `ai/mod.rs` add `enum LlmError { Auth(String), PaymentRequired(String), RateLimited { retry_after_secs: Option<u64>, msg: String }, Server(u16, String), ContextOverflow(String), BadRequest(String), Network(String), Stream(String), Other(String) }` and a PURE `classify_http(status: u16, retry_after: Option<&str>, body: &str) -> LlmError`. Take the message from `error.message` in the JSON body, or the first 200 characters of the body. 401 and 403 are Auth, 402 PaymentRequired, 429 RateLimited, 500 to 599 Server, other 4xx BadRequest. A 400 or 413 whose message contains `context length`, `maximum context`, `too many tokens` or `context_length_exceeded` is ContextOverflow (this feeds the existing compaction retry).
2. Pure `retry_delay(err: &LlmError, attempt: u32, base_ms: u64) -> Option<Duration>`: retry only RateLimited, Server (500, 502, 503, 504, 529), Network and an early-ended Stream. Maximum 4 retries (attempt 0 to 3). Delay = `Retry-After` seconds when present (cap 60), else `base * 2^attempt` (base 1000 ms) plus up to 250 ms of jitter taken from the sub-second part of the clock (no `rand` crate). NEVER retry Auth, PaymentRequired, BadRequest.
3. Retry only when NO content (text or tool call) has been delivered yet for this request. After content was delivered, a failure ends the request with the error and the partial assistant message is saved with `stop: error`. Reason: a retry could repeat actions. Rust note: why a retry after partial output is dangerous for an agent.
4. Timeouts: 15 s to connect; 60 s without any byte on a streaming response (idle timeout, wrap each read of the stream in `tokio::time::timeout`) which becomes `Network("no data for 60s")`. No total cap, long answers are fine. Env knobs for tests: `PIRS_RETRY_BASE_MS` (default 1000), `PIRS_MAX_RETRIES` (default 4), `PIRS_IDLE_TIMEOUT_SECS` (default 60).
5. Show progress: add `AgentEvent::Notice(String)`; on a retry send `Notice("rate limited, retrying in 4s (attempt 2/4)")`. Print mode writes notices to stderr.
6. Friendly messages (one line each): Auth: `Authentication failed (401). Check PIRS_API_KEY.` PaymentRequired: `No credits left, or this API key's spending limit is used up (402).` RateLimited: `Rate limited (429).` plus the provider message. Never print request bodies or keys.
7. Exit codes in print mode: 0 success; 1 the run ended in an error (provider error, stream error, abort, cost limit later); 2 bad usage or configuration. Startup checks that exit with 2 and a clear message: empty model; empty API key unless the base URL host is `localhost` or `127.0.0.1`.
8. Model spec with colons: `split_model_spec("openrouter:deepseek/deepseek-v4-flash:free")` gives provider `openrouter` and model `deepseek/deepseek-v4-flash:free`; the same through `settings.json`. Write unit tests and fix if needed.
E2E tests (the fake server is told `PIRS_RETRY_BASE_MS=10`, `Retry-After: 0` where needed): 429 then success (exit 0, 2 requests, a notice on stderr); 503, 503, then success (3 requests); 401 (exit 1, exactly 1 request, stderr mentions authentication); 402; 400 (no retry); connection refused (point `PIRS_BASE_URL` at `http://127.0.0.1:1/v1`, `PIRS_MAX_RETRIES=1`: exit 1); `Reply::Hang` with `PIRS_IDLE_TIMEOUT_SECS=1` then success (exit 0); error after partial text (exit 1, no second request, partial text in session); missing key exits 2.
Optional if the existing mock-based test already covers it: an e2e test where the fake answers 400 with a context-overflow message once, then a summary stream, then a final answer; expect exit 0.
Done when: all tests pass offline and `docs/BUGS.md` is started.

---

## STAGE 16: Cost tracking, limits, runaway protection

PASTE:
Goal: you can always see what a run cost, and a run can never spend more than you allow. Facts in A16.
Do:
1. `Usage` gets `cost: Option<f64>` with `#[serde(default, skip_serializing_if = "Option::is_none")]`, so old session files still load. The OpenAI and OpenRouter adapters fill it from `usage.cost` when present. `EntryKind::Compaction` gets `summary_usage: Option<Usage>` with `#[serde(default)]`, filled from the summary call, so summaries are counted too.
2. Pure `session_totals(entries: &[Entry]) -> Totals { requests, input, output, cache_read, cost: Option<f64> }` over ALL assistant entries in the file (every branch) plus compaction summary usage. Cost is `None` only if no request reported a cost.
3. Limits, in `settings.json` under `limits` and as CLI flags: `max_cost_per_run` (dollars, default 0.50, flag `--max-cost`), `max_cost_per_session` (default 5.00), `max_rounds` (default 50), `max_output_tokens` (default 8192, sent as `max_tokens`, env `PIRS_MAX_OUTPUT_TOKENS`), `max_tokens_per_run` (default 500000, used when the provider reports no cost). "Run" means one user prompt including all its tool rounds. Before EACH LLM request, check the limits; when one is reached stop with `cost limit reached: $0.52 of $0.50 (limits.max_cost_per_run). Raise it to continue.` and exit code 1.
4. Runaway protection: stop the turn when the model sends the SAME tool name with identical arguments 3 times in a row; or when two consecutive assistant messages have no text and no tool calls. Message: `stopped: repeated the same tool call 3 times`.
5. Show cost: print mode ends with one stderr line: `[pirs] requests=3 in=1234 out=210 cost=$0.0042 time=12.3s` (omit `cost=` when unknown). The TUI status bar shows the session cost. A `/cost` command (both modes) prints the session totals. For OpenRouter add `/budget`: `GET {base_url}/key` with the key and print `limit_remaining`, `limit`, `usage_daily` from `data` (a null `limit_remaining` means no limit). Handle a failed request with a one-line message.
Tests: `session_totals` (branches counted, compaction usage counted, old entries without cost still parse); settings parse for `limits`; e2e: three replies each reporting `cost: 0.30` with `--max-cost 0.5` gives exit 1 and exactly 2 requests; repeated identical tool call stops after 3; `max_tokens` appears in the request body and equals the setting; the summary line appears on stderr.
Done when: tests pass.

---

## STAGE 17: Live tests against OpenRouter (the first real money)

PASTE:
Goal: find the bugs that only a real provider shows, with tiny capped requests. The human has created a spending-limited key and sets `PIRS_API_KEY`, `PIRS_PROVIDER=openrouter`, `PIRS_LIVE=1`, `PIRS_LIVE_MODEL=<paid model id>` in their own terminal. You never see or print the key.
Do:
1. Create `tests/live_openrouter.rs`. EVERY test is `#[ignore]` and returns immediately (prints "skipped") when `PIRS_LIVE` is not `1`. Use the `support` helpers `TempDir` only. Each test runs the real binary with `PIRS_HOME` set to a temp folder, `--yolo` not required yet, `PIRS_MAX_OUTPUT_TOKENS=300`, `--debug` and `--yolo` (a hidden no-op flag until stage 22; add it now if it does not exist yet), and uses the `[pirs] ... cost=` line to assert the run cost is at most 0.05 dollars.
   - L1 reply: prompt `Reply with exactly: PIRS_OK`. Expect exit 0, stdout contains `PIRS_OK`, a cost was reported.
   - L2 tool: `Use the bash tool to run: echo live-tool-test > live.txt , then say done.` Expect `live.txt` exists and contains `live-tool-test`.
   - L3 edit: the project has `notes.txt` with the line `colour=red`. Prompt: `Use the edit tool to change colour=red to colour=blue in notes.txt, then say done.` Expect the file changed.
   - L4 resume: run 1: `Remember the secret word PELICAN. Reply: stored.` Run 2 with `-c`: `What was the secret word? One word only.` Expect `PELICAN` in stdout.
   - L5 budget: run the `/budget` command in print mode (`-p "/budget"`) and expect a line with `limit_remaining`.
   - L6 compaction: a `settings.json` in the project with `context_window` 3000 and compaction `reserve_tokens` 500, `keep_recent_tokens` 800; run 4 short prompts in one session (using `-c`), each asking for a 5 line answer. Expect at least one `compaction` entry in the session file and the last run to exit 0. Cap: 6 requests.
2. `scripts/live-smoke.sh`: checks `PIRS_API_KEY` and `PIRS_LIVE_MODEL` are set (never prints the key), then runs `cargo test --test live_openrouter -- --ignored --test-threads=1 --nocapture` and prints a final line with the sum of the costs it saw.
3. When a live test fails: read `<home>/debug.log` (it is redacted), reproduce the failure with the fake server first (A15), fix, rerun. Specifically check and record in `docs/LIVE_FINDINGS.md`: (a) the name of the reasoning field in this model's stream (`reasoning` or `reasoning_content`); (b) whether a 400 error appears when an assistant turn with tool calls is sent back without its reasoning (if yes: keep the thinking text in the thinking block and send it back as `reasoning_content` in the OpenAI adapter ONLY, then add an e2e test for the request shape); (c) the real `finish_reason` values; (d) whether `usage` arrives; (e) whether `tool_calls` fragments ever lack an `index`.
Done when: the human runs `scripts/live-smoke.sh` and all six tests pass at least once; `LIVE_FINDINGS.md` is filled in; default `cargo test` still passes with no key set.
Don't: put the key anywhere; don't raise `max_tokens` above 300 in live tests; don't run live tests in a loop.

---

## STAGE 18: Eval suite

PASTE:
Goal: measure quality with real small tasks, so prompt and tool changes are decided by numbers.
Layout: `evals/run.py` (Python 3 standard library only), `evals/tasks/<name>/{prompt.txt, setup/, check.sh}`, results appended to `evals/results.jsonl` (add it to `.gitignore`), `evals/README.md`, `evals/CHANGELOG.md`.
Runner behavior: for each selected task: copy `setup/` into a fresh temp folder, `git init` it and commit (so the check can diff), run `pirs --no-session --yolo -p "<prompt>"` there with a wall-clock timeout of 300 s (Python `subprocess` timeout, kill the process group), save stdout and stderr, then run `check.sh` in that folder (exit 0 means pass; the environment variable `PIRS_OUT` points to the saved stdout). Parse the `[pirs] requests=... cost=...` stderr line. Append `{task, passed, seconds, requests, cost, model, git_commit, ts}` to the results file. Flags: `--tasks a,b`, `--repeat N`, `--bin path` (default `target/release/pirs`). Hard stop of the whole run when the summed cost exceeds `EVAL_MAX_COST` (default 0.50). Print a table at the end: task, pass rate, avg seconds, avg cost, and a total line.
Note: once stage 22 exists, print mode only runs bash, write and edit with `--yolo`. The runner already passes it (the flag is a no-op until then).
Tasks (small and deterministic; use only `python3` and `sh`; each `check.sh` must fail on the untouched `setup/`):
1. `hello-file`: create `hello.txt` containing exactly `hello`.
2. `fix-typo`: README.md has "recieve"; fix every occurrence; other text unchanged.
3. `fix-failing-test`: a Python module with an off-by-one bug and a `unittest` file; the agent fixes the module; check runs the tests AND verifies the test file's hash is unchanged.
4. `add-function`: implement `slugify(text)` in `util.py` as specified in the prompt; existing tests cover it.
5. `rename-symbol`: rename `calc_total` to `compute_total` across 3 files; check no old name remains and tests pass.
6. `read-and-answer`: `data.txt` has 137 lines; ask how many lines; check that stdout contains `137`.
7. `multi-file-feature`: add an `--upper` flag to a tiny CLI script and a test already describes it.
8. `run-and-fix`: `app.py` crashes with a traceback; the prompt says run it and fix it; check that it prints the expected output.
9. `big-file-edit`: a 3000-line file with one unique marker line; change that line only (check via diff that exactly one line changed). This exercises read truncation and offsets.
10. `no-op-restraint`: the prompt says "Do not modify any files. Explain what main.py does." Check that every file hash is unchanged and stdout is not empty.
11. `skill-use`: the project has `.pirs/skills/release-notes/SKILL.md` that defines a fixed output format; the prompt asks for release notes; check the output file matches the format.
12. `survives-compaction`: a `.pirs/settings.json` with a tiny context window and low reserve; a multi-step task (create 5 files one by one, each referring to the previous one); check all exist.
Baseline: run all tasks once with the paid model and write `evals/BASELINE.md` (pass count, total cost, failures with one-line reasons).
Tuning loop rules (put in `evals/README.md`): change ONE thing at a time (a sentence in the base prompt, one tool description, one error message). Run all tasks twice. Keep the change only if the pass count does not drop and cost does not grow by more than 20 percent. Log every change and result in `evals/CHANGELOG.md`. Look first at failures where the tool result text could have been clearer; those are the cheapest wins.
Optional (ask the human, costs a few cents): run the baseline once with a stronger model to see whether failures come from the harness or from the cheap model. If the stronger model also fails a task, the bug is probably in `pirs`.
Done when: `python3 evals/run.py --tasks hello-file,fix-typo` works against the real binary, all 12 task folders exist, `check.sh` fails on every untouched `setup/`, and the baseline file exists.
Don't: use any package outside the Python standard library; don't let tasks touch the network.

---

## STAGE 19: TUI display

PASTE:
Goal: make the TUI readable for long answers. First run `cargo add unicode-width`. Pure logic goes in testable functions.
Do:
1. `wrap_text(text: &str, width: usize) -> Vec<String>`: break at spaces when possible, hard-break tokens longer than the width, use `UnicodeWidthChar` for display width (CJK counts 2, tabs become 4 spaces), keep leading indentation on continuation lines for lines that start with spaces. The transcript is wrapped at draw time from the logical lines, and re-wrapped on terminal resize events.
2. Follow mode: while streaming, stay glued to the bottom. If the user scrolls up (PageUp), stop following and show `[scrolled - End to follow]` in the status bar. `End` or sending a prompt jumps to the bottom and follows again. PageUp and PageDown move by a page; `Home` goes to the top.
3. Status bar: while running show a spinner (4 frames, changes every 100 ms) and elapsed seconds; show `AgentEvent::Notice` text (retries) in the status bar; show session cost and context percentage.
4. Pickers (`/tree`, `/resume`): `visible_window(selected, len, height) -> (start, end)` keeps the selected row visible; show `n/total`. Also show a timestamp (from the file's modified time) next to each resume entry.
5. Code fences: lines between triple backticks are drawn dim. No other markdown handling.
Tests: `wrap_text` (plain, long word, CJK, emoji, tab, empty line, width 1); `visible_window` (top, middle, bottom, len smaller than height); app scroll and follow state transitions.
Done when: unit tests pass and the manual checklist passes: a 60-line answer is fully readable and wraps when the terminal is resized; scrolling up while streaming stops the follow; a tree with 100 entries scrolls.
Don't: render markdown, don't write a custom renderer.

---

## STAGE 20: TUI input

PASTE:
Goal: a prompt box that feels like a normal text field. Put the logic in a pure `InputState` (file `src/tui/input.rs`) that stores `Vec<char>` and a cursor index (never byte indexes; Rust note: a `String` index is a byte position and breaks on non-ASCII text).
Do:
1. Editing keys: Left, Right, Home, End (also Ctrl+A, Ctrl+E), Backspace, Delete, Ctrl+W (delete the previous word), Ctrl+U (delete to line start), Ctrl+K (delete to line end). Draw the real cursor in the input box (check the installed ratatui docs for the cursor call; it changed between versions).
2. Multi-line: Enter submits. `Ctrl+J` and `Alt+Enter` insert a newline. The box grows up to 6 lines, then scrolls.
3. Bracketed paste: enable it with crossterm (`EnableBracketedPaste`, handle `Event::Paste(String)`), insert the pasted text exactly, newlines included, and never submit on a paste. Restore terminal modes on exit and on panic.
4. History: Up and Down recall previous prompts when no picker is open and the cursor is on the first or last line. Store up to 500 entries in `<PIRS_HOME>/history.jsonl` (one JSON string per line), skip consecutive duplicates, load at startup.
5. Ctrl+C (this resolves a mismatch between README and PROGRESS): while a run is active it aborts; idle with text in the box it clears the box; idle with an empty box it shows `press Ctrl+C again to quit` for 2 seconds and quits on the second press. Ctrl+D on an empty box quits; on a non-empty box it deletes the character under the cursor. Update the key table in README.
6. Slash hints: when the text starts with `/` show the matching command names in one line under the box; Tab completes when exactly one matches. Add `/help` listing keys and commands.
Tests for `InputState`: insert and delete in the middle, emoji and CJK, Ctrl+W on punctuation and spaces, Home and End, multi-line cursor moves, history navigation (up, up, down, down restores the draft), paste with newlines, `Tab` completion.
Done when: tests pass and the manual checklist passes (paste a 20 line text; edit the middle of a line; recall a previous prompt; Ctrl+C in each state).
Mac note to put in README: in Terminal.app and iTerm2 the Option key must be set to "Esc+" (or "Use Option as Meta key") for Alt+Enter and Alt+arrows to work. `Ctrl+J` always works.

---

## STAGE 21: Steering queue

PASTE:
Goal: typing while the agent works. Messages typed during a run are queued and delivered at safe points.
Do:
1. While a run is active, Enter queues the text instead of saying "busy". Show queued items (dim, prefixed `queued:`) above the input box. Esc aborts the run AND clears the queue, with a notice.
2. `run_prompt` gets a `steer_rx: tokio::sync::mpsc::UnboundedReceiver<String>`. At the START of every loop round (after tool results were appended, before the next LLM request) it drains the receiver with `try_recv()` and appends each message as its own user entry, in order. After the turn would end (no tool calls) and the queue is not empty, start the next turn from the queued messages automatically. Print mode passes an empty receiver. Rust note: channels in 4 lines.
3. Anthropic adapter: consecutive user-role messages (tool results followed by a user text) must be merged into ONE user message with the `tool_result` blocks FIRST, then the text. Add a test. The OpenAI adapter needs no change (a `tool` message followed by a `user` message is legal).
4. Abort consistency: after aborting during a tool run and reopening the session file, `build_context` must still produce a legal history (the existing sanitize step handles it). Add an e2e or agent-level test: abort mid-tool, reload, build context, assert every tool call has a result.
Tests (mock provider): two messages queued during a tool round appear in the NEXT request in order; a leftover queue after the final answer starts a new turn; Esc clears the queue; the Anthropic merge test.
Done when: tests pass and the manual check passes (queue two messages during a slow bash command; both are answered).

---

## STAGE 22: Safety (approvals, path guards, git warnings)

PASTE:
Goal: the agent cannot silently do damage. Be honest in comments and README: these are guardrails, not a sandbox. The real boundary is the Docker container (stage 25).
Do:
1. Approval mode. Setting `approval`: `"ask"` (default) or `"auto"`; flag `--yolo` means auto. `read` never asks. For `bash`, `write` and `edit` in the TUI show a modal with the details: bash shows the full command; write shows the path and the first 20 lines; edit shows the path and the old and new text. Keys: `y` allow once, `a` allow this tool for the rest of the session, `n` deny, Esc deny and abort. A denied call returns the tool result `The user denied this tool call.` so the model can adapt. In print mode there is nobody to ask: with approval `ask`, `bash`, `write` and `edit` return `Approval required. Run pirs with --yolo to allow this.`; `read` still works.
2. Mechanism: `enum Approver { Auto, DenyAll, Interactive }` passed into `run_prompt`. For `Interactive` the agent sends `AgentEvent::ApprovalRequest { tool, preview }` and awaits a reply on an `approval_rx: tokio::sync::mpsc::Receiver<Decision>` with `enum Decision { Allow, AllowAlways, Deny }`. Update the helper `run_pirs` in `tests/support/mod.rs` and the eval runner to pass `--yolo` so existing tests keep working.
3. Path guard (pure function `check_path(cwd, path, op) -> Result<PathBuf, String>`): resolve with `canonicalize` (for a path that does not exist yet, canonicalize its parent). `write` and `edit` outside `cwd` are refused unless the path is under a folder listed in settings `allow_paths` (supports `~`). A symlink inside the project that points outside counts as outside. `read` outside `cwd` is allowed, except under `~/.ssh`, `~/.aws`, `~/.gnupg`, `~/.config/gcloud`, `~/.kube` and the pirs home itself (settings and history), which are refused.
4. Risky bash patterns (pure matcher): `rm -rf /`, `rm -rf ~`, `rm -rf *` at the top level of the home folder, `sudo`, `mkfs`, `dd if=`, `> /dev/sd`, `chmod -R 777 /`, `curl ... | sh`, `wget ... | sh`, `git push --force`. In `ask` mode the modal shows a red `RISKY` marker. In `auto` mode these are refused unless `--allow-risky` is given. State in a comment that a determined model or script can bypass pattern matching; it only stops accidents.
5. Startup notices (both modes): if the working folder is not inside a git repository, `not a git repository: changes cannot be undone with git`; if it is a repository with uncommitted changes, `uncommitted changes present`. Use `git status --porcelain` through `std::process::Command`; if git is missing, say nothing.
Tests: `check_path` with temp folders, a symlink escape, `..` segments, a non-existent target; the risky matcher (positive and negative cases such as `rm -rf build` must NOT match); approver behavior with the mock provider (deny, allow, allow-always remembered for the tool only); print mode without `--yolo` returns the approval text and nothing is written; e2e: write outside the project is refused even with `--yolo`.
Done when: tests pass and the manual check passes in the TUI (a bash command shows the modal, `n` returns control to the model).
Add to the eval suite a 13th task `refuse-outside`: prompt asks to write `../outside.txt`; check that the file does not exist outside.

---

## STAGE 23: Tool hardening

PASTE:
Goal: the four tools survive real files and real commands. Write each case as a test first (A15).
`read`:
- Truncate on CHARACTER boundaries. Never slice a `String` at a raw byte index (that panics on non-ASCII text). Write a helper `truncate_bytes(s, max)` that backs up to a char boundary. Test with a string full of `é` and emoji.
- A NUL byte in the first 8000 bytes means binary: return `error: binary file`. Lines longer than 2000 characters are cut with `...[line truncated]`. A directory path returns a sorted listing (a trailing `/` on folders). `offset` past the end returns a clear message with the file's line count. Handle `\r\n`.
`write`:
- Write to a temp file in the same folder, then rename (atomic). Keep the permissions of an existing file. Refuse when the path is a directory.
`edit`:
- Empty `old_text` is an error. `old_text` equal to `new_text` is an error. If the file uses `\r\n` and `old_text` uses `\n`, match after normalizing and write back with the file's original line endings. When not found, the error says how many lines the file has and reminds the model to check indentation and tabs versus spaces. Keep the permissions.
`bash`:
- Standard input is `/dev/null`, so a command that waits for input fails instead of hanging. Environment: `GIT_TERMINAL_PROMPT=0`, `PAGER=cat`, `GIT_PAGER=cat`, `TERM=dumb`, `NO_COLOR=1`, `CI=1`. Strip ANSI escape sequences from the output with a small state machine (no regex crate).
- Spawn in its own process group (`process_group(0)` on the tokio command, Unix only). On timeout or abort, kill the whole group with `kill -KILL -<pid>` through a second `Command`, then reap the child. Rust note: why killing only the shell leaves child processes alive.
- A background process must not hang the call: `sleep 30 & echo started` returns `started` in under 3 seconds. After the main process exits, drain output for at most 200 ms, then stop reading and kill the group.
- Keep the last 50,000 bytes AND at most the last 2000 lines. Allow `timeout_secs` up to 600.
Tests: all of the above, with temp folders; the background-process test must finish under 3 s; a timeout test must leave no `sleep` process behind (check with `pgrep -f` on a unique sleep duration such as `sleep 4242`).
Done when: tests pass and the human runs the eval suite again with no new failures compared with the baseline.

---

## STAGE 24: Packaging and release hygiene

PASTE:
Goal: `pirs` installs and updates cleanly and the repo is tidy.
Do:
1. `pirs --version` prints `pirs <version from Cargo.toml>`; set the version in `Cargo.toml` to `0.2.0-dev` (stage 26 makes it `0.2.0`). Make `--help` accurate: every flag, the exit codes (0, 1, 2), environment variables, settings keys, slash commands. Compare with the README and fix whichever is wrong.
2. Release profile in `Cargo.toml`: `[profile.release] strip = true`, `lto = "thin"`.
3. `scripts/check.sh` (POSIX sh, `set -eu`): `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo build --release`, and a secret scan: `git grep -nE "sk-(or-)?[A-Za-z0-9_-]{16,}"` must find nothing. Fix everything it reports.
4. `.github/workflows/ci.yml`, only if the repo is on GitHub: Ubuntu and macOS, stable Rust, runs `scripts/check.sh`.
5. `LICENSE` (MIT, with the name from `git config user.name` and the current year) and `CHANGELOG.md` (one entry per stage since 0.1.0).
6. Rewrite `README.md` so it matches reality: build, install, configuration, CLI flags, exit codes, keys, commands, limits, approvals, sessions, the test layers (L1 to L4), and a Docker section placeholder that stage 25 fills.
7. `docs/BUGS.md` and `docs/LIVE_FINDINGS.md` are kept. Make sure `evals/results.jsonl` and any `debug.log` are ignored by git.
Done when: `scripts/check.sh` exits 0 on a clean checkout; `cargo install --path . --force` then `pirs --version` works; README commands were each executed once.

---

## STAGE 25: Docker sandbox

PASTE:
Goal: run `pirs` inside a Linux container that can only see the project folder, so `--yolo` is safe to use. You cannot run Docker commands in every environment; if Docker is missing, write the files, validate what you can (`sh -n` on the script) and say clearly what the human must test.
Create these files exactly (adjust only what fails to build):

`Dockerfile`:
```
# syntax=docker/dockerfile:1
FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
ARG EXTRA_PACKAGES=""
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates libssl3 git curl build-essential pkg-config python3 ripgrep jq less procps ${EXTRA_PACKAGES} \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/pirs /usr/local/bin/pirs
RUN useradd -m -u 1000 -s /bin/bash agent
USER agent
ENV HOME=/home/agent PIRS_HOME=/home/agent/.pirs TERM=xterm-256color
WORKDIR /work
ENTRYPOINT ["pirs"]
```

`.dockerignore`: `target`, `.git`, `evals/results.jsonl`, `*.log`.

`scripts/pirs-docker` (make it executable):
```
#!/bin/sh
# Run pirs inside a container that sees only the current folder.
# Usage: pirs-docker [pirs arguments]    (cd into the project first)
set -eu
IMAGE="${PIRS_IMAGE:-pirs:dev}"
: "${PIRS_API_KEY:?set PIRS_API_KEY first}"
STATE="${PIRS_DOCKER_STATE:-$HOME/.pirs-docker}"
mkdir -p "$STATE"
if [ -t 0 ] && [ -t 1 ]; then TTY="-it"; else TTY="-i"; fi
exec docker run --rm $TTY \
  --cap-drop=ALL --security-opt no-new-privileges \
  --memory="${PIRS_DOCKER_MEMORY:-4g}" --cpus="${PIRS_DOCKER_CPUS:-4}" --pids-limit=512 \
  -e PIRS_API_KEY -e PIRS_MODEL -e PIRS_PROVIDER -e PIRS_BASE_URL -e PIRS_CONTEXT_WINDOW \
  -v "$PWD":/work \
  -v "$STATE":/home/agent/.pirs \
  -w /work \
  "$IMAGE" "$@"
```

`scripts/docker-smoke.sh`: builds the image (`docker build -t pirs:dev .`), then checks and prints PASS or FAIL for each: `docker run --rm --entrypoint id pirs:dev -u` is not `0`; `pirs --version` works in the container; a temp project folder mounted at `/work` is writable from the container and the created file appears on the host; `/home/agent/.ssh` does not exist in the container; the host home folder is not visible (`ls /Users` and `ls /home` show nothing of the host). Exit non-zero if any check fails.
README: fill the Docker section with the commands, a table of "what the container protects and what it does not" (protects: the rest of the machine's files, SSH keys, other repositories; does NOT protect: the mounted project folder, network access, the API key that is in the container's environment), and the tip that `--yolo` belongs inside the container while native runs should keep approvals on.
Optional variants (document only): a Node image with `--build-arg EXTRA_PACKAGES="nodejs npm"`; for Rust projects use the `rust:1-bookworm` image as the final base and set `CARGO_HOME=/home/agent/.cargo`. Mark them as untested.
Done when: `sh -n scripts/pirs-docker` passes and the human runs `scripts/docker-smoke.sh` on the Mac with all checks PASS (see Part 4).

---

## STAGE 26: Stability gate and v0.2.0

PASTE:
Goal: prove stability, then release. Each item is a test or a measured check. Report every result with real output.
1. Flaky test hunt: run `cargo test` 5 times in a row. Any test that fails once is flaky: fix it (usually a timing assumption or shared state) or mark with a comment why it cannot be.
2. Crash safety e2e: start pirs against a fake server that answers with a tool call to `sleep 30`, kill the pirs process with SIGKILL while the tool runs, then run `pirs -c` against a new scripted reply: the session must open, and the request must contain a synthetic error tool result for the interrupted call.
3. Corrupt files: a session file with a corrupted MIDDLE line still opens (the bad line is skipped with a warning on stderr) and `build_context` works; a malformed `settings.json`, an empty skills folder, a `SKILL.md` without frontmatter, and a prompt template with a bad name must not crash anything.
4. Performance: `path_to_root` currently searches the entries list once per step. Build an id-to-index `HashMap` once instead. Add a test with a session of 20,000 entries (a straight chain) that opens and builds context in under 2 seconds in a debug build.
5. Panic scan: run the whole eval suite twice and `grep -c panicked` over all saved stderr files: must be 0.
6. Live regression: run `scripts/live-smoke.sh` and the eval suite 3 times with the paid model. Write the results in `evals/BASELINE.md` (pass rate per task over the 3 runs, average cost per run).
7. Docs: README, CHANGELOG, `docs/BUGS.md` are current. Run `scripts/check.sh`.
8. Release: bump to `0.2.0`, commit, `git tag v0.2.0`, run `cargo install --path . --force` from a fresh clone of the repo in a temp folder to prove it installs.
Done when: the gate in Part 3 is all green.

---

# PART 3: THE STABILITY GATE (you decide it is "stable and usable" when all are true)

- [ ] `scripts/check.sh` passes on a fresh clone (fmt, clippy with warnings as errors, tests, release build, secret scan).
- [ ] `cargo test` passed 5 times in a row with no key and no network.
- [ ] `scripts/live-smoke.sh` passes against OpenRouter and the total cost is a few cents.
- [ ] The eval suite passes at least the pass rate you wrote in `evals/BASELINE.md` on 3 runs in a row, with no panics.
- [ ] A killed run (SIGKILL) can be resumed with `-c`.
- [ ] A run cannot exceed `max_cost_per_run`; you tested it with a tiny limit.
- [ ] In the TUI: long answers wrap, scrolling works, paste works, Ctrl+C behaves, queued messages are delivered, the terminal is normal after quitting or after a crash.
- [ ] Writes outside the project are refused; risky commands are flagged; approvals work.
- [ ] `scripts/docker-smoke.sh` passes on your Mac.
- [ ] You used `pirs` for one real task of your own (a bug fix or a small feature) without falling back to another tool, and every failure became an entry in `docs/BUGS.md`.

If a model-quality problem remains (the cheap model fails tasks the harness handles correctly), try a stronger model through the same OpenRouter key. Only `PIRS_MODEL` changes.

---

# PART 4: BUILD AND USE IT ON YOUR MACBOOK

I could not run macOS or Docker in my environment, so the Mac and Docker commands below are the standard ones but untested by me. Run `scripts/docker-smoke.sh` and the checks in Part 3 to find out.

## 4.1 One-time setup

1. **Command line tools:** `xcode-select --install` (needed for the linker).
2. **Rust:** `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`, accept the defaults, open a new terminal, check `cargo --version`. (Homebrew's `rustup` also works. Use `brew` only if you already prefer it.)
3. **A Docker runtime** (only if you want the sandbox): Docker Desktop, or OrbStack, or Colima. On Apple Silicon the Linux containers run as arm64 natively, so no emulation. Check `docker version`.

## 4.2 Build and install pirs natively

```
cd path/to/pirs
cargo build --release
cargo install --path . --force      # installs to ~/.cargo/bin/pirs
pirs --version
```
If `pirs` is not found, add `export PATH="$HOME/.cargo/bin:$PATH"` to `~/.zshrc` and open a new terminal. To update later: `git pull && cargo install --path . --force`.

## 4.3 Keys and limits

1. In OpenRouter: create a key named `pirs` with a **credit limit** (2 to 5 dollars to start). Keep auto top-up off.
2. Store the key in the macOS Keychain so it never sits in a file or in your shell history:
   ```
   security add-generic-password -a "$USER" -s pirs-openrouter -w
   ```
   (It asks for the key; paste it.) Then add to `~/.zshrc`:
   ```
   export PIRS_API_KEY="$(security find-generic-password -a "$USER" -s pirs-openrouter -w 2>/dev/null)"
   ```
   The first time, macOS may ask permission to read the item; choose Allow.
3. `~/.pirs/settings.json`:
   ```
   {
     "model": "openrouter:deepseek/deepseek-v4-flash",
     "limits": { "max_cost_per_run": 0.30, "max_cost_per_session": 3.00, "max_output_tokens": 8192 }
   }
   ```
   (Confirm the model id on openrouter.ai first.)
4. Check: `pirs --yolo -p "Reply with exactly: OK"` prints `OK` and a `[pirs] ... cost=$...` line. Then `pirs --no-session -p "/budget"` shows what is left on the key.

## 4.4 Two ways to run it

| | Native | Docker sandbox |
|---|---|---|
| Command | `pirs` | `pirs-docker` |
| Approvals | keep ON (default): you confirm bash, write and edit | use `--yolo`: the container is the safety net |
| Can see | your whole Mac | only the project folder you started it in |
| Use for | your own repos, small tasks, with a clean git tree | unfamiliar code, projects that run install scripts or tests you did not write, long unattended runs |
| Toolchains | your Mac's | only what the image contains (see 4.5) |

Daily flow, both modes:
```
cd ~/code/my-project
git switch -c pirs/fix-login      # a branch per task
pirs                              # or: pirs-docker --yolo
# ...work...
git diff                          # review everything
git checkout . && git clean -fd   # throw it all away, or commit what you like
```

## 4.5 The Docker sandbox

```
cd path/to/pirs
docker build -t pirs:dev .                  # builds a Linux pirs inside the image
./scripts/docker-smoke.sh                   # all checks must say PASS
mkdir -p ~/.local/bin && cp scripts/pirs-docker ~/.local/bin/pirs-docker
```
Make sure `~/.local/bin` is on your PATH. Then in any project:
```
cd ~/code/some-repo
pirs-docker --yolo
```
- The wrapper mounts the current folder at `/work`, keeps pirs sessions in `~/.pirs-docker`, and passes only the `PIRS_*` variables. It never mounts your home folder, `~/.ssh`, or your Git credentials, so the agent cannot push or read your keys.
- **What the container does not protect:** the mounted project folder (the agent can delete it; Git is your protection), network access (the agent can reach the internet), and the API key (it is in the container's environment, so a malicious repo that tricks the model could read and send it out). That is why the key has a small spending limit. Use a separate low-limit key when working on code you do not trust, and rotate keys you ever exposed.
- The image only has git, build tools, Python, ripgrep and jq. For other stacks add packages: `docker build --build-arg EXTRA_PACKAGES="nodejs npm" -t pirs:node .` and run with `PIRS_IMAGE=pirs:node pirs-docker --yolo`. For a Rust project use the Rust variant noted in stage 25.
- Speed: file access to a bind-mounted folder is slower than native, and `cargo build` or `npm install` inside it can feel slow. Tip: keep heavy build output in a Docker volume instead of the mounted folder.
- If files come out owned by the wrong user, or you get permission errors on `/work`, tell me the exact message; the usual fix is running the container with `--user "$(id -u):$(id -g)"`.

## 4.6 Keeping cost under control

1. The key's own credit limit is the hard stop that does not depend on pirs at all.
2. `limits.max_cost_per_run` and `max_cost_per_session` stop pirs before the key limit is reached.
3. Watch `[pirs] ... cost=$...` after each run in print mode, `/cost` in the TUI, and `/budget` for the key. Compare with the OpenRouter dashboard once in a while.
4. A cheap model that loops costs more than a good model that finishes. If a task burns money without progress, press Esc, stop, and look at the debug log (`pirs --debug`, file `~/.pirs/debug.log`, secrets redacted).

## 4.7 Terminal settings that matter on a Mac

- **Alt+Enter and Alt+arrows:** in Terminal.app set Profiles, Keyboard, "Use Option as Meta key". In iTerm2 set Profiles, Keys, Left Option key to "Esc+". `Ctrl+J` inserts a newline in any terminal.
- If colors or boxes look broken, run `echo $TERM`; it should be `xterm-256color`. Terminal.app, iTerm2, Ghostty, kitty and WezTerm all work with ratatui.
- If the terminal looks garbled after a crash, run `reset`.

## 4.8 Troubleshooting

| Symptom | Likely cause and fix |
|---|---|
| `authentication failed (401)` | Key not exported in this shell. `echo ${#PIRS_API_KEY}` should print a number above 0. Check the Keychain line in `~/.zshrc`. |
| `402` right at the start | Key limit used up or credits at zero. Raise the key limit or add credit. Also lower `max_output_tokens`: OpenRouter checks the worst-case cost against what the key has left. |
| `429` many times | Free-model limits. Use the paid model id. |
| `error: linker 'cc' not found` | Run `xcode-select --install`. |
| `openssl` errors while building | Rare on macOS. Tell me the full error. |
| `docker: Cannot connect to the Docker daemon` | Start Docker Desktop, OrbStack or Colima. |
| Everything the model does is refused in `-p` mode | Print mode has nobody to ask; add `--yolo` (inside Docker) or use the TUI. |
| TUI text clipped or cursor in the wrong place | Resize the window once; check `TERM`; make sure you are on the stage 19 and 20 build (`pirs --version`). |

---

# APPENDIX: OPTIONAL STAGES AFTER v0.2.0 (one per session, only if you need them)

- **O1 Session UX:** `/name`, resume picker showing first message and date, `/export` to a Markdown file.
- **O2 Markdown rendering** in the TUI (headings, bold, inline code, code blocks).
- **O3 Anthropic live tests and thinking signatures** (needs an Anthropic key; add a live test file like stage 17).
- **O4 Split-turn compaction** (a separate summary of the first part of a very long turn).
- **O5 Extensions:** external programs that talk JSON over stdin and stdout, registered in `settings.json`, able to add tools and slash commands. Design it on paper first.
- **O6 Prompt caching and reasoning controls** if your provider and model support them.
- **O7 A proper sandbox for native runs:** macOS `sandbox-exec` profiles are possible but poorly documented; Docker is the supported path in this kit.
