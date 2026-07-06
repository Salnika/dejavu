//! Repo root detection and git-state metadata capture.
//!
//! All git subprocesses set `DEJAVU_DISABLED=1` so that if the shim dir is on
//! `PATH` (inside a session), the internal `git` call passes straight through
//! instead of re-entering the capture pipeline.

use crate::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn git(repo_root: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(repo_root);
    cmd.env(env::DISABLED, "1");
    cmd
}

/// The repo root via `git rev-parse --show-toplevel`, falling back to `cwd`
/// (canonicalized) when not inside a git repo.
pub fn detect_repo_root(cwd: &Path) -> PathBuf {
    let mut cmd = git(cwd);
    if let Ok(out) = cmd.args(["rev-parse", "--show-toplevel"]).output() {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return PathBuf::from(trimmed);
            }
        }
    }
    std::fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf())
}

/// The current commit hash, if the repo has one.
pub fn git_head(repo_root: &Path) -> Option<String> {
    let out = git(repo_root).args(["rev-parse", "HEAD"]).output().ok()?;
    if out.status.success() {
        let text = String::from_utf8_lossy(&out.stdout);
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    None
}

/// A stable hash of the working-tree state: `sha256(git status --porcelain=v1
/// -z)`. `None` when not a git repo / git absent. Annotation only — never used
/// to filter run comparability.
pub fn git_worktree_hash(repo_root: &Path) -> Option<String> {
    let out = git(repo_root)
        .args(["status", "--porcelain=v1", "-z"])
        .output()
        .ok()?;
    if out.status.success() {
        return Some(crate::util::sha256_hex(&out.stdout));
    }
    None
}
