//! The keyboard map. One key press in, one state change out.

use std::path::PathBuf;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::agent::{AgentEvent, Runtime};
use crate::resources::Resources;
use crate::safety::Decision;
use crate::session::Session;

use super::app::App;
use super::commands::submit;
use super::tasks::{abort_run, open_session, refresh_tokens};

/// One key press. Returns true when the app should quit.
#[allow(clippy::too_many_arguments)]
pub(super) fn handle_key(
    key: KeyEvent,
    app: &mut App,
    runtime: &mut Runtime,
    resources: &Resources,
    event_tx: &mpsc::Sender<AgentEvent>,
    done_tx: &mpsc::Sender<Session>,
    task: &mut Option<JoinHandle<()>>,
    active_path: &mut Option<PathBuf>,
    event_rx: &mut mpsc::Receiver<AgentEvent>,
) -> bool {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);

    // The approval modal owns the keyboard while it is open.
    if app.approval.is_some() {
        match key.code {
            KeyCode::Char('y') => {
                app.answer_approval(Decision::Allow);
            }
            KeyCode::Char('a') => {
                app.answer_approval(Decision::AllowAlways);
            }
            KeyCode::Char('n') => {
                if app.answer_approval(Decision::Deny) {
                    app.info("denied tool call");
                }
            }
            KeyCode::Esc => {
                app.answer_approval(Decision::Deny);
                abort_run(app, task, active_path, event_rx);
                refresh_tokens(app, runtime);
            }
            KeyCode::Char('c') if control => {
                app.answer_approval(Decision::Deny);
                abort_run(app, task, active_path, event_rx);
                refresh_tokens(app, runtime);
            }
            _ => {}
        }
        return false;
    }

    match key.code {
        KeyCode::Char('c') if control => {
            if app.running {
                let dropped = app.clear_queue();
                abort_run(app, task, active_path, event_rx);
                if dropped > 0 {
                    app.info(format!("aborted; dropped {dropped} queued message(s)"));
                }
                refresh_tokens(app, runtime);
            } else if !app.input.is_empty() {
                app.input.clear();
            } else {
                return app.ctrl_c_idle();
            }
        }
        KeyCode::Char('d') if control => {
            if app.input.is_empty() {
                return true;
            }
            app.input.delete();
        }
        KeyCode::Char('j') if control => app.input.insert_newline(),
        KeyCode::Enter if alt => app.input.insert_newline(),
        KeyCode::Enter => {
            if app.tree.is_some() {
                if let Some(path) = app.select_tree() {
                    app.info(format!("path: {path}"));
                }
            } else if app.resume.is_some() {
                if let Some(path) = app.pick_resume() {
                    open_session(app, &path, runtime);
                }
            } else if app.running {
                let text = app.take_input().trim().to_string();
                if !text.is_empty() {
                    app.remember_prompt(&text);
                    if app.queue_prompt(&text) {
                        app.status = format!("queued ({} waiting)", app.queue.len());
                    } else {
                        app.info("cannot queue: no active run");
                    }
                }
            } else {
                submit(
                    app,
                    runtime,
                    resources,
                    event_tx,
                    done_tx,
                    task,
                    active_path,
                );
                if app.status == "quit" {
                    return true;
                }
            }
        }
        KeyCode::Char('a') if control => app.input.home(),
        KeyCode::Char('e') if control => app.input.end(),
        KeyCode::Char('w') if control => app.input.delete_word(),
        KeyCode::Char('u') if control => app.input.kill_to_start(),
        KeyCode::Char('k') if control => app.input.kill_to_end(),
        KeyCode::Backspace => app.input.backspace(),
        KeyCode::Delete => app.input.delete(),
        KeyCode::Tab if app.tree.is_none() && app.resume.is_none() => {
            app.input.tab_complete();
        }
        KeyCode::Left if alt => app.input.word_left(),
        KeyCode::Right if alt => app.input.word_right(),
        KeyCode::Left => app.input.move_left(),
        KeyCode::Right => app.input.move_right(),
        KeyCode::Home => {
            if app.input.is_empty() {
                app.scroll_top();
            } else {
                app.input.home();
            }
        }
        KeyCode::End => {
            if app.input.is_empty() {
                app.end_follow();
            } else {
                app.input.end();
            }
        }
        KeyCode::PageUp => app.scroll_up(app.page),
        KeyCode::PageDown => app.scroll_down(app.page),
        KeyCode::Up if app.tree.is_some() => app.move_tree(-1),
        KeyCode::Down if app.tree.is_some() => app.move_tree(1),
        KeyCode::Up if app.resume.is_some() => app.move_resume(-1),
        KeyCode::Down if app.resume.is_some() => app.move_resume(1),
        KeyCode::Up => {
            if app.input.cursor_line() == 0 {
                app.input.history_up();
            }
        }
        KeyCode::Down => {
            if app.input.cursor_line() + 1 == app.input.line_count() {
                app.input.history_down();
            }
        }
        KeyCode::Esc => {
            if app.tree.is_some() {
                app.cancel_tree();
            } else if app.resume.is_some() {
                app.cancel_resume();
            } else if app.running {
                let dropped = app.clear_queue();
                abort_run(app, task, active_path, event_rx);
                refresh_tokens(app, runtime);
                if dropped > 0 {
                    app.info(format!("aborted; dropped {dropped} queued message(s)"));
                } else {
                    app.info("aborted");
                }
            } else {
                app.input.clear();
            }
        }
        KeyCode::Char(c)
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            app.input.insert_char(c);
        }
        _ => {}
    }
    false
}
