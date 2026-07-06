//! `dejavu repos` — list repos where Dejavu has recorded activity.

use super::{fmt_int, render};
use crate::paths::CacheLayout;
use crate::state;
use crate::store::{Db, StatsAgg};

#[derive(serde::Serialize)]
struct RepoLine {
    repo: String,
    status: &'static str,
    runs: i64,
    sessions: i64,
    estimated_saved_tokens: i64,
    latest_activity: Option<String>,
    cache_dir: String,
}

pub fn run(json: bool, all: bool) -> anyhow::Result<i32> {
    let (mut repos, unreadable) = collect_repos(all)?;
    repos.sort_by(|a, b| {
        b.latest_activity
            .cmp(&a.latest_activity)
            .then_with(|| a.repo.cmp(&b.repo))
    });

    if json {
        let obj = serde_json::json!({
            "repos": repos,
            "repos_tracked": repos.len(),
            "repos_unreadable": unreadable,
            "include_disabled": all,
        });
        println!("{}", serde_json::to_string_pretty(&obj)?);
        return Ok(0);
    }

    let scope = if all {
        "all tracked repos"
    } else {
        "active repos"
    };
    render::title("Dejavu repos");
    render::kv(&[
        ("Scope", scope.to_string()),
        ("Repos listed", repos.len().to_string()),
        ("Unreadable caches", unreadable.to_string()),
    ]);
    if unreadable > 0 {
        println!("\nSome cache directories could not be read.");
    }
    if repos.is_empty() {
        println!();
        println!("No repos found.");
        return Ok(0);
    }
    render::section("Repositories");
    for repo in repos {
        let latest = repo
            .latest_activity
            .map(|t| render::human_time(&t))
            .unwrap_or_else(|| "never".to_string());
        let bullet = if repo.status == "active" {
            render::Style::Green
        } else {
            render::Style::Yellow
        };
        render::record(
            bullet,
            &repo.repo,
            &[
                repo.status.to_string(),
                format!("{} runs", repo.runs),
                format!(
                    "{} session{}",
                    repo.sessions,
                    if repo.sessions == 1 { "" } else { "s" }
                ),
                format!("{} tokens saved", fmt_int(repo.estimated_saved_tokens)),
                format!("latest {latest}"),
            ],
        );
    }
    Ok(0)
}

fn collect_repos(include_disabled: bool) -> anyhow::Result<(Vec<RepoLine>, usize)> {
    let root = match crate::paths::cache_root() {
        Ok(root) => root,
        Err(_) => return Ok((Vec::new(), 0)),
    };
    let entries = match std::fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(_) => return Ok((Vec::new(), 0)),
    };

    let mut repos = Vec::new();
    let mut unreadable = 0usize;
    for entry in entries.flatten() {
        let cache_dir = entry.path();
        let db_path = cache_dir.join("runs.sqlite");
        if !db_path.is_file() {
            continue;
        }
        let layout = CacheLayout::from_dir(cache_dir.clone());
        let disabled = state::load(&layout).disabled;
        if disabled && !include_disabled {
            continue;
        }
        let Ok(db) = Db::open_read_only(&db_path) else {
            unreadable += 1;
            continue;
        };
        let Ok(roots) = db.repo_roots() else {
            unreadable += 1;
            continue;
        };
        if roots.is_empty() {
            continue;
        }
        for repo in roots {
            let agg = db
                .aggregate_stats(Some(&repo))
                .unwrap_or_else(|_| StatsAgg::default());
            let sessions = db.session_count(&repo).unwrap_or(0);
            let latest = db.latest_activity(&repo).unwrap_or(None);
            repos.push(RepoLine {
                repo,
                status: if disabled { "disabled" } else { "active" },
                runs: agg.runs_captured,
                sessions,
                estimated_saved_tokens: agg.saved_tokens,
                latest_activity: latest,
                cache_dir: cache_dir.to_string_lossy().into_owned(),
            });
        }
    }
    Ok((repos, unreadable))
}
