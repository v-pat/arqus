use std::path::PathBuf;

use super::*;

fn tmp(label: &str) -> PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("arqus_theme_{label}_{}_{n}", std::process::id()))
}

#[test]
fn a_good_file_overrides_only_the_named_colors() {
    let theme = parse(
        "ocean",
        r##"{"fg":"#112233","accent":"#00ffcc"}"##,
        ColorDepth::True,
    )
    .unwrap();
    assert_eq!(theme.name, "ocean");
    assert_eq!(theme.fg, ratatui::style::Color::Rgb(0x11, 0x22, 0x33));
    assert_eq!(theme.accent, ratatui::style::Color::Rgb(0x00, 0xff, 0xcc));
    assert_eq!(theme.error, Theme::dark(ColorDepth::True).error);
}

#[test]
fn missing_fields_use_the_dark_defaults() {
    let theme = parse("empty", "{}", ColorDepth::True).unwrap();
    let dark = Theme::dark(ColorDepth::True);
    assert_eq!(theme.fg, dark.fg);
    assert_eq!(theme.selection, dark.selection);
    assert_eq!(theme.code_bg, dark.code_bg);
}

#[test]
fn bad_hex_and_bad_json_are_errors() {
    assert!(parse("bad", r##"{"fg":"red"}"##, ColorDepth::True).is_err());
    assert!(parse("bad", r##"{"fg":"#12345"}"##, ColorDepth::True).is_err());
    assert!(parse("bad", "{ not json", ColorDepth::True).is_err());
}

#[test]
fn color_depth_goes_through_the_theme_builder() {
    let theme = parse("ocean", r##"{"fg":"#112233"}"##, ColorDepth::Indexed256).unwrap();
    assert!(matches!(theme.fg, ratatui::style::Color::Indexed(_)));
}

#[test]
fn all_skips_invalid_files_and_names_them() {
    let home = tmp("all");
    let dir = home.join("themes");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("good.json"), r##"{"fg":"#010203"}"##).unwrap();
    std::fs::write(dir.join("bad.json"), r##"{"fg":"nope"}"##).unwrap();

    let (choices, errors) = all(&home, ColorDepth::True);
    let names: Vec<_> = choices.iter().map(|choice| choice.name.as_str()).collect();
    assert!(names.contains(&"good"));
    assert!(!names.contains(&"bad"));
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("themes/bad.json"));
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn saved_names_resolve_and_traversal_is_refused() {
    let home = tmp("named");
    let dir = home.join("themes");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("ocean.json"), r##"{"fg":"#010203"}"##).unwrap();

    assert_eq!(
        load_named(&home, "ocean", ColorDepth::True).unwrap().name,
        "ocean"
    );
    assert!(load_named(&home, "../ocean", ColorDepth::True).is_none());
    assert!(load_named(&home, "missing", ColorDepth::True).is_none());
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn no_color_forces_mono_at_startup() {
    let home = tmp("nocolor");
    let theme = startup_theme(&home, Some("light"), ColorDepth::None);
    assert_eq!(theme.name, "mono");
    assert_eq!(theme.fg, ratatui::style::Color::Reset);
}
