use super::*;

#[test]
fn symbolic_head_names_the_branch_ref() {
    assert_eq!(
        symbolic_ref("ref: refs/heads/main\n"),
        Some("refs/heads/main")
    );
}

#[test]
fn detached_head_has_no_ref() {
    assert_eq!(
        symbolic_ref("e3fa3f9c0a1b2c3d4e5f60718293a4b5c6d7e8f9\n"),
        None
    );
}

#[test]
fn rerun_paths_follow_a_symbolic_head() {
    let dir = std::env::temp_dir().join(format!("kit-build-rerun-{}", std::process::id()));
    let git = dir.join(".git");
    fs::create_dir_all(&git).unwrap();
    fs::write(git.join("HEAD"), "ref: refs/heads/main\n").unwrap();

    let paths = rerun_paths(&dir);
    fs::remove_dir_all(&dir).unwrap();

    assert_eq!(paths, vec![git.join("HEAD"), git.join("refs/heads/main")]);
}

#[test]
fn rerun_paths_without_a_repo_still_watch_head() {
    let dir = std::env::temp_dir().join("kit-build-no-such-repo");
    assert_eq!(rerun_paths(&dir), vec![dir.join(".git/HEAD")]);
}
