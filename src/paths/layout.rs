//! On-disk layout of a repo's cache dir (spec §7.2).

use super::{cache_root, repo_hash};
use crate::error::PathError;
use std::path::{Path, PathBuf};

/// `<cache>/dejavu/<repo_hash>/` and everything under it.
#[derive(Debug, Clone)]
pub struct CacheLayout {
    pub root: PathBuf,
}

impl CacheLayout {
    /// Derive the layout for a repo from its root path.
    pub fn for_repo(repo_root: &Path) -> Result<Self, PathError> {
        Ok(Self {
            root: cache_root()?.join(repo_hash(repo_root)),
        })
    }

    /// Use an explicit cache dir (e.g. from `DEJAVU_CACHE_DIR` in a session, or
    /// a temp dir in tests).
    pub fn from_dir(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn shims_bin(&self) -> PathBuf {
        self.root.join("shims").join("bin")
    }
    pub fn db(&self) -> PathBuf {
        self.root.join("runs.sqlite")
    }
    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }
    pub fn stdout_log(&self, run_id: &str) -> PathBuf {
        self.logs_dir().join(format!("{run_id}.stdout"))
    }
    pub fn stderr_log(&self, run_id: &str) -> PathBuf {
        self.logs_dir().join(format!("{run_id}.stderr"))
    }
    pub fn normalized_log(&self, run_id: &str) -> PathBuf {
        self.logs_dir().join(format!("{run_id}.normalized.txt"))
    }
    pub fn sessions_dir(&self) -> PathBuf {
        self.root.join("sessions")
    }
    pub fn session_file(&self, session_id: &str) -> PathBuf {
        self.sessions_dir().join(format!("{session_id}.jsonl"))
    }
    pub fn effective_config(&self) -> PathBuf {
        self.root.join("config.effective.json")
    }
    pub fn state_file(&self) -> PathBuf {
        self.root.join("state.json")
    }
    /// Wrapper ZDOTDIR used to re-assert the shim PATH after login-shell init.
    pub fn zdot_dir(&self) -> PathBuf {
        self.root.join("zdot")
    }

    /// `mkdir -p` all cache subdirectories. Idempotent.
    pub fn ensure_dirs(&self) -> Result<(), PathError> {
        for dir in [
            self.root.clone(),
            self.shims_bin(),
            self.logs_dir(),
            self.sessions_dir(),
        ] {
            std::fs::create_dir_all(&dir)?;
        }
        Ok(())
    }
}
