//! The `/sessions` picker: session files in the current project folder.

use std::path::{Path, PathBuf};

use super::app::App;
use super::picker::{Item, PKey, Picker};
use super::sessions::{self, SessionInfo};

#[derive(Debug, Clone)]
pub struct Resume {
    pub picker: Picker<PathBuf>,
    /// Path waiting for a `y`/`n` answer after Ctrl+D.
    pub confirm: Option<PathBuf>,
}

impl App {
    /// Open the picker for the sessions in the current session folder.
    pub fn start_resume(&mut self) -> bool {
        let infos = load(&self.session_dir);
        self.show_resume(infos)
    }

    /// Fill the sessions picker with a list loaded by the effect executor.
    pub fn show_resume(&mut self, sessions: Vec<SessionInfo>) -> bool {
        if sessions.is_empty() {
            self.resume = None;
            self.info("no sessions for this folder yet");
            return false;
        }
        let current = self.session.as_ref().map(|session| session.path.clone());
        let items = sessions
            .into_iter()
            .map(|info| {
                let is_current = current.as_deref() == Some(info.path.as_path());
                let detail = info.detail();
                Item::new(info.title, info.path)
                    .detail(detail)
                    .current(is_current)
            })
            .collect();
        self.resume = Some(Resume {
            picker: Picker::new("sessions", items),
            confirm: None,
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

    pub fn resume_confirming(&self) -> bool {
        self.resume
            .as_ref()
            .map(|resume| resume.confirm.is_some())
            .unwrap_or(false)
    }

    /// Ctrl+D: ask before moving the highlighted session to `trash/`.
    pub fn ask_delete_resume(&mut self) -> bool {
        let current = self.session.as_ref().map(|session| session.path.clone());
        let path = self
            .resume
            .as_ref()
            .and_then(|resume| resume.picker.selected_item())
            .map(|item| item.value.clone());
        let Some(path) = path else {
            return false;
        };
        if current.as_deref() == Some(path.as_path()) {
            self.toast("cannot delete the open session", super::msg::Level::Warn);
            return false;
        }
        if let Some(resume) = &mut self.resume {
            resume.confirm = Some(path);
            return true;
        }
        false
    }

    /// Answer the delete question. `Some(path)` means the caller must delete.
    pub fn answer_delete_resume(&mut self, yes: bool) -> Option<PathBuf> {
        let resume = self.resume.as_mut()?;
        let path = resume.confirm.take()?;
        if yes {
            Some(path)
        } else {
            None
        }
    }

    pub fn cancel_resume(&mut self) {
        self.resume = None;
    }
}

/// Read every session summary in `dir`, newest first.
pub fn load(dir: &Path) -> Vec<SessionInfo> {
    let mut infos: Vec<SessionInfo> = crate::session::list_sessions(dir)
        .iter()
        .filter_map(|path| sessions::read(path))
        .collect();
    infos.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| b.path.cmp(&a.path))
    });
    infos
}
