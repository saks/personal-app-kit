use super::*;

#[test]
fn skips_serialize_as_snake_case() {
    let back: SyncSkip = serde_json::from_str("\"debounced\"").unwrap();
    assert_eq!(back, SyncSkip::Debounced);
    for (skip, json) in [
        (SyncSkip::NotConfigured, "\"not_configured\""),
        (SyncSkip::AlreadyRunning, "\"already_running\""),
        (SyncSkip::Debounced, "\"debounced\""),
    ] {
        assert_eq!(serde_json::to_string(&skip).unwrap(), json);
        assert_eq!(serde_json::from_str::<SyncSkip>(json).unwrap(), skip);
    }
}
