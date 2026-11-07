# Live findings against OpenRouter

Status: **first live run done 2026-11-07; L1 to L5 passed, L6 config fixed,
rerun pending.** Model used: `deepseek/deepseek-v4.1-flash-1017` via provider
`Morph`. Values below come from `<PIRS_HOME>/debug.log` (redacted) and the
smoke output. No key was logged.

## Questions to answer

| Item | Question | Observed | Decision |
|---|---|---|---|
| (a) | Is reasoning streamed as `delta.reasoning` or `delta.reasoning_content`? | A reasoning delta was streamed and stored: the next request body carried `"reasoning_content":"The user wants exactly 5 short lines naming five colors. No tools needed."` on the assistant turn. The pasted log tail does not show which delta key the wire used. | `src/ai/sse.rs` accepts `reasoning_content` first, then `reasoning`. No change. |
| (b) | Does a 400 appear when an assistant turn with tool calls is sent back without its reasoning? | No 400. L2 (bash) and L3 (edit) passed, and tool turns are replayed with `reasoning_content` when a thinking block exists. | Keep sending `reasoning_content` in the OpenAI adapter only. A fake-server request-shape test still guards this. |
| (c) | Which real `finish_reason` values appear? | `stop` in every L6 reply (`native_finish_reason` also `stop`). `tool_calls` is accepted: L2 and L3 ran tools and exited 0. | Mapping is sufficient for these tests. |
| (d) | Does a `usage` chunk arrive, with `cost`? | Yes. Example: `prompt_tokens:926`, `completion_tokens:15`, `total_tokens:941`, `cost:0.000017156`, `cached_tokens:512`. Every L1 to L5 run printed a `[pirs] ... cost=$` line. | Cost tracking works; no change. |
| (e) | Do `tool_calls` fragments ever lack an `index`? | Not observed. L2 and L3 passed with the real stream. | The parser defaults a missing index to 0, so a missing index cannot break tool calls. No change. |

## First smoke run

- L1 reply, L2 bash, L3 edit, L4 resume, L5 budget: passed.
- L6 compaction: failed after 4 requests because the stage settings
  (`context_window` 3000, `keep_recent_tokens` 800) could not compact this
  model. Four 5-line replies add about 19 estimated tokens each, and real
  prompt usage stayed near 1000 tokens.
- Fix in `tests/live_openrouter.rs`: `context_window` 1200,
  `reserve_tokens` 500 (unchanged), `keep_recent_tokens` 180, and each prompt
  supplies enough context so the first three turns stay inside the kept region.
  Request cap stays at 6.

## How to re-check

1. `scripts/live-smoke.sh`
2. Confirm six tests pass and the final line shows the cost sum.
3. If L6 still fails, read the `debug.log tail` in the panic and check the
   four `requests=` values and the session file for `"type":"compaction"`.

## Notes

- The key is never written to the log or to this file.
- Live tests cap `max_tokens` at 300 and fail a run above $0.05.
- Test L6 counts requests and fails above 6.
