use super::*;
use crate::settings::KeyBindSpec;

fn spec(entries: &[(&str, KeyBindSpec)]) -> BTreeMap<String, KeyBindSpec> {
    entries
        .iter()
        .map(|(name, value)| ((*name).to_string(), value.clone()))
        .collect()
}

fn one(text: &str) -> KeyBindSpec {
    KeyBindSpec::One(text.to_string())
}

fn many(keys: &[&str]) -> KeyBindSpec {
    KeyBindSpec::Many(keys.iter().map(|key| key.to_string()).collect())
}

#[test]
fn one_key_replaces_the_default_pair() {
    let merged = merge(&spec(&[("palette", one("ctrl+k"))]));
    let key = parse_key("ctrl+k").unwrap();
    assert_eq!(merged.keymap.action(key), Some(Action::Palette));
    assert_eq!(
        merged.keymap.action(parse_key("ctrl+p").unwrap()),
        None,
        "the default palette key is removed"
    );
    assert!(merged
        .notices
        .iter()
        .any(|notice| notice == "keybinds: ctrl+k now runs palette (was kill_to_end)"));
}

#[test]
fn an_array_and_a_comma_list_both_bind_several_keys() {
    let merged = merge(&spec(&[
        ("newline", one("alt+enter,ctrl+j")),
        ("help", many(&["f1", "ctrl+h"])),
    ]));
    for text in ["alt+enter", "ctrl+j"] {
        assert_eq!(
            merged.keymap.action(parse_key(text).unwrap()),
            Some(Action::Newline)
        );
    }
    for text in ["f1", "ctrl+h"] {
        assert_eq!(
            merged.keymap.action(parse_key(text).unwrap()),
            Some(Action::Help)
        );
    }
}

#[test]
fn unknown_actions_and_unparsable_keys_are_reported() {
    let merged = merge(&spec(&[
        ("foo", one("ctrl+k")),
        ("palette", one("bogus+key")),
    ]));
    assert!(merged
        .notices
        .iter()
        .any(|notice| notice == "keybinds: unknown action \"foo\""));
    assert!(merged
        .notices
        .iter()
        .any(|notice| notice.contains("unparsable key \"bogus+key\"")));
    assert_eq!(
        merged.keymap.action(parse_key("ctrl+p").unwrap()),
        Some(Action::Palette),
        "an invalid key never removes the default"
    );
}

#[test]
fn a_shared_key_goes_to_the_later_action() {
    let merged = merge(&spec(&[
        ("models", one("ctrl+l")),
        ("palette", one("ctrl+l")),
    ]));
    assert_eq!(
        merged.keymap.action(parse_key("ctrl+l").unwrap()),
        Some(Action::Palette)
    );
    assert!(merged
        .notices
        .iter()
        .any(|notice| notice == "keybinds: ctrl+l now runs palette (was models)"));
}

#[test]
fn unparsed_specs_keep_defaults() {
    let merged = merge(&spec(&[("palette", many(&[]))]));
    assert_eq!(
        merged.keymap.action(parse_key("ctrl+p").unwrap()),
        Some(Action::Palette)
    );
}
