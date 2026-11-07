//! The global pirs folder.
//!
//! Default is `~/.pirs`. `PIRS_HOME` overrides it, which makes the app
//! relocatable and lets end-to-end tests use a temp folder.
//!
//! Rust note: an environment variable is process-wide. Unit tests that set
//! one can disturb each other, so the path logic is split into a pure
//! `pirs_home_from` helper and only one test touches the real variable.

use std::path::PathBuf;

/// The global folder: `PIRS_HOME` when set and not empty, else `~/.pirs`.
pub fn pirs_home() -> PathBuf {
    let env = std::env::var("PIRS_HOME")
        .ok()
        .filter(|text| !text.is_empty());
    pirs_home_from(env.as_deref())
}

/// Pure version of `pirs_home`, so tests can pass a value instead of setting
/// the process environment.
pub fn pirs_home_from(value: Option<&str>) -> PathBuf {
    match value {
        Some(text) => PathBuf::from(text),
        None => dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".pirs"),
    }
}

#[cfg(test)]
#[path = "home_tests.rs"]
mod tests;
