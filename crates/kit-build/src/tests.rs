use std::path::PathBuf;

use super::*;

fn prefix() -> EnvPrefix {
    "OCTOBUDGET".parse().unwrap()
}

#[test]
fn directives_match_the_apps_build_rs() {
    let info = BuildInfo {
        sha: ShortSha::parse("e3fa3f9").map_or(GitSha::Unknown, GitSha::Known),
        built_at: "2026-11-09 23:56 UTC".into(),
        rerun_if_changed: vec![
            PathBuf::from("/app/.git/HEAD"),
            PathBuf::from("/app/.git/refs/heads/main"),
        ],
    };
    assert_eq!(
        info.directives(&prefix()),
        [
            "cargo:rerun-if-changed=/app/.git/HEAD",
            "cargo:rerun-if-changed=/app/.git/refs/heads/main",
            "cargo:rerun-if-env-changed=GITHUB_SHA",
            "cargo:rustc-env=OCTOBUDGET_GIT_SHA=e3fa3f9",
            "cargo:rustc-env=OCTOBUDGET_BUILT_AT=2026-11-09 23:56 UTC",
        ]
    );
}

#[test]
fn unknown_sha_is_reported_as_unknown() {
    let info = BuildInfo {
        sha: GitSha::Unknown,
        built_at: "2026-11-09 23:56 UTC".into(),
        rerun_if_changed: Vec::new(),
    };
    let tracker: EnvPrefix = "TRACKER".parse().unwrap();
    assert!(
        info.directives(&tracker)
            .contains(&"cargo:rustc-env=TRACKER_GIT_SHA=unknown".to_owned())
    );
}

#[test]
fn collect_reports_a_utc_build_time() {
    let info = BuildInfo::collect(&PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    assert!(info.built_at.ends_with(" UTC"));
    assert_eq!(info.built_at.len(), "YYYY-MM-DD HH:MM UTC".len());
}
