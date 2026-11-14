# Test and Eval Plan: arqus flow

*Rewritten for Rust and arqus on 15 Nov 2026. Companion to document 5. Part A is the automated suite that proves the harness works. Part B is the eval suite that tells you whether it saves cloud tokens without losing accuracy. Thresholds in Part B are my suggestions: set yours before the first run and write them down. Helper and file names follow the README's layering; Stage F0 confirms the real ones.*

# Part A: Automated tests

## A1. Layers and gate

arqus already has four layers (README): L1 unit tests, L2 black-box tests against in-process mock SSE servers, L3 live smoke tests, L4 evals. flow adds to each.

| Layer | flow additions | Command |
| --- | --- | --- |
| L1 Unit | Tests for every module under `src/flow`, in sibling `tests.rs` files | `cargo test --bin arqus flow` |
| L2 E2E | `tests/e2e_flow_plan.rs`, `e2e_flow_ladder.rs`, `e2e_flow_resume.rs`: black-box `arqus flow` runs against two mock servers (orchestrator and worker) and a temp git repo | `cargo test --test 'e2e_flow_*'` |
| L3 Live | `tests/live_flow.rs`, every test `#[ignore]` | `scripts/live-smoke.sh` (extended) |
| L4 Evals | The flow evals in Part B | `scripts/run-flow-evals.sh` |

**Gate:** `scripts/check.sh` (fmt check, clippy with `-D warnings`, tests, release build, secret scan) plus a new step, `scripts/audit-flow.sh`. The audit fails when:

- A non-test file under `src/flow` contains `unsafe`, `.unwrap()`, `.expect(`, `panic!`, `todo!`, `unimplemented!`, `dbg!`, `#[allow(`, `TODO`, `FIXME`, `tokio::spawn`, `join!`, `std::thread`, `sh -c`, or `Command::new(` followed by `sh`. `println!` is allowed only in `cli.rs`.
- Any `#[ignore]` appears outside `tests/live_*.rs`.
- Any file under `src/flow` or any `tests/*flow*` file is longer than 300 lines.
- A `src/flow` file other than `mod.rs` defines a function but has no sibling tests file.

The audit has its own test, `scripts/test-audit-flow.sh`, which builds temp trees containing each violation and a clean one. The audit is a guardrail against a coding agent weakening tests or cutting corners.

Rules for every test outside L3: no network except loopback mock servers, no real model, no real sleeping beyond milliseconds (set `ARQUS_RETRY_BASE_MS` tiny), deterministic, temp git repos only (`tempfile`), and assertions on behaviour, not only on mock calls.

## A2. Test doubles and helpers

- **Scripted provider:** the existing `ai/mock.rs`. Extend it additively if it cannot script an error item or record the requests it received.
- **`tests/common/`:** `MockServer` (SSE responses from a queue per server, records request bodies and counts, can return HTTP errors or delay, can report each request to the test), `make_repo(files)` (temp dir, `git init`, local identity, commit), `run_flow(args, env)`, and `kill_child_on_request(server, n)`.
- **Golden prompt files** under `tests/golden/flow/`, rewritten only when `UPDATE_GOLDEN=1` is set, with the diff reviewed by a human.
- **`scripted_plan(tasks)`:** builds a valid plan reply so tests do not hand-write JSON.

## A3. Unit tests by module

**config.rs**

- A valid `flow` section loads; an unknown key is rejected; an unknown default worker is rejected; the `workers` array is replaced, not merged, between global and project settings.
- A model name with a colon (`qwen3.5:9b`) survives intact. A key is read from the named environment variable; a missing variable gives an error that names the variable and never a value; an empty `api_key_env` sends `local`.
- A non-empty `by_category` is ignored with exactly one warning. CLI overrides replace the orchestrator model and worker.
- A malformed `flow` section makes `arqus flow` exit 2 while a malformed unrelated section keeps its existing behaviour.
- The redactor removes both role keys: set two fake keys, log a line containing each, and assert neither survives.

**schema.rs**

- Ids: accepts `T01`, `T123`, `T07.1`; rejects `t1`, `T1`, `T07.`, `X07`.
- Limits on title, objective, constraints and interface notes; a line range with only one end is rejected; serde round-trips; unknown fields rejected.
- `PlannerReply` is chosen by its `type` tag; each `RedesignDecision` variant parses; a variant missing its required field fails with a useful message.

**model/json.rs**

- Strips think tags, including an unterminated one. Extracts JSON from a fenced block, from prose around it, and from the first balanced object. Rejects an array where an object is required.
- A serde error becomes a repair message listing the field path.

**model/complete.rs** (scripted provider)

- Joins streamed text; collects usage including cache read and write; a provider error maps to `ProviderError` with its detail.

**model/ledger.rs**

- One valid JSON line per call. Cost is the provider-reported cost when present, otherwise computed from the price table with cached input priced separately, otherwise null. Totals by role, with the cloud and local split.

**Adapter change (`ai/openai.rs`)** (mock server captures the request body)

- `extra_body` is merged shallowly; it is absent when empty; it cannot override `model` or `messages`.

**repo/git.rs** (real git in temp dirs)

- A clean repo is accepted; a dirty one is refused (modified and untracked files both count).
- Branch and worktree are created. `.arqus/flow/` is added to `.git/info/exclude` and `.gitignore` is untouched.
- `reset_hard_clean` removes modified, deleted and untracked files. The commit author is arqus-flow, the user's git config is unchanged, and the base branch HEAD is unchanged after a run.

**exec/sandbox.rs and verify.rs**

- Allowlist: an exact leading-token match is accepted (`cargo test --lib`), look-alikes are rejected (`cargo testx`, and `pytest; rm -rf /`, whose first token is `pytest;`), an empty command is rejected, and quoted arguments split correctly.
- Runner: the exit code is captured; a timeout kills the whole process group (the test spawns a child that sleeps); output tails are cut to 4,000 chars; the environment contains only passthrough keys (set a fake secret and assert it is absent); a risky pattern from `safety.rs` is rejected.
- Scope check: allowed paths pass; an extra file, a deleted file, a renamed file and a file outside the repo all fail; a diff over `max_changed_lines_hard` fails.

**exec/edits.rs** (the most important unit tests)

- Parse: a single SEARCH block, several blocks, a FILE block, mixed blocks, prose around blocks, CRLF input, look-alike markers inside content, a missing terminator giving `ParseError`, `CANNOT_COMPLETE` giving `Declined`, empty output giving `NoOutput`.
- Apply: one exact match; no match, with a hint in the message; two matches giving `AmbiguousMatch`; a search spanning lines; a whitespace difference as a strict non-match; a CRLF file edited with an LF search and written back as CRLF; permissions kept; FILE creating and replacing; a path outside `allowed_writes`, an absolute path, and `..` all giving `ScopeViolation`; two blocks on one file applied in order; atomic, so a failing second block leaves the first unwritten.

**repo/map.rs**

- File tree from `git ls-files` only. Symbol scanners for Rust (`fn`, `struct`, `enum`, `trait`, `impl`), Python (`def`, `class`), Go (`func`, `type`) and TypeScript (`function`, `class`, `interface`, exported `const`) on fixture files. Size cap with a truncation marker. Stable output: the same input gives the same bytes.

**repo/context.rs**

- A whole file and a line range both render. A missing context file is a typed error. A new file is listed as new. Upstream summaries come from state. An over-budget prompt fails with the largest file named. Rendering is byte-deterministic.

**planning/validator.rs**

- One test per rule in spec section 7, plus: a valid plan returns no issues; a cycle is reported with the ids involved; overlapping writes are rejected without an ordering and accepted with one; a context path created by an upstream task is accepted; a worker task over any limit is rejected while the same task assigned to the orchestrator is accepted; a non-allowlisted verify command and a protected path are rejected.

**planning/capability.rs**

- The sheet renders limits, strengths and weaknesses. Observed stats appear only for categories with n of at least 5. Golden snapshot.

**planning/planner.rs** (scripted)

- A valid plan directly; a need-files round then a plan (contents injected, truncated at 60 KB); more than `max_file_read_rounds` forces a plan request; invalid JSON is repaired; an invalid plan has its issues fed back and fixed; an exhausted repair budget returns `PlanningFailed`.
- The system message and the repo-map message are byte-identical across all calls of a run (cache-prefix test).

**exec/worker.rs, takeover.rs, redesign.rs** (scripted)

- Worker success on the first call. The second call carries the failure kind and the truncated previous output. A failure after `max_worker_attempts` returns a `FailureRecord`. `Declined` is recorded as such.
- Redesign: each action parses; invalid output counts as takeover; the prompt holds at most 2 failure records and respects the truncation caps.
- The takeover prompt includes the failure history and goes to the orchestrator model, never the worker model.

**engine/state.rs, scheduler.rs, router.rs, report.rs**

- Every allowed transition succeeds and every other raises `InvalidTransition` (a test loops over the full status matrix).
- Dependents become Skipped when a dependency fails, transitively. Next-ready is deterministic and respects dependencies. A split rewires dependents to the leaf subtasks, supersedes the old task, and ids follow `<old>.N`.
- Persistence is atomic (simulate a crash between the temp write and the rename and assert the old file is intact) and round-trips.
- `DefaultRouter` returns the default worker even when `by_category` is set.
- The report lists each task with executor, attempts, redesigns, tokens and status, and its totals equal the ledger exactly.

## A4. End-to-end scenarios (L2, black box)

Each test writes a `settings.json` into a temp repo pointing the orchestrator and worker at two mock servers, sets two distinct fake keys in the environment, runs `arqus flow`, and asserts the exit code, the commits on the flow branch, the exact number of requests each server received (so token waste is visible), and that the user's working tree and base branch are untouched.

| ID | Scenario | Key assertions |
| --- | --- | --- |
| S1 | Three linear tasks, all succeed first try | 3 commits in order; worker server got 3 requests; orchestrator server got 1 (the planner); exit 0 |
| S2 | Worker fails verify, passes on attempt 2 | Request 2 carries the failure kind and detail; 1 commit; no redesign request |
| S3 | Worker exhausts attempts, redesign revises, worker then succeeds | Exactly 1 redesign request; attempt counter reset; no takeover |
| S4 | Redesign splits the task | Original is Superseded in `state.json`; subtasks run; dependents unblocked and run |
| S5 | Redesign chooses takeover; orchestrator succeeds | Commit body names the orchestrator as executor; the worker server is not called again |
| S6 | `max_redesigns` reached | Automatic takeover with no extra redesign request |
| S7 | Takeover fails | Task Failed; dependents Skipped; independent tasks still run; exit 1 |
| S8 | Planner returns invalid JSON, then valid | One repair request; run proceeds |
| S9 | Plan violates worker limits | Issues appear in the repair request; the corrected plan is accepted |
| S10 | Repair budget exhausted | Exit 1; the flow branch has no commits beyond base; working tree untouched |
| S11 | Worker edits a file outside `allowed_writes` | Rolled back; worktree clean; `ScopeViolation` recorded |
| S12 | Plan includes a non-allowlisted verify command | Rejected at validation; never executed (a marker file proves it) |
| S13 | Crash, then resume | The test kills `arqus` when the worker server sees its 3rd request; `flow resume` finishes; accepted tasks are not re-run (request counts prove it); the final tree equals an uninterrupted run |
| S14 | Cloud budget exceeded mid-run | Exit 1 naming `max_cloud_tokens_per_run`; state resumable; no uncommitted partial changes |
| S15 | Dirty repository | Exit 1 before anything is created |
| S16 | Task assigned to the orchestrator | The worker server receives no request for it |
| S17 | Provider returns 500 on every retry | `ProviderError` after the configured retries (tiny `ARQUS_MAX_RETRIES` and `ARQUS_RETRY_BASE_MS`); the ladder continues |
| S18 | Existing CLI unchanged | `arqus --version`, `arqus -p` against a mock, and `arqus flow --help` behave as specified; the existing e2e suites pass in the same run |
| S19 | Worker outputs `CANNOT_COMPLETE` | Counts as a failed attempt; the second attempt still runs with feedback; then redesign as normal |
| S20 | Key hygiene | Neither fake key appears in stdout, stderr, `debug.log`, `events.jsonl`, `usage.jsonl` or `state.json` |

## A5. Property tests (proptest)

- Random DAGs: the validator and the scheduler agree on acyclicity, and topological order always respects dependencies.
- Random file contents with a unique substring: applying a SEARCH edit and reading the file back yields exactly the replacement.
- Random strings fed to the edit parser and to `validate_shape` never panic and only return typed errors or issues.

## A6. Live tests (`tests/live_flow.rs`, every test `#[ignore]`, run by `scripts/live-smoke.sh`)

Not part of `scripts/check.sh`. They run against the configured models and cost cents:

- `arqus flow models-check` passes for every configured model, and usage fields are present or the estimate path is used.
- The orchestrator returns a valid plan for a fixed tiny objective on a fixture repo.
- The local worker produces parseable edit blocks for 3 trivial contracts, with at least 2 of 3 passing verification.
- Provider quirks found here (base URL, model name, thinking-mode handling) go into `settings.json` through `extra_body`, never into code.

# Part B: Eval suite

## B1. Three layers

The end-to-end result is a product of three things, so measure them separately. Otherwise a failure cannot be traced.

1. **Nano eval:** can the worker do hand-written contracts? Output: first-attempt pass rate per category for each worker model.
2. **Planner eval:** can the orchestrator produce contracts the validator accepts and the worker completes? Output: validity rate and downstream success per planner model.
3. **End-to-end eval:** does the whole ladder match the existing agent on accuracy while spending fewer cloud tokens?

## B2. What the existing evals can and cannot tell you

The README reports 13 of 13 tasks passing for $0.0073 in total (average 7.9 s) on `deepseek/deepseek-v4.1-flash-1017`, about 0.06 cents per task. Two consequences:

- The existing agent is already saturated on those tasks, so they cannot show that flow is more accurate, and any token saving there is a fraction of a cent. Keep them as a regression check (flow should pass them too), not as a judge.
- The judging set needs tasks where the existing agent either fails sometimes or spends real tokens. **Admission rule (my suggestion):** a task joins the judging set only if the existing agent fails it at least once in 3 runs, or averages at least 50,000 cloud tokens. Tasks that fail this rule are too easy to be informative.

## B3. Task format

Follow the format the existing `evals/` tasks use (Stage F10 inspects it first) and add only the fields below. Existing fields keep their meaning.

| Field | Meaning |
| --- | --- |
| `language` | `rust`, `python`, `go` or `typescript` |
| `size` | S single function, M single module, L multi-file |
| `category` | One of the `Category` values from the spec |
| `repo` | A fixture directory with git history, or a pinned URL and commit |
| `objective` | The prompt given to both D0 and H1, word for word |
| `public_verify` | Commands the planner may see and use as verify commands |
| `hidden_verify` | Commands run only after the last model call, with hidden test files copied in then |
| `regression_verify` | The project's full existing suite |
| `protected` | Paths whose edits are ignored by the final check, such as the tests directory |
| `reference_patch` | A known-good diff, used only to validate the task |

**Task validity checks**, run once when a task is added: the repo is clean; `hidden_verify` fails before the reference patch and passes after; `regression_verify` passes before and after; `public_verify` fails before. A task failing any check is rejected.

## B4. Task sets

- **Smoke set (built in F10):** 6 tiny fixtures made by the coding agent, used only to test the eval machinery with scripted models. They never judge the system.
- **Judging set (built by you, 30 tasks):** from your own repositories or pinned open-source ones. Mix: 10 S, 12 M, 8 L, with at least 6 each of Rust, Python, Go and TypeScript, and every task passing the admission rule in B2. Hold out 6 tasks that you do not look at while tuning prompts.
- **Nano set (60 contracts):** hand-written contracts across the categories (at least 8 per category), each with `hidden_verify`.
- **Planner set:** the 30 objectives, without contracts.

## B5. Conditions

| ID | Name | What runs |
| --- | --- | --- |
| D0 | existing\_agent | `arqus -p OBJECTIVE` with the orchestrator model, with sessions kept so usage can be read |
| H1 | flow\_default | `arqus flow run` with the full ladder |
| H2 | flow\_no\_redesign | H1 with `max_redesigns = 0` (retry, then takeover) |
| H3 | flow\_no\_ladder | H1 with `max_worker_attempts = 1` and `max_redesigns = 0` (raw worker quality) |
| H4 | flow\_alt\_planner | H1 with each other candidate orchestrator |
| H5 | flow\_alt\_worker | H1 with each candidate local worker |

D0 is a full agent loop, so a flow win has to come from saving cloud tokens at equal accuracy, not from beating a weak baseline. Report both accuracy and tokens for every task, and never average a saving over tasks where D0 already costs next to nothing.

## B6. Metrics

Per run, pass is true only if `hidden_verify` and `regression_verify` both exit 0 in the final worktree after the hidden files are copied in.

- **Accuracy:** pass per task (majority over repeats) and per-run pass rate. Scope violations (must be 0) and regressions.
- **Cloud tokens:** for D0, input, output, cache read and cache write from the session JSONL usage fields (or the stderr summary line when only totals are needed). For H1, the sum over planner, redesign and takeover rows in `usage.jsonl`. Report raw, and report a cost-weighted figure using provider-reported cost, or the price table with cached input at its cached price.
- **Cloud dollars per solved task:** total cloud cost divided by solved tasks.
- **Token saving:** for each task solved by both D0 and H1, `1 - cloud_tokens_H1 / cloud_tokens_D0`. Report the median and the aggregate.
- **Delegation:** share of tasks finished by a worker; worker first-attempt pass rate; redesign, takeover and split rates.
- **Overheads:** planner share of cloud tokens; redesign plus takeover share; tokens spent on tasks that ended Failed.
- **Contract quality:** plans accepted without repair; repair rounds per plan; contracts rejected by the validator; contracts whose worker failed the first attempt.
- **Local cost:** local tokens, wall time per task, peak memory (optional).
- **Stability:** per-task pass count out of R repeats, flagging tasks that are neither always passed nor always failed.

## B7. Protocol

- The runner is `evals/flow_run.py`, Python standard library only, to match `evals/run.py`. It builds isolated git workspaces the same way the existing runner does. arqus itself stays pure Rust.
- R = 3 repeats per task and condition. Cloud temperature 0.2 (or the lowest supported), local temperature 0.1.
- Pin and record: model names and versions, quantisation, provider, prompt version, settings hash, arqus commit, `rustc --version`, date and hardware. Do not change any of these inside one comparison.
- Interleave conditions per task so that provider load and price changes hit all conditions alike. Note peak versus off-peak windows for providers that price by time.
- Never show the planner hidden tests. Hidden files are copied in only after the run's last model call.
- Report failed runs and aborts, not only successes.
- With 30 tasks and 3 repeats the eval detects only large differences. Report paired per-task outcomes (both pass, both fail, only D0 passes, only H1 passes) and add an exact sign test on the discordant tasks for reference, without claiming more than it supports.

## B8. Order of work and gates

1. **Gate A:** `scripts/check.sh` passes, including all L1 and L2 tests and the audit.
2. **Nano eval** for each candidate worker. A category counts as delegable for a worker when first-attempt pass is at least 80% with n of at least 8. Output `profile_stats.json`, which feeds the capability sheet. If no category is delegable, stop: that worker is not useful here.
3. **Planner eval** (H3 conditions on the 30 objectives, validator plus oracle execution): at least 90% of plans valid within 2 repairs and at least 70% of worker-assigned contracts in delegable categories passing first attempt. Pick the planner on this basis.
4. **End-to-end eval:** D0 versus H1, then H2 and H3 as ablations if H1 passes.

**Decision table for the end-to-end result** (my suggested thresholds):

| Result | Action |
| --- | --- |
| H1 loses at most 2 tasks to D0 (paired) and the median cloud-token saving is at least 30% | Make flow the recommended mode for large tasks |
| Parity holds but the median saving is 10 to 30% | Keep flow, but tell the planner to delegate only delegable categories and keep the rest |
| Saving under 10% | Do not recommend decomposition. Keep only the cheap lane: local worker for delegable categories, orchestrator for everything else |
| H1 loses more than 2 tasks | Read every lost task, fix the top cause (contract quality, context, verifier gaps), and re-run once on the held-out set. If it still fails, stop |
| H2 matches H1 | The redesign step is not earning its tokens; drop it from the defaults |
| H3 close to H1 | The ladder adds little because the worker is already reliable on delegated tasks |

## B9. Failure analysis template

For every task where H1 failed or took over, record the first failure kind and whether the contract was at fault (missing context, wrong interface, too large, bad verify command), the worker was at fault (could not follow a good contract), or the verifier was at fault (passed wrong code or failed right code). Tally the three causes. They point to different fixes: planner prompt, worker choice, or test quality.

## B10. Reporting

`python3 evals/flow_report.py <results_dir>` writes `evals/FLOW_REPORT.md` and appends rows to `evals/flow_results.jsonl`, beside the existing `BASELINE.md` and `results.jsonl`, which stay untouched. The report holds the configuration and pins, the paired accuracy table, token and dollar tables per condition, the B6 metrics per category and per language, the B9 tally, and the decision-table row that applies. Raw results and every run's `usage.jsonl` are kept so any number can be recomputed. The Python tooling has its own self-tests (`python3 -m unittest discover evals/tests`), which `scripts/check.sh` runs when `python3` is present.
