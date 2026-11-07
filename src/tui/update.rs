//! The pure update function: one `Msg` in, a changed `App` and `Vec<Effect>` out.
//!
//! No I/O, no await, no sleep. Everything that touches the world becomes an
//! `Effect` and runs in `tui::execute`.

use super::app::App;
use super::commands;
use super::keys::{Action, Key, KeyCodeX};
use super::msg::{Effect, Msg};
use super::overlay::{Overlay, OverlayOutcome};
use super::picker::PKey;
use crate::safety::Decision;

pub fn update(app: &mut App, msg: Msg) -> Vec<Effect> {
    app.dirty = true;
    match msg {
        Msg::Key(key) => on_key(app, key),
        Msg::Paste(text) => {
            if app.overlays.is_empty()
                && app.approval.is_none()
                && app.tree.is_none()
                && app.resume.is_none()
            {
                app.input.insert_text(&text);
            }
            Vec::new()
        }
        Msg::Resize(width, height) => {
            app.set_size(width, height);
            Vec::new()
        }
        Msg::Tick => {
            app.on_tick();
            Vec::new()
        }
        Msg::Agent(event) => {
            app.apply_event(event);
            Vec::new()
        }
        Msg::Done(session) => {
            app.set_session(session);
            app.running = false;
            app.status = "ready".to_string();
            app.steer_tx = None;
            app.approval = None;
            app.approval_tx = None;
            let dropped = app.clear_queue();
            if dropped > 0 {
                app.info(format!("dropped {dropped} queued message(s)"));
            }
            vec![Effect::RefreshTokens]
        }
        Msg::Toast(text, level) => {
            app.toast(text, level);
            Vec::new()
        }
        Msg::Sessions(sessions) => {
            app.show_resume(sessions);
            Vec::new()
        }
        Msg::Opened { session, status } => {
            app.set_session(session);
            app.clear_transcript();
            app.status = status;
            vec![Effect::RefreshTokens]
        }
    }
}

fn on_key(app: &mut App, key: Key) -> Vec<Effect> {
    if let Some(overlay) = app.overlays.last_mut() {
        if overlay.handle_key(key) == OverlayOutcome::Close {
            app.pop_overlay();
        }
        return Vec::new();
    }
    if app.approval.is_some() {
        return approval_key(app, key);
    }
    if app.resume.is_some() {
        return resume_key(app, key);
    }
    if app.tree.is_some() {
        return tree_key(app, key);
    }
    if key.code == KeyCodeX::Char('?') && app.input.is_empty() {
        app.push_overlay(Overlay::Help);
        return Vec::new();
    }
    let Some(action) = app.keymap.action(key) else {
        return Vec::new();
    };
    match action {
        Action::InputChar(c) => app.input.insert_char(c),
        Action::Submit => return submit(app),
        Action::Newline => app.input.insert_newline(),
        Action::Escape => return escape(app),
        Action::Quit => return vec![Effect::Quit],
        Action::CtrlC => return ctrl_c(app),
        Action::CtrlD => {
            if app.input.is_empty() {
                return vec![Effect::Quit];
            }
            app.input.delete();
        }
        Action::Backspace => app.input.backspace(),
        Action::Delete => app.input.delete(),
        Action::MoveLeft => app.input.move_left(),
        Action::MoveRight => app.input.move_right(),
        Action::WordLeft => app.input.word_left(),
        Action::WordRight => app.input.word_right(),
        Action::LineStart => app.input.home(),
        Action::LineEnd => app.input.end(),
        Action::DeleteWord => app.input.delete_word(),
        Action::KillToStart => app.input.kill_to_start(),
        Action::KillToEnd => app.input.kill_to_end(),
        Action::TabComplete => {
            app.input.tab_complete();
        }
        Action::HistoryUp => {
            if app.input.cursor_line() == 0 {
                app.input.history_up();
            }
        }
        Action::HistoryDown => {
            if app.input.cursor_line() + 1 == app.input.line_count() {
                app.input.history_down();
            }
        }
        Action::ScrollUp => app.scroll_up(app.page),
        Action::ScrollDown => app.scroll_down(app.page),
        Action::ScrollTop => {
            if app.input.is_empty() {
                app.scroll_top();
            } else {
                app.input.home();
            }
        }
        Action::ScrollBottom => {
            if app.input.is_empty() {
                app.end_follow();
            } else {
                app.input.end();
            }
        }
        Action::Help => app.push_overlay(Overlay::Help),
    }
    Vec::new()
}

fn ctrl_c(app: &mut App) -> Vec<Effect> {
    if app.running {
        let dropped = app.clear_queue();
        if dropped > 0 {
            app.info(format!("aborted; dropped {dropped} queued message(s)"));
        }
        vec![Effect::Abort, Effect::RefreshTokens]
    } else if !app.input.is_empty() {
        app.input.clear();
        Vec::new()
    } else if app.ctrl_c_idle() {
        vec![Effect::Quit]
    } else {
        Vec::new()
    }
}

fn escape(app: &mut App) -> Vec<Effect> {
    if app.running {
        let dropped = app.clear_queue();
        if dropped > 0 {
            app.info(format!("aborted; dropped {dropped} queued message(s)"));
        } else {
            app.info("aborted");
        }
        vec![Effect::Abort, Effect::RefreshTokens]
    } else {
        app.input.clear();
        Vec::new()
    }
}

fn submit(app: &mut App) -> Vec<Effect> {
    let text = app.take_input();
    let text = text.trim().to_string();
    if text.is_empty() {
        return Vec::new();
    }
    app.end_follow();
    app.remember_prompt(&text);
    if app.running {
        if app.queue_prompt(&text) {
            app.status = format!("queued ({} waiting)", app.queue.len());
        } else {
            app.info("cannot queue: no active run");
        }
        return Vec::new();
    }
    if text.starts_with('/') {
        return commands::dispatch(app, &text);
    }
    app.push_user(&text);
    app.running = true;
    app.elapsed_ticks = 0;
    app.status = "running".to_string();
    vec![Effect::RunPrompt(text)]
}

fn approval_key(app: &mut App, key: Key) -> Vec<Effect> {
    match key.code {
        KeyCodeX::Char('y') => {
            app.answer_approval(Decision::Allow);
        }
        KeyCodeX::Char('a') => {
            app.answer_approval(Decision::AllowAlways);
        }
        KeyCodeX::Char('n') => {
            if app.answer_approval(Decision::Deny) {
                app.info("denied tool call");
            }
        }
        KeyCodeX::Esc => {
            app.answer_approval(Decision::Deny);
            return vec![Effect::Abort, Effect::RefreshTokens];
        }
        KeyCodeX::Char('c') if key.ctrl => {
            app.answer_approval(Decision::Deny);
            return vec![Effect::Abort, Effect::RefreshTokens];
        }
        _ => {}
    }
    Vec::new()
}

fn picker_key(key: Key) -> Option<PKey> {
    Some(match key.code {
        KeyCodeX::Up => PKey::Up,
        KeyCodeX::Down => PKey::Down,
        KeyCodeX::PageUp => PKey::PageUp,
        KeyCodeX::PageDown => PKey::PageDown,
        KeyCodeX::Home => PKey::Home,
        KeyCodeX::End => PKey::End,
        KeyCodeX::Enter => PKey::Enter,
        KeyCodeX::Esc => PKey::Esc,
        KeyCodeX::Backspace => PKey::Backspace,
        KeyCodeX::Char('u') if key.ctrl => PKey::ClearQuery,
        KeyCodeX::Char(c) if !key.ctrl && !key.alt => PKey::Char(c),
        _ => return None,
    })
}

fn resume_key(app: &mut App, key: Key) -> Vec<Effect> {
    let Some(pkey) = picker_key(key) else {
        return Vec::new();
    };
    let Some(resume) = app.resume.as_mut() else {
        return Vec::new();
    };
    match resume.picker.handle_key(pkey) {
        super::picker::PickerEvent::Selected(path) => {
            app.resume = None;
            vec![Effect::OpenSession(path)]
        }
        super::picker::PickerEvent::Cancel => {
            app.resume = None;
            Vec::new()
        }
        super::picker::PickerEvent::None => Vec::new(),
    }
}

fn tree_key(app: &mut App, key: Key) -> Vec<Effect> {
    let Some(pkey) = picker_key(key) else {
        return Vec::new();
    };
    let Some(state) = app.tree.as_mut() else {
        return Vec::new();
    };
    match state.picker.handle_key(pkey) {
        super::picker::PickerEvent::Selected(_) => {
            if let Some(path) = app.select_tree() {
                app.info(format!("path: {path}"));
            }
        }
        super::picker::PickerEvent::Cancel => app.cancel_tree(),
        super::picker::PickerEvent::None => {}
    }
    Vec::new()
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod tests;
