//! Shared logic of both apps' root `build.rs`.
//!
//! ```ignore
//! // build.rs, with `kit-build` and `kit-core` as build-dependencies
//! fn main() {
//!     match "OCTOBUDGET".parse::<kit_core::EnvPrefix>() {
//!         Ok(prefix) => kit_build::emit_git_info(&prefix),
//!         Err(err) => println!("cargo:warning=bad env prefix: {err}"),
//!     }
//! }
//! ```
//!
//! That sets `OCTOBUDGET_GIT_SHA` (7-char lowercase SHA, or `unknown`) and
//! `OCTOBUDGET_BUILT_AT` (`YYYY-MM-DD HH:MM UTC`) for `env!`.

mod git;
mod sha;

use std::path::{Path, PathBuf};

use kit_core::EnvPrefix;

pub use sha::{GitSha, ShortSha};

/// CI env var read when `git rev-parse` is unavailable.
const CI_SHA_VAR: &str = "GITHUB_SHA";

/// Everything one build script run reports to cargo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildInfo {
    pub sha: GitSha,
    pub built_at: String,
    pub rerun_if_changed: Vec<PathBuf>,
}

impl BuildInfo {
    /// Reads git state for the crate at `manifest_dir` and the current UTC time.
    pub fn collect(manifest_dir: &Path) -> Self {
        let sha = git::head_sha(manifest_dir)
            .or_else(|| {
                std::env::var(CI_SHA_VAR)
                    .ok()
                    .as_deref()
                    .and_then(ShortSha::parse)
            })
            .map_or(GitSha::Unknown, GitSha::Known);
        Self {
            sha,
            built_at: chrono::Utc::now().format("%Y-%m-%d %H:%M UTC").to_string(),
            rerun_if_changed: git::rerun_paths(manifest_dir),
        }
    }

    /// The `cargo:` lines for this info, in print order.
    pub fn directives(&self, prefix: &EnvPrefix) -> Vec<String> {
        let mut out: Vec<String> = self
            .rerun_if_changed
            .iter()
            .map(|p| format!("cargo:rerun-if-changed={}", p.display()))
            .collect();
        out.push(format!("cargo:rerun-if-env-changed={CI_SHA_VAR}"));
        out.push(format!(
            "cargo:rustc-env={}={}",
            prefix.var("GIT_SHA"),
            self.sha
        ));
        out.push(format!(
            "cargo:rustc-env={}={}",
            prefix.var("BUILT_AT"),
            self.built_at
        ));
        out
    }
}

/// Call from a build script: emits `<PREFIX>_GIT_SHA` and `<PREFIX>_BUILT_AT`
/// for the crate being built, plus the rerun triggers for a new commit.
pub fn emit_git_info(prefix: &EnvPrefix) {
    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into()));
    for line in BuildInfo::collect(&manifest_dir).directives(prefix) {
        println!("{line}");
    }
}

#[cfg(test)]
mod tests;
