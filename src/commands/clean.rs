//! `dejavu clean` — remove cached runs and logs (spec §17.8). Never touches the
//! repo.

use super::{fmt_int, render};
use crate::cli::AppCtx;
use crate::store::Db;
use chrono::{Duration, Utc};

pub fn run(older_than: Option<String>, all: bool) -> anyhow::Result<i32> {
    let ctx = AppCtx::resolve()?;

    if all {
        remove_repo_cache(&ctx)?;
        render::title("Dejavu cache removed");
        render::kv(&[("Cache", ctx.layout.root.display().to_string())]);
        return Ok(0);
    }

    let retention_days = ctx.config.retention_days;
    let duration = match older_than {
        Some(s) => parse_duration(&s)?,
        None => Duration::days(retention_days as i64),
    };
    let cutoff = (Utc::now() - duration).to_rfc3339_opts(chrono::SecondsFormat::Micros, true);

    let db = Db::open(&ctx.layout.db())?;
    let repo = ctx.repo_root_str();
    let old = db.runs_before(&repo, &cutoff)?;

    let mut bytes_freed: u64 = 0;
    for run in &old {
        for path in [&run.stdout_path, &run.stderr_path, &run.normalized_path]
            .into_iter()
            .flatten()
        {
            if let Ok(meta) = std::fs::metadata(path) {
                bytes_freed += meta.len();
            }
            let _ = std::fs::remove_file(path);
        }
    }
    let deleted = db.delete_runs_before(&repo, &cutoff)?;

    render::title("Dejavu cleanup");
    render::kv(&[
        ("Repo", ctx.repo_root.display().to_string()),
        ("Removed runs", deleted.to_string()),
        ("Older than", humanize(duration)),
        ("Bytes freed", fmt_int(bytes_freed as i64)),
    ]);
    Ok(0)
}

pub fn uninstall() -> anyhow::Result<i32> {
    let ctx = AppCtx::resolve()?;
    remove_repo_cache(&ctx)?;
    render::title("Dejavu uninstalled for repo");
    render::kv(&[
        ("Cache", ctx.layout.root.display().to_string()),
        ("Binary", "not removed".to_string()),
        ("Remove binary", "cargo uninstall dejavu".to_string()),
    ]);
    Ok(0)
}

fn remove_repo_cache(ctx: &AppCtx) -> anyhow::Result<()> {
    // Wipe the whole per-repo cache. Guard: must be under the cache root.
    let root = &ctx.layout.root;
    anyhow::ensure!(
        root.components().count() > 3,
        "refusing to remove suspicious cache path {}",
        root.display()
    );
    if root.exists() {
        std::fs::remove_dir_all(root)?;
    }
    Ok(())
}

/// Parse a retention like `14d`, `36h`, `90m`. Falls back to humantime.
fn parse_duration(s: &str) -> anyhow::Result<Duration> {
    let s = s.trim();
    if let Some(num) = s.strip_suffix('d') {
        if let Ok(n) = num.trim().parse::<i64>() {
            return Ok(Duration::days(n));
        }
    }
    if let Some(num) = s.strip_suffix('h') {
        if let Ok(n) = num.trim().parse::<i64>() {
            return Ok(Duration::hours(n));
        }
    }
    if let Some(num) = s.strip_suffix('m') {
        if let Ok(n) = num.trim().parse::<i64>() {
            return Ok(Duration::minutes(n));
        }
    }
    let std_dur =
        humantime::parse_duration(s).map_err(|e| anyhow::anyhow!("invalid duration `{s}`: {e}"))?;
    Ok(Duration::from_std(std_dur)?)
}

fn humanize(d: Duration) -> String {
    let days = d.num_days();
    if days > 0 {
        return format!("{days}d");
    }
    let hours = d.num_hours();
    if hours > 0 {
        return format!("{hours}h");
    }
    format!("{}m", d.num_minutes())
}
