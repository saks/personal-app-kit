use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ShortSha;

/// `git rev-parse HEAD` in `dir`, or `None` without git or a repo.
pub(crate) fn head_sha(dir: &Path) -> Option<ShortSha> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    ShortSha::parse(std::str::from_utf8(&output.stdout).ok()?)
}

/// Files whose change means a new commit: `.git/HEAD`, and the branch ref it
/// points at when HEAD is symbolic.
pub(crate) fn rerun_paths(dir: &Path) -> Vec<PathBuf> {
    let git_dir = dir.join(".git");
    let head_path = git_dir.join("HEAD");
    let head = fs::read_to_string(&head_path).ok();
    let branch = head
        .as_deref()
        .and_then(symbolic_ref)
        .map(|r| git_dir.join(r));
    std::iter::once(head_path).chain(branch).collect()
}

/// The ref in a symbolic `.git/HEAD` (`ref: refs/heads/main`), or `None` for
/// a detached HEAD.
pub(crate) fn symbolic_ref(head: &str) -> Option<&str> {
    head.trim().strip_prefix("ref: ")
}

#[cfg(test)]
mod tests;
