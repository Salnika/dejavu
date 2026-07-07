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
    let global_dir = crate::commands::shellenv::global_shim_dir().ok();
    let path_has_session = split_path(&path).iter().any(|p| p == &shim_dir);
    let path_has_global = global_dir
        .as_ref()
        .is_some_and(|g| split_path(&path).iter().any(|p| p == g));
    let path_has_shim = path_has_session || path_has_global;
    // The shim dir whose entries should win PATH resolution here.
    let active_dir: &Path = if path_has_session {
        &shim_dir
    } else if path_has_global {
        global_dir.as_ref().unwrap()
    } else {
        &shim_dir
    };
    checks.push(chk(
        "shim directory in PATH",
        if path_has_shim { "ok" } else { "warn" },
        if path_has_session {
            format!("session shims on PATH ({})", shim_dir.display())
        } else if path_has_global {
            format!(
                "global activation on PATH ({})",
                global_dir.as_ref().unwrap().display()
            )
        } else {
            "no shim dir on PATH — use `dejavu start …` or `dejavu shellenv --install`".to_string()
        },
    ));

    let mut not_via_shim = Vec::new();
    if path_has_shim {
        for name in &enabled {
            let expected = active_dir.join(name);
            if !first_on_path(name, &path).is_some_and(|p| same_path(&p, &expected)) {
                not_via_shim.push(name.clone());
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

    // Build type: a debug binary adds ~25ms to every shimmed command.
    if cfg!(debug_assertions) {
        checks.push(chk(
            "build",
            "warn",
            "debug build (+~25ms per command) — install a release build and re-run \
             `dejavu shellenv --install`",
        ));
    } else {
        checks.push(chk("build", "ok", "release build"));
    }
    // Shims baked against a debug binary keep the cost even after a rebuild.
    let mut debug_shims = Vec::new();
    for dir in [Some(&shim_dir), global_dir.as_ref()].into_iter().flatten() {
        let self_shim = "dejavu".to_string();
        for name in enabled.iter().chain(std::iter::once(&self_shim)) {
            if let Ok(body) = std::fs::read_to_string(dir.join(name)) {
                if body.contains("/target/debug/") {
                    debug_shims.push(format!("{}", dir.join(name).display()));
                }
            }
        }
    }
    if !debug_shims.is_empty() {
        checks.push(chk(
            "shim binary",
            "warn",
            format!(
                "{} shim(s) point at a target/debug binary — re-run `dejavu shellenv --install` \
                 (or `dejavu start`) from a release build",
                debug_shims.len()
            ),
        ));
    }

    // VS Code / Copilot probe: agent-mode terminals source the shell profiles,
    // so global activation is what makes Copilot's commands hit the shims.
    if std::env::var_os("TERM_PROGRAM").is_some_and(|v| v == "vscode") {
        checks.push(chk(
            "vscode",
            if path_has_shim { "ok" } else { "warn" },
            if path_has_shim {
                "Copilot ready: shims active in this VS Code terminal"
            } else {
                "shims not on PATH in this VS Code terminal — run `dejavu shellenv --install`, \
                 then fully restart VS Code"
            },
        ));
    }

    // Reduction gate: is THIS invocation eligible for reduction, and if not,
    // why? Answers the most common confusion — "shims are on PATH but nothing
    // looks compacted." (Under `dejavu doctor`, stdout is usually a pipe.)
    {
        use std::io::IsTerminal;
        let tty = std::io::stdout().is_terminal();
        let (status, detail): (&'static str, String) = if env::is_active() {
            ("ok", "active — inside a `dejavu start` session".to_string())
        } else if env::is_forced() {
            ("ok", "active — DEJAVU_FORCE=1".to_string())
        } else if env::pipe_agent_marker_present() {
            (
                "ok",
                "active — a pipe-capturing agent (Claude Code / Codex / Cursor) is driving this shell"
                    .to_string(),
            )
        } else if env::pty_agent_marker_present() && tty {
            ("ok", "active — Copilot agent marker + terminal".to_string())
        } else if env::pty_agent_marker_present() {
            (
                "warn",
                "gated off — agent marker present but stdout is not a terminal; \
                 run the command inside the agent, or set DEJAVU_FORCE=1"
                    .to_string(),
            )
        } else {
            (
                "ok",
                "passthrough — no agent context; your own commands run raw at native speed"
                    .to_string(),
            )
        };
        checks.push(chk("reduction", status, detail));
    }

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
            None => missing.push(name.clone()),
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

fn summarize(names: &[String]) -> String {
    const MAX: usize = 8;
    if names.len() <= MAX {
        return names.join(", ");
    }
    format!("{} (+{} more)", names[..MAX].join(", "), names.len() - MAX)
}
