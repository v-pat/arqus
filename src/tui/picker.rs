//! The `/resume` picker state.

use std::path::PathBuf;

use crate::session::list_sessions;

use super::app::App;
use super::layout::resume_time;

#[derive(Debug, Clone, Default)]
pub struct Resume {
    pub paths: Vec<PathBuf>,
    /// Modified time per path, formatted for display.
    pub times: Vec<String>,
    pub selected: usize,
}

impl App {
    /// Open the picker for the sessions in the current session folder.
    pub fn start_resume(&mut self) -> bool {
        let paths = list_sessions(&self.session_dir);
        if paths.is_empty() {
            self.info("no sessions for this folder yet");
            return false;
        }
        let times = paths.iter().map(|p| resume_time(p)).collect();
        self.resume = Some(Resume {
            paths,
            times,
            selected: 0,
        });
        true
    }

    pub fn move_resume(&mut self, delta: isize) {
        if let Some(resume) = &mut self.resume {
            let last = resume.paths.len().saturating_sub(1);
            let next = resume.selected as isize + delta;
            resume.selected = next.clamp(0, last as isize) as usize;
        }
    }

    pub fn pick_resume(&mut self) -> Option<std::path::PathBuf> {
        let resume = self.resume.take()?;
        resume.paths.get(resume.selected).cloned()
    }

    pub fn cancel_resume(&mut self) {
        self.resume = None;
    }
}
