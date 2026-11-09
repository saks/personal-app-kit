use super::*;

#[test]
fn tokens_are_trimmed_and_one_word() {
    let token: AuthToken = "  dummy-token \n".parse().unwrap();
    assert_eq!(token.expose(), "dummy-token");
    assert_eq!("".parse::<AuthToken>(), Err(TokenError::Empty));
    assert_eq!("  ".parse::<AuthToken>(), Err(TokenError::Empty));
    assert_eq!(
        "two words".parse::<AuthToken>(),
        Err(TokenError::Whitespace)
    );
}

#[test]
fn debug_never_prints_the_token() {
    let token: AuthToken = "dummy-secret-token".parse().unwrap();
    let debug = format!("{token:?}");
    assert!(!debug.contains("dummy-secret-token"), "{debug}");
    assert_eq!(debug, "AuthToken(<redacted>)");
}
