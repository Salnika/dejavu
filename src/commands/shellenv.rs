//! `dejavu shellenv` — global activation. Generates a repo-independent shim
//! directory and either prints the shell line that puts it first on `PATH`:
//!
//! ```sh
//! eval "$(dejavu shellenv)"
//! ```
//!
//! …or, with `--install`, writes a managed block directly into your shell
//! profile(s) (`~/.zshrc`, `~/.bashrc`, `~/.profile`). `--uninstall` removes it.
//!
//! This covers agents Dejavu cannot launch itself: IDE integrated terminals
//! (VS Code + Copilot agent mode), GUI apps that run commands through a login
//! shell, etc. Shims are self-sufficient without any `DEJAVU_*` variable: the
//! runtime rebuilds the repo context from the working directory, and shim
//! self-identification prevents recursion.

use crate::config::Config;
use crate::exec::shim::{generate_shims, ShimContext};
use std::path::{Path, PathBuf};

const BEGIN: &str = "# >>> dejavu shellenv >>>";
const END: &str = "# <<< dejavu shellenv <<<";

/// The repo-independent shim dir: `<cache_root>/shims/bin`. Never collides
/// with per-repo caches (`<cache_root>/<16-hex-hash>/`) and holds no
/// `runs.sqlite`, so `stats --all` skips it.
pub fn global_shim_dir() -> anyhow::Result<PathBuf> {
    Ok(crate::paths::cache_root()?.join("shims").join("bin"))
}

pub fn run(install: bool, uninstall: bool, shell: Option<String>) -> anyhow::Result<i32> {
    if uninstall {
        return edit_profiles(shell, None);
    }
    if install {
        let dir = ensure_shim_dir()?;
        return edit_profiles(shell, Some(&managed_block(&shim_line(&dir))));
    }
    // Default: print the eval-able line.
    let dir = ensure_shim_dir()?;
    println!("{}", shim_line(&dir));
    Ok(0)
}

/// Create the global shim dir and (re)generate the shims into it.
fn ensure_shim_dir() -> anyhow::Result<PathBuf> {
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
    Ok(dir)
}

/// The idempotent POSIX guard that prepends the shim dir to `PATH`.
fn shim_line(dir: &Path) -> String {
    let d = dir.display();
    format!("case \":$PATH:\" in *\":{d}:\"*) ;; *) export PATH=\"{d}:$PATH\" ;; esac")
}

fn managed_block(line: &str) -> String {
    format!(
        "{BEGIN}\n# Added by `dejavu shellenv --install`; remove with `dejavu shellenv --uninstall`.\n{line}\n{END}\n"
    )
}

/// Install (`block = Some`) or uninstall (`block = None`) the managed block in
/// the profile(s) for the selected shell(s).
fn edit_profiles(shell: Option<String>, block: Option<&str>) -> anyhow::Result<i32> {
    let targets = target_profiles(shell.as_deref());
    if targets.is_empty() {
        anyhow::bail!("no known shell profile to edit");
    }
    for (name, path) in &targets {
        let action = write_block(path, block)?;
        println!("{name:<4}  {:<40}  {}", path.display(), action);
    }
    if block.is_some() {
        println!("\nOpen a new terminal (or `source` the file) to activate Dejavu.");
    } else {
        println!("\nDejavu global activation removed. Open a new terminal to apply.");
    }
    Ok(0)
}

fn target_profiles(shell: Option<&str>) -> Vec<(&'static str, PathBuf)> {
    let want: &[&str] = match shell {
        Some("zsh") => &["zsh"],
        Some("bash") => &["bash"],
        Some("sh") => &["sh"],
        _ => &["zsh", "bash", "sh"], // default / "all"
    };
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for s in want {
        match *s {
            "zsh" => {
                let zdot = zsh_zdotdir();
                // .zshrc: interactive shells (IDE terminals). .zprofile: LOGIN
                // shells including non-interactive `zsh -lc`, which GUI apps
                // (Codex, launchers) use to run commands — .zshrc is NOT read
                // there, so both files are needed for full coverage.
                out.push(("zsh", zdot.join(".zshrc")));
                out.push(("zsh", zdot.join(".zprofile")));
            }
            "bash" => {
                out.push(("bash", home.join(".bashrc")));
                // A login bash reads .bash_profile and then IGNORES .profile.
                // Only add the block there if the file already exists —
                // creating it would shadow the user's .profile.
                let bp = home.join(".bash_profile");
                if bp.exists() {
                    out.push(("bash", bp));
                }
            }
            "sh" => out.push(("sh", home.join(".profile"))),
            _ => {}
        }
    }
    out
}

/// Where zsh reads `.zshrc`. Inside a `dejavu start` session `ZDOTDIR` points
/// at our wrapper dir, so fall back to the user's original one.
fn zsh_zdotdir() -> PathBuf {
    let raw = if crate::env::is_active() {
        std::env::var_os(crate::env::ORIG_ZDOTDIR)
    } else {
        std::env::var_os("ZDOTDIR")
    };
    raw.map(PathBuf::from)
        .or_else(dirs::home_dir)
        .unwrap_or_default()
}

/// Replace-or-append (block=Some) or remove (block=None) the managed block.
/// Returns a human status. Idempotent.
fn write_block(path: &Path, block: Option<&str>) -> std::io::Result<&'static str> {
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let had = existing.contains(BEGIN);
    let stripped = strip_block(&existing);

    let updated = match block {
        Some(b) => {
            let mut out = stripped;
            if !out.is_empty() {
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push('\n'); // blank-line separator before the block
            }
            out.push_str(b);
            out
        }
        None => stripped,
    };

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, updated)?;

    Ok(match (block.is_some(), had) {
        (true, true) => "updated",
        (true, false) => "installed",
        (false, true) => "removed",
        (false, false) => "not present",
    })
}

/// Remove the managed block (markers inclusive) and any blank line that
/// immediately preceded it, preserving the rest of the file verbatim.
fn strip_block(s: &str) -> String {
    let (Some(begin), Some(end_marker)) = (s.find(BEGIN), s.find(END)) else {
        return s.to_string();
    };
    if begin > end_marker {
        return s.to_string();
    }
    // Start of the line containing BEGIN.
    let block_start = s[..begin].rfind('\n').map(|i| i + 1).unwrap_or(0);
    // End just past END and its trailing newline.
    let mut block_end = end_marker + END.len();
    if s[block_end..].starts_with('\n') {
        block_end += 1;
    }
    let before = s[..block_start].trim_end_matches('\n');
    let after = &s[block_end..];
    let mut out = String::with_capacity(before.len() + after.len() + 1);
    out.push_str(before);
    // Keep exactly one newline after the preceding content (restores the
    // file's own line ending; drops the separator blank line we inserted).
    if !before.is_empty() {
        out.push('\n');
    }
    out.push_str(after);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_appends_a_managed_block() {
        let tmp = tempfile::tempdir().unwrap();
        let rc = tmp.path().join(".zshrc");
        std::fs::write(&rc, "export FOO=1\n").unwrap();
        let block = managed_block("export PATH=\"/shims:$PATH\"");

        assert_eq!(write_block(&rc, Some(&block)).unwrap(), "installed");
        let body = std::fs::read_to_string(&rc).unwrap();
        assert!(body.starts_with("export FOO=1\n"));
        assert!(body.contains(BEGIN) && body.contains(END));
        assert!(body.contains("/shims:$PATH"));
    }

    #[test]
    fn reinstall_is_idempotent_single_block() {
        let tmp = tempfile::tempdir().unwrap();
        let rc = tmp.path().join(".zshrc");
        let block = managed_block("export PATH=\"/shims:$PATH\"");
        write_block(&rc, Some(&block)).unwrap();
        assert_eq!(write_block(&rc, Some(&block)).unwrap(), "updated");
        let body = std::fs::read_to_string(&rc).unwrap();
        assert_eq!(body.matches(BEGIN).count(), 1);
        assert_eq!(body.matches(END).count(), 1);
    }

    #[test]
    fn uninstall_restores_original_content() {
        let tmp = tempfile::tempdir().unwrap();
        let rc = tmp.path().join(".zshrc");
        std::fs::write(&rc, "export FOO=1\n").unwrap();
        let block = managed_block("export PATH=\"/shims:$PATH\"");
        write_block(&rc, Some(&block)).unwrap();
        assert_eq!(write_block(&rc, None).unwrap(), "removed");
        assert_eq!(std::fs::read_to_string(&rc).unwrap(), "export FOO=1\n");
    }

    #[test]
    fn uninstall_when_absent_is_a_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let rc = tmp.path().join(".zshrc");
        std::fs::write(&rc, "export FOO=1\n").unwrap();
        assert_eq!(write_block(&rc, None).unwrap(), "not present");
        assert_eq!(std::fs::read_to_string(&rc).unwrap(), "export FOO=1\n");
    }

    #[test]
    fn install_creates_missing_profile() {
        let tmp = tempfile::tempdir().unwrap();
        let rc = tmp.path().join("sub/.profile");
        let block = managed_block("export PATH=\"/shims:$PATH\"");
        assert_eq!(write_block(&rc, Some(&block)).unwrap(), "installed");
        assert!(rc.exists());
    }
}
