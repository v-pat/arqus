use super::*;

fn tmp(label: &str) -> PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("arqus_state_{label}_{}_{n}", std::process::id()))
}

#[test]
fn missing_file_is_empty() {
    let dir = tmp("missing");
    assert_eq!(State::load(&dir.join("state.json")), State::default());
}

#[test]
fn roundtrip_and_recent_order() {
    let dir = tmp("roundtrip");
    let path = dir.join("state.json");
    let mut state = State::default();
    state.set_model("openrouter", "a");
    state.set_model("openrouter", "b");
    state.set_model("anthropic", "c");
    state.set_model("openrouter", "a");
    state.theme = Some("light".to_string());
    state
        .base_urls
        .insert("custom".to_string(), "http://localhost:9/v1".to_string());
    state.save(&path).unwrap();

    let back = State::load(&path);
    assert_eq!(back, state);
    assert_eq!(back.model.as_ref().unwrap().id, "a");
    assert_eq!(
        back.recent
            .iter()
            .map(|m| m.id.as_str())
            .collect::<Vec<_>>(),
        vec!["a", "c", "b"]
    );
    assert_eq!(back.recent_for("openrouter").len(), 2);
}

#[test]
fn recent_is_capped_at_eight() {
    let mut state = State::default();
    for i in 0..12 {
        state.set_model("p", &format!("m{i}"));
    }
    assert_eq!(state.recent.len(), 8);
    assert_eq!(state.recent[0].id, "m11");
    assert_eq!(state.recent[7].id, "m4");
}

#[test]
fn corrupt_file_is_moved_aside_and_never_crashes() {
    let dir = tmp("corrupt");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("state.json");
    fs::write(&path, "{ not json").unwrap();

    let state = State::load(&path);
    assert_eq!(state, State::default());
    assert!(!path.exists());
    let bad: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().contains(".bad-"))
        .collect();
    assert_eq!(bad.len(), 1);
    assert_eq!(fs::read_to_string(bad[0].path()).unwrap(), "{ not json");
}

#[test]
fn empty_file_is_empty_state() {
    let dir = tmp("empty");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("state.json");
    fs::write(&path, "  \n").unwrap();
    assert_eq!(State::load(&path), State::default());
}
