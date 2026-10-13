//! Typed domain errors.

use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("no config file found; searched: {0}")]
    ConfigNotFound(String),

    #[error("config file {0} does not exist")]
    ConfigMissing(PathBuf),

    #[error("failed to parse config {path}: {source}")]
    ConfigParse {
        path: PathBuf,
        source: toml::de::Error,
    },

    #[error("invalid config: {0}")]
    ConfigInvalid(String),

    #[error("invalid exclude pattern {pattern:?}: {source}")]
    Glob {
        pattern: String,
        source: globset::Error,
    },

    #[error("home directory could not be determined")]
    NoHomeDir,

    #[error("source directory {0} does not exist or is not a directory")]
    SourceMissing(PathBuf),

    #[error("I/O error on {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to walk {path}: {source}")]
    Walk {
        path: PathBuf,
        source: walkdir::Error,
    },

    #[error("invalid manifest {path}: {message}")]
    Manifest { path: PathBuf, message: String },

    #[error("unsafe path in manifest: {0:?}")]
    UnsafePath(String),

    #[error("failed to run `{program}`: {source}")]
    Spawn {
        program: String,
        source: std::io::Error,
    },

    #[error("`{command}` failed ({status}): {stderr}")]
    Command {
        command: String,
        status: String,
        stderr: String,
    },

    #[error("GPG key for recipient {0:?} was not found")]
    KeyNotFound(String),

    #[error("checksum mismatch for {path}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        path: String,
        expected: String,
        actual: String,
    },

    #[error("refusing to overwrite existing file {0} (use --force)")]
    WouldOverwrite(PathBuf),

    #[error("{failed} file operation(s) failed; first error: {first}")]
    Partial { failed: usize, first: String },
}

impl Error {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }
}
