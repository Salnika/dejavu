//! `DEJAVU_*` environment variable names and the ambient session view.

use std::path::PathBuf;

pub const ACTIVE: &str = "DEJAVU_ACTIVE";
pub const BIN: &str = "DEJAVU_BIN";
pub const REPO_ROOT: &str = "DEJAVU_REPO_ROOT";
pub const CACHE_DIR: &str = "DEJAVU_CACHE_DIR";
pub const SESSION_ID: &str = "DEJAVU_SESSION_ID";
pub const SHIM_DIR: &str = "DEJAVU_SHIM_DIR";
pub const MODE: &str = "DEJAVU";
pub const DISABLED: &str = "DEJAVU_DISABLED";
/// The user's original ZDOTDIR (or $HOME), sourced by the wrapper zdot files.
pub const ORIG_ZDOTDIR: &str = "DEJAVU_ORIG_ZDOTDIR";

/// True when the user asks Dejavu to force passthrough everywhere.
pub fn is_disabled() -> bool {
    let legacy_flag = std::env::var_os(DISABLED).is_some_and(|v| v == "1");
    let mode_off = std::env::var_os(MODE).is_some_and(|v| {
        matches!(
            v.to_string_lossy().to_ascii_lowercase().as_str(),
            "off" | "0" | "false" | "disabled"
        )
    });
    legacy_flag || mode_off
}

/// True when running inside a `dejavu start` session.
pub fn is_active() -> bool {
    std::env::var_os(ACTIVE).is_some_and(|v| v == "1")
}

/// The ambient session, reconstructed from `DEJAVU_*` env vars. `None` when a
/// shim is somehow invoked outside a session.
#[derive(Debug, Clone)]
pub struct AgentEnv {
    pub bin: PathBuf,
    pub repo_root: PathBuf,
    pub cache_dir: PathBuf,
    pub session_id: String,
    pub shim_dir: PathBuf,
}

impl AgentEnv {
    pub fn from_current() -> Option<AgentEnv> {
        Some(AgentEnv {
            bin: std::env::var_os(BIN)?.into(),
            repo_root: std::env::var_os(REPO_ROOT)?.into(),
            cache_dir: std::env::var_os(CACHE_DIR)?.into(),
            session_id: std::env::var_os(SESSION_ID)?.to_string_lossy().into_owned(),
            shim_dir: std::env::var_os(SHIM_DIR)?.into(),
        })
    }
}
