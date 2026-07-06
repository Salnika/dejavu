//! `ls` / `tree` / `find` directory listings (spec §16.10).

use super::SpecialOutput;
use crate::reduce::Classification;
use regex::Regex;
use std::sync::LazyLock;

static TREE_FOOTER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d+ director(?:y|ies)(?:, \d+ files?)?$").unwrap());
static TREE_CONNECTORS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[│├└─\s]+").unwrap());

fn entries(normalized: &str) -> Vec<String> {
    normalized
        .lines()
        .filter(|l| !l.trim().is_empty() && !TREE_FOOTER.is_match(l.trim()))
        .map(|l| l.to_string())
        .collect()
}

/// Top-level names: for `tree`, entries whose only connector prefix is the
/// first level; for `ls`/`find`, the first several entries.
fn top_level(entries: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for e in entries {
        let cleaned = TREE_CONNECTORS.replace(e, "").to_string();
        let cleaned = cleaned.trim().to_string();
        if !cleaned.is_empty() {
            out.push(cleaned);
        }
        if out.len() >= 8 {
            break;
        }
    }
    out
}

pub fn reduce(
    normalized: &str,
    _prior: Option<&str>,
    class: Classification,
    prev_short: Option<&str>,
    _max_lines: usize,
) -> Option<SpecialOutput> {
    let entries = entries(normalized);
    if entries.is_empty() {
        return None;
    }
    let count = entries.len();
    let prev = prev_short.unwrap_or("?");
    let count_line = format!("{count} entries.");

    let (status, body) = match class {
        Classification::Unchanged => (
            format!("directory listing unchanged since run {prev}."),
            count_line.clone(),
        ),
        Classification::FirstSeen => {
            let mut lines = vec![count_line.clone(), "Top-level:".to_string()];
            for name in top_level(&entries) {
                lines.push(format!("- {name}"));
            }
            ("directory listing.".to_string(), lines.join("\n"))
        }
        _ => {
            let mut lines = vec![count_line.clone(), "Top-level:".to_string()];
            for name in top_level(&entries) {
                lines.push(format!("- {name}"));
            }
            (
                format!("directory listing changed since run {prev}."),
                lines.join("\n"),
            )
        }
    };

    Some(SpecialOutput {
        status,
        body,
        full_label: "Full output",
        summary: count_line,
    })
}
