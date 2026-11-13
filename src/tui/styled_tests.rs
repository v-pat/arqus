use super::*;
use crate::tui::markdown::{Span, SpanStyle};

fn plain(text: &str) -> Span {
    Span::plain(text)
}

fn styled(text: &str, style: SpanStyle) -> Span {
    Span {
        text: text.to_string(),
        style,
    }
}

#[test]
fn display_width_counts_columns_not_bytes() {
    assert_eq!(display_width("abc"), 3);
    assert_eq!(display_width("你好"), 4);
    assert_eq!(display_width("😀"), 2);
    assert_eq!(display_width("\t"), 4);
}

#[test]
fn wrap_spans_breaks_at_spaces_and_keeps_styles() {
    let spans = vec![plain("hello "), styled("bold", SpanStyle::Bold)];
    assert_eq!(
        wrap_spans(&spans, 8),
        vec![vec![plain("hello")], vec![styled("bold", SpanStyle::Bold)]]
    );
}

#[test]
fn wrap_spans_hard_breaks_a_long_word() {
    let spans = vec![plain("abcdefgh")];
    assert_eq!(
        wrap_spans(&spans, 3),
        vec![vec![plain("abc")], vec![plain("def")], vec![plain("gh")]]
    );
}

#[test]
fn wrap_spans_counts_display_columns() {
    let spans = vec![plain("你好世界")];
    assert_eq!(
        wrap_spans(&spans, 4),
        vec![vec![plain("你好")], vec![plain("世界")]]
    );
}

#[test]
fn wrap_spans_width_one_and_empty() {
    assert_eq!(
        wrap_spans(&[plain("ab")], 1),
        vec![vec![plain("a")], vec![plain("b")]]
    );
    assert_eq!(wrap_spans(&[], 10), vec![Vec::<Span>::new()]);
}

#[test]
fn wrap_spans_keeps_explicit_newlines() {
    let spans = vec![plain("one\ntwo")];
    assert_eq!(
        wrap_spans(&spans, 10),
        vec![vec![plain("one")], vec![plain("two")]]
    );
}
