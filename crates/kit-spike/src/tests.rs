use super::Tone;

#[test]
fn tones_are_full_daisyui_literals() {
    assert_eq!(Tone::Accent.class(), "badge badge-accent badge-xl");
    assert_eq!(Tone::Warning.class(), "badge badge-warning badge-outline");
}
