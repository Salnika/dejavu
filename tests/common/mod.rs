//! Shared helpers for integration tests. Not every test binary uses every
//! helper, so silence the per-binary dead-code warnings.
#![allow(dead_code)]

use assert_cmd::Command;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

const DEJAVU_ENV: &[&str] = &[
    "DEJAVU",
    "DEJAVU_ACTIVE",
    "DEJAVU_BIN",
    "DEJAVU_CACHE_DIR",
    "DEJAVU_DISABLED",
    "DEJAVU_ORIG_ZDOTDIR",
    "DEJAVU_REPO_ROOT",
    "DEJAVU_SESSION_ID",
    "DEJAVU_SHIM_DIR",
];

/// Spawn the test binary without ambient Dejavu session variables from the
/// developer shell. The tests create their own isolated Dejavu environments.
pub fn dejavu_cmd() -> Command {
    let mut cmd = Command::cargo_bin("dejavu").unwrap();
    for key in DEJAVU_ENV {
        cmd.env_remove(key);
    }
    cmd
}

/// Write an executable script (0755) into `dir`.
pub fn write_exec(dir: &Path, name: &str, body: &str) {
    let path = dir.join(name);
    fs::write(&path, body).unwrap();
    let mut perms = fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms).unwrap();
}

/// A `PATH` value with `fake_dir` first, then the inherited PATH (so `sh` etc.
/// remain resolvable).
pub fn path_with(fake_dir: &Path) -> String {
    let orig = std::env::var("PATH").unwrap_or_default();
    format!("{}:{}", fake_dir.display(), orig)
}

/// Recursively find the first file under `root` with extension `ext`.
pub fn find_ext(root: &Path, ext: &str) -> Option<std::path::PathBuf> {
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_ext(&path, ext) {
                return Some(found);
            }
        } else if path.extension().is_some_and(|e| e == ext) {
            return Some(path);
        }
    }
    None
}
