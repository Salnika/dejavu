//! `dejavu doctor` — diagnose the setup for the current repo (spec §17.3).

use crate::cli::AppCtx;
use crate::commands::render;
use crate::env;
use crate::exec::path::split_path;
use crate::exec::resolve::{resolve_real, ResolveEnv};
use crate::store::Db;
use std::path::{Path, PathBuf};

#[derive(serde::Serialize)]
struct Check {
    name: &'static str,
    status: &'static str, // ok | warn | fail
    detail: String,
}

fn chk(name: &'static str, status: &'static str, detail: impl Into<String>) -> Check {
    Check {
        name,
        status,
        detail: detail.into(),
    }
}

pub fn run(json: bool) -> anyhow::Result<i32> {
    let ctx = AppCtx::resolve()?;
    let mut checks: Vec<Check> = Vec::new();

    checks.push(chk("version", "ok", env!("CARGO_PKG_VERSION")));
    checks.push(chk(
        "os",
        "ok",
        format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
    ));
    checks.push(chk(
        "shell",
        if std::env::var_os("SHELL").is_some() {
            "ok"
        } else {
            "warn"
        },
        std::env::var("SHELL").unwrap_or_else(|_| "SHELL is not set".to_string()),
    ));
    checks.push(chk("repo root", "ok", ctx.repo_root.display().to_string()));
    checks.push(chk(
        "cache directory",
        "ok",
        ctx.layout.root.display().to_string(),
    ));

    // Dejavu binary.
    match std::env::current_exe() {
        Ok(p) => checks.push(chk("dejavu binary", "ok", p.display().to_string())),
        Err(e) => checks.push(chk("dejavu binary", "fail", e.to_string())),
    }

    // Cache writable.
    match write_probe(&ctx.layout.root) {
        Ok(()) => checks.push(Check {
            name: "storage writable",
            status: "ok",
            detail: ctx.layout.root.display().to_string(),
        }),
        Err(e) => checks.push(Check {
            name: "storage writable",
            status: "fail",
            detail: e.to_string(),
        }),
    }

    // SQLite integrity.
    match Db::open(&ctx.layout.db()).and_then(|db| {
        let v: String = db
            .conn
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        Ok(v)
    }) {
        Ok(v) if v == "ok" => checks.push(Check {
            name: "sqlite",
            status: "ok",
            detail: "integrity_check ok".to_string(),
        }),
        Ok(v) => checks.push(Check {
            name: "sqlite",
            status: "fail",
            detail: v,
        }),
        Err(e) => checks.push(Check {
            name: "sqlite",
            status: "fail",
            detail: e.to_string(),
        }),
    }

    // Shims generated.
    let enabled = ctx.config.intercept.enabled_shims();
    let shim_dir = ctx.layout.shims_bin();
    let present = enabled.iter().filter(|n| shim_dir.join(n).exists()).count();
    checks.push(Check {
        name: "shims generated",
        status: if present == enabled.len() {
            "ok"
        } else {
            "warn"
        },
        detail: format!(
            "{present}/{} present in {}",
            enabled.len(),
            shim_dir.display()
        ),
    });

    let path = std::env::var_os("PATH").unwrap_or_default();
    let path_has_shim = split_path(&path).iter().any(|p| p == &shim_dir);
    checks.push(chk(
        "shim directory in PATH",
        if path_has_shim { "ok" } else { "warn" },
        if path_has_shim {
            format!("{} is on PATH", shim_dir.display())
        } else {
            format!("{} is not on PATH", shim_dir.display())
        },
    ));

    let mut not_via_shim = Vec::new();
    if path_has_shim {
        for name in &enabled {
            let expected = shim_dir.join(name);
            if !first_on_path(name, &path).is_some_and(|p| same_path(&p, &expected)) {
                not_via_shim.push(*name);
            }
        }
    }
    checks.push(chk(
        "commands resolve through Dejavu",
        if path_has_shim && not_via_shim.is_empty() {
            "ok"
        } else {
            "warn"
        },
        if path_has_shim {
            if not_via_shim.is_empty() {
                format!(
                    "{} enabled commands resolve to generated shims",
                    enabled.len()
                )
            } else {
                format!("not first on PATH: {}", summarize(&not_via_shim))
            }
        } else {
            "run inside `dejavu start -- dejavu doctor` to verify active shims".to_string()
        },
    ));

    // Real binary resolved per shim.
    let dejavu_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    let mut missing = Vec::new();
    let mut examples = Vec::new();
    for name in &enabled {
        let found = resolve_real(
            name,
            &ResolveEnv {
                path: &path,
                shim_dir: &shim_dir,
                dejavu_dir: &dejavu_dir,
            },
        );
        match found {
            Some(path) if examples.len() < 5 => {
                examples.push(format!("{name}={}", path.display()));
            }
            Some(_) => {}
            None => missing.push(*name),
        }
    }
    checks.push(Check {
        name: "real binaries",
        status: if missing.is_empty() { "ok" } else { "warn" },
        detail: if missing.is_empty() {
            if examples.is_empty() {
                "all intercepted tools resolve".to_string()
            } else {
                format!(
                    "all intercepted tools resolve; examples: {}",
                    examples.join(", ")
                )
            }
        } else {
            format!("not installed: {}", summarize(&missing))
        },
    });

    checks.push(chk(
        "bypass",
        "ok",
        if env::is_disabled() {
            "bypass is currently active"
        } else {
            "`DEJAVU=off` and `DEJAVU_DISABLED=1` force passthrough"
        },
    ));

    // PATH active in session.
    if env::is_active() {
        checks.push(chk(
            "session PATH",
            if path_has_shim { "ok" } else { "warn" },
            if path_has_shim {
                "shim dir on PATH"
            } else {
                "DEJAVU_ACTIVE set but shim dir missing from PATH"
            },
        ));
    } else {
        checks.push(chk(
            "session PATH",
            "ok",
            "not in a dejavu session (informational)",
        ));
    }

    // Config valid (it loaded, so it's valid).
    checks.push(chk("config", "ok", "loaded and valid"));

    let any_fail = checks.iter().any(|c| c.status == "fail");

    if json {
        println!("{}", serde_json::to_string_pretty(&checks)?);
    } else {
        render::title("Dejavu doctor");
        render::kv(&[("Repo", ctx.repo_root.display().to_string())]);
        render::section("Checks");
        let rows: Vec<Vec<String>> = checks
            .iter()
            .map(|c| vec![c.status.to_string(), c.name.to_string(), c.detail.clone()])
            .collect();
        render::table_styled(
            &["Status", "Check", "Detail"],
            &rows,
            &[render::Style::Status],
        );
    }
    Ok(if any_fail { 2 } else { 0 })
}

fn write_probe(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let probe = dir.join(".dejavu-doctor-probe");
    std::fs::write(&probe, b"ok")?;
    std::fs::remove_file(&probe)?;
    Ok(())
}

fn first_on_path(name: &str, path: &std::ffi::OsStr) -> Option<PathBuf> {
    split_path(path)
        .into_iter()
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file() && is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

fn same_path(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

fn summarize(names: &[&str]) -> String {
    const MAX: usize = 8;
    if names.len() <= MAX {
        return names.join(", ");
    }
    format!("{} (+{} more)", names[..MAX].join(", "), names.len() - MAX)
}
