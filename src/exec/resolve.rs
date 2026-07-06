//! Anti-recursion real-binary resolution. Walk `PATH` left→right, skipping the
//! shim dir and dejavu's own dir, and return the first matching executable.
//!
//! We return the *path found* (e.g. `entry/pnpm`) and never canonicalize the
//! exec target — version-manager wrappers (volta/asdf/nvm) dispatch on the name
//! they are invoked as, so canonicalizing to the underlying `node` would break
//! them. Canonicalization is only used for the directory-exclusion comparison.

use super::path::split_path;
use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub struct ResolveEnv<'a> {
    pub path: &'a OsStr,
    pub shim_dir: &'a Path,
    pub dejavu_dir: &'a Path,
}

/// Find the real binary named `name`, excluding the shim dir and dejavu's dir.
pub fn resolve_real(name: &str, env: &ResolveEnv) -> Option<PathBuf> {
    let shim_canon = std::fs::canonicalize(env.shim_dir).ok();
    let dejavu_canon = std::fs::canonicalize(env.dejavu_dir).ok();

    for entry in split_path(env.path) {
        if let Ok(canon) = std::fs::canonicalize(&entry) {
            if shim_canon.as_ref() == Some(&canon) || dejavu_canon.as_ref() == Some(&canon) {
                continue;
            }
        }
        let candidate = entry.join(name);
        if is_executable(&candidate) && !is_dejavu_shim(&candidate) {
            return Some(candidate);
        }
    }
    None
}

/// Content-based self-identification: a generated shim always contains the
/// `DEJAVU_BIN` marker in its first line. Directory exclusion alone is not
/// enough — a shim can be invoked with no `DEJAVU_SHIM_DIR` in the environment
/// (GUI apps, global PATH setups, stale shims), and resolving the shim itself
/// as the "real" binary would recurse forever.
fn is_dejavu_shim(path: &Path) -> bool {
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut buf = [0u8; 256];
    let n = std::io::Read::read(&mut file, &mut buf).unwrap_or(0);
    let head = &buf[..n];
    head.starts_with(b"#!/bin/sh") && head.windows(10).any(|w| w == b"DEJAVU_BIN")
}

fn is_executable(path: &Path) -> bool {
    // metadata() follows symlinks — a wrapper symlink to a real file is fine.
    match std::fs::metadata(path) {
        Ok(meta) => meta.is_file() && (meta.permissions().mode() & 0o111 != 0),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::fs;
    use std::io::Write;

    fn make_exec(dir: &Path, name: &str) {
        let p = dir.join(name);
        let mut f = fs::File::create(&p).unwrap();
        writeln!(f, "#!/bin/sh\ntrue").unwrap();
        let mut perms = f.metadata().unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&p, perms).unwrap();
    }

    #[test]
    fn skips_shim_dir_and_finds_real() {
        let tmp = tempfile::tempdir().unwrap();
        let shim = tmp.path().join("shim");
        let real = tmp.path().join("real");
        let dejavu = tmp.path().join("dejavu");
        for d in [&shim, &real, &dejavu] {
            fs::create_dir_all(d).unwrap();
        }
        // A shim named `pnpm` sits earlier; the real one is later.
        make_exec(&shim, "pnpm");
        make_exec(&real, "pnpm");

        let path = std::env::join_paths([&shim, &real]).unwrap();
        let env = ResolveEnv {
            path: &path,
            shim_dir: &shim,
            dejavu_dir: &dejavu,
        };
        let found = resolve_real("pnpm", &env).unwrap();
        assert_eq!(found, real.join("pnpm"));
    }

    #[test]
    fn skips_shim_by_content_even_outside_known_shim_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let rogue = tmp.path().join("rogue"); // a shim dir NOT excluded by env
        let real = tmp.path().join("real");
        let other = tmp.path().join("other");
        for d in [&rogue, &real, &other] {
            fs::create_dir_all(d).unwrap();
        }
        // A real dejavu shim body in a dir the resolver does not know about.
        let p = rogue.join("git");
        fs::write(
            &p,
            "#!/bin/sh\nexec \"${DEJAVU_BIN:-/usr/local/bin/dejavu}\" run --shim-name git -- \"$@\"\n",
        )
        .unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        make_exec(&real, "git");

        let path = std::env::join_paths([&rogue, &real]).unwrap();
        let env = ResolveEnv {
            path: &path,
            shim_dir: &other, // wrong exclusion — content check must save us
            dejavu_dir: &other,
        };
        let found = resolve_real("git", &env).unwrap();
        assert_eq!(found, real.join("git"));
    }

    #[test]
    fn returns_none_when_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let env = ResolveEnv {
            path: &OsString::from(tmp.path()),
            shim_dir: tmp.path(),
            dejavu_dir: tmp.path(),
        };
        assert!(resolve_real("definitely-not-a-binary", &env).is_none());
    }
}
