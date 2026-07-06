//! `PATH` manipulation, always over `OsString` (paths may be non-UTF8).

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// Split `PATH` into entries, dropping empty entries (POSIX "current dir").
pub fn split_path(path: &OsStr) -> Vec<PathBuf> {
    std::env::split_paths(path)
        .filter(|p| !p.as_os_str().is_empty())
        .collect()
}

/// `dir` prepended to `current`, with any pre-existing occurrence of `dir`
/// removed (dedupe, so nested `dejavu start` doesn't stack shim dirs).
pub fn prepend_dedup(dir: &Path, current: &OsStr) -> OsString {
    let mut entries: Vec<PathBuf> = vec![dir.to_path_buf()];
    for entry in split_path(current) {
        if entry != dir {
            entries.push(entry);
        }
    }
    std::env::join_paths(entries).unwrap_or_else(|_| current.to_os_string())
}

/// `current` with every occurrence of `dir` removed — the sanitized PATH the
/// real command runs against, so nested tool calls don't re-enter a shim.
pub fn without_dir(dir: &Path, current: &OsStr) -> OsString {
    let entries: Vec<PathBuf> = split_path(current)
        .into_iter()
        .filter(|entry| entry != dir)
        .collect();
    std::env::join_paths(entries).unwrap_or_else(|_| current.to_os_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepend_puts_dir_first_and_dedups() {
        let current = OsString::from("/usr/bin:/shim:/bin");
        let out = prepend_dedup(Path::new("/shim"), &current);
        let entries = split_path(&out);
        assert_eq!(entries[0], PathBuf::from("/shim"));
        assert_eq!(
            entries.iter().filter(|p| *p == Path::new("/shim")).count(),
            1
        );
    }

    #[test]
    fn without_dir_removes_all_occurrences() {
        let current = OsString::from("/shim:/usr/bin:/shim:/bin");
        let out = without_dir(Path::new("/shim"), &current);
        let entries = split_path(&out);
        assert!(!entries.iter().any(|p| p == Path::new("/shim")));
        assert_eq!(
            entries,
            vec![PathBuf::from("/usr/bin"), PathBuf::from("/bin")]
        );
    }
}
