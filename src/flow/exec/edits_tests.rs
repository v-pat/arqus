use super::*;
use crate::flow::errors::FlowError;

#[path = "edits_apply_tests.rs"]
mod apply_tests;
#[path = "edits_prop_tests.rs"]
mod prop_tests;

fn sb(p: &str, s: &str, r: &str) -> EditBlock {
    EditBlock::Search {
        path: p.into(),
        search: s.into(),
        replace: r.into(),
    }
}

fn fb(p: &str, c: &str) -> EditBlock {
    EditBlock::File {
        path: p.into(),
        content: c.into(),
    }
}

#[test]
fn test_parse_single_search_block() {
    let input = "<<<<<<< SEARCH src/lib.rs\nold\n=======\nnew\n>>>>>>> REPLACE";
    assert_eq!(
        parse_edits(input).unwrap(),
        vec![sb("src/lib.rs", "old", "new")]
    );
}

#[test]
fn test_parse_several_blocks() {
    let input = "<<<<<<< SEARCH a.rs\n1\n=======\n2\n>>>>>>> REPLACE\n<<<<<<< SEARCH b.rs\n3\n=======\n4\n>>>>>>> REPLACE";
    assert_eq!(parse_edits(input).unwrap().len(), 2);
}

#[test]
fn test_parse_file_block() {
    let input = "<<<<<<< FILE src/new.rs\nfn main() {}\n>>>>>>> END";
    assert_eq!(
        parse_edits(input).unwrap(),
        vec![fb("src/new.rs", "fn main() {}\n")]
    );
}

#[test]
fn test_parse_mixed_blocks() {
    let input = "<<<<<<< FILE a.txt\nhello\n>>>>>>> END\n<<<<<<< SEARCH b.txt\nold\n=======\nnew\n>>>>>>> REPLACE";
    let edits = parse_edits(input).unwrap();
    assert_eq!(edits.len(), 2);
    assert!(matches!(edits[0], EditBlock::File { .. }));
    assert!(matches!(edits[1], EditBlock::Search { .. }));
}

#[test]
fn test_parse_prose_around_blocks() {
    let input = "Here is the edit:\n<<<<<<< SEARCH a.rs\nold\n=======\nnew\n>>>>>>> REPLACE\nHope this helps!";
    assert_eq!(parse_edits(input).unwrap().len(), 1);
}

#[test]
fn test_parse_crlf_input() {
    let input = "<<<<<<< SEARCH a.rs\r\nold\r\n=======\r\nnew\r\n>>>>>>> REPLACE\r\n";
    assert_eq!(parse_edits(input).unwrap(), vec![sb("a.rs", "old", "new")]);
}

#[test]
fn test_parse_lookalike_markers_inside_content() {
    let input = "<<<<<<< SEARCH a.rs\n// ======= comment\n=======\n// ======== 8 eq\n>>>>>>> REPLACE\n<<<<<<< FILE b.rs\n<<<<<<< SEARCH lookalike\n=======\n>>>>>>> END";
    assert_eq!(parse_edits(input).unwrap().len(), 2);
}

#[test]
fn test_parse_missing_terminator_giving_parse_error() {
    assert!(matches!(
        parse_edits("<<<<<<< SEARCH a.rs\nold\n=======\nnew"),
        Err(FlowError::ParseError(_))
    ));
    assert!(matches!(
        parse_edits("<<<<<<< SEARCH a.rs\nold\n>>>>>>> REPLACE"),
        Err(FlowError::ParseError(_))
    ));
    assert!(matches!(
        parse_edits("<<<<<<< FILE a.rs\ncontent"),
        Err(FlowError::ParseError(_))
    ));
}

#[test]
fn test_parse_cannot_complete_giving_declined() {
    let input = "CANNOT_COMPLETE: task requirements are ambiguous";
    assert_eq!(
        parse_edits(input),
        Err(FlowError::Declined(
            "task requirements are ambiguous".into()
        ))
    );
}

#[test]
fn test_parse_empty_output_giving_no_output() {
    assert_eq!(parse_edits(""), Err(FlowError::NoOutput));
    assert_eq!(parse_edits("   \n\t  "), Err(FlowError::NoOutput));
    assert_eq!(
        parse_edits("Just some text without any edit blocks"),
        Err(FlowError::NoOutput)
    );
}
