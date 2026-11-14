//! Wiring for stage 22: choose the approver and build the path guard.

use std::path::Path;

use crate::home;
use crate::safety::{self, Approver, Guard};
use crate::settings::{ApprovalMode, Settings};

/// Resolve execution mode from CLI flags and settings into (approver, plan).
pub fn resolve_mode(
    mode_flag: Option<&str>,
    yolo: bool,
    settings: &Settings,
    interactive: bool,
) -> Result<(Approver, bool), String> {
    let mode_str = mode_flag.map(|s| s.trim().to_ascii_lowercase());
    match mode_str.as_deref() {
        Some("auto") => Ok((Approver::Auto, false)),
        Some("plan") => {
            let approver = if interactive {
                Approver::Interactive
            } else {
                Approver::DenyAll
            };
            Ok((approver, true))
        }
        Some("ask") => {
            let approver = if interactive {
                Approver::Interactive
            } else {
                Approver::DenyAll
            };
            Ok((approver, false))
        }
        Some(other) => Err(format!(
            "unknown mode '{other}': expected ask, auto, or plan"
        )),
        None => {
            if yolo || settings.approval == Some(ApprovalMode::Auto) {
                Ok((Approver::Auto, false))
            } else if settings.approval == Some(ApprovalMode::Plan) {
                let approver = if interactive {
                    Approver::Interactive
                } else {
                    Approver::DenyAll
                };
                Ok((approver, true))
            } else if interactive {
                Ok((Approver::Interactive, false))
            } else {
                Ok((Approver::DenyAll, false))
            }
        }
    }
}

/// Auto with `--yolo` or `approval = "auto"`. Otherwise ask in the TUI and
/// refuse in print mode, where nobody can answer.
pub fn approver(yolo: bool, settings: &Settings, interactive: bool) -> Approver {
    resolve_mode(None, yolo, settings, interactive)
        .map(|(a, _)| a)
        .unwrap_or(Approver::Interactive)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_mode_priority_and_fallbacks() {
        let settings = Settings::default();
        assert_eq!(
            resolve_mode(Some("auto"), false, &settings, true).unwrap(),
            (Approver::Auto, false)
        );
        assert_eq!(
            resolve_mode(Some("plan"), false, &settings, true).unwrap(),
            (Approver::Interactive, true)
        );
        assert_eq!(
            resolve_mode(Some("ask"), false, &settings, true).unwrap(),
            (Approver::Interactive, false)
        );
        assert!(resolve_mode(Some("invalid"), false, &settings, true).is_err());

        let auto_settings = Settings {
            approval: Some(ApprovalMode::Auto),
            ..Default::default()
        };
        assert_eq!(
            resolve_mode(None, false, &auto_settings, true).unwrap(),
            (Approver::Auto, false)
        );

        let plan_settings = Settings {
            approval: Some(ApprovalMode::Plan),
            ..Default::default()
        };
        assert_eq!(
            resolve_mode(None, false, &plan_settings, true).unwrap(),
            (Approver::Interactive, true)
        );
        assert_eq!(
            resolve_mode(Some("ask"), false, &plan_settings, true).unwrap(),
            (Approver::Interactive, false)
        );
    }
}
