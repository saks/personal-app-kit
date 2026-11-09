//! Env overrides shared by both apps, named from the app's [`EnvPrefix`]
//! (`<PREFIX>_DATA_DIR`, `<PREFIX>_DB_PATH`, `<PREFIX>_CLOCK`), and the
//! local `tursodb` CLI lookup their host tests use.
//!
//! Resolving the real data dir (Tauri's `app_data_dir`) and the DB file name
//! stay in each app.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use kit_core::EnvPrefix;

use crate::error::{Error, Result};

/// Points tests at a specific local `tursodb` CLI. Shared by both apps and
/// not app-prefixed, like the CLI itself.
pub const TURSO_DB_BIN_ENV: &str = "TURSO_DB_BIN";

/// Trimmed, non-empty path from an env value. Empty / whitespace is unset.
pub fn parse_path_override(raw: Option<&str>) -> Option<PathBuf> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Trimmed `YYYY-MM-DD`. Empty / whitespace is unset. `var` names the
/// variable in the error.
pub fn parse_clock_override(raw: Option<&str>, var: &str) -> Result<Option<NaiveDate>> {
    let Some(trimmed) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    NaiveDate::parse_from_str(trimmed, "%Y-%m-%d")
        .map(Some)
        .map_err(|_| Error::InvalidClock {
            var: var.to_owned(),
        })
}

/// The app's override variables, named once from its prefix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvOverrides {
    data_dir: String,
    db_path: String,
    clock: String,
}

impl EnvOverrides {
    pub fn new(prefix: &EnvPrefix) -> Self {
        Self {
            data_dir: prefix.var("DATA_DIR"),
            db_path: prefix.var("DB_PATH"),
            clock: prefix.var("CLOCK"),
        }
    }

    pub fn data_dir_var(&self) -> &str {
        &self.data_dir
    }

    pub fn db_path_var(&self) -> &str {
        &self.db_path
    }

    pub fn clock_var(&self) -> &str {
        &self.clock
    }

    /// `<PREFIX>_DATA_DIR`, if set.
    pub fn data_dir(&self) -> Option<PathBuf> {
        parse_path_override(std::env::var(&self.data_dir).ok().as_deref())
    }

    /// `<PREFIX>_DB_PATH`, if set.
    pub fn db_path(&self) -> Option<PathBuf> {
        parse_path_override(std::env::var(&self.db_path).ok().as_deref())
    }

    /// `<PREFIX>_CLOCK` as a local calendar day, if set.
    pub fn clock_date(&self) -> Result<Option<NaiveDate>> {
        parse_clock_override(std::env::var(&self.clock).ok().as_deref(), &self.clock)
    }
}

/// Where both apps' `scripts/fetch-tursodb.sh` puts the pinned CLI, given the
/// app's `src-tauri` manifest dir: `<manifest_dir>/../e2e/.tursodb/tursodb`.
pub fn workspace_tursodb_bin(manifest_dir: &Path) -> PathBuf {
    manifest_dir.join("../e2e/.tursodb/tursodb")
}

/// `TURSO_DB_BIN`, then `workspace`, then `~/.turso/tursodb`, then `PATH`.
pub fn find_tursodb_bin(workspace: &Path) -> Option<PathBuf> {
    find_tursodb_bin_in(
        std::env::var(TURSO_DB_BIN_ENV).ok().as_deref(),
        std::env::var_os("HOME").as_deref().map(Path::new),
        std::env::var_os("PATH").as_deref(),
        workspace,
    )
}

/// [`find_tursodb_bin`] with every input passed in, for tests.
pub fn find_tursodb_bin_in(
    turso_db_bin: Option<&str>,
    home: Option<&Path>,
    path_var: Option<&OsStr>,
    workspace: &Path,
) -> Option<PathBuf> {
    if let Some(path) = parse_path_override(turso_db_bin)
        && path.is_file()
    {
        return Some(path);
    }
    if workspace.is_file() {
        return Some(workspace.to_path_buf());
    }
    if let Some(home) = home {
        let path = home.join(".turso/tursodb");
        if path.is_file() {
            return Some(path);
        }
    }
    std::env::split_paths(path_var?)
        .map(|dir| dir.join("tursodb"))
        .find(|candidate| candidate.is_file())
}

/// The CLI for a host test that needs it. The app's test unwraps, so a
/// missing CLI fails loudly with the fetch hint instead of skipping.
pub fn tursodb_bin_for_test(test_name: &str, workspace: &Path) -> Result<PathBuf> {
    find_tursodb_bin(workspace).ok_or_else(|| Error::TursodbNotFound {
        test: test_name.to_owned(),
        workspace: workspace.to_path_buf(),
    })
}

#[cfg(test)]
mod tests;
