use super::{format_timestamp, visible_window};

#[test]
fn window_keeps_the_top_visible() {
    assert_eq!(visible_window(0, 10, 5), (0, 5));
    assert_eq!(visible_window(4, 10, 5), (0, 5));
}

#[test]
fn window_follows_the_middle_selection() {
    assert_eq!(visible_window(7, 10, 5), (3, 8));
}

#[test]
fn window_keeps_the_bottom_visible() {
    assert_eq!(visible_window(9, 10, 5), (5, 10));
}

#[test]
fn window_shrinks_when_the_list_is_short() {
    assert_eq!(visible_window(1, 3, 10), (0, 3));
    assert_eq!(visible_window(0, 0, 10), (0, 0));
    assert_eq!(visible_window(0, 3, 0), (0, 0));
}

#[test]
fn timestamps_are_utc() {
    assert_eq!(format_timestamp(0), "1970-01-01 00:00");
    assert_eq!(format_timestamp(1_700_000_000), "2023-11-14 22:13");
}
