use super::*;

#[test]
fn clean_branch_parses_without_a_star() {
    let info = parse("main\n", "").unwrap();
    assert_eq!(info.branch, "main");
    assert!(!info.dirty);
}

#[test]
fn porcelain_output_marks_the_branch_dirty() {
    let info = parse("feature/login\n", " M src/main.rs\n?? notes.md\n").unwrap();
    assert_eq!(info.branch, "feature/login");
    assert!(info.dirty);
}

#[test]
fn no_repository_has_no_facts() {
    assert_eq!(parse("", ""), None);
    assert_eq!(parse("   \n", " M a.rs\n"), None);
}

#[test]
fn only_the_first_branch_line_is_used() {
    let info = parse("  main  \nsecond\n", "").unwrap();
    assert_eq!(info.branch, "main");
}
