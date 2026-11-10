use super::*;

#[test]
fn env_prefix_accepts_both_apps() {
    for ok in ["OCTOBUDGET", "SICKNESS_TRACKER", "TRACKER", "A1"] {
        let p: EnvPrefix = ok.parse().unwrap();
        assert_eq!(p.as_str(), ok);
    }
}

#[test]
fn env_prefix_builds_var_names() {
    let p: EnvPrefix = "OCTOBUDGET".parse().unwrap();
    assert_eq!(p.var("GIT_SHA"), "OCTOBUDGET_GIT_SHA");
}

#[test]
fn env_prefix_rejects_bad_input() {
    let kind = IdentityKind::EnvPrefix;
    assert_eq!(
        "".parse::<EnvPrefix>(),
        Err(ParseIdentityError::Empty(kind))
    );
    assert_eq!(
        "octobudget".parse::<EnvPrefix>(),
        Err(ParseIdentityError::InvalidChar {
            kind,
            at: 0,
            ch: 'o'
        })
    );
    assert_eq!(
        "1APP".parse::<EnvPrefix>(),
        Err(ParseIdentityError::InvalidChar {
            kind,
            at: 0,
            ch: '1'
        })
    );
    assert_eq!(
        "APP_".parse::<EnvPrefix>(),
        Err(ParseIdentityError::InvalidChar {
            kind,
            at: 3,
            ch: '_'
        })
    );
    assert_eq!(
        "MY-APP".parse::<EnvPrefix>(),
        Err(ParseIdentityError::InvalidChar {
            kind,
            at: 2,
            ch: '-'
        })
    );
    assert_eq!(
        "A".repeat(33).parse::<EnvPrefix>(),
        Err(ParseIdentityError::TooLong {
            kind,
            max: 32,
            len: 33
        })
    );
}

#[test]
fn log_tag_accepts_both_apps() {
    for ok in ["octobudget", "sickness-tracker", "a.b_c"] {
        assert_eq!(LogTag::try_from(ok).unwrap().to_string(), ok);
    }
}

#[test]
fn log_tag_rejects_bad_input() {
    let kind = IdentityKind::LogTag;
    assert_eq!(
        "-tag".parse::<LogTag>(),
        Err(ParseIdentityError::InvalidChar {
            kind,
            at: 0,
            ch: '-'
        })
    );
    assert_eq!(
        "my tag".parse::<LogTag>(),
        Err(ParseIdentityError::InvalidChar {
            kind,
            at: 2,
            ch: ' '
        })
    );
    assert_eq!(
        "t".repeat(24).parse::<LogTag>(),
        Err(ParseIdentityError::TooLong {
            kind,
            max: 23,
            len: 24
        })
    );
}

#[test]
fn storage_prefix_builds_keys() {
    let p: StoragePrefix = "sickness-tracker".parse().unwrap();
    assert_eq!(p.key("theme"), "sickness-tracker.theme");
    assert_eq!(p.key("mode"), "sickness-tracker.mode");
}

#[test]
fn storage_prefix_rejects_bad_input() {
    let kind = IdentityKind::StoragePrefix;
    assert_eq!(
        "Octobudget".parse::<StoragePrefix>(),
        Err(ParseIdentityError::InvalidChar {
            kind,
            at: 0,
            ch: 'O'
        })
    );
    assert_eq!(
        "app.theme".parse::<StoragePrefix>(),
        Err(ParseIdentityError::InvalidChar {
            kind,
            at: 3,
            ch: '.'
        })
    );
    assert_eq!(
        "app-".parse::<StoragePrefix>(),
        Err(ParseIdentityError::InvalidChar {
            kind,
            at: 3,
            ch: '-'
        })
    );
}

#[test]
fn errors_read_well() {
    let err = "app-".parse::<StoragePrefix>().unwrap_err();
    assert_eq!(
        err.to_string(),
        "storage prefix has '-' at position 3, which is not allowed there"
    );
}

#[test]
fn literal_matches_parse() {
    let cases = [
        "",
        "octobudget",
        "sickness-tracker",
        "a.b_c",
        "-tag",
        "tag-",
        "my tag",
        "tåg",
        "t🦀g",
        "SICKNESS_TRACKER",
        "APP_",
        "app-",
        "Octobudget",
    ];
    for s in cases {
        assert_eq!(
            LogTag::literal(s).map(LogTag::from_literal),
            s.parse::<LogTag>(),
            "{s:?}"
        );
        assert_eq!(
            EnvPrefix::literal(s).map(EnvPrefix::from_literal),
            s.parse::<EnvPrefix>(),
            "{s:?}"
        );
        assert_eq!(
            StoragePrefix::literal(s).map(StoragePrefix::from_literal),
            s.parse::<StoragePrefix>(),
            "{s:?}"
        );
    }
    let long = "t".repeat(24);
    assert_eq!(
        LogTag::literal(long.leak()),
        Err(ParseIdentityError::TooLong {
            kind: IdentityKind::LogTag,
            max: 23,
            len: 24
        })
    );
}

#[test]
fn non_ascii_is_reported_as_the_char() {
    assert_eq!(
        LogTag::literal("t🦀g"),
        Err(ParseIdentityError::InvalidChar {
            kind: IdentityKind::LogTag,
            at: 1,
            ch: '🦀'
        })
    );
    assert_eq!(
        "ta€".parse::<LogTag>(),
        Err(ParseIdentityError::InvalidChar {
            kind: IdentityKind::LogTag,
            at: 2,
            ch: '€'
        })
    );
}

#[test]
fn the_macro_checks_at_compile_time() {
    const TAG: LogTag = crate::log_tag!("kit-test");
    assert_eq!(TAG, "kit-test".parse().unwrap());
    assert_eq!(crate::log_tag!("a.b_c").as_str(), "a.b_c");
}
