use std::fmt;

/// First 7 hex digits of a commit SHA, lowercased.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ShortSha(String);

impl ShortSha {
    pub const LEN: usize = 7;

    /// Parses `git rev-parse HEAD` output or a CI SHA. Surrounding whitespace
    /// is ignored; anything shorter than 7 hex digits is `None`.
    pub fn parse(full: &str) -> Option<Self> {
        let head = full.trim().get(..Self::LEN)?;
        head.chars()
            .all(|c| c.is_ascii_hexdigit())
            .then(|| Self(head.to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ShortSha {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The SHA a build reports: a known commit, or `unknown` (no git, no CI var).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GitSha {
    Known(ShortSha),
    Unknown,
}

impl fmt::Display for GitSha {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Known(sha) => sha.fmt(f),
            Self::Unknown => f.write_str("unknown"),
        }
    }
}

#[cfg(test)]
mod tests;
