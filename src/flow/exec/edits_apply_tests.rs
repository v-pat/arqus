use std::fs;
use std::path::Path;

use tempfile::tempdir;

use super::*;
use crate::flow::errors::FlowError;

fn setup_file(dir: &Path, rel: &str, content: &str) {
    let p = dir.join(rel);
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(p, content).unwrap();
}

#[test]
fn test_apply_one_exact_match() {
    let dir = tempdir().unwrap();
    setup_file(dir.path(), "a.rs", "fn foo() { old(); }\n");
    apply_edits(
        &[sb("a.rs", "old()", "new()")],
        dir.path(),
        &["a.rs".into()],
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("a.rs")).unwrap(),
        "fn foo() { new(); }\n"
    );
}

#[test]
fn test_apply_no_match_with_hint_in_message() {
    let dir = tempdir().unwrap();
    setup_file(dir.path(), "a.rs", "line1\nline2\n");
    let err = apply_edits(&[sb("a.rs", "missing", "x")], dir.path(), &["a.rs".into()]).unwrap_err();
    match err {
        FlowError::NoMatch(msg) => {
            assert!(
                msg.contains("2 lines") || msg.contains("not found"),
                "{msg}"
            );
            assert!(
                msg.contains("indentation") || msg.contains("whitespace") || msg.contains("spaces"),
                "{msg}"
            );
        }
        other => panic!("expected NoMatch, got {other:?}"),
    }
}

#[test]
fn test_apply_two_matches_giving_ambiguous_match() {
    let dir = tempdir().unwrap();
    setup_file(dir.path(), "a.rs", "dup\nline\ndup\n");
    let err = apply_edits(&[sb("a.rs", "dup", "x")], dir.path(), &["a.rs".into()]).unwrap_err();
    match err {
        FlowError::AmbiguousMatch(msg) => assert!(msg.contains("2 times"), "{msg}"),
        other => panic!("expected AmbiguousMatch, got {other:?}"),
    }
}

#[test]
fn test_apply_search_spanning_lines() {
    let dir = tempdir().unwrap();
    setup_file(
        dir.path(),
        "a.rs",
        "fn foo() {\n    let a = 1;\n    let b = 2;\n}\n",
    );
    apply_edits(
        &[sb(
            "a.rs",
            "    let a = 1;\n    let b = 2;",
            "    let sum = 3;",
        )],
        dir.path(),
        &["a.rs".into()],
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("a.rs")).unwrap(),
        "fn foo() {\n    let sum = 3;\n}\n"
    );
}

#[test]
fn test_apply_whitespace_difference_as_strict_non_match() {
    let dir = tempdir().unwrap();
    setup_file(dir.path(), "a.rs", "    let x = 1;\n");
    let err = apply_edits(
        &[sb("a.rs", "\tlet x = 1;", "let x = 2;")],
        dir.path(),
        &["a.rs".into()],
    )
    .unwrap_err();
    assert!(matches!(err, FlowError::NoMatch(_)));
}

#[test]
fn test_apply_crlf_file_edited_with_lf_search_and_written_back_as_crlf() {
    let dir = tempdir().unwrap();
    setup_file(dir.path(), "a.rs", "line 1\r\nline 2\r\nline 3\r\n");
    apply_edits(
        &[sb("a.rs", "line 2\nline 3", "line B\nline C")],
        dir.path(),
        &["a.rs".into()],
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("a.rs")).unwrap(),
        "line 1\r\nline B\r\nline C\r\n"
    );
}

#[test]
fn test_apply_permissions_kept() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        setup_file(dir.path(), "a.rs", "secret\n");
        let p = dir.path().join("a.rs");
        fs::set_permissions(&p, fs::Permissions::from_mode(0o640)).unwrap();
        apply_edits(
            &[sb("a.rs", "secret", "public")],
            dir.path(),
            &["a.rs".into()],
        )
        .unwrap();
        assert_eq!(
            fs::metadata(&p).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }
}

#[test]
fn test_apply_file_creating_and_replacing() {
    let dir = tempdir().unwrap();
    apply_edits(
        &[fb("sub/new.txt", "new content\n")],
        dir.path(),
        &["sub/new.txt".into()],
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("sub/new.txt")).unwrap(),
        "new content\n"
    );

    apply_edits(
        &[fb("sub/new.txt", "replaced\n")],
        dir.path(),
        &["sub/new.txt".into()],
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("sub/new.txt")).unwrap(),
        "replaced\n"
    );
}

#[test]
fn test_apply_path_outside_allowed_writes_absolute_and_dotdot_all_scope_violation() {
    let dir = tempdir().unwrap();
    setup_file(dir.path(), "a.rs", "data");
    assert!(matches!(
        apply_edits(&[fb("b.rs", "x")], dir.path(), &["a.rs".into()]),
        Err(FlowError::ScopeViolation(_))
    ));
    assert!(matches!(
        apply_edits(
            &[fb("/etc/passwd", "x")],
            dir.path(),
            &["/etc/passwd".into()]
        ),
        Err(FlowError::ScopeViolation(_))
    ));
    assert!(matches!(
        apply_edits(
            &[fb("../outside.txt", "x")],
            dir.path(),
            &["../outside.txt".into()]
        ),
        Err(FlowError::ScopeViolation(_))
    ));
}

#[test]
fn test_apply_two_blocks_on_one_file_applied_in_order() {
    let dir = tempdir().unwrap();
    setup_file(dir.path(), "a.rs", "apple\nbanana\n");
    apply_edits(
        &[
            sb("a.rs", "apple", "apricot"),
            sb("a.rs", "banana", "blueberry"),
        ],
        dir.path(),
        &["a.rs".into()],
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("a.rs")).unwrap(),
        "apricot\nblueberry\n"
    );
}

#[test]
fn test_apply_atomic_failing_second_block_leaves_first_unwritten() {
    let dir = tempdir().unwrap();
    setup_file(dir.path(), "a.rs", "original A\n");
    setup_file(dir.path(), "b.rs", "original B\n");
    let err = apply_edits(
        &[
            sb("a.rs", "original A", "modified A"),
            sb("b.rs", "nonexistent", "fail"),
        ],
        dir.path(),
        &["a.rs".into(), "b.rs".into()],
    )
    .unwrap_err();
    assert!(matches!(err, FlowError::NoMatch(_)));
    assert_eq!(
        fs::read_to_string(dir.path().join("a.rs")).unwrap(),
        "original A\n"
    );
}
