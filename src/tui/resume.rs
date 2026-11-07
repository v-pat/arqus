//! The `/resume` picker: session files in the current project folder.

use std::path::PathBuf;

use crate::session::list_sessions;

use super::app::App;
use super::layout::resume_time;
use super::picker::{Item, PKey, Picker};

#[derive(Debug, Clone)]
pub struct Resume {
    pub picker: Picker<PathBuf>,
}

impl App {
    /// Open the picker for the sessions in the current session folder.
    pub fn start_resume(&mut self) -> bool {
        let paths = list_sessions(&self.session_dir);
        let sessions = paths
            .into_iter()
            .map(|path| {
                let time = resume_time(&path);
                (path, time)
            })
            .collect();
        self.show_resume(sessions)
    }

    /// Fill the resume picker with a list loaded by the effect executor.
    pub fn show_resume(&mut self, sessions: Vec<(PathBuf, String)>) -> bool {
        if sessions.is_empty() {
            self.info("no sessions for this folder yet");
            return false;
        }
        let items = sessions
            .into_iter()
            .map(|(path, time)| {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                Item::new(name, path).detail(time)
            })
            .collect();
        self.resume = Some(Resume {
            picker: Picker::new("resume", items),
        });
        true
    }

    pub fn move_resume(&mut self, delta: isize) {
        if let Some(resume) = &mut self.resume {
            let key = if delta < 0 { PKey::Up } else { PKey::Down };
            resume.picker.handle_key(key);
        }
    }

    pub fn pick_resume(&mut self) -> Option<PathBuf> {
        let resume = self.resume.take()?;
        resume.picker.selected_item().map(|item| item.value.clone())
    }

    pub fn cancel_resume(&mut self) {
        self.resume = None;
    }
}
