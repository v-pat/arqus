use super::*;

fn items() -> Vec<Item<u32>> {
    vec![
        Item::new("OpenRouter", 1).group("Recommended"),
        Item::new("Anthropic", 2).group("Recommended"),
        Item::new("OpenAI", 3).group("Other"),
        Item::new("DeepSeek", 4).group("Other"),
        Item::new("Ollama (local)", 5).group("Other"),
    ]
}

fn type_str(p: &mut Picker<u32>, s: &str) {
    for c in s.chars() {
        p.handle_key(PKey::Char(c));
    }
}

#[test]
fn enter_returns_the_value_of_the_selection() {
    let mut p = Picker::new("Connect", items());
    p.handle_key(PKey::Down);
    assert_eq!(p.handle_key(PKey::Enter), PickerEvent::Selected(2));
}

#[test]
fn typing_filters_and_ranks() {
    let mut p = Picker::new("Connect", items());
    type_str(&mut p, "open");
    assert_eq!(p.match_count(), 2); // OpenRouter, OpenAI
    assert_eq!(p.handle_key(PKey::Enter), PickerEvent::Selected(3));
    let mut p = Picker::new("Connect", items());
    type_str(&mut p, "zzz");
    assert_eq!(p.handle_key(PKey::Enter), PickerEvent::None);
}

#[test]
fn esc_clears_query_first_then_cancels() {
    let mut p = Picker::new("x", items());
    type_str(&mut p, "ol");
    assert_eq!(p.handle_key(PKey::Esc), PickerEvent::None);
    assert!(p.query.is_empty());
    assert_eq!(p.match_count(), 5);
    assert_eq!(p.handle_key(PKey::Esc), PickerEvent::Cancel);
}

#[test]
fn up_and_down_wrap_around() {
    let mut p = Picker::new("x", items());
    p.handle_key(PKey::Up);
    assert_eq!(p.position(), (5, 5));
    p.handle_key(PKey::Down);
    assert_eq!(p.position(), (1, 5));
}

#[test]
fn headers_only_when_query_is_empty() {
    let mut p = Picker::new("x", items());
    let rows = p.rows();
    assert_eq!(rows[0], Row::Header("Recommended".into()));
    assert_eq!(rows.len(), 5 + 2);
    type_str(&mut p, "o");
    assert!(p.rows().iter().all(|r| matches!(r, Row::Item { .. })));
}

#[test]
fn current_item_is_preselected() {
    let mut it = items();
    it[3] = Item::new("DeepSeek", 4).group("Other").current(true);
    let mut p = Picker::new("x", it);
    assert_eq!(p.handle_key(PKey::Enter), PickerEvent::Selected(4));
}

#[test]
fn window_scrolls_to_keep_selection_visible() {
    let many: Vec<Item<u32>> = (0..30)
        .map(|i| Item::new(format!("model-{i:02}"), i))
        .collect();
    let mut p = Picker::new("x", many);
    assert_eq!(p.window(5).len(), 5);
    for _ in 0..12 {
        p.handle_key(PKey::Down);
    }
    let w = p.window(5);
    assert!(w.iter().any(|r| matches!(r, Row::Item { index: 12, .. })));
    p.handle_key(PKey::End);
    let w = p.window(5);
    assert!(w.iter().any(|r| matches!(r, Row::Item { index: 29, .. })));
    p.handle_key(PKey::Home);
    let w = p.window(5);
    assert!(w.iter().any(|r| matches!(r, Row::Item { index: 0, .. })));
}

#[test]
fn window_shows_group_header_with_first_item() {
    let mut p = Picker::new("x", items());
    let w = p.window(3);
    assert!(matches!(w[0], Row::Header(_)));
    for _ in 0..2 {
        p.handle_key(PKey::Down);
    }
    let w = p.window(3);
    assert!(w
        .iter()
        .any(|r| matches!(r, Row::Header(h) if h == "Other")));
}

#[test]
fn set_items_keeps_the_query() {
    let mut p: Picker<u32> = Picker::new("Models", vec![]);
    p.status = Some("Loading...".into());
    type_str(&mut p, "fla");
    p.set_items(vec![Item::new("deepseek-flash", 1), Item::new("gpt", 2)]);
    assert_eq!(p.query, "fla");
    assert_eq!(p.match_count(), 1);
}

#[test]
fn page_keys_move_by_page_and_clamp() {
    let many: Vec<Item<u32>> = (0..20).map(|i| Item::new(format!("m{i:02}"), i)).collect();
    let mut p = Picker::new("x", many);
    p.page = 8;
    p.handle_key(PKey::PageDown);
    assert_eq!(p.position().0, 9);
    p.handle_key(PKey::PageDown);
    p.handle_key(PKey::PageDown);
    assert_eq!(p.position().0, 20);
    p.handle_key(PKey::PageUp);
    assert_eq!(p.position().0, 12);
}
