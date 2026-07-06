//! `rg` / `grep` search results (spec §16.7).

use super::SpecialOutput;
use crate::reduce::Classification;
use regex::Regex;
use std::collections::BTreeSet;
use std::sync::LazyLock;

static MATCH_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([^:\n]+):(\d+):(.*)$").unwrap());

struct Match {
    file: String,
    line: String,
    text: String,
}

fn parse(normalized: &str) -> Vec<Match> {
    normalized
        .lines()
        .filter_map(|l| MATCH_LINE.captures(l))
        .map(|c| Match {
            file: c[1].to_string(),
            line: c[2].to_string(),
            text: c[3].trim().to_string(),
        })
        .collect()
}

fn key(m: &Match) -> String {
    format!("{}:{}:{}", m.file, m.line, m.text)
}

pub fn reduce(
    normalized: &str,
    prior: Option<&str>,
    class: Classification,
    prev_short: Option<&str>,
    max_lines: usize,
) -> Option<SpecialOutput> {
    let matches = parse(normalized);
    // If the output isn't the plain `path:line:text` form (e.g. --json, -l),
    // degrade to the generic reducer.
    if matches.is_empty() && !normalized.trim().is_empty() {
        return None;
    }
    let files: BTreeSet<&str> = matches.iter().map(|m| m.file.as_str()).collect();
    let count_line = format!("{} matches in {} files.", matches.len(), files.len());
    let prev = prev_short.unwrap_or("?");

    let (status, body) = match class {
        Classification::Unchanged => (
            format!("search unchanged since run {prev}."),
            count_line.clone(),
        ),
        Classification::SmallDelta | Classification::LargeDelta => {
            let prior_matches = prior.map(parse).unwrap_or_default();
            let prev_keys: BTreeSet<String> = prior_matches.iter().map(key).collect();
            let cur_keys: BTreeSet<String> = matches.iter().map(key).collect();
            let mut lines = vec![count_line.clone()];
            let added: Vec<&Match> = matches
                .iter()
                .filter(|m| !prev_keys.contains(&key(m)))
                .collect();
            if !added.is_empty() {
                lines.push("Added matches:".to_string());
                for m in added.iter().take(max_lines) {
                    lines.push(format!("+ {}:{} {}", m.file, m.line, m.text));
                }
            }
            let removed: Vec<&Match> = prior_matches
                .iter()
                .filter(|m| !cur_keys.contains(&key(m)))
                .collect();
            if !removed.is_empty() {
                lines.push("Removed matches:".to_string());
                for m in removed.iter().take(max_lines) {
                    lines.push(format!("- {}:{} {}", m.file, m.line, m.text));
                }
            }
            (
                format!("search changed since run {prev}."),
                lines.join("\n"),
            )
        }
        Classification::FirstSeen => {
            let mut lines = vec![count_line.clone()];
            if !files.is_empty() {
                lines.push("Files:".to_string());
                for f in files.iter().take(8) {
                    lines.push(format!("- {f}"));
                }
            }
            lines.push("Sample matches:".to_string());
            for m in matches.iter().take(max_lines.min(20)) {
                lines.push(format!("{}:{} {}", m.file, m.line, m.text));
            }
            ("search result.".to_string(), lines.join("\n"))
        }
    };

    Some(SpecialOutput {
        status,
        body,
        full_label: "Full output",
        summary: count_line,
    })
}
