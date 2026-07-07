//! Repo root detection and git-state metadata capture.
//!
//! All git subprocesses set `DEJAVU_DISABLED=1` so that if a shim is somehow
//! reached anyway, the nested `dejavu run` passes straight through instead of
//! re-entering the capture pipeline.

use crate::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

/// How internal git metadata queries are spawned.
///
/// On the hot shim path this carries the *resolved real* git binary and the
/// sanitized `PATH` (shim dir removed), so each internal query costs one
/// process. The default (`git` on the ambient `PATH`) is for
/// latency-insensitive callers — CLI commands, the session launcher — where,
/// under global activation, the call may route through a shim and terminate
/// via `DEJAVU_DISABLED=1` (correct, just three processes instead of one).
#[derive(Debug, Clone)]
pub struct GitInvoker {
    program: PathBuf,
    env_path: Option<OsString>,
}

impl Default for GitInvoker {
    fn default() -> Self {
        GitInvoker {
            program: PathBuf::from("git"),
            env_path: None,
        }
    }
}

impl GitInvoker {
    /// An invoker that spawns `program` directly with `PATH=env_path`.
    pub fn resolved(program: PathBuf, env_path: OsString) -> GitInvoker {
        GitInvoker {
            program,
            env_path: Some(env_path),
        }
    }

    fn cmd(&self, dir: &Path) -> Command {
        let mut cmd = Command::new(&self.program);
        cmd.arg("-C").arg(dir);
        if let Some(p) = &self.env_path {
            cmd.env("PATH", p);
        }
        cmd.env(env::DISABLED, "1");
        cmd
    }

    /// The repo root via `git rev-parse --show-toplevel`, falling back to `cwd`
    /// (canonicalized) when not inside a git repo.
    pub fn detect_repo_root(&self, cwd: &Path) -> PathBuf {
        if let Ok(out) = self.cmd(cwd).args(["rev-parse", "--show-toplevel"]).output() {
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
    pub fn head(&self, repo_root: &Path) -> Option<String> {
        let out = self.cmd(repo_root).args(["rev-parse", "HEAD"]).output().ok()?;
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        None
    }

    /// A stable hash of the working-tree state: `sha256(git status
    /// --porcelain=v1 -z)`. `None` when not a git repo / git absent.
    /// Annotation only — never used to filter run comparability.
    pub fn worktree_hash(&self, repo_root: &Path) -> Option<String> {
        let out = self
            .cmd(repo_root)
            .args(["status", "--porcelain=v1", "-z"])
            .output()
            .ok()?;
        if out.status.success() {
            return Some(crate::util::sha256_hex(&out.stdout));
        }
        None
    }

    fn state(&self, repo_root: &Path) -> GitState {
        GitState {
            head: self.head(repo_root),
            worktree_hash: self.worktree_hash(repo_root),
        }
    }
}

/// Git-state metadata stored with a run. Annotation only (spec decision #3):
/// it feeds the "across code changes" / "possible flaky" note, never the
/// comparability match.
pub struct GitState {
    pub head: Option<String>,
    pub worktree_hash: Option<String>,
}

/// `HEAD` + worktree hash computed on a background thread **while the real
/// command runs**, joined only when the run is stored. On big worktrees `git
/// status` costs hundreds of ms; overlapping it with the command hides that
/// cost entirely whenever the command outlasts it.
///
/// The snapshot starts at command start instead of after completion — for the
/// read-only commands Dejavu optimizes the two are identical, and for test
/// runs that write ignored files, start-state is the more faithful fingerprint
/// of what produced the output. Any thread failure degrades to `None`s.
pub enum GitStatePrefetch {
    Spawned(std::thread::JoinHandle<GitState>),
    /// Thread spawn failed — compute inline at join time.
    Inline { invoker: GitInvoker, repo_root: PathBuf },
}

impl GitStatePrefetch {
    pub fn spawn(invoker: GitInvoker, repo_root: PathBuf) -> GitStatePrefetch {
        let thread_invoker = invoker.clone();
        let thread_root = repo_root.clone();
        match std::thread::Builder::new()
            .name("dejavu-git-state".to_string())
            .spawn(move || thread_invoker.state(&thread_root))
        {
            Ok(handle) => GitStatePrefetch::Spawned(handle),
            Err(_) => GitStatePrefetch::Inline { invoker, repo_root },
        }
    }

    pub fn join(self) -> GitState {
        match self {
            GitStatePrefetch::Spawned(handle) => handle.join().unwrap_or(GitState {
                head: None,
                worktree_hash: None,
            }),
            GitStatePrefetch::Inline { invoker, repo_root } => invoker.state(&repo_root),
        }
    }
}

/// The repo root using the default (ambient `PATH`) invoker — for
/// latency-insensitive callers like `dejavu start` and the CLI commands.
pub fn detect_repo_root(cwd: &Path) -> PathBuf {
    GitInvoker::default().detect_repo_root(cwd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefetch_joins_to_nones_outside_a_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let state =
            GitStatePrefetch::spawn(GitInvoker::default(), tmp.path().to_path_buf()).join();
        assert!(state.head.is_none());
        assert!(state.worktree_hash.is_none());
    }

    #[test]
    fn prefetch_captures_state_in_a_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let run = |args: &[&str]| {
            std::process::Command::new("git")
                .arg("-C")
                .arg(tmp.path())
                .args(args)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_SYSTEM", "/dev/null")
                .output()
                .unwrap()
        };
        run(&["init", "-q", "."]);
        run(&["config", "user.email", "t@t"]);
        run(&["config", "user.name", "t"]);
        std::fs::write(tmp.path().join("f.txt"), "x").unwrap();
        run(&["add", "-A"]);
        run(&["-c", "commit.gpgsign=false", "commit", "-qm", "init"]);

        let state =
            GitStatePrefetch::spawn(GitInvoker::default(), tmp.path().to_path_buf()).join();
        assert!(state.head.is_some(), "HEAD should exist after a commit");
        assert!(state.worktree_hash.is_some());

        // The worktree hash moves when the tree changes.
        std::fs::write(tmp.path().join("g.txt"), "y").unwrap();
        let dirty =
            GitStatePrefetch::spawn(GitInvoker::default(), tmp.path().to_path_buf()).join();
        assert_ne!(state.worktree_hash, dirty.worktree_hash);
    }
}
