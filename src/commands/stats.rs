//! `dejavu stats` — token-savings report for the current repo (spec §17.6),
//! or across every tracked repo with `--all`.

use super::{fmt_int, render};
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

    render::title("Dejavu stats");
    let mut scope_rows = vec![("Scope", scope.to_string())];
    if repos_tracked > 0 || repos_unreadable > 0 {
        scope_rows.push(("Repos tracked", repos_tracked.to_string()));
        scope_rows.push(("Repos unreadable", repos_unreadable.to_string()));
    }
    render::kv(&scope_rows);

    render::section("Runs");
    render::table(
        &["Metric", "Value"],
        &[
            vec![
                "Commands intercepted".to_string(),
                agg.runs_captured.to_string(),
            ],
            vec!["Runs captured".to_string(), agg.runs_captured.to_string()],
            vec!["Full outputs stored".to_string(), agg.optimized.to_string()],
            vec![
                "Compact outputs returned".to_string(),
                agg.optimized.to_string(),
            ],
            vec!["Optimized runs".to_string(), agg.optimized.to_string()],
            vec![
                "Unchanged outputs (higher = better)".to_string(),
                agg.unchanged.to_string(),
            ],
            vec!["Small deltas".to_string(), agg.small_delta.to_string()],
            vec!["Large deltas".to_string(), agg.large_delta.to_string()],
            vec!["Passthrough".to_string(), agg.passthrough.to_string()],
        ],
    );

    render::section("Tokens");
    render::table(
        &["Metric", "Value"],
        &[
            vec!["Estimated raw tokens".to_string(), fmt_int(agg.raw_tokens)],
            vec![
                "Estimated emitted tokens".to_string(),
                fmt_int(agg.emitted_tokens),
            ],
            vec![
                "Estimated saved tokens (higher = better)".to_string(),
                fmt_int(agg.saved_tokens),
            ],
            vec![
                "Estimated output tokens suppressed".to_string(),
                fmt_int(agg.saved_tokens),
            ],
        ],
    );
    println!();
    println!(
        "{}",
        render::meter("Reduction on intercepted outputs", reduction)
    );

    render::section("Quality");
    render::table(
        &["Metric", "Value"],
        &[
            vec![
                "Full-output requests (lower = better)".to_string(),
                format!("{} ({full_ratio:.1}%)", agg.full_output_requested),
            ],
            vec![
                "Internal fallback (lower = better)".to_string(),
                format!("{fallback_ratio:.1}%"),
            ],
            vec![
                "Average overhead (lower = better)".to_string(),
                format!("{:.0}ms", agg.avg_overhead_ms),
            ],
        ],
    );
    if public {
        println!("\nPublic mode: repo paths and command details omitted.");
    }

    if !top.is_empty() {
        render::section("Top savings");
        let rows: Vec<Vec<String>> = top
            .iter()
            .map(|(cmd, saved)| vec![fmt_int(*saved), cmd.clone()])
            .collect();
        render::table_styled(
            &["Saved tokens", "Command"],
            &rows,
            &[render::Style::Green, render::Style::Cyan],
        );
    }
    if !repos.is_empty() {
        render::section("Per repo");
        let paths: Vec<&str> = repos.iter().map(|r| r.repo.as_str()).collect();
        let names = short_repo_names(&paths);
        for (r, name) in repos.iter().zip(names) {
            render::record(
                render::Style::Green,
                &name,
                &[
                    format!("{} runs", r.runs),
                    format!("{} tokens saved", fmt_int(r.saved)),
                    format!("{:.1}% reduction", pct(r.saved, r.raw)),
                ],
            );
        }
    }
    Ok(0)
}

/// Short display names for repo paths: the last path component, extended with
/// parent components until every name in the list is unique — ten benchmark
/// repos all named `proj` must not collapse into one label. The full path
/// stays available in `--json` and `dejavu repos`.
fn short_repo_names(paths: &[&str]) -> Vec<String> {
    let comps: Vec<Vec<&str>> = paths
        .iter()
        .map(|p| p.split('/').filter(|c| !c.is_empty()).collect())
        .collect();
    let mut depth: Vec<usize> = vec![1; paths.len()];
    loop {
        let names: Vec<String> = comps
            .iter()
            .zip(&depth)
            .map(|(c, d)| c[c.len().saturating_sub(*d)..].join("/"))
            .collect();
        let mut grew = false;
        for i in 0..names.len() {
            let colliding = names
                .iter()
                .enumerate()
                .any(|(j, n)| j != i && n == &names[i]);
            if colliding && depth[i] < comps[i].len() {
                depth[i] += 1;
                grew = true;
            }
        }
        if !grew {
            return names;
        }
    }
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
#[cfg(test)]
mod tests {
    use super::short_repo_names;

    #[test]
    fn short_names_disambiguate_colliding_basenames() {
        let names = short_repo_names(&[
            "/tmp/bench/runs/codex_low_dv_r1/proj",
            "/tmp/bench/runs/codex_xhigh_dv_r2/proj",
            "/home/alexis/work/dejavu",
        ]);
        assert_eq!(names[0], "codex_low_dv_r1/proj");
        assert_eq!(names[1], "codex_xhigh_dv_r2/proj");
        assert_eq!(names[2], "dejavu");
    }

    #[test]
    fn short_names_stay_short_when_unique() {
        let names = short_repo_names(&["/a/b/alpha", "/a/b/beta"]);
        assert_eq!(names, vec!["alpha", "beta"]);
    }
}
