//! The sync database URL (from ST `src-tauri/src/settings.rs`).

use std::fmt;
use std::net::Ipv4Addr;
use std::str::FromStr;

/// A database URL the sync engines accept: `libsql://`, `https://` or
/// `turso://` with a host, or loopback `http://127.0.0.1:<port>` /
/// `http://localhost:<port>` for a local `tursodb --sync-server`. No path,
/// query or credentials, so a token can't hide in it. Trailing `/` and
/// surrounding spaces are dropped.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SyncUrl(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UrlError {
    Empty,
    Whitespace,
    HttpNotLoopback,
    UnknownScheme,
    HasPathOrCredentials,
}

impl fmt::Display for UrlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Empty => "enter the Turso database URL",
            Self::Whitespace => "the database URL must not contain spaces",
            Self::HttpNotLoopback => {
                "http:// is only for a local server (http://127.0.0.1:<port>); use libsql://"
            }
            Self::UnknownScheme => "the database URL must start with libsql://",
            Self::HasPathOrCredentials => {
                "the database URL is just libsql://<database host>, without a path or token"
            }
        })
    }
}

impl std::error::Error for UrlError {}

/// Schemes that need a host and nothing else.
const HOST_SCHEMES: [&str; 3] = ["libsql://", "https://", "turso://"];
const LOOPBACK_SCHEME: &str = "http://";
const LOOPBACK_NAME: &str = "localhost";

impl FromStr for SyncUrl {
    type Err = UrlError;

    fn from_str(raw: &str) -> Result<Self, UrlError> {
        let url = raw.trim().trim_end_matches('/');
        if url.is_empty() {
            return Err(UrlError::Empty);
        }
        if url.chars().any(char::is_whitespace) {
            return Err(UrlError::Whitespace);
        }
        if let Some(rest) = url.strip_prefix(LOOPBACK_SCHEME) {
            return if is_loopback_host_port(rest) {
                Ok(Self(url.to_string()))
            } else {
                Err(UrlError::HttpNotLoopback)
            };
        }
        let host = HOST_SCHEMES
            .iter()
            .find_map(|scheme| url.strip_prefix(scheme))
            .ok_or(UrlError::UnknownScheme)?;
        if host.is_empty() || has_path_or_credentials(host) {
            return Err(UrlError::HasPathOrCredentials);
        }
        Ok(Self(url.to_string()))
    }
}

fn has_path_or_credentials(rest: &str) -> bool {
    rest.contains(['/', '?', '#', '@'])
}

/// `127.0.0.1:<port>` or `localhost:<port>`, port 1–65535, nothing else.
fn is_loopback_host_port(rest: &str) -> bool {
    if has_path_or_credentials(rest) {
        return false;
    }
    let Some((host, port)) = rest.split_once(':') else {
        return false;
    };
    let loopback = host == LOOPBACK_NAME || host.parse::<Ipv4Addr>() == Ok(Ipv4Addr::LOCALHOST);
    loopback && matches!(port.parse::<u16>(), Ok(parsed) if parsed > 0)
}

impl AsRef<str> for SyncUrl {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SyncUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests;
