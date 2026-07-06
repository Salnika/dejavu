//! Read-only / management CLI command handlers.

pub mod bench;
pub mod clean;
pub mod doctor;
pub mod grep;
pub mod init;
pub mod render;
pub mod repos;
pub mod shellenv;
pub mod show;
pub mod stats;

use crate::store::{Db, RunRecord};
use anyhow::Context;

/// Resolve a `show`/`grep` target: `latest`, an exact id, or a unique short-id
/// prefix (git-style; errors on ambiguity).
pub(crate) fn resolve_target(db: &Db, repo_root: &str, target: &str) -> anyhow::Result<RunRecord> {
    if target == "latest" {
        return db
            .latest_run(repo_root)?
            .context("no runs recorded yet for this repo");
    }
    match db.get_run(target)? {
        None => anyhow::bail!("no run matching id `{target}`"),
        Some(run) => {
            // If we matched by prefix (not exact), ensure it was unambiguous.
            if run.id != target {
                let count = db.count_prefix(target)?;
                if count > 1 {
                    anyhow::bail!("ambiguous run id prefix `{target}` ({count} matches)");
                }
            }
            Ok(run)
        }
    }
}

/// Format an integer with thousands separators, e.g. `620700` -> `620,700`.
pub(crate) fn fmt_int(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let bytes = digits.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(len + len / 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*b as char);
    }
    if n < 0 {
        format!("-{out}")
    } else {
        out
    }
}
