use super::*;

fn input(text: &str) -> InputState {
    let mut state = InputState::new();
    state.set_text(text);
    state
}

#[test]
fn insert_and_delete_in_the_middle() {
    let mut state = input("helo");
    state.move_left();
    state.insert_char('l');
    assert_eq!(state.text(), "hello");
    state.delete();
    assert_eq!(state.text(), "hell");
    state.backspace();
    assert_eq!(state.text(), "hel");
}

#[test]
fn emoji_and_cjk_use_char_indexes() {
    let mut state = input("a🙂b");
    state.move_left();
    state.backspace();
    assert_eq!(state.text(), "ab");

    let mut state = input("你好");
    state.backspace();
    assert_eq!(state.text(), "你");
    state.insert_char('世');
    assert_eq!(state.text(), "你世");
    assert_eq!(state.cursor_column(), 4);
}

#[test]
fn ctrl_w_removes_words_and_punctuation() {
    let mut state = input("hello, world  ");
    state.delete_word();
    assert_eq!(state.text(), "hello, ");

    let mut state = input("hello,");
    state.delete_word();
    assert_eq!(state.text(), "");

    let mut state = input("one  two");
    state.delete_word();
    assert_eq!(state.text(), "one  ");

    let mut state = input("   ");
    state.delete_word();
    assert_eq!(state.text(), "");
}

#[test]
fn home_and_end_move_inside_the_current_line() {
    let mut state = input("abc\ndef");
    state.home();
    assert_eq!(state.cursor(), 4);
    state.end();
    assert_eq!(state.cursor(), 7);
    state.move_left();
    state.move_left();
    state.home();
    assert_eq!(state.cursor(), 4);
}

#[test]
fn ctrl_j_inserts_a_newline_and_the_cursor_moves() {
    let mut state = input("ab");
    state.insert_newline();
    state.insert_char('c');
    assert_eq!(state.text(), "ab\nc");
    assert_eq!(state.line_count(), 2);
    assert_eq!(state.cursor_line(), 1);
    assert_eq!(state.line_text(1), "c");
    state.home();
    assert_eq!(state.cursor(), 3);
}

#[test]
fn kill_to_start_and_end_stay_on_the_line() {
    let mut state = input("abc\ndef");
    state.home();
    state.move_right();
    state.kill_to_start();
    assert_eq!(state.text(), "abc\nef");

    let mut state = input("ab\ncd");
    state.home();
    state.kill_to_end();
    assert_eq!(state.text(), "ab\n");

    let mut state = input("abc");
    state.move_left();
    state.move_left();
    state.kill_to_end();
    assert_eq!(state.text(), "a");
}

#[test]
fn history_walks_up_and_down_and_restores_the_draft() {
    let mut history = History::new();
    history.remember("one");
    history.remember("two");
    let mut state = InputState::with_history(history);
    state.set_text("draft");

    assert!(state.history_up());
    assert_eq!(state.text(), "two");
    assert!(state.history_up());
    assert_eq!(state.text(), "one");
    assert!(!state.history_up());

    assert!(state.history_down());
    assert_eq!(state.text(), "two");
    assert!(state.history_down());
    assert_eq!(state.text(), "draft");
    assert!(!state.history_down());
}

#[test]
fn editing_after_a_recall_stops_history_mode() {
    let mut history = History::new();
    history.remember("old");
    let mut state = InputState::with_history(history);
    assert!(state.history_up());
    state.insert_char('!');
    assert_eq!(state.text(), "old!");
    assert!(!state.history_down());
    assert!(state.history_up());
    assert_eq!(state.text(), "old");
}

#[test]
fn paste_keeps_newlines_and_never_submits() {
    let mut state = input("ab");
    state.move_left();
    state.insert_text("x\ny");
    assert_eq!(state.text(), "ax\nyb");
    assert_eq!(state.line_count(), 2);
}

#[test]
fn view_scrolls_to_keep_the_cursor_visible() {
    let state = input("1\n2\n3\n4\n5\n6\n7\n8");
    assert_eq!(state.view(6), (2, 8));

    let mut state = input("1\n2\n3\n4\n5\n6\n7\n8");
    state.home();
    assert_eq!(state.view(6), (2, 8));

    let state = input("1\n2");
    assert_eq!(state.view(6), (0, 2));
    assert_eq!(state.view(1), (1, 2));
}

#[test]
fn word_moves_skip_whitespace_and_punctuation() {
    let mut state = input("one two, three");
    state.word_left();
    assert_eq!(state.cursor(), 9);
    state.word_left();
    assert_eq!(state.cursor(), 4);
    state.word_right();
    assert_eq!(state.cursor(), 9);
}

#[test]
fn ctrl_k_then_yank_restores_the_line_end() {
    let mut state = input("hello world");
    state.home();
    state.move_right();
    state.move_right();
    state.kill_to_end();
    assert_eq!(state.text(), "he");
    state.yank();
    assert_eq!(state.text(), "hello world");
    assert_eq!(state.cursor(), 11);
}

#[test]
fn ctrl_u_and_ctrl_w_are_yankable_too() {
    let mut state = input("one two three");
    state.delete_word();
    assert_eq!(state.text(), "one two ");
    state.yank();
    assert_eq!(state.text(), "one two three");

    let mut state = input("keep drop");
    state.end();
    state.move_left();
    state.move_left();
    state.move_left();
    state.move_left();
    state.kill_to_start();
    assert_eq!(state.text(), "drop");
    state.yank();
    assert_eq!(state.text(), "keep drop");
}

#[test]
fn a_large_paste_shows_one_marker_but_keeps_the_text() {
    let big = "line\n".repeat(900);
    let mut state = InputState::new();
    state.insert_paste(&big);
    assert_eq!(state.text(), big);
    assert_eq!(state.line_count(), 1);
    assert_eq!(
        state.line_parts(0),
        vec![LinePart::Paste(big.chars().count())]
    );
    assert_eq!(
        super::paste_label(big.chars().count()),
        "[pasted 4,500 characters]"
    );
}

#[test]
fn a_small_paste_is_inserted_exactly() {
    let mut state = InputState::new();
    state.insert_paste("short\ntext");
    assert_eq!(
        state.line_parts(0),
        vec![LinePart::Text("short".to_string())]
    );
    assert_eq!(state.line_text(1), "text");
}

#[test]
fn deleting_the_paste_marker_removes_the_whole_paste() {
    let mut state = InputState::new();
    state.insert_paste(&"z".repeat(3000));
    state.backspace();
    assert_eq!(state.text(), "");
    state.insert_char('a');
    assert_eq!(state.text(), "a");
}

#[test]
fn paste_label_groups_thousands() {
    assert_eq!(super::paste_label(4210), "[pasted 4,210 characters]");
    assert_eq!(super::paste_label(999), "[pasted 999 characters]");
    assert_eq!(
        super::paste_label(1_000_000),
        "[pasted 1,000,000 characters]"
    );
}
