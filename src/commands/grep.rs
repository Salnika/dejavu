//! `dejavu grep` — search the stored output of a run (spec §17.5). Does not
//! mark `full_output_requested` (a targeted probe, not a full read).

use super::resolve_target;
use crate::cli::AppCtx;
use crate::store::Db;
use regex::Regex;

pub fn run(target: &str, pattern: &str, normalized: bool) -> anyhow::Result<i32> {
    let ctx = AppCtx::resolve()?;
    let db = Db::open(&ctx.layout.db())?;
    let run = resolve_target(&db, &ctx.repo_root_str(), target)?;

    let re = match Regex::new(pattern) {
        Ok(re) => re,
        Err(err) => {
            eprintln!("dejavu grep: invalid pattern: {err}");
            return Ok(2);
        }
    };

    let files: Vec<Option<String>> = if normalized {
        vec![run.normalized_path.clone()]
    } else {
        vec![run.stdout_path.clone(), run.stderr_path.clone()]
    };

    let mut any_readable = false;
    let mut matched = false;
    for path in files.into_iter().flatten() {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        any_readable = true;
        let text = String::from_utf8_lossy(&bytes);
        for line in text.lines() {
            if re.is_match(line) {
                println!("{line}");
                matched = true;
            }
        }
    }

    if !any_readable {
        eprintln!("dejavu grep: no stored output for this run");
        return Ok(2);
    }
    Ok(if matched { 0 } else { 1 })
}
