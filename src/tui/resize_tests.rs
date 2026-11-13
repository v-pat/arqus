//! Tests for the resize debounce rule.

use super::*;

#[test]
fn only_the_last_size_of_a_burst_is_applied() {
    let mut debounce = ResizeDebounce::default();
    assert_eq!(debounce.deadline_ms(), None);
    debounce.push(80, 24, 0);
    debounce.push(100, 40, 10);
    debounce.push(120, 50, 20);
    assert_eq!(debounce.ready(40), None, "still inside the quiet window");
    assert_eq!(debounce.ready(69), None, "not 50ms after the last event");
    assert_eq!(debounce.ready(70), Some((120, 50)));
    assert_eq!(debounce.ready(200), None, "nothing pending any more");
}

#[test]
fn deadline_is_relative_to_the_last_event() {
    let mut debounce = ResizeDebounce::default();
    debounce.push(80, 24, 1_000);
    assert_eq!(debounce.deadline_ms(), Some(1_050));
    debounce.push(90, 30, 1_030);
    assert_eq!(debounce.deadline_ms(), Some(1_080));
}

#[test]
fn a_single_event_applies_after_the_delay() {
    let mut debounce = ResizeDebounce::default();
    debounce.push(60, 20, 0);
    assert_eq!(debounce.ready(49), None);
    assert_eq!(debounce.ready(50), Some((60, 20)));
}
