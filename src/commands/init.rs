//! `dejavu init` — create the cache for the current repo. Never modifies the repo.

use crate::cli::AppCtx;
use crate::commands::render;
use crate::store::Db;

pub fn run() -> anyhow::Result<i32> {
    let ctx = AppCtx::resolve()?;
    ctx.layout.ensure_dirs()?;
    // Opening the DB creates and migrates it.
    let _db = Db::open(&ctx.layout.db())?;
    ctx.config.write_effective(&ctx.layout)?;

    render::title("Dejavu initialized");
    render::kv(&[
        ("Repo", ctx.repo_root.display().to_string()),
        ("Cache", ctx.layout.root.display().to_string()),
        ("Next", "dejavu start -- codex".to_string()),
    ]);
    Ok(0)
}
