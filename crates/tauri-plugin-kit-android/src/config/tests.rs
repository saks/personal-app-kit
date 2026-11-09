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
