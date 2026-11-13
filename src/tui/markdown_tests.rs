use super::*;

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
fn plain_text_is_one_span() {
    assert_eq!(parse_inline("hello world"), vec![plain("hello world")]);
    assert_eq!(parse_inline(""), Vec::new());
}

#[test]
fn bold_italic_and_code() {
    assert_eq!(
        parse_inline("a **b** c"),
        vec![plain("a "), styled("b", SpanStyle::Bold), plain(" c")]
    );
    assert_eq!(
        parse_inline("a *b* c"),
        vec![plain("a "), styled("b", SpanStyle::Italic), plain(" c")]
    );
    assert_eq!(
        parse_inline("run `ls -la` now"),
        vec![
            plain("run "),
            styled("ls -la", SpanStyle::Code),
            plain(" now")
        ]
    );
}

#[test]
fn half_finished_bold_stays_plain() {
    assert_eq!(parse_inline("**bol"), vec![plain("**bol")]);
    assert_eq!(parse_inline("a **b"), vec![plain("a **b")]);
}

#[test]
fn half_finished_code_stays_plain() {
    assert_eq!(parse_inline("run `ls"), vec![plain("run `ls")]);
    assert_eq!(parse_inline("a *b"), vec![plain("a *b")]);
}

#[test]
fn empty_markers_stay_literal() {
    assert_eq!(parse_inline("****"), vec![plain("****")]);
    assert_eq!(parse_inline("``"), vec![plain("``")]);
}

#[test]
fn urls_are_plain_text() {
    assert_eq!(
        parse_inline("see https://example.com/a_b"),
        vec![plain("see https://example.com/a_b")]
    );
}

#[test]
fn headings_bullets_numbers_and_quotes() {
    assert_eq!(
        parse_blocks("# Title\n- one\n* two\n1. first\n> quote"),
        vec![
            MdBlock::Heading {
                level: 1,
                spans: vec![plain("Title")]
            },
            MdBlock::Bullet(vec![plain("one")]),
            MdBlock::Bullet(vec![plain("two")]),
            MdBlock::Numbered {
                marker: "1.".to_string(),
                spans: vec![plain("first")]
            },
            MdBlock::Quote(vec![plain("quote")]),
        ]
    );
}

#[test]
fn a_fence_groups_lines_and_keeps_the_language() {
    let blocks = parse_blocks("before\n```rust\nlet x = 1;\n\n```\nafter");
    assert_eq!(
        blocks,
        vec![
            MdBlock::Paragraph(vec![plain("before")]),
            MdBlock::Code {
                lang: "rust".to_string(),
                lines: vec!["let x = 1;".to_string(), String::new()],
                closed: true
            },
            MdBlock::Paragraph(vec![plain("after")]),
        ]
    );
}

#[test]
fn an_unclosed_fence_is_still_code() {
    let blocks = parse_blocks("```\nlet x = 1;");
    assert_eq!(
        blocks,
        vec![MdBlock::Code {
            lang: String::new(),
            lines: vec!["let x = 1;".to_string()],
            closed: false
        }]
    );
}

#[test]
fn empty_lines_are_empty_paragraphs() {
    assert_eq!(
        parse_blocks("a\n\nb"),
        vec![
            MdBlock::Paragraph(vec![plain("a")]),
            MdBlock::Paragraph(Vec::new()),
            MdBlock::Paragraph(vec![plain("b")]),
        ]
    );
}

#[test]
fn tables_and_links_stay_plain() {
    assert_eq!(
        parse_blocks("| a | b |"),
        vec![MdBlock::Paragraph(vec![plain("| a | b |")])]
    );
    assert_eq!(
        parse_blocks("[text](https://x.y)"),
        vec![MdBlock::Paragraph(vec![plain("[text](https://x.y)")])]
    );
}
