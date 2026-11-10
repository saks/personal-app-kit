use super::*;

#[test]
fn lib_names_are_bare_identifiers() {
    for good in ["sickness_tracker_lib", "budget_app_lib", "a1"] {
        assert_eq!(good.parse::<LibName>().unwrap().as_str(), good);
    }
    for (bad, why) in [
        ("", LibNameError::Empty),
        ("libfoo.so", LibNameError::FileName),
        ("foo-lib", LibNameError::InvalidChar { at: 3, ch: '-' }),
        ("1foo", LibNameError::InvalidChar { at: 0, ch: '1' }),
        ("foo bar", LibNameError::InvalidChar { at: 3, ch: ' ' }),
    ] {
        assert_eq!(bad.parse::<LibName>(), Err(why), "{bad:?}");
    }
}

#[test]
fn literals_follow_the_same_rules() {
    for raw in [
        "sickness_tracker_lib",
        "a1",
        "",
        "libfoo.so",
        "foo-lib",
        "1foo",
        "fö",
        "a🦀",
        "so",
    ] {
        assert_eq!(
            LibName::literal(raw).map(LibName::from_literal),
            raw.parse::<LibName>(),
            "{raw:?}"
        );
    }
    assert_eq!(
        LibName::literal("fö"),
        Err(LibNameError::InvalidChar { at: 1, ch: 'ö' })
    );
}

#[test]
fn the_macro_checks_at_compile_time() {
    const NAME: LibName = crate::lib_name!("budget_app_lib");
    assert_eq!(NAME, "budget_app_lib".parse().unwrap());
}
