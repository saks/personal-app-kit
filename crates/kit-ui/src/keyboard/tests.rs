use super::*;

#[test]
fn desktop_has_no_ime_cover() {
    assert_eq!(ime_cover_px(false, true, 400.0, 800.0), 0.0);
}

#[test]
fn fallback_only_while_focused_and_unmeasured() {
    assert_eq!(ime_cover_px(true, true, 0.0, 800.0), 360.0);
    assert_eq!(ime_cover_px(true, false, 0.0, 800.0), 0.0);
    assert_eq!(ime_cover_px(true, true, 50.0, 800.0), 360.0);
}

#[test]
fn blur_keeps_measured_coverage() {
    assert_eq!(ime_cover_px(true, false, 300.0, 800.0), 300.0);
    assert_eq!(ime_cover_px(true, true, 300.0, 800.0), 300.0);
}
