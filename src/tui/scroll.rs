//! Scrolling, follow mode and the run clock.

use super::app::App;

impl App {
    pub fn set_page(&mut self, page: usize) {
        self.page = page.max(1);
        if self.follow {
            self.follow_bottom();
        }
    }

    /// Re-wrap the transcript when the terminal width changes.
    pub fn set_width(&mut self, width: usize) {
        if width == self.wrap_width {
            return;
        }
        self.wrap_width = width;
        self.refresh_render();
        if self.follow {
            self.follow_bottom();
        } else {
            self.scroll = self.scroll.min(self.bottom_scroll());
        }
    }

    pub fn scroll_up(&mut self, lines: usize) {
        self.follow = false;
        self.scroll = self.scroll.saturating_sub(lines);
    }

    pub fn scroll_down(&mut self, lines: usize) {
        let bottom = self.bottom_scroll();
        self.scroll = (self.scroll + lines).min(bottom);
        if self.scroll == bottom {
            self.follow = true;
        }
    }

    /// `Home`: jump to the oldest line and stop following.
    pub fn scroll_top(&mut self) {
        self.follow = false;
        self.scroll = 0;
    }

    /// `End` or a new prompt: jump to the newest line and follow again.
    pub fn end_follow(&mut self) {
        self.follow = true;
        self.follow_bottom();
    }

    /// One 100 ms tick: advance the spinner and the elapsed seconds.
    pub fn on_tick(&mut self) {
        self.ticks = self.ticks.wrapping_add(1);
        if self.running {
            self.elapsed_ticks = self.elapsed_ticks.saturating_add(1);
        }
        // The Ctrl+C quit warning lives for 2 seconds (20 ticks).
        if let Some(tick) = self.ctrl_c_tick {
            if self.ticks.saturating_sub(tick) > 20 {
                self.ctrl_c_tick = None;
                if self.status == "press Ctrl+C again to quit" {
                    self.status = "ready".to_string();
                }
            }
        }
        self.prune_toasts();
    }

    pub fn spinner(&self) -> &'static str {
        const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        FRAMES[(self.ticks % FRAMES.len() as u64) as usize]
    }

    pub fn elapsed_secs(&self) -> u64 {
        self.elapsed_ticks / 10
    }

    pub(super) fn bottom_scroll(&self) -> usize {
        self.wrapped.len().saturating_sub(self.page)
    }
    pub(super) fn follow_bottom(&mut self) {
        if self.follow {
            self.scroll = self.bottom_scroll();
        }
    }
}
