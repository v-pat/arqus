use super::wrap_text;

#[test]
fn plain_text_breaks_at_spaces() {
    assert_eq!(wrap_text("hello world", 8), vec!["hello", "world"]);
    assert_eq!(wrap_text("short", 20), vec!["short"]);
}

#[test]
fn long_word_is_hard_broken() {
    assert_eq!(wrap_text("abcdefghij", 4), vec!["abcd", "efgh", "ij"]);
}

#[test]
fn cjk_counts_two_columns() {
    assert_eq!(wrap_text("你好世界", 4), vec!["你好", "世界"]);
}

#[test]
fn emoji_counts_two_columns() {
    assert_eq!(wrap_text("😀😀😀", 4), vec!["😀😀", "😀"]);
}

#[test]
fn tab_becomes_four_spaces() {
    assert_eq!(wrap_text("\tx", 10), vec!["    x"]);
}

#[test]
fn empty_line_stays_one_empty_line() {
    assert_eq!(wrap_text("", 10), vec![""]);
}

#[test]
fn width_one_puts_one_column_per_line() {
    assert_eq!(wrap_text("ab cd", 1), vec!["a", "b", "c", "d"]);
    assert_eq!(wrap_text("你好", 1), vec!["你", "好"]);
}

#[test]
fn continuation_keeps_leading_spaces() {
    assert_eq!(
        wrap_text("  one two three", 9),
        vec!["  one two", "  three"]
    );
}
