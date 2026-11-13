//! Wiring for stage 22: choose the approver and build the path guard.

use std::path::Path;

use crate::home;
use crate::safety::{self, Approver, Guard};
use crate::settings::{ApprovalMode, Settings};

/// Auto with `--yolo` or `approval = "auto"`. Otherwise ask in the TUI and
/// refuse in print mode, where nobody can answer.
pub fn approver(yolo: bool, settings: &Settings, interactive: bool) -> Approver {
    if yolo || settings.approval == Some(ApprovalMode::Auto) {
        Approver::Auto
    } else if interactive {
        Approver::Interactive
    } else {
        Approver::DenyAll
    }
}

/// Path rules: project folder, settings allow_paths, arqus home, user home.
pub fn guard(settings: &Settings, cwd: &Path) -> Guard {
    let user_home = dirs::home_dir();
    Guard {
        cwd: cwd.to_path_buf(),
        allow_paths: safety::expand_allow_paths(&settings.allow_paths, cwd, user_home.as_deref()),
        arqus_home: home::arqus_home(),
        user_home,
    }
}
