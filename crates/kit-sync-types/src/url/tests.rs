//! Accept/reject table ported from ST `settings.rs`
//! (`urls_are_turso_hosts_or_loopback`). Example and loopback hosts only.

use super::*;

const URL: &str = "libsql://db.example.turso.io";

#[test]
fn urls_are_turso_hosts_or_loopback() {
    assert_eq!(
        format!("  {URL}/ ").parse::<SyncUrl>().unwrap().as_ref(),
        URL
    );
    assert!("https://db.example.turso.io".parse::<SyncUrl>().is_ok());
    assert!("turso://db.example.turso.io".parse::<SyncUrl>().is_ok());
    assert!("http://127.0.0.1:8080".parse::<SyncUrl>().is_ok());
    assert!("http://localhost:9".parse::<SyncUrl>().is_ok());
    for (bad, why) in [
        ("", UrlError::Empty),
        ("   ", UrlError::Empty),
        ("libsql://db.example .turso.io", UrlError::Whitespace),
        ("db.example.turso.io", UrlError::UnknownScheme),
        ("ftp://db.example.turso.io", UrlError::UnknownScheme),
        ("libsql://", UrlError::UnknownScheme),
        (
            "libsql://db.example.turso.io?authToken=x",
            UrlError::HasPathOrCredentials,
        ),
        (
            "libsql://user@db.example.turso.io",
            UrlError::HasPathOrCredentials,
        ),
        (
            "libsql://db.example.turso.io/path",
            UrlError::HasPathOrCredentials,
        ),
        ("http://example.test:8080", UrlError::HttpNotLoopback),
        ("http://127.0.0.1", UrlError::HttpNotLoopback),
        ("http://127.0.0.1:0", UrlError::HttpNotLoopback),
        ("http://127.0.0.1:8080/path", UrlError::HttpNotLoopback),
        ("http://127.0.0.2:8080", UrlError::HttpNotLoopback),
        ("http://127.000.000.001:8080", UrlError::HttpNotLoopback),
    ] {
        assert_eq!(bad.parse::<SyncUrl>(), Err(why), "{bad:?}");
    }
}

#[test]
fn display_is_the_parsed_url() {
    let url: SyncUrl = "http://localhost:9/".parse().unwrap();
    assert_eq!(url.to_string(), "http://localhost:9");
}

#[test]
fn errors_keep_the_st_messages() {
    assert_eq!(UrlError::Empty.to_string(), "enter the Turso database URL");
    assert_eq!(
        UrlError::UnknownScheme.to_string(),
        "the database URL must start with libsql://"
    );
}
