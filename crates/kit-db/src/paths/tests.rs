use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use kit_core::EnvPrefix;

use super::*;

fn prefix() -> EnvPrefix {
    "MY_APP".parse().unwrap()
}

#[test]
fn blank_override_is_unset() {
    assert_eq!(parse_path_override(None), None);
    assert_eq!(parse_path_override(Some("")), None);
    assert_eq!(parse_path_override(Some("   ")), None);
}

#[test]
fn override_trims_whitespace() {
    assert_eq!(
        parse_path_override(Some(" /tmp/my-app-e2e ")),
        Some(PathBuf::from("/tmp/my-app-e2e"))
    );
}

#[test]
fn blank_clock_is_unset() {
    for raw in [None, Some(""), Some("   ")] {
        assert_eq!(parse_clock_override(raw, "MY_APP_CLOCK").unwrap(), None);
    }
}

#[test]
fn clock_parses_local_calendar_day() {
    assert_eq!(
        parse_clock_override(Some(" 2026-08-25 "), "MY_APP_CLOCK").unwrap(),
        Some(NaiveDate::from_ymd_opt(2026, 8, 25).unwrap())
    );
}

#[test]
fn clock_rejects_bad_input_and_names_the_variable() {
    for raw in ["August", "2026-13-01", "2026-08"] {
        let err = parse_clock_override(Some(raw), "MY_APP_CLOCK").unwrap_err();
        assert_eq!(err.to_string(), "MY_APP_CLOCK must be YYYY-MM-DD", "{raw}");
    }
}

#[test]
fn override_names_come_from_the_prefix() {
    let vars = EnvOverrides::new(&prefix());
    assert_eq!(vars.data_dir_var(), "MY_APP_DATA_DIR");
    assert_eq!(vars.db_path_var(), "MY_APP_DB_PATH");
    assert_eq!(vars.clock_var(), "MY_APP_CLOCK");
}

#[test]
fn workspace_tursodb_bin_is_under_e2e() {
    let path = workspace_tursodb_bin(Path::new("/repo/src-tauri"));
    assert_eq!(
        path,
        PathBuf::from("/repo/src-tauri/../e2e/.tursodb/tursodb")
    );
}

fn touch(path: &Path) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, []).unwrap();
}

#[test]
fn find_tursodb_prefers_existing_override() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("custom-tursodb");
    touch(&bin);
    let found = find_tursodb_bin_in(
        Some(bin.to_str().unwrap()),
        None,
        None,
        &dir.path().join("unused"),
    );
    assert_eq!(found.as_deref(), Some(bin.as_path()));
}

#[test]
fn find_tursodb_ignores_missing_override() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("e2e/.tursodb/tursodb");
    touch(&workspace);
    let found = find_tursodb_bin_in(
        Some(dir.path().join("missing").to_str().unwrap()),
        None,
        None,
        &workspace,
    );
    assert_eq!(found.as_deref(), Some(workspace.as_path()));
}

#[test]
fn find_tursodb_uses_home_then_path() {
    let dir = tempfile::tempdir().unwrap();
    let home_bin = dir.path().join("home/.turso/tursodb");
    touch(&home_bin);
    let found_home = find_tursodb_bin_in(
        None,
        Some(&dir.path().join("home")),
        None,
        &dir.path().join("unused"),
    );
    assert_eq!(found_home.as_deref(), Some(home_bin.as_path()));

    let path_dir = dir.path().join("bin");
    let path_bin = path_dir.join("tursodb");
    touch(&path_bin);
    let found_path = find_tursodb_bin_in(
        None,
        None,
        Some(path_dir.as_os_str()),
        &dir.path().join("unused"),
    );
    assert_eq!(found_path.as_deref(), Some(path_bin.as_path()));
    assert_eq!(
        find_tursodb_bin_in(None, None, None, Path::new("/no/such/tursodb")),
        None
    );
}

#[test]
fn missing_tursodb_error_names_the_test_and_the_fetch_script() {
    let err = Error::TursodbNotFound {
        test: "cdc_round_trip".into(),
        workspace: PathBuf::from("/repo/e2e/.tursodb/tursodb"),
    };
    let msg = err.to_string();
    assert!(
        msg.starts_with("cdc_round_trip: tursodb not found"),
        "{msg}"
    );
    assert!(msg.contains("scripts/fetch-tursodb.sh"), "{msg}");
}
