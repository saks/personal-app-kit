use super::*;

fn short(s: &str) -> Option<String> {
    ShortSha::parse(s).map(|sha| sha.to_string())
}

#[test]
fn takes_the_first_seven_hex_digits() {
    assert_eq!(
        short("e3fa3f9c0a1b2c3d4e5f60718293a4b5c6d7e8f9").as_deref(),
        Some("e3fa3f9")
    );
}

#[test]
fn trims_rev_parse_newline_and_spaces() {
    assert_eq!(short("1e6df1a0123456\n").as_deref(), Some("1e6df1a"));
    assert_eq!(short("  1e6df1a  ").as_deref(), Some("1e6df1a"));
}

#[test]
fn lowercases() {
    assert_eq!(short("ABCDEF0123").as_deref(), Some("abcdef0"));
}

#[test]
fn exactly_seven_is_enough() {
    assert_eq!(short("abcdef0").as_deref(), Some("abcdef0"));
}

#[test]
fn rejects_short_empty_and_non_hex() {
    assert_eq!(short(""), None);
    assert_eq!(short("   \n"), None);
    assert_eq!(short("abc123"), None);
    assert_eq!(short("abcdefg123"), None);
    assert_eq!(short("ref: refs/heads/main"), None);
}

#[test]
fn ignores_what_follows_the_prefix() {
    // Only the first 7 characters must be hex, as in the apps' build.rs.
    assert_eq!(short("abcdef0-dirty").as_deref(), Some("abcdef0"));
}

#[test]
fn rejects_multibyte_without_panicking() {
    assert_eq!(short("abcdeé0123"), None);
    assert_eq!(short("ééééééé"), None);
}

#[test]
fn unknown_displays_as_unknown() {
    assert_eq!(GitSha::Unknown.to_string(), "unknown");
    let known = ShortSha::parse("abcdef0").map(GitSha::Known);
    assert_eq!(known.map(|s| s.to_string()).as_deref(), Some("abcdef0"));
}
