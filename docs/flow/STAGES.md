# Build Order and Agent Instructions: arqus flow

*Rewritten for Rust and arqus on 15 Nov 2026. Companion to document 5 (spec) and document 6 (tests and evals). This is the order in which to hand the work to Antigravity with Gemini 3.8 Flash, with the rules file and prompts. Antigravity details come from third-party guides and a changelog summary, so confirm them in your installed version. Everything about arqus internals comes from its README, which is why Stage F0 comes first.*

## 1. Setup

1. Export documents 5, 6 and 7 as Markdown into the arqus repo as `docs/flow/SPEC.md`, `docs/flow/TESTS.md` and `docs/flow/STAGES.md`. The agent reads these files; they are the source of truth.
2. Add the rules in section 2 to the repo's `AGENTS.md`. Antigravity reads `AGENTS.md` (since IDE 1.20.5, per a third-party summary) and limits each rules file to 12,000 characters. arqus also sends its own `AGENTS.md` with every request it makes, so keep the flow section short. If you would rather not enlarge arqus's own prompt, put the flow rules in `GEMINI.md` instead, which third-party docs describe as Antigravity-only; check that your version applies it.
3. Use Planning mode, not Fast mode, for every stage. Planning mode produces a plan the agent must show before it codes.
4. One stage per conversation. Start a fresh conversation for the next stage instead of continuing a long thread. That is my judgment, not a tested result, but long threads are where agents drift from specs.
5. Commit after every green stage, and update `PROGRESS.md`, which the README says tracks the project's honest status.
6. Add the role key variables you chose in settings (for example `ARQUS_ORCHESTRATOR_KEY`) to `.env.example` so `scripts/dev.sh` loads them.

**Prompt to paste for each stage** (replace Fx):

```
Implement stage Fx from docs/flow/STAGES.md.
Read docs/flow/SPEC.md and docs/flow/TESTS.md first.
Before writing code, show me: the files you will create or change, and the test
names you will write. Wait for my approval.
Then write the tests first, run them and confirm they fail, then implement.
Do not change code outside this stage unless a test here requires it, and tell
me why if you do. Changes to existing modules must be additive or a pure
extraction, with the existing tests unchanged.
Finish when scripts/check.sh passes. Paste its summary and list anything you
were unsure about or chose to interpret.
```

When the agent reports something it was unsure about, answer from the spec or paste the question back here and I will check it against the spec.

## 2. Rules to add to AGENTS.md (copy as is)

```
# arqus flow rules (apply when working on src/flow)

Read docs/flow/SPEC.md, docs/flow/TESTS.md and docs/flow/STAGES.md first. They
are the source of truth. If something is ambiguous or looks wrong, stop and ask.
Do not invent behaviour.

## Workflow
- Do ONE stage at a time: the one the user names. Never start the next.
- Write the stage's tests first, see them fail, then implement.
- Finish with scripts/check.sh passing. Paste its summary. Update PROGRESS.md.
- Commit as 'F<n>: <title>' only after check.sh passes.
- Never weaken, skip, ignore or delete a test to make it pass. Fix the code.
  If a test seems wrong, explain why and ask.
- Do not add features, flags, abstractions or files that are not in the spec.
- Changes to existing modules: additive only, or a pure extraction of a function
  into a shared module with the existing tests unchanged and passing. Commit
  extractions separately from feature work.

## Rust rules
- Edition 2021. Use String, Vec, Option and anyhow::Result. Avoid lifetimes and
  generic abstractions.
- New crates allowed: shlex; dev only: proptest, tempfile. Ask before adding
  anything else.
- Every file under 300 lines; split into submodules. Unit tests go in sibling
  tests.rs files, never inline.
- No unwrap, expect, panic!, todo!, unsafe or dbg! outside tests. println! only
  in flow/cli.rs.
- Sequential only: no tokio::spawn, join! or threads in src/flow.
- Spawn commands with explicit argument lists. Never sh -c in flow code. Commands
  that come from contracts must pass the allowlist.
- Model-supplied paths: reject absolute paths and '..', use the safety.rs checks,
  resolve symlinks.
- Never print, log or store API keys. The redactor must cover every resolved
  role key.
- No network in cargo test except loopback mock servers. Never run live tests
  unless the user asks.
- Imports: flow may use types, ai, cost, safety, settings, debug and extracted
  helpers. It must not import session, tui, agent, runtime or compaction. Only
  main.rs imports flow.
- Typed error enums in flow/errors.rs with hand-written Display; convert to
  anyhow at the CLI boundary.
- Prompts live in flow/planning/prompts.rs with a PROMPT_VERSION constant.
  Golden files change only on purpose, in the same commit.

## Commands
- scripts/check.sh                full gate (fmt, clippy -D warnings, tests,
                                  release build, secret scan, flow audit)
- cargo test --bin arqus flow     fast loop
- cargo test --test 'e2e_flow_*'  black-box flow tests

## When you finish
Report: files changed, tests added, check.sh summary, decisions made where the
spec was silent, anything you were unsure about.
```

## 3. Stages

Each stage lists what to deliver, which tests must exist (names refer to document 6, sections A3 to A6), and when it is done. Implement them in order and do not let the agent combine them.

**F0. Orientation and gate (no feature code).** Deliver: (1) `docs/flow/NOTES.md` listing, for each row of the spec's section 3, the real file, type and function names and signatures, and whether the planned change is additive, with every wrong assumption flagged. (2) The new crates (`shlex`; dev: `proptest`, `tempfile`). (3) `scripts/audit-flow.sh`, its test `scripts/test-audit-flow.sh`, and the audit wired into `scripts/check.sh`. (4) If each e2e test file carries its own mock server, extract one into `tests/common/` as a pure move. Tests: the audit test script (each violation fails, a clean tree passes). The existing 629 tests still pass unchanged. Done when: `scripts/check.sh` passes. Then YOU read NOTES.md against the spec and settle any mismatch before F1.

**F1. Schemas, errors, config.** Deliver: `flow/schema.rs`, `errors.rs`, `config.rs` (the `flow` section of `settings.json`, precedence, role key lookup, CLI override structs), the redactor extended to every role key, and a `flow` clap subcommand skeleton whose `--help` works. Tests: A3 for config and schema, including the colon-in-model-name case and the two-key redaction test. Done when: tests pass.

**F2. Model layer.** Deliver: `model/complete.rs` (consume a stream into text, usage and latency), `model/json.rs`, `model/ledger.rs`, the additive `extra_body` change in the OpenAI-compatible adapter, an additive extension of `ai/mock.rs` if needed, and `arqus flow models-check`. Tests: A3 for model/complete, json, ledger and the adapter change. Done when: tests pass. Then YOU run `arqus flow models-check` against your real orchestrator and local worker and bring any failure here. Fix base URLs, model names or `extra_body` in settings, not in code.

**F3. Git, sandbox, verification.** Deliver: `repo/git.rs`, `exec/sandbox.rs`, `exec/verify.rs`, plus a pure extraction of the shared process-group spawn and kill helper and the atomic-write helper if the existing tools do not already share them. Tests: A3 for git, sandbox and verify, using real git in temp directories. Existing bash and MCP tests unchanged and passing. Done when: tests pass, including the process-group timeout kill and the environment scrubbing test.

**F4. Edit format.** Deliver: `exec/edits.rs` (parse and atomic apply) exactly as in spec section 8, calling the matching logic extracted from `tools/edit.rs`. Tests: all of A3 for edits, the edit property tests from A5, and the unchanged existing edit-tool tests. Done when: tests pass. This is the most error-prone module for small models, so read its tests yourself before accepting the stage.

**F5. Repo map and context builder.** Deliver: `repo/map.rs` (tracked files; Rust, Python, Go and TypeScript symbol scanners by line scanning; size cap) and `repo/context.rs` (rendering, budget enforcement, upstream summaries). Tests: A3 for map and context. Done when: tests pass and rendering is byte-deterministic.

**F6. Validator, capability sheet, planner.** Deliver: `planning/validator.rs`, `capability.rs`, `prompts.rs`, `planner.rs` (need-files rounds, repair loop), and `arqus flow run --dry-plan`. Tests: A3 for validator, capability and planner, with golden prompt snapshots. Done when: tests pass, including the cache-prefix identity test. Then YOU run `--dry-plan` on two real repos with your configured orchestrator and read the plans. This is the first real look at contract quality.

**F7. Worker, takeover, redesign.** Deliver: `exec/worker.rs`, `exec/takeover.rs`, `planning/redesign.rs` as in spec sections 6 and 8. Tests: A3 for worker, takeover and redesign. Done when: tests pass.

**F8a. State, scheduler, router.** Deliver: `engine/state.rs` (types, transitions, atomic persistence), `scheduler.rs`, `router.rs` (`DefaultRouter` only). Tests: A3 for state, scheduler and router, including the full transition matrix and split rewiring. Done when: tests pass.

**F8b. Execution loop.** Deliver: `engine/run_loop.rs` implementing spec section 6, with accept and rollback and the usage ledger wired in. Tests: e2e scenarios S1 to S7, S11, S16, S17, S19. Done when: those pass.

**F8c. Planning failures, resume, budget, report.** Deliver: planning-failure handling, `resume`, the cloud-token and cost budget stop, `engine/report.rs`, exit codes. Tests: e2e scenarios S8 to S10, S12 to S15, S18, S20, plus A3 for report. Done when: S1 to S20 all pass.

**F9. CLI wiring, stats, Docker, docs.** Deliver: the full `arqus flow` CLI from spec section 11, the end-of-run append to `<ARQUS_HOME>/flow/stats.jsonl`, the Docker wrapper forwarding the role key variables, a `Flow` section in the README with honest limitations, and `PROGRESS.md` updated. Tests: CLI parsing tests, stats line format, and a check that the wrapper script lists the new variables. Done when: tests pass and `arqus flow run` works end to end against mock servers.

**F10. Eval harness (Python, standard library only).** Deliver: first inspect the format of the existing `evals/` tasks and `evals/run.py`, then add `flow_run.py` (conditions D0 and H1 to H5, repeats, interleaving), `flow_metrics.py` (every metric in document 6 B6, the paired table, cost-weighted tokens, token saving, sign test), `flow_report.py`, `flow_profile.py` (builds `profile_stats.json` from `stats.jsonl`, categories with n of at least 5 only), the task validity checker, loaders for nano, planner and e2e tasks, 6 smoke fixtures, and `scripts/run-flow-evals.sh`. Tests (`python3 -m unittest`): metrics on synthetic results with hand-computed expected values; the validity checker rejects each class of bad task; the runner completes on the smoke fixtures against mock servers and produces a report. Done when: tests pass and `scripts/check.sh` runs them. The smoke fixtures judge nothing; they only prove the machinery.

**F11. Live shakedown and first eval (you, with this chat).**

1. Write the 30-task judging set and the 60-contract nano set (document 6, B3 and B4). Apply the admission rule from B2 using D0. Hold out 6 tasks.
2. Run the nano eval for each candidate worker and keep the delegable categories.
3. Run the planner eval for each candidate orchestrator and pick one.
4. Run D0 and H1 with R = 3, then H2 and H3 if H1 passes the gate.
5. Read the report, apply the decision table in B8, and bring the report here so we can interpret it together.

## 4. Review checklist for each stage

Coding agents commonly slip in these ways (my expectation from experience with agent-written code, not a measured list). Check before accepting:

- `git diff --stat` is about the size you expected, with no unrelated files.
- `git diff Cargo.toml` shows only the allowed crates.
- `grep -rn 'unwrap()\|expect(\|todo!\|unsafe\|#\[allow\|#\[ignore\]' src/flow tests` returns nothing unexpected (the audit also checks this).
- Existing test files are unchanged apart from pure extractions into `tests/common/`.
- No import of `session`, `tui`, `agent`, `runtime` or `compaction` from `src/flow`.
- Tests assert behaviour, not just that a mock was called.
- New code has not added fuzzy matching to SEARCH edits, a parallel executor, a vector store, or an agentic worker loop.
- Errors from models and subprocesses are typed and carry enough detail for a retry prompt.
- State is written atomically after every status change.
- Provider quirks are in settings, not hard-coded.
- No prompt text changed without a golden file update in the same commit.

## 5. If the agent gets stuck

- A test failing three times for the same reason: stop the thread, restate the failing test and the relevant spec section in a fresh conversation, and ask for a diagnosis before any change.
- A mismatch between the spec and the real arqus code: record it in `docs/flow/NOTES.md`, decide it, update the spec, then the tests, then the code, in that order.
- Spec ambiguity: decide it, write the decision into `docs/flow/SPEC.md` under a 'Decisions' heading, and tell the agent to follow it.
