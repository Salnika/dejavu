//! Typed error enums for the library. Binaries convert these at the boundary
//! with `anyhow`.

use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PathError {
    #[error("could not determine the home directory")]
    NoHome,
    #[error("could not determine the cache directory")]
    NoCacheDir,
    #[error("path is not writable: {0}")]
    NotWritable(PathBuf),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("failed to parse TOML config at {path}: {source}")]
    Toml {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Path(#[from] PathError),
}

#[derive(Error, Debug)]
pub enum StoreError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
