//! `dejavu show` — reprint a captured run (spec §17.4).

use super::{render, resolve_target};
use crate::cli::AppCtx;
use crate::store::Db;
use crate::util::short_id;

pub fn run(target: &str, stdout: bool, stderr: bool, normalized: bool) -> anyhow::Result<i32> {
    let ctx = AppCtx::resolve()?;
    let db = Db::open(&ctx.layout.db())?;
    let run = resolve_target(&db, &ctx.repo_root_str(), target)?;

    // Mark that the agent asked for more than the compact output — but only
    // inside an active session (a human inspecting shouldn't skew the metric).
    if ctx.session_id.is_some() {
        let _ = db.mark_full_output_requested(&run.id);
    }

    if stdout {
        return print_file(run.stdout_path.as_deref(), "stdout");
    }
    if stderr {
        return print_file(run.stderr_path.as_deref(), "stderr");
    }
    if normalized {
        return print_file(run.normalized_path.as_deref(), "normalized output");
    }

    // Default: a metadata block, plus the stored compact summary if present.
    let short = short_id(&run.id);
    render::title(&format!("Dejavu run {short}"));
    println!("Command: {}", run.command_original);
    println!("Exit code: {}", run.exit_code);
    println!("Classification: {}", run.classification);
    println!(
        "Captured: {} bytes (stdout {}, stderr {})",
        run.raw_total_bytes, run.raw_stdout_bytes, run.raw_stderr_bytes
    );
    if let Some(summary) = &run.summary {
        if !summary.trim().is_empty() {
            render::section("Compact output");
            println!("{summary}");
        }
    }
    render::section("Recovery");
    println!("Full output: dejavu show {short} --stdout");
    Ok(0)
}

fn print_file(path: Option<&str>, what: &str) -> anyhow::Result<i32> {
    match path {
        Some(p) if std::path::Path::new(p).exists() => {
            let bytes = std::fs::read(p)?;
            use std::io::Write;
            std::io::stdout().write_all(&bytes)?;
            Ok(0)
        }
        _ => {
            eprintln!("dejavu: no stored {what} for this run");
            Ok(1)
        }
    }
}
