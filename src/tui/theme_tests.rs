use super::*;

#[test]
fn xterm_cube_and_gray_ramp() {
    assert_eq!(rgb_to_xterm256(0, 0, 0), 16);
    assert_eq!(rgb_to_xterm256(255, 255, 255), 231);
    assert_eq!(rgb_to_xterm256(255, 0, 0), 196);
    assert_eq!(rgb_to_xterm256(128, 128, 128), 244);
}

#[test]
fn depth_detection_follows_the_environment() {
    assert_eq!(
        ColorDepth::detect(Some("truecolor"), Some("xterm"), None),
        ColorDepth::True
    );
    assert_eq!(
        ColorDepth::detect(Some("24bit"), Some("xterm"), None),
        ColorDepth::True
    );
    assert_eq!(
        ColorDepth::detect(None, Some("xterm-256color"), None),
        ColorDepth::Indexed256
    );
    assert_eq!(
        ColorDepth::detect(Some("truecolor"), Some("xterm"), Some("1")),
        ColorDepth::None
    );
    assert_eq!(
        ColorDepth::detect(None, Some("dumb"), None),
        ColorDepth::None
    );
    assert_eq!(
        ColorDepth::detect(None, Some("xterm"), None),
        ColorDepth::Ansi16
    );
}

#[test]
fn truecolor_uses_rgb_and_indexed_uses_a_number() {
    let theme = Theme::dark(ColorDepth::True);
    assert_eq!(theme.fg, Color::Rgb(205, 214, 244));
    let theme = Theme::dark(ColorDepth::Indexed256);
    assert!(matches!(theme.fg, Color::Indexed(_)));
}

#[test]
fn mono_has_no_colors() {
    let theme = Theme::mono();
    assert_eq!(theme.fg, Color::Reset);
    assert_eq!(theme.error, Color::Reset);
    assert_eq!(theme.selection, Color::Reset);
}
