//! Shim generation. Each shim is a minimal `/bin/sh` script that re-invokes
//! `dejavu run`. Writes are idempotent (temp + atomic rename) and only touch
//! files whose content or exec bit differs.

pub use crate::config::SHIM_NAMES;

use std::collections::HashSet;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub struct ShimContext {
    pub shim_dir: PathBuf,
    /// Absolute path to the `dejavu` binary, baked in as the fallback when
    /// `DEJAVU_BIN` is unset.
    pub dejavu_bin: PathBuf,
    /// Builtin + user `[intercept] extra` names to generate shims for.
    pub enabled: Vec<String>,
}

/// Generate/refresh shims for the enabled set; remove shims for anything else
/// (disabled builtins AND `extra` entries removed from config) so `which` no
/// longer finds them. Returns the number of enabled shims.
pub fn generate_shims(ctx: &ShimContext) -> std::io::Result<usize> {
    std::fs::create_dir_all(&ctx.shim_dir)?;
    let enabled: HashSet<&str> = ctx.enabled.iter().map(String::as_str).collect();

    // Content-based sweep: a generated command shim is self-identifying (it
    // contains the DEJAVU_BIN marker), so we can safely remove stale ones
    // without keeping a registry of past names. The `dejavu` self-shim and
    // any foreign file are left untouched.
    if let Ok(entries) = std::fs::read_dir(&ctx.shim_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if name == "dejavu" || enabled.contains(name) {
                continue;
            }
            let path = entry.path();
            if std::fs::read_to_string(&path)
                .is_ok_and(|body| body.starts_with("#!/bin/sh") && body.contains("DEJAVU_BIN"))
            {
                let _ = std::fs::remove_file(&path);
            }
        }
    }

    for name in &ctx.enabled {
        let path = ctx.shim_dir.join(name);
        let body = shim_script(name, &ctx.dejavu_bin);
        write_if_changed(&path, &body)?;
    }

    // Self-shim: the reduced envelope tells the agent to run `dejavu show <id>`;
    // that must work even when the binary itself is not otherwise on PATH.
    // A plain exec (no `run --shim-name`), and deliberately no DEJAVU_BIN
    // marker so `is_dejavu_shim` never mistakes it for a command shim.
    let self_body = format!("#!/bin/sh\nexec \"{}\" \"$@\"\n", ctx.dejavu_bin.display());
    write_if_changed(&ctx.shim_dir.join("dejavu"), &self_body)?;

    Ok(ctx.enabled.len())
}

fn shim_script(name: &str, dejavu_bin: &Path) -> String {
    format!(
        "#!/bin/sh\nexec \"${{DEJAVU_BIN:-{bin}}}\" run --shim-name {name} -- \"$@\"\n",
        bin = dejavu_bin.display(),
    )
}

/// Write the shim only if content/perms differ. Uses temp + rename so a
/// concurrent `dejavu start` never sees a torn file.
fn write_if_changed(path: &Path, body: &str) -> std::io::Result<bool> {
    if let Ok(existing) = std::fs::read_to_string(path) {
        if existing == body {
            if let Ok(meta) = std::fs::metadata(path) {
                if meta.permissions().mode() & 0o111 != 0 {
                    return Ok(false);
                }
            }
        }
    }
    let tmp = path.with_extension("dejavu-tmp");
    {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(body.as_bytes())?;
        let mut perms = file.metadata()?.permissions();
        perms.set_mode(0o755);
        file.set_permissions(perms)?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_executable_shims_and_removes_disabled() {
        let tmp = tempfile::tempdir().unwrap();
        let shim_dir = tmp.path().join("shims/bin");
        let ctx = ShimContext {
            shim_dir: shim_dir.clone(),
            dejavu_bin: PathBuf::from("/opt/dejavu"),
            enabled: vec!["pnpm".to_string(), "git".to_string()],
        };
        let n = generate_shims(&ctx).unwrap();
        assert_eq!(n, 2);

        let pnpm = shim_dir.join("pnpm");
        assert!(pnpm.exists());
        let mode = std::fs::metadata(&pnpm).unwrap().permissions().mode();
        assert!(mode & 0o111 != 0);
        let body = std::fs::read_to_string(&pnpm).unwrap();
        assert!(body.contains("run --shim-name pnpm --"));
        assert!(body.contains("${DEJAVU_BIN:-/opt/dejavu}"));

        // Re-run with pnpm disabled -> its shim is removed.
        let ctx2 = ShimContext {
            shim_dir: shim_dir.clone(),
            dejavu_bin: PathBuf::from("/opt/dejavu"),
            enabled: vec!["git".to_string()],
        };
        generate_shims(&ctx2).unwrap();
        assert!(!pnpm.exists());
        assert!(shim_dir.join("git").exists());
    }
}
