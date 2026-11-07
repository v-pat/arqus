# Bug log

One line per fixed bug: symptom, cause, test. Add the test FIRST (A15).

| Date | Symptom | Cause | Test |
|---|---|---|---|
| 2026-11-07 | Print mode exited 0 after a provider error, so scripts could not tell success from failure | `run_prompt` printed an `AgentEvent::Error` and returned `Ok(())` | `tests/e2e_errors.rs::auth_error_exits_1_without_retry`, `::error_after_partial_text_is_not_retried` |
| 2026-11-07 | Retry notices never reached print mode stderr | `StreamEvent::Notice` was not matched in the agent loop | `tests/e2e_errors.rs::rate_limited_then_success` |
| 2026-11-07 | A missing API key caused a real HTTP request and a 401, instead of a clear startup error | No configuration check before the first request | `tests/e2e_errors.rs::missing_key_exits_2`, `::empty_model_exits_2` |
| 2026-11-07 | Anthropic failures were not classified, had no connect or idle timeout, and an early-ended stream looked like success | Adapter still used plain `anyhow` errors | unit checks in `src/ai/error_tests.rs`; no Anthropic-specific stream test yet (fake server speaks OpenAI) |
| 2026-11-07 | Parallel tool tests sometimes failed with `No such file or directory`, because two `temp_dir()` calls in the same nanosecond shared one folder and one test deleted it | Folder name used only process id and timestamp | `src/tools/tests.rs::temp_dirs_are_unique`, plus a test counter in `temp_dir()` |
| 2026-11-07 | A corrupt line in the middle of a session file was dropped without a warning, so real damage was invisible | `Session::open` skipped every bad line silently | `src/session_tests.rs::corrupt_middle_line_is_skipped_and_the_rest_still_loads`, `tests/e2e_crash.rs::corrupt_middle_line_opens_with_a_warning_on_stderr` |
