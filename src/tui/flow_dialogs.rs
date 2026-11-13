use super::app::App;
use super::keys::{Key, KeyCodeX};
use super::msg::Effect;
use super::picker::{to_pkey, PickerEvent};
use crate::safety::Decision;

pub(super) fn approval_key(app: &mut App, key: Key) -> Vec<Effect> {
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

pub(super) fn resume_key(app: &mut App, key: Key) -> Vec<Effect> {
    if app.resume_confirming() {
        match key.code {
            KeyCodeX::Char('y') => {
                if let Some(path) = app.answer_delete_resume(true) {
                    return vec![Effect::DeleteSession(path)];
                }
            }
            KeyCodeX::Char('n') | KeyCodeX::Esc => {
                app.answer_delete_resume(false);
            }
            _ => {}
        }
        return Vec::new();
    }
    if key.ctrl && key.code == KeyCodeX::Char('d') {
        app.ask_delete_resume();
        return Vec::new();
    }
    let Some(pkey) = to_pkey(key) else {
        return Vec::new();
    };
    let Some(resume) = app.resume.as_mut() else {
        return Vec::new();
    };
    match resume.picker.handle_key(pkey) {
        PickerEvent::Selected(path) => {
            app.resume = None;
            vec![Effect::OpenSession(path)]
        }
        PickerEvent::Cancel => {
            app.resume = None;
            Vec::new()
        }
        PickerEvent::None => Vec::new(),
    }
}

pub(super) fn tree_key(app: &mut App, key: Key) -> Vec<Effect> {
    let Some(pkey) = to_pkey(key) else {
        return Vec::new();
    };
    let Some(state) = app.tree.as_mut() else {
        return Vec::new();
    };
    match state.picker.handle_key(pkey) {
        PickerEvent::Selected(_) => {
            if let Some(path) = app.select_tree() {
                app.info(format!("path: {path}"));
            }
        }
        PickerEvent::Cancel => app.cancel_tree(),
        PickerEvent::None => {}
    }
    Vec::new()
}
