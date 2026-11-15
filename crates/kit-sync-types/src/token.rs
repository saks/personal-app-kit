//! The sync database token (from ST `src-tauri/src/settings.rs`).

use std::fmt;
use std::str::FromStr;

use zeroize::{Zeroize, ZeroizeOnDrop};

/// A database token. Never printed: `Debug` redacts it and there is no
/// `Display`; [`AuthToken::expose`] is the one way out. Its heap copy is
/// wiped on drop; copies made from [`AuthToken::expose`] are the caller's.
#[derive(Clone, PartialEq, Eq)]
pub struct AuthToken(String);

/// Why a pasted token was rejected. `Display` is the message for the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenError {
    /// Nothing left after trimming the surrounding spaces.
    Empty,
    /// Spaces (or other whitespace) inside the token.
    Whitespace,
}

impl fmt::Display for TokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Empty => "paste a Turso token",
            Self::Whitespace => "the token must not contain spaces",
        })
    }
}

impl std::error::Error for TokenError {}

impl FromStr for AuthToken {
    type Err = TokenError;

    fn from_str(raw: &str) -> Result<Self, TokenError> {
        let token = raw.trim();
        if token.is_empty() {
            return Err(TokenError::Empty);
        }
        if token.chars().any(char::is_whitespace) {
            return Err(TokenError::Whitespace);
        }
        Ok(Self(token.to_string()))
    }
}

impl AuthToken {
    /// The raw token, for the engine's auth header only. Never log it.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

// Not `Zeroize`: a public `zeroize()` would leave an empty token behind.
impl Drop for AuthToken {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl ZeroizeOnDrop for AuthToken {}

impl fmt::Debug for AuthToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AuthToken(<redacted>)")
    }
}

#[cfg(test)]
mod tests;
