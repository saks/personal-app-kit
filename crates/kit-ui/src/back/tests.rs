use super::{
    BackAction, BackClaim, BackListen, BackRelease, OverlayPresence, REGISTER_ATTEMPTS,
    back_action, should_retry_register,
};

#[test]
fn back_listens_only_while_an_overlay_is_open() {
    let mut claim = BackClaim::new();
    assert_eq!(claim.acquire(), BackListen::Start);
    assert_eq!(claim.acquire(), BackListen::Hold);
    assert_eq!(claim.release(), BackRelease::Hold);
    assert_eq!(claim.release(), BackRelease::Stop);
    assert_eq!(claim.release(), BackRelease::Hold);
    assert_eq!(claim.acquire(), BackListen::Start);
}

#[test]
fn back_with_an_overlay_is_escape() {
    assert_eq!(
        back_action(OverlayPresence::Open, false),
        BackAction::Escape
    );
    assert_eq!(back_action(OverlayPresence::Open, true), BackAction::Escape);
    assert_eq!(
        back_action(OverlayPresence::Unknown, false),
        BackAction::Escape
    );
}

#[test]
fn back_during_unregister_does_not_exit() {
    assert_eq!(
        back_action(OverlayPresence::Closed, true),
        BackAction::HistoryBack
    );
    assert_eq!(
        back_action(OverlayPresence::Closed, false),
        BackAction::Ignore
    );
}

#[test]
fn register_failure_retries_while_an_overlay_is_open() {
    assert!(should_retry_register(1, 1));
    assert!(should_retry_register(1, REGISTER_ATTEMPTS - 1));
    assert!(!should_retry_register(1, REGISTER_ATTEMPTS));
    assert!(!should_retry_register(0, 1));
}
