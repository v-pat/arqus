

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
