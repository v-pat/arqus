//! Slash-command dispatch. Pure: it changes `App` and returns `Effect` values.

use super::app::App;
use super::msg::Effect;
use super::overlay::help::HELP;

pub fn dispatch(app: &mut App, text: &str) -> Vec<Effect> {
    if let Some(rest) = text
        .strip_prefix("/compact")
        .filter(|_| text == "/compact" || text.starts_with("/compact "))
    {
        let focus = rest.trim();
        let focus = if focus.is_empty() {
            None
        } else {
            Some(focus.to_string())
        };
        app.push_user(text);
        app.running = true;
        app.elapsed_ticks = 0;
        app.status = "compacting".to_string();
        return vec![Effect::Compact(focus)];
    }
    match text {
        "/help" => app.info(HELP),
        "/tree" => {
            app.start_tree();
        }
        "/fork" => return vec![Effect::Fork],
        "/model" => app.info(format!("model: {}", app.model)),
        "/cost" => {
            let line = match app.session.as_ref() {
                Some(session) => {
                    crate::cost::totals_line(&crate::cost::session_totals(&session.entries))
                }
                None => "no session yet".to_string(),
            };
            app.info(line);
        }
        "/budget" => {
            app.info("checking budget...");
            return vec![Effect::Budget];
        }
        "/new" => return vec![Effect::NewSession],
        "/quit" | "/exit" => return vec![Effect::Quit],
        "/resume" => return vec![Effect::LoadSessions],
        _ if text.starts_with("/model ") => {
            let spec = text.trim_start_matches("/model").trim().to_string();
            return vec![Effect::SwitchModel(spec)];
        }
        _ => {
            app.push_user(text);
            app.running = true;
            app.elapsed_ticks = 0;
            app.status = "running".to_string();
            return vec![Effect::RunPrompt(text.to_string())];
        }
    }
    Vec::new()
}
