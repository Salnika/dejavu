//! Cache/config path resolution and the per-repo cache layout.
//!
//! Note the deliberate macOS asymmetry (spec §7): the *cache* follows the
//! platform convention (`~/Library/Caches` on macOS, `$XDG_CACHE_HOME` on
//! Linux/WSL), but the *config* is XDG-style (`~/.config`) on **all**
//! platforms — never `~/Library/Application Support`.

mod layout;
pub use layout::CacheLayout;

use crate::error::PathError;
use crate::util::sha256_hex;
use std::path::{Path, PathBuf};

fn home() -> Result<PathBuf, PathError> {
    dirs::home_dir().ok_or(PathError::NoHome)
}

/// `<cache>/dejavu` — the root under which every repo gets a `<repo_hash>` dir.
/// macOS: `~/Library/Caches/dejavu`. Linux/WSL: `$XDG_CACHE_HOME/dejavu` then
/// `~/.cache/dejavu`. `dirs::cache_dir()` already encodes this platform matrix.
pub fn cache_root() -> Result<PathBuf, PathError> {
    let base = dirs::cache_dir().ok_or(PathError::NoCacheDir)?;
    Ok(base.join("dejavu"))
}

/// The repo-independent shim dir used by global activation (`dejavu
/// shellenv`): `<cache_root>/shims/bin`. Never collides with per-repo caches
/// (`<cache_root>/<16-hex-hash>/`).
pub fn global_shims_bin() -> Result<PathBuf, PathError> {
    Ok(cache_root()?.join("shims").join("bin"))
}

/// Global config file: `$XDG_CONFIG_HOME/dejavu/config.toml`, else
/// `~/.config/dejavu/config.toml` — XDG on every platform.
pub fn config_file_path() -> Result<PathBuf, PathError> {
    let base = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(v) if !v.is_empty() && Path::new(&v).is_absolute() => PathBuf::from(v),
        _ => home()?.join(".config"),
    };
    Ok(base.join("dejavu").join("config.toml"))
}

/// Stable 16-hex-char id for a repo, derived from its canonicalized absolute
/// path (symlinks resolved so two paths to the same repo collide correctly).
pub fn repo_hash(repo_root: &Path) -> String {
    let canonical = std::fs::canonicalize(repo_root).unwrap_or_else(|_| repo_root.to_path_buf());
    let full = sha256_hex(canonical.to_string_lossy().as_bytes());
    full[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_hash_is_stable_and_16_hex() {
        let a = repo_hash(Path::new("/nonexistent/path/one"));
        let b = repo_hash(Path::new("/nonexistent/path/one"));
        let c = repo_hash(Path::new("/nonexistent/path/two"));
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 16);
        assert!(a.chars().all(|ch| ch.is_ascii_hexdigit()));
    }
}
