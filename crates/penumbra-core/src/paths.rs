//! Rules for relative paths inside an install folder. Enforced when publishing *and* when installing, so a
//! malicious or broken manifest can never write outside the install directory.

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PathError {
    #[error("path is empty")]
    Empty,
    #[error("path must be relative and use '/' separators: {0}")]
    NotRelative(String),
    #[error("path contains an empty, '.' or '..' segment: {0}")]
    BadSegment(String),
    #[error("path contains a character not allowed on Windows: {0}")]
    BadChar(String),
    #[error("path uses a reserved Windows name: {0}")]
    Reserved(String),
    #[error("path segment ends with a dot or space: {0}")]
    TrailingDotOrSpace(String),
    #[error("path is too long: {0}")]
    TooLong(String),
}

const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
    "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Validates a manifest path like `data/audio/BGM/song.ogg`.
pub fn validate_rel_path(p: &str) -> Result<(), PathError> {
    if p.is_empty() {
        return Err(PathError::Empty);
    }
    if p.len() > 400 {
        return Err(PathError::TooLong(p.into()));
    }
    if p.starts_with('/') || p.contains('\\') || p.as_bytes().get(1) == Some(&b':') {
        return Err(PathError::NotRelative(p.into()));
    }
    for seg in p.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." {
            return Err(PathError::BadSegment(p.into()));
        }
        if seg.chars().any(|c| c < ' ' || "<>:\"|?*".contains(c)) {
            return Err(PathError::BadChar(p.into()));
        }
        if seg.ends_with('.') || seg.ends_with(' ') {
            return Err(PathError::TrailingDotOrSpace(p.into()));
        }
        let stem = seg.split('.').next().unwrap_or(seg).to_ascii_uppercase();
        if RESERVED.contains(&stem.as_str()) {
            return Err(PathError::Reserved(p.into()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_normal_paths() {
        for p in ["a.exe", "data/audio/BGM/Area - Gym.ogg", "Pokemon - Gracidea Version.exe", ".penumbra-x/y"] {
            assert_eq!(validate_rel_path(p), Ok(()), "{p}");
        }
    }

    #[test]
    fn rejects_escapes_and_windows_traps() {
        for p in [
            "", "/etc/passwd", "C:/x", "a\\b", "../x", "a/../b", "a//b", "./a", "con", "data/NUL.txt",
            "lpt1.log", "a/b.", "a/b ", "a/b?c", "a/b\u{1}c",
        ] {
            assert!(validate_rel_path(p).is_err(), "{p:?} should be rejected");
        }
    }
}
