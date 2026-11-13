# arqus: a real TUI (stages T0 to T11)

You chose the name **arqus** and want the terminal UI to feel like opencode, Claude Code and pi: `arqus` opens the TUI at once, and you set up providers and models from inside it, by picking from lists, not by typing commands or setting environment variables.

This document is the plan for that. It follows the same style as the earlier kits: one stage per fresh opencode session, the PREFIX line, the IRON RULES, `PROGRESS.md`, commit after every stage.

**Files delivered with this one:** `arqus-tui-starter.zip`, five standalone Rust files with 32 passing tests (fuzzy search, a filterable-list state machine, the key store, the provider catalog, and the model-list parser). They have no dependency on ratatui, so the small model starts from verified logic and only has to write the drawing code.

---

# PART 0: HOW THIS FITS WITH THE EARLIER STAGES

1. **This supersedes** stages 19, 20 and 21 (TUI display, input, steering queue) and the approval dialog of stage 22. If you have not done them yet, skip them. If you did, T1 will absorb their code.
2. **Stages 12 to 18** (fake server, retries, cost limits, live tests, evals) are about the engine, not the UI. They are still valid. Stages 24 to 26 (packaging, Docker, stability gate) still come last.
3. **Recommended order:** T0 rename, T1 to T4 (so you can connect a provider from the TUI), then 12 to 18, then T5 to T11, then 24 to 26.
4. **Everything in the earlier kits that says `pirs` now says `arqus`**: the environment variables are `ARQUS_*`, the home folder is `~/.arqus`, the project folder is `.arqus/`. Stage T0 does the rename in the code. To fix the two earlier documents too, run this from the folder that holds them:
   ```
   perl -pi -e 's/PIRS_/ARQUS_/g; s/\.pirs/.arqus/g; s/pirs_home/arqus_home/g; s/CARGO_BIN_EXE_pirs/CARGO_BIN_EXE_arqus/g; s/\bpirs\b/arqus/g; s/\bPirs\b/Arqus/g' pirs-agent-prompt.md pirs-stages-12-plus.md
   ```
   (`perl -pi -e` works the same on macOS and Linux; plain `sed -i` does not.)

**What I studied.** The opencode docs (the `/connect` flow, `/models`, `Ctrl+P` palette, `Ctrl+X` leader, `@` files, `!` shell, `~/.local/share/opencode/auth.json`), the Claude Code cheat sheet (slash menu, `?` shortcuts, `Shift+Tab` modes, `Esc Esc` rewind, `Ctrl+O` transcript, status line) and the pi README (footer with cwd, tokens, cost and model; `Ctrl+L` model selector; `Ctrl+O` and `Ctrl+T` collapsing; the message queue). The spec below takes the best of each.

---

# PART 1: THE PRODUCT SPEC (what arqus should feel like)

## 1.1 Principles

1. `arqus` with no arguments opens the full-screen TUI immediately. `arqus -p "..."` stays the scripting mode.
2. First run is guided. With no provider configured, a Connect dialog opens on top of the welcome screen.
3. Never make the user type what we can list: providers, models, sessions, files, commands.
4. Everything is reachable three ways: a `/command`, the `Ctrl+P` palette, and a direct key for the common ones.
5. Every dialog has the same shape: title, search box, list, one hint line. `Esc` always goes back exactly one level.
6. State is always visible: model, context percent, cost, mode, folder and git branch.
7. The UI never blocks. Network and model work run in background tasks. A spinner shows progress and `Esc` interrupts.
8. Secrets are never shown, logged or put in a snapshot. Keys are masked as you type.
9. It must work at 80x24, without a mouse, and without truecolor (macOS Terminal.app has only 256 colors).

## 1.2 Screens

**Welcome (connected, empty session)**
```
                                                                                
                                                                                
                                   a r q u s                                    
                           a coding agent for your terminal                     
                                                                                
         ╭────────────────────────────────────────────────────────────╮         
         │ ▌ Ask anything...   try: "fix the failing test"            │         
         ╰────────────────────────────────────────────────────────────╯         
           openrouter · deepseek/deepseek-v4-flash                              
                                                                                
              ctrl+p commands    ctrl+l models    ? shortcuts                   
                                                                                
 ~/code/my-project  main*                                  ctx 0%   $0.00  ask 
```

**Welcome, first run (no provider): the Connect dialog opens by itself**
```
   ╭─ Connect a provider ─────────────────────────────────────── 1/8 ─╮
   │ › search                                                          │
   │  Recommended                                                      │
   │ ▸ OpenRouter                  One key, hundreds of models         │
   │   Anthropic                   Claude models                       │
   │  Other                                                            │
   │   OpenAI                      GPT models                          │
   │   DeepSeek                    DeepSeek models directly            │
   │   Ollama (local)              no key needed                       │
   │                                                                   │
   │ ↑↓ select · enter connect · esc close                             │
   ╰───────────────────────────────────────────────────────────────────╯
```

**API key dialog (masked, validated)**
```
   ╭─ Connect OpenRouter ──────────────────────────────────────────────╮
   │ Create a key at openrouter.ai/settings/keys                       │
   │ (ctrl+o opens it in your browser). Paste it below.                │
   │                                                                   │
   │ API key  ••••••••••••••••••••••••••                               │
   │                                                                   │
   │ enter validate · ctrl+u clear · esc back                          │
   ╰───────────────────────────────────────────────────────────────────╯
```
While validating: `⠋ Checking key...`. On failure the dialog stays open with a red line: `Key rejected by OpenRouter (401). Check it and paste again.` On success: a green `✓ Key OK` (for OpenRouter also `limit remaining $1.94`), then the model picker opens by itself.

**Model picker (`/models`, `Ctrl+L`)**
```
   ╭─ Select model · openrouter ────────────────────────────── 3/212 ─╮
   │ › flash                                                           │
   │  Recent                                                           │
   │ ✓ deepseek/deepseek-v4-flash        1M ctx · $0.14/M in · tools   │
   │  All models                                                       │
   │   google/gemini-flash               1M ctx · $0.30/M in · tools   │
   │   vendor/some-flash:free            8K ctx · free in              │
   │                                                                   │
   │ tab: tool-capable only (on) · ↑↓ select · enter use · esc back    │
   ╰───────────────────────────────────────────────────────────────────╯
```

**Chat**
```
 ┃ fix the failing test in util.py                                              
                                                                                
 I'll run the tests first to see the failure.                                   
                                                                                
 ● bash   python3 -m unittest                                      ✓ 1.2s       
   │ FAIL: test_slugify (test_util.TestUtil)                                    
   │ … +12 lines (ctrl+o to expand)                                             
 ● edit   util.py  (-1 +1)                                         ✓            
   │ - return text.lower().replace(" ", "_")                                    
   │ + return text.lower().replace(" ", "-")                                    
                                                                                
 Fixed: slugify now uses "-". The tests pass.                                   
                                                                                
 ⠋ Working... 14s · esc to interrupt                                            
╭──────────────────────────────────────────────────────────────────────────────╮
│ ▌                                                                            │
╰──────────────────────────────────────────────────────────────────────────────╯
 ~/code/proj  main*   deepseek/deepseek-v4-flash   ctx 12%  $0.004  ask        
```
Tool states: `⠋` running, `✓` green, `✗` red. Thinking shows as one dim line `▸ Thinking... (ctrl+t)` unless expanded.

**Slash menu** (typing `/` at the start of the input)
```
 ┌──────────────────────────────────────────────────┐
 │ ▸ /models      Switch model            ctrl+l    │
 │   /connect     Add or change a provider          │
 │   /mode        ask · auto · plan       shift+tab │
 │   /compact     Summarize old context             │
 └──────────────────────────────────────────────────┘
╭──────────────────────────────────────────────────────────────────────────────╮
│ /mo▌                                                                         │
```

**Approval dialog**
```
   ╭─ Run command? ───────────────────────────────────────────────╮
   │ bash                                                         │
   │   rm -rf build && cargo build --release                      │
   │                                                              │
   │ y allow once · a always allow bash · n deny · esc stop run   │
   ╰──────────────────────────────────────────────────────────────╯
```

## 1.3 Keys (final)

| Key | Action | Seen in |
|---|---|---|
| `Enter` | send (while a run is active: queue as steering) | all |
| `Alt+Enter`, `Ctrl+J` | newline | Claude Code, pi |
| `Esc` | close dialog; else interrupt the run; else clear queue | all |
| `Esc Esc` (empty input, idle) | open the session tree (rewind) | pi, Claude Code |
| `Ctrl+C` | interrupt a run; else clear the input; on an empty input press twice to quit | Claude Code, pi |
| `Ctrl+D` | quit on an empty input | opencode, Claude Code |
| `Ctrl+P` | command palette | opencode |
| `Ctrl+L` | model picker | pi |
| `F2` | cycle the recent models | opencode |
| `Shift+Tab` | cycle mode: ask, auto, plan | Claude Code |
| `Ctrl+O` | expand or collapse all tool output | pi, Claude Code |
| `Ctrl+T` | show or hide thinking | pi |
| `Up` / `Down` | prompt history (when no menu is open) | all |
| `Ctrl+R` | search prompt history | Claude Code |
| `PageUp` `PageDown` `Home` `End` | scroll the transcript | all |
| `?` (empty input) | shortcuts dialog | Claude Code |
| `/` (start of input) | command menu | all |
| `@` | file mention picker | all |
| `!` (start of input) | run a shell command yourself | opencode, pi |
| `Ctrl+A` `Ctrl+E` `Ctrl+W` `Ctrl+U` `Ctrl+K` | line editing | Claude Code |

Users can override any of these in `~/.arqus/settings.json` (stage T9).

## 1.4 Slash commands

`/connect` `/models` `/new` (alias `/clear`) `/sessions` (alias `/resume`) `/rename` `/tree` `/fork` `/compact [focus]` `/cost` `/budget` `/mode [ask|auto|plan]` `/theme` `/settings` `/thinking` `/copy` `/export` `/help` `/quit` (alias `/exit`). Prompt templates (`/name`) and skills (`/skill:name`) from stage 8 appear in the same menu under their own headings.

## 1.5 Files and who wins

```
~/.arqus/                 (env ARQUS_HOME overrides; created with mode 0700)
  auth.json               API keys saved by the Connect dialog (mode 0600)
  state.json              what the TUI remembers: last model, recent models, theme, custom base URLs
  settings.json           what YOU edit by hand (limits, compaction, skill folders, keybinds ...)
  history.jsonl           prompt history
  sessions/<project>/*.jsonl
  debug.log
```
**Key for a provider** (first hit wins): the provider's own variable (`OPENROUTER_API_KEY`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `DEEPSEEK_API_KEY`, `GROQ_API_KEY`), then `ARQUS_API_KEY` (the old universal variable; never sent to local providers), then `auth.json`.
**Model** (first hit wins): `--model provider:model`, `ARQUS_PROVIDER` / `ARQUS_MODEL`, `state.json` (the last choice made in the TUI), project `settings.json`, global `settings.json`, none (then the TUI opens the picker).

## 1.6 Colors

All colors come from a semantic `Theme` (`fg dim accent user assistant thinking tool success warning error border border_focus selection diff_add diff_del code_bg`). Built-in themes: `dark`, `light`, `mono`. Color depth is detected: truecolor (`COLORTERM` contains `truecolor` or `24bit`), else 256 colors, else 16. `NO_COLOR` selects `mono`.

---

# PART 2: AGENTS.md ADDENDUM FOR THE TUI WORK (append to AGENTS.md)

## A19. Architecture (the rule that keeps a big TUI manageable)

Use the Elm pattern: ONE state, ONE message type, a PURE update function, a render function.

```
src/tui/
  mod.rs        run(): terminal setup and restore, the event loop, the effect executor
  app.rs        struct App: ALL UI state (transcript, input, overlays, toasts, mode, footer facts)
  msg.rs        enum Msg (things that happen) and enum Effect (things to do)
  update.rs     fn update(app: &mut App, msg: Msg) -> Vec<Effect>     (pure, no IO, no await)
  view.rs       fn view(app: &mut App, frame: &mut Frame)             (draws; no state changes except caches)
  keys.rs       crossterm KeyEvent -> Key; the KeyMap (Key -> Action) with the defaults from section 1.3
  theme.rs      Theme, built-ins, ColorDepth, rgb_to_xterm256()
  overlay/      one file per dialog: connect, models, palette, help, approval, sessions, tree, theme, settings
  widgets/      dialog, picker_view, transcript, footer, editor, toast, markdown, diff
  testing.rs    #[cfg(test)] snapshot and key-replay helpers
```
Pure logic that does not need ratatui goes in plain modules (`fuzzy.rs`, `picker.rs`, `auth.rs`, `catalog.rs`, `models.rs` come from the starter) so it can be tested without a screen.

## A20. Rules for TUI code

1. `update()` never does IO, never awaits, never sleeps. When something must happen in the world (save a file, call the network, start the agent) it returns an `Effect`. The effect executor in `mod.rs` performs it and sends the result back as a `Msg`. Rust note: this is why every flow can be unit-tested without a terminal or a network.
2. The event loop never blocks. Use `tokio::select!` over: terminal events, the `Msg` channel, and a tick timer. Tick at 100 ms only while a spinner or toast is active; otherwise wait for events. Redraw only when `app.dirty` is set.
3. Every dialog is an `Overlay` variant with `handle_key(&mut self, key) -> OverlayOutcome` and a draw function. A stack of overlays is allowed; `Esc` pops one.
4. All colors come from `Theme`. No `Color::Rgb` or `Color::Red` outside `theme.rs`.
5. Measure text with `unicode-width`, never `.len()` or `.chars().count()`, when you compute layout.
6. User input never causes a panic: no `unwrap` or index-out-of-range on anything typed, pasted or received from the network.
7. Never print, log or draw a secret. Masked input stores the real value in memory only; the screen gets bullets.
8. Check the installed ratatui and crossterm versions before writing drawing code (rule 5 of the IRON RULES). The API names changed between versions (`frame.area()` vs `frame.size()`, `ratatui::init()`, the cursor call).
9. Each stage ends with: unit tests, snapshot tests for every new screen at 80x24 (and 60x20 for dialogs), and a short manual checklist for the human.

## A21. Snapshot tests (how the small model can "see" the screen)

In `src/tui/testing.rs` build two helpers on `ratatui::backend::TestBackend`:
- `render_to_string(app: &mut App, width: u16, height: u16) -> String`: draw one frame into a `TestBackend` and return the buffer as text, one line per row, trailing spaces trimmed. Colors are ignored.
- `assert_snapshot(name: &str, actual: &str)`: compares with `tests/snapshots/<name>.txt`. If the file is missing or the environment variable `UPDATE_SNAPSHOTS=1` is set, it writes the file instead and the test passes. A changed snapshot must be shown to the human in the stage report; never update snapshots silently.
- `replay(app: &mut App, keys: &str) -> Vec<Effect>`: feed a space-separated key script such as `"ctrl+p m o d enter"` (`ctrl+x`, `alt+x`, `shift+tab`, `enter`, `esc`, `up`, `down`, `tab`, `backspace`, single characters; `paste:<text>` pastes) through `update` and return all effects produced.
A snapshot is a text file: the small model can print it and read the layout.

## A22. PREFIX for this part

> Read AGENTS.md (including the A13 to A22 sections that exist) and PROGRESS.md first. Do ONLY the stage below. Follow the IRON RULES and the TUI rules A19 to A21. If a file I mention exists, read it and extend it; do not rewrite it. Show me every changed snapshot.

---

# PART 3: STAGE PROMPTS

Copy each stage from its "PASTE" line down to the next `---`.

---

## STAGE T0: Rename to arqus

PASTE:
Goal: the whole project is called `arqus`. No new features.
Do:
1. Make sure `git status` is clean (commit first if not).
2. Run these commands from the project root (they work on macOS and Linux):
```
git grep -Il 'PIRS_' | xargs perl -pi -e 's/PIRS_/ARQUS_/g'
git grep -Il '\.pirs' | xargs perl -pi -e 's/\.pirs/.arqus/g'
git grep -Il 'pirs_home' | xargs perl -pi -e 's/pirs_home/arqus_home/g'
git grep -Il 'CARGO_BIN_EXE_pirs' | xargs perl -pi -e 's/CARGO_BIN_EXE_pirs/CARGO_BIN_EXE_arqus/g'
git grep -Il 'pirs' | xargs perl -pi -e 's/\bpirs\b/arqus/g; s/\bPirs\b/Arqus/g'
```
   Then rename files: `git mv scripts/pirs-docker scripts/arqus-docker` and any other file or folder with `pirs` in its name (`git ls-files | grep -i pirs`).
3. In `Cargo.toml` set `name = "arqus"`. The binary must be called `arqus`.
4. The log line `[pirs] ...` becomes `[arqus] ...`. The Docker image tag becomes `arqus:dev`.
5. One-time home migration: at startup, if `~/.arqus` does not exist and `~/.pirs` does, rename the folder and print one line to stderr: `moved ~/.pirs to ~/.arqus`. Add a test with a temp home (pass the home folder into the function; do not touch the real one).
6. Run `cargo build`, `cargo test`, then `git grep -in pirs` and fix every leftover (comments, docs, scripts, evals). Show the final grep result (it must be empty).
Done when: `cargo test` passes, `cargo run -- --version` prints `arqus ...`, and `git grep -in pirs` prints nothing.
Don't: change behavior; touch anything inside `target/`.

---

## STAGE T1: TUI foundation (architecture, theme, test harness)

PASTE:
Goal: restructure `src/tui/` into the architecture of AGENTS.md A19 WITHOUT changing what the user sees, except the colors now come from a theme. This is the most important stage; take it slowly and commit often.
Do, in this order, running `cargo check` after each file:
1. Read the existing `src/tui/*` files. Write a mapping in PROGRESS.md: for each existing function or struct, which new file it moves to. Do not start moving code before this list exists.
2. `msg.rs`: `enum Msg { Key(Key), Paste(String), Resize(u16, u16), Tick, Agent(AgentEvent), Done(Session), Toast(String, Level), ... }` and `enum Effect { RunPrompt(String), Abort, Quit, ... }`. Start with what the current TUI needs; later stages add variants.
3. `keys.rs`: our own `Key { code: KeyCodeX, ctrl: bool, alt: bool, shift: bool }` (a small enum for codes: `Char(char)`, `Enter`, `Esc`, `Tab`, `BackTab`, `Backspace`, `Delete`, `Up`, `Down`, `Left`, `Right`, `Home`, `End`, `PageUp`, `PageDown`, `F(u8)`), `fn from_crossterm(KeyEvent) -> Option<Key>` that ignores key-release events, and `fn parse_key(s: &str) -> Option<Key>` for strings like `ctrl+p`, `alt+enter`, `shift+tab`, `f2`. Then `enum Action` and `struct KeyMap` with the default bindings from section 1.3 of the spec (only the actions that exist today are active). Tests: `parse_key` on all of those forms; `from_crossterm` for Shift+Tab (crossterm reports `BackTab`).
4. `theme.rs`: `struct Theme` with the semantic colors from section 1.6, built-ins `dark`, `light`, `mono`, `enum ColorDepth { True, Indexed256, Ansi16, None }` detected from `COLORTERM`, `TERM` and `NO_COLOR` (pass the values in as arguments so it can be tested), and a pure `rgb_to_xterm256(r, g, b) -> u8` that picks the nearest of the 6x6x6 color cube and the 24 grays. `Theme::color(&self, rgb) -> ratatui Color` returns `Color::Rgb` for truecolor, `Color::Indexed` otherwise. Tests: `rgb_to_xterm256` for black, white, a pure red, a mid gray; depth detection for several env combinations; `mono` returns no colors.
5. `app.rs`, `update.rs`, `view.rs`: move the current state, key handling and drawing into the new shape. `update` returns effects; `view` only draws. Keep the existing behavior (transcript, input box, status bar, `/tree` and `/resume` pickers, abort, slash commands as they are now).
6. `overlay/mod.rs`: `enum Overlay { Help, ... }` with `Vec<Overlay>` on the `App`; `Esc` pops. `widgets/dialog.rs`: `centered_rect(area, width, height) -> Rect` and `draw_dialog(frame, area, title, hint) -> Rect` which clears the area, draws a bordered box with the title on the top-left and a dim hint line on the bottom, and returns the inner area. Implement only `Help` (a list of the keys) to prove the mechanism; open it with `?` on an empty input.
7. `mod.rs`: the event loop from A20 rule 2, with the effect executor in a separate function. Terminal setup: alternate screen, raw mode, bracketed paste; restore on exit and on panic.
8. `testing.rs`: the snapshot helpers from A21.
9. If the old code has `Picker`-like logic for `/tree` and `/resume`, replace it with `picker.rs` from the starter kit (copy it, `fuzzy.rs` too) and a `widgets/picker_view.rs` that draws a `Picker` inside `draw_dialog`: query line `› text`, rows with matched characters in the accent color, a `✓` for the current item, details right-aligned and dim, group headers dim, the selected row in the selection color, `n/total` at the top right, a status line under the list.
Tests: all pure logic above; snapshot of the empty app at 80x24 and of the Help overlay; replay tests: typing `hello` then Enter produces `Effect::RunPrompt("hello")`; `?` opens Help and `esc` closes it; Ctrl+C on an empty input starts the quit sequence.
Done when: `cargo test` passes, no source file is over 300 lines, and the human's manual check passes: the TUI behaves as before, `?` shows help, resizing the window works, the terminal is normal after quitting.
Don't: add any feature that is not listed; keep the old files until the new ones work, then delete them in a separate commit.

---

## STAGE T2: Launch, welcome screen, footer, toasts

PASTE:
Goal: `arqus` looks like a product on launch and always shows its state.
Do:
1. `arqus` with no arguments starts the TUI. If stdout is not a terminal, print `arqus needs a terminal; use -p "prompt" for scripts` and exit with code 2. If the terminal is smaller than 60x15 draw only `terminal too small (need 60x15)`.
2. Welcome view (when the transcript is empty), as in the mockup of section 1.2: the wordmark `a r q u s` in the accent color, the tagline `a coding agent for your terminal`, the input box centered (about 60 columns wide, never wider than the screen), the line `<provider> · <model>` (or `no provider connected`), and the hint line `ctrl+p commands    ctrl+l models    ? shortcuts`. The view switches to the chat layout as soon as the first block exists.
3. Footer (one line, bottom): left `~/short/path` (home shown as `~`, shortened from the left with `…`) and the git branch with `*` when there are uncommitted changes; right side: the model (`provider/model`, truncated with `…`), `ctx N%` (green under 60, yellow under 85, red from 85), the session cost `$0.004` when known, and the mode chip (`ask`, `auto`, `plan`). When the width is short drop things in this order: hints, cost, branch, then shorten the path.
4. Git facts: `Effect::RefreshGit` runs `git rev-parse --abbrev-ref HEAD` and `git status --porcelain` through `std::process::Command` in a blocking task, at startup and after each finished run; the result arrives as `Msg::Git { branch, dirty }`. Not in a repo: show nothing.
5. Running line above the input box: `⠋ Working... 14s · esc to interrupt` (spinner frames change every 100 ms). A retry notice from the engine replaces the text: `Retrying in 4s (attempt 2/4)...`.
6. Toasts: `VecDeque<Toast { text, level, expires_at }>` in the `App`, drawn at the top right (max 3, newest first), removed on tick when expired (3 seconds, errors 6). `Msg::Toast` adds one.
7. `App.provider_state`: `Connected { provider, model }` or `NeedsProvider`. For now it is filled from the existing configuration (environment variables and settings); the next stages fill it from the Connect dialog.
Tests: footer layout at widths 120, 80, 60 (snapshots); ctx color thresholds; git parsing (pure function over the two command outputs); toast expiry; snapshots of the welcome view (connected and not connected), the chat view, and the too-small message.
Done when: tests pass and the manual check passes (launch in a git folder and in a non-git folder; resize from wide to narrow; run a prompt and watch the running line).

---

## STAGE T3: Pickers, command registry, slash menu, palette

PASTE:
Goal: one command system used everywhere: the `/` menu, the `Ctrl+P` palette, the `?` help.
Do:
1. If not done in T1, copy `fuzzy.rs` and `picker.rs` from the starter kit and keep their tests green.
2. `commands.rs`: a single table `Vec<Command { name, aliases, description, group, key_hint, action: Action }>` with the commands of section 1.4 (actions that do not exist yet map to `Action::NotYet` which shows a toast `coming in a later stage`). Also palette-only actions without a slash name: `Toggle thinking`, `Toggle tool output`, `Cycle mode`, `Cycle model`, `Quit`. Dynamic commands from stage 8 (prompt templates, skills) are added at startup under the groups `Prompts` and `Skills`. Test: names and aliases are unique; every key hint matches the `KeyMap` default for that action.
3. Slash menu: when the input starts with `/` and has no space yet, show a popup above the input with up to 8 matches (fuzzy on name and aliases), each row `/name  description  key_hint`. `Up`/`Down` select, `Tab` completes the name, `Enter` runs the selected command (or sends the text when nothing matches), `Esc` closes the popup and keeps the text. Commands with arguments (`/compact focus text`, `/mode plan`) work: once a space is typed the popup closes and `Enter` runs the command with the argument.
4. `Ctrl+P` opens the palette: a `Picker` over all commands, grouped (`Session`, `Model`, `View`, `Help`, `Prompts`, `Skills`), with the key hint as the detail. `Enter` runs the action and closes the palette.
5. Help (`?` on an empty input, `/help`): two columns, generated from the `KeyMap` and the command table, never hand-written.
Tests: registry rules above; slash menu state (filtering, selection, completion, argument case, Esc); replay `ctrl+p m o d e enter` finds the `/mode` action; snapshots: slash menu at 80x24, palette at 80x24 and 60x20, help at 80x24.
Done when: tests pass and the manual check passes (type `/`, filter, complete with Tab; open the palette and run a command; `?` lists every key from section 1.3 that exists so far).

---

## STAGE T4: Connect a provider and pick a model from the TUI (the headline feature)

PASTE:
Goal: a new user runs `arqus`, picks a provider from a list, pastes a key, sees it validated, picks a model, and starts chatting. No environment variables, no config files.
First: copy `auth.rs`, `catalog.rs` and `models.rs` from the starter kit into `src/` (with their tests) and keep the tests green. Read them; do not rewrite them. They define `AuthStore` (`<ARQUS_HOME>/auth.json`, mode 0600), `catalog::PROVIDERS`, `resolve_key` (env, then `ARQUS_API_KEY`, then saved), `validate_url`, `models_url`, `auth_headers`, `parse_models`, `model_detail`, `mask_key` and `clean_pasted_key`.
Do:
1. `state.rs`: `State { model: Option<{provider, id}>, recent: Vec<{provider, id}> (max 8, newest first), theme: Option<String>, base_urls: BTreeMap<String,String> }` stored in `<ARQUS_HOME>/state.json` with atomic save and tolerant load (a corrupt file is renamed to `state.json.bad-<ts>` and an empty state is used; never crash). Tests.
2. `config.rs`: a PURE `resolve_model(cli, env, state, settings) -> Result<ModelConfig, ConfigError>` implementing the order in section 1.5, building the `ModelConfig` from the provider's catalog entry, the base URL (state override, else catalog), and `resolve_key`. `ConfigError::NoProvider` and `ConfigError::NoModel { provider }` and `ConfigError::NoKey { provider }`. Use it in BOTH the TUI and print mode, so a key saved from the TUI also works for `arqus -p`. In print mode the error text is: `No provider connected. Run arqus and use /connect, or set OPENROUTER_API_KEY (or another provider's variable).` Exit code 2. Tests for every precedence rule and every error.
3. Overlays `Connect` and `Models`. The Connect state machine (a plain enum, so it can be tested):
   - `PickProvider`: a `Picker` over `PROVIDERS`, grouped `Recommended` / `Other`; the detail column shows `✓ connected`, `env`, `local` or the blurb; the current provider is marked.
     - A provider that is already connected opens a small picker: `Choose model`, `Replace key`, `Disconnect` (removes the saved key; for a key that comes from an environment variable say that it can only be removed by unsetting the variable).
     - A provider that needs a key goes to `EnterKey`.
     - A local provider (`ollama`, `lmstudio`) and `custom` go to `EnterBaseUrl` first (a text field prefilled with the default URL; `custom` starts empty), then `EnterKey` (optional for these: an empty key is allowed).
   - `EnterKey`: shows the provider name, where to create a key (the `key_url`; `Ctrl+O` runs `open <url>` on macOS and `xdg-open <url>` on Linux through `Effect::OpenUrl`), a masked field. Pasting goes through `clean_pasted_key`. `Enter` emits `Effect::ValidateKey { provider, key, base_url }` and shows `⠋ Checking key...`. The field is masked with bullets; `Ctrl+U` clears; `Esc` goes back to `PickProvider`.
   - Validation effect: an HTTP GET to `validate_url` with `auth_headers`, 10 second timeout. Result `Msg::KeyChecked(Result<KeyInfo, KeyError>)`. `401` or `403`: stay in the dialog with `Key rejected by <name> (401). Check it and paste again.` Network error or timeout: show `Could not reach <host>: <reason>` and offer `y` to save the key anyway or `n` to edit it. For OpenRouter, read `data.limit_remaining` and `data.limit` from the answer and show `✓ Key OK · limit remaining $X` (or `no spending limit` when null).
   - On success emit `Effect::SaveAuth { provider, key }` (written through `AuthStore::save`), toast `Connected to <name>`, then `Effect::FetchModels { provider }` and open the `Models` overlay in its loading state.
4. The `Models` overlay (`/models`, `Ctrl+L`): a `Picker` titled `Select model · <provider>` with the status line `Loading models...`. `Effect::FetchModels` does a GET on `models_url` (15 second timeout; for OpenRouter ask server-side for tool-capable models with the `tools_only` flag), parses with `parse_models`, and sends `Msg::ModelsLoaded(Result<Vec<ModelInfo>, String>)`. Rows: label = model id, detail = `model_detail`. Groups: `Recent` (from `state.recent`, only this provider) then `All models`, sorted by id. The current model has the `✓`. `Tab` toggles between `tool-capable only` and `all` (filter client-side where the provider reports `supported_parameters`; when it reports nothing, show all and hide the toggle). If the list cannot be fetched, show the error in the status line and let the user type a model id: when the query does not exactly match any model, add a last row `Use "<query>" as model id`. `Enter` emits `Effect::SetModel { provider, id }`, which updates the `Runtime` (reuse `switch_model`), writes `state.json` (model and recent), and shows the toast `Model: <id>`.
   - `F2` cycles through `state.recent` (all providers) with the same effect.
   - Switching to a provider with no key opens the Connect dialog first.
5. First-run behavior: at startup, if `resolve_model` fails with `NoProvider`, open `Connect` automatically with the heading line `Welcome to arqus. Connect a provider to start.` (`Esc` closes it; the welcome screen then shows `no provider connected · press ctrl+p, or type /connect`). If a provider is connected but there is no model (`NoModel`), open the `Models` overlay automatically. Sending a prompt with no provider shows a toast and opens `Connect`.
6. The footer and the welcome line show the active model; `Msg::ModelChanged` updates them without restarting the session. A `model_change` entry is appended to the session (the engine already supports it).
Tests (key replay with a fake effect executor that records effects and feeds back scripted messages):
   - fresh state: `Connect` opens by itself; typing `open` then Enter selects OpenRouter and shows the key dialog;
   - `paste:  sk-or-v1-TESTKEY123456  ` then Enter emits `ValidateKey` with the CLEANED key; feeding `KeyChecked(Ok)` emits `SaveAuth` then `FetchModels`; feeding `ModelsLoaded(Ok(list))` shows the picker with the list; selecting a row emits `SetModel`;
   - a 401 keeps the dialog open with the error and does not emit `SaveAuth`;
   - a network error offers `y` and `n`;
   - disconnect removes the key; an environment-variable key shows `env` and cannot be removed;
   - the typed-model-id row appears only when nothing matches exactly;
   - SECRET TEST: after the whole flow, none of the snapshots, toasts or log lines contain the test key; the dialog snapshot shows only bullets.
   - snapshots at 80x24: provider list, key dialog (empty and masked), validating, error, models loading, models loaded, models error with typed-id row.
   - `config.rs` precedence tests; `state.json` corrupt-file test.
Done when: tests pass and this manual check passes on the Mac: `ARQUS_HOME=$(mktemp -d) arqus`, connect OpenRouter with a REAL key (the one with the spending limit), see it validated with the remaining limit, pick a model, send "say hi", then quit and start `arqus` again: it opens straight into the chat with the same model and no dialogs. Also `arqus -p "say hi"` works with the saved key and no environment variables.
Don't: show, log or snapshot the key; add OAuth or browser login; fetch models at startup (only when the dialog opens).

---

## STAGE T5: Transcript rendering (blocks, tool cards, diffs, markdown, collapsing)

PASTE:
Goal: the chat view from the mockup in section 1.2. Everything is a `Block` and each block knows how to draw itself.
Do:
1. `enum Block { User(String), Assistant { text: String, streaming: bool }, Thinking { text: String }, Tool { id, name, summary, state: Running | Ok | Err, output: String, started: Instant, elapsed: Option<Duration>, args: Value }, Notice(String, Level), Compaction { tokens_before: u64 } }` built from `AgentEvent`s in `update`. Replaying a loaded session (resume) builds the same blocks from entries.
2. Rendering returns `Vec<Line>` for a given width and theme, with a CACHE per block keyed by `(width, content length, expanded flags)`. Streaming blocks re-render only the last block. Wrap with `wrap_text` from the stage 19 spec (display width aware); if it does not exist yet, write it now with tests (plain, long word, CJK, emoji, tabs, empty lines, width 1).
3. User blocks: a `┃` bar in the user color on every line. Assistant text: plain with markdown-lite: `**bold**`, `*italic*`, `` `code` `` (code color), fenced code blocks drawn with a left gutter `│` in dim and the code color (an unclosed fence while streaming counts as open), `#` headings bold in the accent color, `-` and `*` bullets shown as `•`, numbered lists unchanged, `>` quotes dim. No tables, no links (show the URL as plain text). Write the markdown parser as a PURE function `parse_inline(&str) -> Vec<Span>` and `parse_blocks(&str) -> Vec<MdBlock>` with tests, including half-finished input such as `**bol` and a fence that is not closed yet.
4. Tool cards: header `● <name>  <summary>` with the state icon at the right edge (`⠋` while running, `✓` or `✗` plus elapsed seconds after). Summaries: `read src/a.rs (lines 1-200)`, `write path (42 lines)`, `edit path (-3 +4)`, `bash <first line of the command, truncated>`. Output below the header with a `│` gutter in dim: the first 5 lines, then `… +N lines (ctrl+o to expand)`. `Ctrl+O` toggles the expanded state of ALL tool blocks (and shows full output up to 200 lines, then a final truncation line). Errors are drawn in the error color.
5. `edit` cards show a diff: the removed lines with `-` in `diff_del` and the added lines with `+` in `diff_add`, at most 12 lines each, then `… +N more`. `write` cards show the line count and the first 8 lines dim.
6. Thinking blocks: one dim line `▸ Thinking... (ctrl+t)` when hidden; with `Ctrl+T` the text is shown dim italic. The state is remembered (`state.json`, key `show_thinking`).
7. Scrolling: follow mode while streaming; `PageUp` leaves follow mode and shows `scrolled · End to follow` in the footer; `End`, a new prompt, or `Ctrl+End` follows again; `Home` goes to the top. Keep the cache valid across scrolling.
Tests: markdown parser; block building from a scripted list of `AgentEvent`s; the summaries; the collapse rules (5 lines, expand, 200 limit); the diff drawing; cache invalidation on resize; snapshots at 80x24 for a chat with a user message, markdown answer with a code block, a running tool, a finished bash tool with long output (collapsed and expanded), an edit diff, a thinking block (hidden and shown), an error tool.
Done when: tests pass and the manual check passes with a real run (ask it to fix a failing test in a scratch folder).

---

## STAGE T6: Editor (multi-line, history, paste, @ files, ! shell, queue)

PASTE:
Goal: the prompt box feels like a good text field. If `InputState` from the earlier stage 20 exists, extend it; otherwise create it as a pure struct that stores `Vec<char>` and a cursor index (never byte indexes).
Do:
1. Editing: `Left` `Right` `Home` `End` (also `Ctrl+A`, `Ctrl+E`), `Backspace`, `Delete`, `Ctrl+W` (word), `Ctrl+U`, `Ctrl+K`, `Ctrl+Y` (paste the last deleted text). `Alt+Enter` and `Ctrl+J` insert a newline; `Enter` sends. The box grows to 6 lines then scrolls. The real terminal cursor is placed at the edit position (check the installed ratatui for the cursor call). The border color follows the mode (`ask` = border, `auto` = warning, `plan` = accent).
2. Bracketed paste: insert exactly, never send. Large pastes (over 2000 characters) are kept in full but shown as one dim line `[pasted 4,210 characters]` inside the editor and expanded again on send.
3. History: `Up`/`Down` when no menu is open and the cursor is on the first or last line; stored in `<ARQUS_HOME>/history.jsonl` (500 entries, no consecutive duplicates). `Ctrl+R` opens a `Picker` over the history (newest first), `Enter` puts the text in the editor.
4. `@` mentions: when the word at the cursor starts with `@`, show a popup `Picker` of project files. The file list comes from `Effect::ScanFiles` at startup: `git ls-files` inside a git repo, otherwise a directory walk that skips hidden folders, `target`, `node_modules` and `.git`, capped at 20,000 files. `Tab` or `Enter` inserts `@path ` and closes the popup. On send, the text stays as typed, and a block is appended to the message sent to the model: for each mentioned file under 8 KB (max 3 files, 20 KB in total) a `<file path="...">...</file>` block with its contents; for larger files only `(file mentioned: path, N lines; use the read tool)`.
5. `!command` at the start of the input: run it yourself right away through the bash tool's code (no approval, you typed it), show it as a tool card, and add ONE user message to the session: `I ran \`command\` locally. Output:` followed by the output in a code block (truncated like the bash tool). It does not start the model.
6. Queue (steering): while a run is active `Enter` queues the text; queued items show above the editor as dim `queued: ...` lines. `Esc` interrupts and puts the queued text back into the editor (joined by newlines) instead of discarding it. `Alt+Up` takes the last queued item back into the editor. The agent consumes the queue as specified in stage 21 (at the start of each loop round, and as follow-up turns after the run); if stage 21 was skipped, implement that part now with an `UnboundedReceiver<String>` passed to `run_prompt`.
Tests: `InputState` (insert and delete in the middle, emoji and CJK, `Ctrl+W` on punctuation and spaces, multi-line moves, undo of `Ctrl+K` with `Ctrl+Y`); history navigation keeps the draft; the file scanner on a temp tree; `@` token detection at the cursor; mention expansion limits; the `!` message format; queue behavior with the mock provider; snapshots: editor with 1 line, 6 lines, 9 lines, the `@` popup, the history picker, the queue.
Done when: tests pass and the manual check passes (paste 20 lines; edit the middle; `@` a file; `!ls`; queue two messages during a slow command).
Mac note to show once in a toast the first time `Alt+Enter` is not received: set Option as Meta in the terminal; `Ctrl+J` always works.

---

## STAGE T7: Sessions in the TUI

PASTE:
Goal: manage sessions without leaving the TUI.
Do:
1. `/sessions` (alias `/resume`): `Effect::LoadSessions` lists the files of this project folder in a background task reading only the first lines of each file: a title (the session name if set, else the first user message cut to 60 characters), the modified time, and the message count (count lines; do not parse the whole file). Picker rows: `<title>` with the detail `2h ago · 14 msgs`; sorted newest first; the current session is marked. `Enter` switches: replay the session into blocks (T5) and continue it. `Ctrl+D` in the picker deletes after a `y/n` confirm (moves the file to a `trash/` folder inside the sessions folder, never a hard delete).
2. `/new` (alias `/clear`): a new session file; keeps the model and mode. `/rename <name>`: appends a `custom` entry `session_name` and updates the title (the title is read from the last such entry).
3. `/tree`: a `Picker` of the entries of the current session indented by depth (two spaces per level, max 8 levels shown then `…`), each row `role: first 60 characters` with `*` on the current path. `Enter` moves the leaf and rebuilds the transcript from that point. `Esc Esc` on an empty idle input opens the same tree (rewind).
4. `/fork`: copies the current path into a new session file, switches to it, and toasts `Forked`.
5. Startup flags: `arqus -c` continues the latest session of this folder; `arqus -r` opens the sessions picker at startup; with no flag a fresh session starts.
6. `/export [file]`: writes the visible conversation as Markdown (default `arqus-session-<date>.md` in the project folder) and toasts the path.
7. `/copy`: copies the last assistant message (see T10 for the clipboard helper; for now use the same helper and toast `Copied` or the error).
Tests: title extraction (first message, renamed, empty session); the fast file reader on a 10,000-line file finishes in well under a second; tree rows and depth; leaf move rebuilds blocks; fork keeps only the path; export format; snapshots: sessions picker, tree picker (with a branch), rename toast.
Done when: tests pass and the manual check passes (create two sessions, switch, branch with the tree, fork, resume with `-c`).

---

## STAGE T8: Modes and approvals in the TUI

PASTE:
Goal: safe by default, one key to change how much the agent may do. If stage 22's engine part (the `Approver`, `check_path`, the risky-command matcher, the git notices) exists, reuse it; if not, implement exactly the behavior in stage 22 points 1 to 4 now.
Do:
1. Modes: `ask` (default), `auto`, `plan`. `Shift+Tab` and `/mode [ask|auto|plan]` cycle or set. The chip in the footer and the editor border color show the mode. Switching to `auto` for the first time in a session shows a warning toast: `auto: tools run without asking. Use a git branch or the Docker sandbox.` The mode is not remembered: every launch starts in `ask` unless `--yolo` or the setting `approval: "auto"` is given.
2. `plan` mode: `write`, `edit` and `bash` return the tool result `Plan mode: changes are disabled. Describe the plan instead.` and the system prompt gets one extra line saying the same. `read` works.
3. Approval dialog (the mockup in section 1.2): `AgentEvent::ApprovalRequest { id, tool, preview }` opens the `Approval` overlay (it cannot be closed by anything but a decision). Preview: bash shows the full command (wrapped, max 10 lines); write shows the path and the first 20 lines; edit shows the path and the removed and added lines. A risky command shows a red `RISKY` marker. Keys: `y` allow once, `a` allow this tool until the session ends, `n` deny, `Esc` deny and interrupt the run. The decision goes to the engine through the approval channel as a `Decision`. A denied call returns `The user denied this tool call.` to the model.
4. While waiting for a decision the running line says `waiting for your approval`, and the terminal bell rings once (`\x07`) if the window was idle for 10 seconds or more.
5. Startup notices from stage 22 point 5 (not a git repo, uncommitted changes) appear as toasts.
Tests: mode cycling order; `plan` results; approval flow with the mock provider through the whole TUI update path (request, `y`, tool runs; request, `n`, denied text; `a` remembered per tool; `Esc` interrupts); the risky marker; snapshots for the approval dialog (bash, write, edit, risky) and for the three footer chips.
Done when: tests pass and the manual check passes (ask for a file edit in `ask` mode and approve; deny a bash command; try `plan`; `Shift+Tab` through the modes).

---

## STAGE T9: Themes, settings dialog, key overrides

PASTE:
Goal: personal taste without editing files.
Do:
1. `/theme`: a `Picker` over `dark`, `light`, `mono`, `dark (256 colors)`. Moving the selection previews the theme immediately; `Enter` keeps it and writes `state.json`; `Esc` restores the previous theme. A theme can also be a JSON file in `<ARQUS_HOME>/themes/<name>.json` with the semantic colors as `#rrggbb` strings; invalid files are skipped with a toast naming the file.
2. `/settings`: a `Picker` whose rows are settings with their current value as the detail; `Enter` cycles or toggles: theme, show thinking, expand tool output, mouse capture (default off), terminal bell on approval and on finished runs, default mode for new sessions (`ask` or `auto`), compaction on or off. All persist to `state.json`.
3. Key overrides: `settings.json` may contain `"keybinds": { "palette": "ctrl+k", "models": ["ctrl+l", "f3"], "newline": "alt+enter,ctrl+j" }` (action name, one or several keys). Use `parse_key`. Unknown action names and unparsable keys produce a toast at startup (`keybinds: unknown action "foo"`) and are ignored. Two actions on the same key: the later wins and a toast says so. `/help` shows the effective keys.
4. `NO_COLOR` forces `mono`; `COLORTERM` decides truecolor; on a 256-color terminal every theme color goes through `rgb_to_xterm256`.
Tests: theme file parsing (good, bad, missing fields use the dark defaults); preview and revert state machine; keybind merging (unknown action, conflicts, multi-key); snapshots of the theme picker and the settings picker; a test that scans the `view` sources for forbidden direct color use outside `theme.rs` (a simple `include_str!` search for `Color::Rgb(` and `Color::Red`).
Done when: tests pass and the manual check passes in Terminal.app (256 colors) and in a truecolor terminal (the themes look right in both).

---

## STAGE T10: Terminal robustness

PASTE:
Goal: it never leaves the terminal broken and behaves on every Mac and Linux terminal.
Do:
1. Restore the terminal on every exit path: normal quit, panic (a panic hook that restores, then prints the panic message after the restore), `SIGTERM` and `SIGHUP` (use `tokio::signal` on Unix to send `Msg::Quit`), and an error returned from `run()`. Add a test that the restore function is idempotent.
2. Resize: debounce resize events (apply only the last one per 50 ms), invalidate the render caches, keep the scroll position anchored to the same block.
3. Clipboard helper `copy_to_clipboard(text) -> Result<()>`: macOS `pbcopy`; Linux `wl-copy`, then `xclip -selection clipboard`, then `xsel -b`; if none works, fall back to the OSC 52 escape sequence (`ESC ] 52 ; c ; <base64> BEL`; write your own small base64 encoder with tests). `/copy` and a mouse-free way to copy code blocks: `Ctrl+Y` is taken by the editor, so add `/copy code` (last code block of the last answer).
4. Mouse: off by default (so text selection works). With `mouse` on in settings, enable mouse capture and map wheel events to scrolling; document that Shift-drag selects text in most terminals while capture is on.
5. Terminal title: set `arqus · <folder name>` with `ESC ] 0 ; ... BEL` at start and reset on exit.
6. Performance: no busy loop (the loop waits on events), redraw only when dirty, a 30 frames per second cap while streaming, and the transcript draws only the visible blocks. Add a test with a 5,000-block transcript that one `view` call finishes in under 100 ms in release mode or 1 second in debug.
7. Bell and notifications: when a run finishes while the terminal was idle for 10 seconds or more, ring the bell if the setting is on.
8. Edge cases: stdin closed (EOF) quits cleanly; a very long single line (100,000 characters) in a tool result does not freeze the UI (cut lines at 2000 characters when building blocks); invalid UTF-8 in tool output is shown with replacement characters; a terminal narrower than 40 columns shows the too-small message.
Tests: base64 and the OSC 52 string; the restore idempotence; the performance test; the long-line cut; resize debounce logic as a pure function.
Done when: tests pass and the manual check passes (kill the process with `kill -TERM` and check the terminal; resize rapidly; open a 100,000 character line; run with `TERM=xterm` and with `NO_COLOR=1`).

---

## STAGE T11: tmux smoke tests and the acceptance checklist

PASTE:
Goal: test the REAL program in a REAL terminal, from scripts, so you do not have to click through everything by hand. This needs `tmux` (`brew install tmux`). If tmux is missing, write the files, say so, and do not fail the stage.
Do:
1. A fake server you can run as a program: `examples/fake_llm.rs` that reuses `tests/support/mod.rs` through `#[path]`, takes a port and a JSON script file, and answers the scripted replies in order (`{"kind":"json","body":{...}}`, `{"kind":"text","pieces":["Hel","lo"]}`, `{"kind":"tool","name":"bash","args":{...}}`, `{"kind":"status","code":401,"body":"..."}`). It prints `listening on <port>` and runs until killed.
2. `scripts/tui-smoke.sh` (POSIX sh) with helpers `start`, `keys`, `type`, `wait_for "text" [seconds]`, `screen`, `stop` built on `tmux new-session -d -x 100 -y 30`, `tmux send-keys`, and `tmux capture-pane -p`. Every scenario uses a temp `ARQUS_HOME`, a temp project folder, and the fake server; no real network.
3. Scenarios (each prints PASS or FAIL with the last screen on failure; the script exits non-zero on any FAIL):
   a. First run: the screen contains `Connect a provider`; typing `open` filters the list; `Esc` closes it and the welcome screen says `no provider connected`.
   b. Connect: choose OpenRouter, paste `test-key-abc123`, the fake server answers the key check (script a `/key` JSON with `limit_remaining`) and the models list (script a `/models` JSON with 3 models, one with tools); the screen shows `Key OK`, then the model picker lists the models; pick one; the footer shows the model id.
   c. Chat: send `hello`; the fake streams a reply; the screen shows it and `ctx`.
   d. Tool call and approval: the fake returns a `bash` call `echo smoke > out.txt`; in `ask` mode the approval dialog appears; `y`; `out.txt` exists in the project folder; the card shows `✓`.
   e. Deny: same, press `n`; the file does not exist.
   f. Slash menu and palette: type `/mo` and see `/models`; `Ctrl+P` shows the palette; `Esc`.
   g. Quit and restore: `Ctrl+D` on an empty input exits; the tmux pane is back at the shell prompt; the secret `test-key-abc123` appears nowhere on the captured screens and not in `debug.log`.
   h. Restart: start again with the same `ARQUS_HOME`: it opens in the chat view with the saved model and no dialog.
4. `docs/TUI_ACCEPTANCE.md`: a checklist for the human for what tmux cannot judge (colors in Terminal.app and in a truecolor terminal, glyph widths, feel of scrolling, Option-as-Meta keys), with one line per item.
Done when: `scripts/tui-smoke.sh` passes all scenarios on the Mac (or on the machine the human uses) and the acceptance file exists. The human runs the acceptance checklist once.

---

# PART 4: THE GATE FOR "THE TUI IS GOOD ENOUGH"

- [ ] `cargo test` passes, including every snapshot, with no key and no network.
- [ ] `scripts/tui-smoke.sh` passes all scenarios.
- [ ] A new user can go from `arqus` to a working chat in under a minute using only the dialogs: no environment variables, no config files, no commands except `arqus`.
- [ ] The key never appears on screen, in snapshots, in `debug.log`, or in git; `auth.json` has mode 600.
- [ ] A saved key and model also work for `arqus -p`.
- [ ] Footer always shows model, context percent, cost (when known), mode, folder and branch; at 60 columns it degrades without breaking.
- [ ] Long answers wrap and scroll; tool output collapses and expands; diffs are readable; the running line shows elapsed time and the interrupt hint.
- [ ] `/` menu, `Ctrl+P` palette and `?` help list the same commands; the keys match section 1.3.
- [ ] Approvals work and `Shift+Tab` cycles modes; plan mode blocks changes.
- [ ] Resize, `kill -TERM`, a panic and a closed stdin all leave the terminal usable.
- [ ] Works in Terminal.app (256 colors) and in a truecolor terminal; `NO_COLOR` gives a readable mono theme.

---

# PART 5: MAC AND DOCKER NOTES FOR THE NEW TUI

- **Terminals:** Terminal.app works but has only 256 colors (arqus detects this). iTerm2, Ghostty, WezTerm and kitty give truecolor. For `Alt+Enter` and `Alt+arrows` set Option as Meta (Terminal.app: Profiles, Keyboard; iTerm2: Profiles, Keys, Left Option key to `Esc+`). `Ctrl+J` always inserts a newline.
- **Docker:** the TUI needs a TTY: the `arqus-docker` wrapper already uses `-it` when it is attached to a terminal. The Connect dialog saves `auth.json` inside the mounted state folder (`~/.arqus-docker`, mode 0600), so you connect once and it stays. For a throwaway key, pass `OPENROUTER_API_KEY` in the environment instead. The container has no `open` or `xdg-open`, so `Ctrl+O` in the key dialog cannot open a browser there: the URL helper must fall back to a toast that shows the URL when the command fails.
- **Keychain:** `auth.json` is a plain file with mode 0600, like opencode and pi. If you prefer the macOS Keychain, keep using the `security` command from the earlier kit and export `OPENROUTER_API_KEY` in `~/.zshrc`; the environment variable wins over the saved key.

# PART 6: NOT NOW (opencode and Claude Code have these; they are not in this plan)

A sidebar with files and todos, multiple agents and subagents, session sharing, MCP servers, images and drag-and-drop, a desktop app or web UI, a built-in diff review screen, OAuth login for subscriptions, and `/undo` of file edits (use git). Add them later as separate stages; the architecture of Part 2 makes each one a new overlay plus a few messages and effects.
