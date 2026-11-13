use super::*;
use crate::tui::history::History;

fn history(texts: &[&str]) -> History {
    let mut history = History::new();
    for text in texts {
        history.remember(text);
    }
    history
}

#[test]
fn rows_are_newest_first() {
    let picker = HistoryPicker::new(&history(&["one", "two", "three"]));
    let labels: Vec<&str> = picker
        .picker
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(labels, vec!["three", "two", "one"]);
    assert_eq!(picker.picker.items[0].value, "three".to_string());
}

#[test]
fn long_prompts_show_the_first_line_only() {
    let picker = HistoryPicker::new(&history(&["first line\nsecond line"]));
    assert_eq!(picker.picker.items[0].label, "first line");
}

#[test]
fn enter_inserts_and_escape_closes() {
    let mut picker = HistoryPicker::new(&history(&["old prompt"]));
    let outcome = picker.handle_key(Key::new(KeyCodeX::Enter));
    assert_eq!(outcome, OverlayOutcome::Insert("old prompt".to_string()));
    let outcome = picker.handle_key(Key::new(KeyCodeX::Esc));
    assert_eq!(outcome, OverlayOutcome::Close);
}

#[test]
fn empty_history_has_no_rows_and_enter_does_nothing() {
    let mut picker = HistoryPicker::new(&History::new());
    assert!(picker.picker.items.is_empty());
    assert_eq!(
        picker.handle_key(Key::new(KeyCodeX::Enter)),
        OverlayOutcome::None
    );
}
