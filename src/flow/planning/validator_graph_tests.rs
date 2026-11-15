use super::{build_transitive_deps, matches_pattern};
use crate::flow::schema::types::{Assignee, Category, TaskContract};

fn make_task(id: &str, deps: Vec<&str>) -> TaskContract {
    TaskContract {
        id: id.to_string(),
        title: "Test".to_string(),
        category: Category::SingleFunction,
        assignee: Assignee::Worker,
        objective: "Objective".to_string(),
        context: vec![],
        allowed_writes: vec!["file.rs".to_string()],
        depends_on: deps.into_iter().map(|s| s.to_string()).collect(),
        constraints: vec![],
        acceptance_criteria: vec![],
        interface_notes: String::new(),
        verify: vec!["cargo test".to_string()],
        est_changed_lines: 10,
    }
}

#[test]
fn test_matches_pattern() {
    assert!(matches_pattern(".git/**", ".git/config"));
    assert!(matches_pattern(".git/**", ".git/refs/heads"));
    assert!(matches_pattern(".git/**", ".git"));
    assert!(!matches_pattern(".git/**", ".gitignore"));
    assert!(matches_pattern("Cargo.lock", "Cargo.lock"));
    assert!(!matches_pattern("Cargo.lock", "src/Cargo.lock"));
}

#[test]
fn test_build_transitive_deps_linear() {
    let t1 = make_task("T01", vec![]);
    let t2 = make_task("T02", vec!["T01"]);
    let t3 = make_task("T03", vec!["T02"]);

    let (deps, cycle) = build_transitive_deps(&[t1, t2, t3]);
    assert!(cycle.is_none());
    assert!(deps["T03"].contains("T02"));
    assert!(deps["T03"].contains("T01"));
    assert!(deps["T02"].contains("T01"));
    assert!(deps["T01"].is_empty());
}

#[test]
fn test_build_transitive_deps_cycle() {
    let t1 = make_task("T01", vec!["T02"]);
    let t2 = make_task("T02", vec!["T01"]);

    let (_, cycle) = build_transitive_deps(&[t1, t2]);
    assert!(cycle.is_some());
    let mut cycle_ids = cycle.unwrap();
    cycle_ids.sort();
    assert_eq!(cycle_ids, vec!["T01", "T02"]);
}
