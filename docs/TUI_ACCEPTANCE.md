# TUI acceptance checklist

Things `scripts/tui-smoke.sh` cannot judge. Run these once by hand on the Mac.

- [ ] Start `arqus` in Terminal.app: the dark theme is readable and no color looks black-on-black.
- [ ] Start `arqus` in a truecolor terminal (iTerm2, Ghostty, WezTerm or kitty): the same screen shows smooth RGB colors, not banded 256-color steps.
- [ ] Run `NO_COLOR=1 arqus`: the mono theme is readable, with bold or inverse text standing in for color.
- [ ] Check glyph widths in the footer, tool cards, spinner and dialog borders: no character overlaps the next cell or leaves a gap.
- [ ] Type a long line (over 200 columns) in the editor: it wraps or scrolls without hiding the cursor.
- [ ] Paste a multi-line block with `Cmd+V`: the paste marker appears, and `Enter` sends the whole block.
- [ ] Scroll a long transcript with the mouse wheel and with `PageUp`/`PageDown`: the speed feels right and `End` returns to the newest line.
- [ ] Set Option as Meta in Terminal.app (Profiles, Keyboard) and in iTerm2 (Profiles, Keys, Left Option key to `Esc+`): `Alt+Enter` inserts a newline and `Alt+Left`/`Alt+Right` move by word.
- [ ] Press `Ctrl+J`: it always inserts a newline, whatever the Option setting is.
- [ ] Resize the window while a reply streams: the layout settles without flicker and the same block stays at the top.
- [ ] Press `Ctrl+C` once on an empty idle prompt, then again: the first press warns, the second restores the shell.
- [ ] Send `kill -TERM` to arqus: the shell prompt returns with echo and the cursor visible.
- [ ] Open the Connect dialog and paste a real key: the screen shows only bullets, and the key never appears in `~/.arqus/debug.log`.
- [ ] Approve a `bash` command in `ask` mode: the dialog is readable at 80x24 and 60x20, and the tool card shows `✓` when it finishes.
- [ ] Switch the theme to `light` in Terminal.app: every dialog and the footer stay readable.
