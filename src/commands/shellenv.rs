//! `dejavu shellenv` — global activation. Generates a repo-independent shim
//! directory and prints the shell line that puts it first on `PATH`:
//!
//! ```sh
//! eval "$(dejavu shellenv)"   # at the END of ~/.zprofile
//! ```
//!
//! This covers agents Dejavu cannot launch itself: IDE integrated terminals
//! (VS Code + Copilot agent mode), GUI apps that run commands through a login
//! zsh, etc. Shims are self-sufficient without any `DEJAVU_*` variable: the
//! runtime rebuilds the repo context from the working directory, and shim
//! self-identification prevents recursion.

use crate::config::Config;
use crate::exec::shim::{generate_shims, ShimContext};
use std::path::PathBuf;

/// The repo-independent shim dir: `<cache_root>/shims/bin`. Never collides
/// with per-repo caches (`<cache_root>/<16-hex-hash>/`) and holds no
/// `runs.sqlite`, so `stats --all` skips it.
pub fn global_shim_dir() -> anyhow::Result<PathBuf> {
    Ok(crate::paths::cache_root()?.join("shims").join("bin"))
}

pub fn run() -> anyhow::Result<i32> {
    let dir = global_shim_dir()?;
    std::fs::create_dir_all(&dir)?;

    let exe = std::env::current_exe()?;
    let dejavu_bin = std::fs::canonicalize(&exe).unwrap_or(exe);

    // Global config only: the cache root contains no project `.dejavu.toml`.
    let config = Config::load(&crate::paths::cache_root()?)?;
    generate_shims(&ShimContext {
        shim_dir: dir.clone(),
        dejavu_bin,
        enabled: config.intercept.enabled_shims(),
    })?;

    // Idempotent POSIX guard: safe to eval multiple times.
    let d = dir.display();
    println!("case \":$PATH:\" in *\":{d}:\"*) ;; *) export PATH=\"{d}:$PATH\" ;; esac");
    Ok(0)
}
