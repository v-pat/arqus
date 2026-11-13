use super::*;
use crate::tui::commands::registry;

fn names(query: &str) -> Vec<String> {
    let commands = registry();
    matches(query, &commands)
        .into_iter()
        .map(|index| commands[index].name.clone())
        .collect()
}

#[test]
fn query_needs_a_single_slash_word() {
    assert_eq!(query_of("/"), Some(String::new()));
    assert_eq!(query_of("/mo"), Some("mo".to_string()));
    assert_eq!(query_of("/mode plan"), None);
    assert_eq!(query_of("hello"), None);
    assert_eq!(query_of("\n/mo"), None);
}

#[test]
fn fuzzy_matches_names_and_aliases() {
    let found = names("mod");
    assert!(found.contains(&"mode".to_string()));
    assert!(found.contains(&"models".to_string()));
    assert!(found.contains(&"model".to_string()));

    let aliases = names("clear");
    assert_eq!(aliases, vec!["new".to_string()]);
}

#[test]
fn results_are_capped() {
    assert!(names("").len() <= MAX_MATCHES);
}

#[test]
fn sync_opens_and_keeps_the_selection_in_range() {
    let commands = registry();
    let mut state = SlashState::default();
    sync(&mut state, "/mod", &commands);
    assert!(state.open);
    assert!(!state.matched.is_empty());
    state.selected = 99;
    sync(&mut state, "/mod", &commands);
    assert!(state.selected < state.matched.len());
}

#[test]
fn selection_wraps_in_both_directions() {
    let commands = registry();
    let mut state = SlashState::default();
    sync(&mut state, "/", &commands);
    let last = state.matched.len() - 1;
    state.select_previous();
    assert_eq!(state.selected, last);
    state.select_next();
    assert_eq!(state.selected, 0);
}

#[test]
fn escape_dismisses_until_the_text_changes() {
    let commands = registry();
    let mut state = SlashState::default();
    sync(&mut state, "/mo", &commands);
    assert!(state.open);
    dismiss(&mut state, "/mo");
    sync(&mut state, "/mo", &commands);
    assert!(!state.open);
    sync(&mut state, "/mod", &commands);
    assert!(state.open);
}

#[test]
fn a_space_closes_the_popup() {
    let commands = registry();
    let mut state = SlashState::default();
    sync(&mut state, "/mode plan", &commands);
    assert!(!state.open);
}
