use std::path::PathBuf;

/// Everything kit-db can fail with. Each app maps it into its own IPC error.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Turso(#[from] turso::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    NotUtf8(#[from] std::string::FromUtf8Error),
    #[error("value is not a number")]
    NotANumber,
    #[error("value is not an integer")]
    NotAnInteger,
    #[error("u64 value does not fit in SQLite integer")]
    IntegerOverflow,
    #[error("unsupported sea-query enum value")]
    UnsupportedEnum,
    #[error("{var} must be YYYY-MM-DD")]
    InvalidClock { var: String },
    #[error("timestamp is required")]
    EmptyTimestamp,
    #[error("timestamp must be an ISO-8601 datetime")]
    InvalidTimestamp,
    #[error(
        "{test}: tursodb not found (looked in {}); run ./scripts/fetch-tursodb.sh (make check / make e2e)",
        workspace.display()
    )]
    TursodbNotFound { test: String, workspace: PathBuf },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    /// Turso is busy (sync apply / checkpoint in flight), so the call may
    /// succeed if retried. Retrying is the app's policy, not the kit's.
    pub fn is_busy(&self) -> bool {
        match self {
            Error::Turso(turso::Error::Busy(_) | turso::Error::BusySnapshot(_)) => true,
            Error::Turso(other) => is_busy_message(&other.to_string()),
            _ => false,
        }
    }
}

/// turso 0.8.3's SyncBusy gate and some paths only say "Database is busy" in
/// text (`sync.rs` `is_sync_busy_error` matches the same string). Drop this
/// once every busy path returns the typed `turso::Error::Busy`.
pub(crate) fn is_busy_message(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("database is busy") || lower.contains("sync operation is in progress")
}

#[cfg(test)]
mod tests;
