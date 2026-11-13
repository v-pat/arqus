use super::*;

#[test]
fn depth_is_capped_at_eight_levels_with_an_ellipsis() {
    assert_eq!(indent(0), "");
    assert_eq!(indent(2), "    ");
    assert_eq!(indent(8), "                ");
    assert_eq!(indent(9), "                … ");
    assert_eq!(indent(20), "                … ");
}
