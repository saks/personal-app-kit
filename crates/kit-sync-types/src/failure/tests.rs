use super::*;

/// An app's kinds, as an app would classify them.
#[derive(Clone, Copy)]
enum AppKind {
    TokenRejected,
    Offline,
}

impl FailureKind for AppKind {
    fn class(self) -> FailureClass {
        match self {
            Self::TokenRejected => FailureClass::NeedsUser,
            Self::Offline => FailureClass::ClearsByItself,
        }
    }
}

#[test]
fn an_app_kind_picks_its_class() {
    assert_eq!(AppKind::TokenRejected.class(), FailureClass::NeedsUser);
    assert_eq!(AppKind::Offline.class(), FailureClass::ClearsByItself);
}
