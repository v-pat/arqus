//! Terminal setup and teardown that must run on every exit path.
//!
//! The guard is a plain flag so `restore()` can be tested without a terminal.
//! All escape writes live in `finish`, `set_title` and the panic hook.

use std::io::{stdout, Write};
use std::path::Path;

use ratatui::crossterm::event::{DisableBracketedPaste, DisableMouseCapture};
use ratatui::crossterm::execute;

/// Remembers whether the terminal was already put back.
#[derive(Debug, Clone, Default)]
pub struct TerminalGuard {
    restored: bool,
    paste: bool,
    mouse: bool,
}

impl TerminalGuard {
    pub fn new(paste: bool, mouse: bool) -> TerminalGuard {
        TerminalGuard {
            restored: false,
            paste,
            mouse,
        }
    }

    /// Flip the flag. True only the first time, so a normal exit, a panic and
    /// an error path can all call it without fighting.
    pub fn restore(&mut self) -> bool {
        if self.restored {
            return false;
        }
        self.restored = true;
        true
    }

    pub fn restored(&self) -> bool {
        self.restored
    }

    /// Restore the real terminal once. Later calls do nothing.
    pub fn finish(&mut self) {
        if !self.restore() {
            return;
        }
        stop_terminal(self.paste, self.mouse);
    }
}

/// Disable every mode we turned on, reset the title, then leave the
/// alternate screen and raw mode.
pub fn stop_terminal(paste: bool, mouse: bool) {
    if paste {
        let _ = execute!(stdout(), DisableBracketedPaste);
    }
    if mouse {
        let _ = execute!(stdout(), DisableMouseCapture);
    }
    let _ = write_stdout(reset_title());
    ratatui::restore();
}

/// `ESC ] 0 ; arqus · <folder> BEL`.
pub fn title(folder: &str) -> String {
    format!("\x1b]0;arqus · {folder}\x07")
}

pub fn reset_title() -> &'static str {
    "\x1b]0;\x07"
}

/// Folder name for the title bar, or `arqus` when the path has no name.
pub fn folder_name(cwd: &str) -> String {
    Path::new(cwd)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "arqus".to_string())
}

pub fn set_title(cwd: &str) {
    let _ = write_stdout(&title(&folder_name(cwd)));
}

/// Capture or release the mouse. Off at startup by default so drag-to-select
/// keeps working in the terminal.
pub fn set_mouse(on: bool) {
    if on {
        let _ = execute!(stdout(), ratatui::crossterm::event::EnableMouseCapture);
    } else {
        let _ = execute!(stdout(), DisableMouseCapture);
    }
}

/// Bring back the terminal before the default hook prints the panic message.
pub fn install_panic_hook(paste: bool, mouse: bool) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        stop_terminal(paste, mouse);
        previous(info);
    }));
}

fn write_stdout(text: &str) -> std::io::Result<()> {
    let mut out = stdout();
    out.write_all(text.as_bytes())?;
    out.flush()
}

#[cfg(test)]
#[path = "terminal_tests.rs"]
mod tests;
