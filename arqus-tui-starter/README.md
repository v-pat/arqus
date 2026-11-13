# arqus TUI starter: pure logic for the new TUI (tested)

Five standalone files. They do not depend on ratatui, crossterm, tokio or on any other part of arqus.
Dependencies: `serde` (derive), `serde_json`, `anyhow`.

    src/fuzzy.rs     fuzzy matcher with highlight positions (pickers, palette, @file search)
    src/picker.rs    filterable list state machine: groups, wrap-around, scroll window, Esc clears query first
    src/auth.rs      <ARQUS_HOME>/auth.json (mode 0600, atomic save, corrupt-file backup), mask_key, clean_pasted_key
    src/catalog.rs   provider list for the Connect dialog, key lookup order, models URL, auth headers
    src/models.rs    parse GET /models (OpenRouter, Anthropic, OpenAI, Ollama) into picker rows

How to use: copy the five files into `src/` of the arqus project (into `src/tui/` is fine), add `mod` lines, and run `cargo test`.
Expect 32 tests to pass. They were checked in a sandbox with Rust 1.75 and older pinned crate versions, not on a current toolchain.
The key_url values in catalog.rs were written from memory: open each once and fix any that moved.
