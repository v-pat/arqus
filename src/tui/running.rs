//! The `⠋ Working... 14s · esc to interrupt` line above the prompt box.

use super::app::App;

/// Turn an engine retry notice into the footer text.
///
/// Input:  `rate limited, retrying in 4s (attempt 2/4)`
/// Output: `Retrying in 4s (attempt 2/4)...`
pub fn retry_text(notice: &str) -> Option<String> {
    let start = notice.to_ascii_lowercase().find("retrying in")?;
    let rest = notice.get(start..)?;
    let mut chars = rest.chars();
    let head = chars.next()?.to_ascii_uppercase();
    let tail: String = chars.collect();
    let tail = tail.trim_end_matches('.');
    Some(format!("{head}{tail}..."))
}

/// The running line, or `None` when nothing is running. A retry notice wins.
pub fn line(app: &App) -> Option<String> {
    if !app.running {
        return None;
    }
    if app.approval.is_some() {
        return Some("waiting for your approval".to_string());
    }
    if let Some(notice) = &app.retry_notice {
        return Some(notice.clone());
    }
    Some(format!(
        "{} Working... {}s · esc to interrupt",
        app.spinner(),
        app.elapsed_secs()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::App;
    use std::path::PathBuf;

    fn app() -> App {
        App::new(
            PathBuf::from("/tmp"),
            "/tmp".to_string(),
            "test-model".to_string(),
            128_000,
        )
    }

    #[test]
    fn retry_notice_keeps_the_attempt_numbers() {
        assert_eq!(
            retry_text("rate limited, retrying in 4s (attempt 2/4)"),
            Some("Retrying in 4s (attempt 2/4)...".to_string())
        );
        assert_eq!(retry_text("all good"), None);
    }

    #[test]
    fn running_line_counts_seconds() {
        let mut app = app();
        assert_eq!(line(&app), None);
        app.running = true;
        for _ in 0..140 {
            app.on_tick();
        }
        assert_eq!(
            line(&app),
            Some(format!(
                "{} Working... 14s · esc to interrupt",
                app.spinner()
            ))
        );
    }

    #[test]
    fn retry_notice_replaces_the_working_text() {
        let mut app = app();
        app.running = true;
        app.retry_notice = retry_text("busy, retrying in 2s (attempt 1/4)");
        assert_eq!(
            line(&app),
            Some("Retrying in 2s (attempt 1/4)...".to_string())
        );
    }
}
