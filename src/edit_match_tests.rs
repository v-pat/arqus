//! Tests for the extracted exact matching helper.

use super::*;

#[test]
fn test_exact_single_match_replaces() {
    let content = "hello\nworld\n";
    let res = apply_exact_edit(content, "world", "rust").unwrap();
    assert_eq!(res, "hello\nrust\n");
}

#[test]
fn test_zero_matches_returns_zero_matches_error_with_line_count() {
    let content = "line 1\nline 2\n";
    let err = apply_exact_edit(content, "missing", "x").unwrap_err();
    assert_eq!(err, EditMatchError::ZeroMatches { line_count: 2 });
}

#[test]
fn test_multiple_matches_returns_multiple_matches_error_with_count() {
    let content = "dup\nline\ndup\n";
    let err = apply_exact_edit(content, "dup", "x").unwrap_err();
    assert_eq!(err, EditMatchError::MultipleMatches { count: 2 });
}

#[test]
fn test_empty_old_text_returns_error() {
    let content = "hello\n";
    let err = apply_exact_edit(content, "", "x").unwrap_err();
    assert_eq!(err, EditMatchError::EmptyOldText);
}

#[test]
fn test_crlf_file_matched_with_lf_and_preserves_crlf() {
    let content = "first\r\nsecond\r\nthird\r\n";
    let res = apply_exact_edit(content, "second\nthird", "SECOND\nTHIRD").unwrap();
    assert_eq!(res, "first\r\nSECOND\r\nTHIRD\r\n");
}
