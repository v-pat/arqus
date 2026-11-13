//! Tests for the terminal guard and the title escapes.

use super::*;

#[test]
fn restore_is_idempotent() {
    let mut guard = TerminalGuard::new(true, true);
    assert!(!guard.restored());
    assert!(guard.restore());
    assert!(guard.restored());
    assert!(!guard.restore());
    assert!(!guard.restore());
}

#[test]
fn title_names_the_folder() {
    assert_eq!(title("my-project"), "\x1b]0;arqus · my-project\x07");
    assert_eq!(reset_title(), "\x1b]0;\x07");
}

#[test]
fn folder_name_handles_edges() {
    assert_eq!(folder_name("/Users/me/code/arqus"), "arqus");
    assert_eq!(folder_name("/Users/me/code/arqus/"), "arqus");
    assert_eq!(folder_name("/"), "arqus");
    assert_eq!(folder_name(""), "arqus");
}

#[test]
fn invalid_utf8_uses_replacement_characters() {
    let bytes = b"ok \xff\xfe done";
    let text = String::from_utf8_lossy(bytes);
    assert!(text.contains('\u{FFFD}'));
    assert!(text.starts_with("ok "));
}
