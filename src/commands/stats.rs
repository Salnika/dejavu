//! `dejavu stats` — token-savings report for the current repo (spec §17.6),
//! or across every tracked repo with `--all`.

use super::fmt_int;
use crate::cli::AppCtx;
use crate::store::{Db, StatsAgg};
use std::collections::BTreeMap;

pub fn run(json: bool, all: bool, public: bool) -> anyhow::Result<i32> {
    if all {
        return run_all(json, public);
    }
    let ctx = AppCtx::resolve()?;
    ctx.layout.ensure_dirs()?;
    let db = Db::open(&ctx.layout.db())?;
    let repo = ctx.repo_root_str();
    let agg = db.aggregate_stats(Some(&repo))?;
    let top = if public {
        Vec::new()
    } else {
        db.top_savings(Some(&repo), Some(5))?
    };
    let scope = if public {
        "current repo".to_string()
    } else {
        format!("current repo ({repo})")
    };
    render(
        json,
        &RenderMeta {
            scope: &scope,
            public,
            repos_tracked: 1,
            repos_unreadable: 0,
        },
        &agg,
        &top,
        &[],
    )
}

/// One repo's contribution to the `--all` report.
struct RepoLine {
    repo: String,
    saved: i64,
    raw: i64,
    runs: i64,
}

fn run_all(json: bool, public: bool) -> anyhow::Result<i32> {
    let root = crate::paths::cache_root()?;
    let mut total = StatsAgg::default();
    let mut overhead_weighted = 0.0f64;
    let mut by_command: BTreeMap<String, i64> = BTreeMap::new();
    let mut repos: Vec<RepoLine> = Vec::new();
    let mut skipped = 0usize;

    let entries = match std::fs::read_dir(&root) {
        Ok(e) => e,
        // No cache at all yet: report zeros, not an error.
        Err(_) => {
            return render(
                json,
                &RenderMeta {
                    scope: "all repos (0 repos)",
                    public,
                    repos_tracked: 0,
                    repos_unreadable: 0,
                },
                &total,
                &[],
                &[],
            )
        }
    };
    for entry in entries.flatten() {
        let db_path = entry.path().join("runs.sqlite");
        if !db_path.is_file() {
            continue;
        }
        // Read-only: a report must never migrate/WAL-touch another repo's DB
        // (possibly being written by a live session right now).
        let Ok(db) = Db::open_read_only(&db_path) else {
            skipped += 1;
            continue;
        };
        let Ok(agg) = db.aggregate_stats(None) else {
            skipped += 1;
            continue;
        };
        if agg.runs_captured == 0 {
            continue;
        }
        // All-or-nothing per DB: totals and breakdown must not diverge.
        let (Ok(roots), Ok(commands)) = (db.repo_roots(), db.top_savings(None, None)) else {
            skipped += 1;
            continue;
        };
        for root in roots {
            // Per-repo breakdown (a cache normally holds exactly one root).
            if let Ok(sub) = db.aggregate_stats(Some(&root)) {
                repos.push(RepoLine {
                    repo: root,
                    saved: sub.saved_tokens,
                    raw: sub.raw_tokens,
                    runs: sub.runs_captured,
                });
            }
        }
        // Unlimited per-DB so cross-repo sums are exact, truncated after merge.
        for (cmd, saved) in commands {
            *by_command.entry(cmd).or_insert(0) += saved;
        }
        overhead_weighted += agg.avg_overhead_ms * agg.runs_captured as f64;
        total.runs_captured += agg.runs_captured;
        total.optimized += agg.optimized;
        total.unchanged += agg.unchanged;
        total.small_delta += agg.small_delta;
        total.large_delta += agg.large_delta;
        total.passthrough += agg.passthrough;
        total.raw_tokens += agg.raw_tokens;
        total.emitted_tokens += agg.emitted_tokens;
        total.saved_tokens += agg.saved_tokens;
        total.full_output_requested += agg.full_output_requested;
        total.internal_error += agg.internal_error;
    }
    if total.runs_captured > 0 {
        total.avg_overhead_ms = overhead_weighted / total.runs_captured as f64;
    }

    let mut top: Vec<(String, i64)> = if public {
        Vec::new()
    } else {
        by_command.into_iter().collect()
    };
    if !public {
        top.sort_by(|a, b| b.1.cmp(&a.1));
        top.truncate(5);
    }

    repos.sort_by(|a, b| b.saved.cmp(&a.saved));
    let repo_count = repos.len();
    if public {
        repos.clear();
    }

    let scope = if skipped > 0 {
        format!("all repos ({repo_count} repos, {skipped} unreadable)")
    } else {
        format!("all repos ({repo_count} repos)")
    };
    render(
        json,
        &RenderMeta {
            scope: &scope,
            public,
            repos_tracked: repo_count,
            repos_unreadable: skipped,
        },
        &total,
        &top,
        &repos,
    )
}

/// Presentation context shared by the text and JSON renderers.
struct RenderMeta<'a> {
    scope: &'a str,
    public: bool,
    repos_tracked: usize,
    repos_unreadable: usize,
}

fn render(
    json: bool,
    meta: &RenderMeta,
    agg: &StatsAgg,
    top: &[(String, i64)],
    repos: &[RepoLine],
) -> anyhow::Result<i32> {
    let RenderMeta {
        scope,
        public,
        repos_tracked,
        repos_unreadable,
    } = *meta;
    let pct = |num: i64, den: i64| {
        if den > 0 {
            num as f64 / den as f64 * 100.0
        } else {
            0.0
        }
    };
    let reduction = pct(agg.saved_tokens, agg.raw_tokens);
    let full_ratio = pct(agg.full_output_requested, agg.optimized);
    let fallback_ratio = pct(agg.internal_error, agg.runs_captured);

    if json {
        let obj = serde_json::json!({
            "scope": scope,
            "public": public,
            "repos_tracked": repos_tracked,
            "repos_unreadable": repos_unreadable,
            "commands_intercepted": agg.runs_captured,
            "runs_captured": agg.runs_captured,
            "full_outputs_stored": agg.optimized,
            "compact_outputs_returned": agg.optimized,
            "optimized_runs": agg.optimized,
            "unchanged_outputs": agg.unchanged,
            "small_deltas": agg.small_delta,
            "large_deltas": agg.large_delta,
            "passthrough": agg.passthrough,
            "estimated_raw_tokens": agg.raw_tokens,
            "estimated_emitted_tokens": agg.emitted_tokens,
            "estimated_saved_tokens": agg.saved_tokens,
            "estimated_output_tokens_suppressed": agg.saved_tokens,
            "estimated_reduction_pct": reduction,
            "full_output_requested_pct": full_ratio,
            "full_output_requests": agg.full_output_requested,
            "internal_fallback_pct": fallback_ratio,
            "avg_overhead_ms": agg.avg_overhead_ms,
            "top_savings": top
                .iter()
                .map(|(cmd, saved)| serde_json::json!({"command": cmd, "saved_tokens": saved}))
                .collect::<Vec<_>>(),
            "repos": repos
                .iter()
                .map(|r| serde_json::json!({
                    "repo": r.repo,
                    "runs": r.runs,
                    "estimated_raw_tokens": r.raw,
                    "estimated_saved_tokens": r.saved,
                    "estimated_reduction_pct": pct(r.saved, r.raw),
                }))
                .collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&obj)?);
        return Ok(0);
    }

    println!("Dejavu stats for {scope}\n");
    println!("Commands intercepted: {}", agg.runs_captured);
    println!("Runs captured: {}", agg.runs_captured);
    println!("Full outputs stored: {}", agg.optimized);
    println!("Compact outputs returned: {}", agg.optimized);
    println!("Optimized runs: {}", agg.optimized);
    println!("Unchanged outputs: {}", agg.unchanged);
    println!("Small deltas: {}", agg.small_delta);
    println!("Large deltas: {}", agg.large_delta);
    println!("Passthrough: {}", agg.passthrough);
    println!();
    println!("Estimated raw tokens: {}", fmt_int(agg.raw_tokens));
    println!("Estimated emitted tokens: {}", fmt_int(agg.emitted_tokens));
    println!("Estimated saved tokens: {}", fmt_int(agg.saved_tokens));
    println!(
        "Estimated output tokens suppressed: {}",
        fmt_int(agg.saved_tokens)
    );
    println!("Reduction on intercepted outputs: {reduction:.1}%");
    println!();
    println!("Quality:");
    println!(
        "Full-output requests: {} ({full_ratio:.1}%)",
        agg.full_output_requested
    );
    println!("Internal fallback: {fallback_ratio:.1}%");
    println!("Average overhead: {:.0}ms", agg.avg_overhead_ms);
    if public {
        println!("\nPublic mode: repo paths and command details omitted.");
    }

    if !top.is_empty() {
        println!("\nTop savings:");
        for (cmd, saved) in top {
            println!("- {cmd}: {} tokens saved", fmt_int(*saved));
        }
    }
    if !repos.is_empty() {
        println!("\nPer repo:");
        for r in repos {
            println!(
                "- {}: {} tokens saved ({:.1}%, {} runs)",
                r.repo,
                fmt_int(r.saved),
                pct(r.saved, r.raw),
                r.runs
            );
        }
    }
    Ok(0)
}

pub fn report(redact: bool) -> anyhow::Result<i32> {
    let ctx = AppCtx::resolve()?;
    ctx.layout.ensure_dirs()?;
    let db = Db::open(&ctx.layout.db())?;
    let repo = ctx.repo_root_str();
    let agg = db.aggregate_stats(Some(&repo))?;
    let top = if redact {
        Vec::new()
    } else {
        db.top_savings(Some(&repo), Some(5))?
    };

    let pct = |num: i64, den: i64| {
        if den > 0 {
            num as f64 / den as f64 * 100.0
        } else {
            0.0
        }
    };
    let reduction = pct(agg.saved_tokens, agg.raw_tokens);
    let full_ratio = pct(agg.full_output_requested, agg.optimized);
    let fallback_ratio = pct(agg.internal_error, agg.runs_captured);
    let scope = if redact { "redacted" } else { &repo };

    println!("# Dejavu Report\n");
    println!("Scope: {scope}");
    println!("Redacted: {}\n", if redact { "yes" } else { "no" });
    println!("## Summary\n");
    println!("| Metric | Value |");
    println!("|---|---:|");
    println!("| Commands intercepted | {} |", agg.runs_captured);
    println!("| Full outputs stored | {} |", agg.optimized);
    println!("| Compact outputs returned | {} |", agg.optimized);
    println!("| Unchanged outputs | {} |", agg.unchanged);
    println!("| Small deltas | {} |", agg.small_delta);
    println!("| Large deltas | {} |", agg.large_delta);
    println!("| Estimated raw tokens | {} |", fmt_int(agg.raw_tokens));
    println!(
        "| Estimated emitted tokens | {} |",
        fmt_int(agg.emitted_tokens)
    );
    println!(
        "| Estimated output tokens suppressed | {} |",
        fmt_int(agg.saved_tokens)
    );
    println!("| Reduction on intercepted outputs | {reduction:.1}% |");
    println!("| Average overhead | {:.0} ms |", agg.avg_overhead_ms);
    println!(
        "| Full-output requests | {} ({full_ratio:.1}%) |",
        agg.full_output_requested
    );
    println!("| Internal fallback | {fallback_ratio:.1}% |");

    if top.is_empty() {
        println!("\n## Top Savings\n");
        if redact {
            println!("Omitted by `--redact`.");
        } else {
            println!("No savings recorded yet.");
        }
    } else {
        println!("\n## Top Savings\n");
        for (cmd, saved) in top {
            println!("- `{cmd}`: {} estimated tokens suppressed", fmt_int(saved));
        }
    }

    println!("\n## Notes\n");
    println!(
        "- Generated locally by `dejavu report{}`.",
        if redact { " --redact" } else { "" }
    );
    println!("- Full command output logs are not included in this report.");
    println!("- Token counts are estimates using Dejavu's configured estimator.");
    Ok(0)
}
