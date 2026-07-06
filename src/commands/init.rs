//! `dejavu init` — create the cache for the current repo. Never modifies the repo.

use crate::cli::AppCtx;
use crate::store::Db;

pub fn run() -> anyhow::Result<i32> {
    let ctx = AppCtx::resolve()?;
    ctx.layout.ensure_dirs()?;
    // Opening the DB creates and migrates it.
    let _db = Db::open(&ctx.layout.db())?;
    ctx.config.write_effective(&ctx.layout)?;

    println!("Dejavu initialized.");
    println!("Cache: {}", ctx.layout.root.display());
    println!("Run an agent with:");
    println!("  dejavu start claude");
    Ok(0)
}
