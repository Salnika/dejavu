//! `git status` summary (spec §16.9): branch + change counts, and what changed
//! since the previous status.

use super::SpecialOutput;
use crate::reduce::Classification;
use regex::Regex;
use std::collections::BTreeSet;
use std::sync::LazyLock;

static PORCELAIN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([ MADRCU?]{2}) (.+)$").unwrap());

#[derive(Default)]
struct Status {
    branch: Option<String>,
    entries: BTreeSet<String>, // "code path" for delta detection
    modified: usize,
    added: usize,
    deleted: usize,
    untracked: usize,
}

fn parse(normalized: &str) -> Status {
    let mut s = Status::default();
    let mut in_untracked = false;
    for line in normalized.lines() {
        if let Some(b) = line.strip_prefix("## ") {
            s.branch = Some(b.split("...").next().unwrap_or(b).trim().to_string());
            continue;
        }
        if let Some(b) = line.strip_prefix("On branch ") {
            s.branch = Some(b.trim().to_string());
            continue;
        }
        if let Some(c) = PORCELAIN.captures(line) {
            let xy = c[1].to_string();
            // A blank code ("  ") is not a real porcelain entry — it's an
            // indented human line (indentation became spaces after detabbing).
            if !xy.trim().is_empty() {
                let path = c[2].trim().to_string();
                s.entries.insert(format!("{xy} {path}"));
                if xy == "??" {
                    s.untracked += 1;
                } else if xy.contains('A') {
                    s.added += 1;
                } else if xy.contains('D') {
                    s.deleted += 1;
                } else {
                    s.modified += 1;
                }
                continue;
            }
        }
        // Human form.
        let t = line.trim();
        if t.starts_with("Untracked files:") {
            in_untracked = true;
            continue;
        }
        if let Some(rest) = t.strip_prefix("modified:") {
            s.modified += 1;
            s.entries.insert(format!("M {}", rest.trim()));
        } else if let Some(rest) = t.strip_prefix("new file:") {
            s.added += 1;
            s.entries.insert(format!("A {}", rest.trim()));
        } else if let Some(rest) = t.strip_prefix("deleted:") {
            s.deleted += 1;
            s.entries.insert(format!("D {}", rest.trim()));
        } else if in_untracked
            && line.starts_with(['\t', ' '])
            && !t.is_empty()
            && !t.starts_with('(')
        {
            s.untracked += 1;
            s.entries.insert(format!("?? {t}"));
        } else if t.is_empty() {
            in_untracked = false;
        }
    }
    s
}

pub fn reduce(
    normalized: &str,
    prior: Option<&str>,
    class: Classification,
    prev_short: Option<&str>,
    _max_lines: usize,
) -> Option<SpecialOutput> {
    let s = parse(normalized);
    if s.branch.is_none() && s.entries.is_empty() {
        return None; // not a recognizable status → generic
    }
    let prev = prev_short.unwrap_or("?");
    let branch = s.branch.clone().unwrap_or_else(|| "(unknown)".to_string());

    let mut body = vec![
        format!("Branch: {branch}"),
        format!("Modified: {}", s.modified),
        format!("Added: {}", s.added),
        format!("Deleted: {}", s.deleted),
        format!("Untracked: {}", s.untracked),
    ];

    let status = match class {
        Classification::Unchanged => format!("git status unchanged since run {prev}."),
        Classification::FirstSeen => "git status.".to_string(),
        _ => {
            if let Some(prior_text) = prior {
                let prior_status = parse(prior_text);
                let added_entries: Vec<&String> =
                    s.entries.difference(&prior_status.entries).collect();
                if !added_entries.is_empty() {
                    body.push("Changed since previous status:".to_string());
                    for e in added_entries.iter().take(10) {
                        body.push(format!("+ {e}"));
                    }
                }
            }
            format!("git status changed since run {prev}.")
        }
    };

    let summary = format!(
        "Branch {branch}: {} modified, {} added, {} deleted, {} untracked",
        s.modified, s.added, s.deleted, s.untracked
    );
    Some(SpecialOutput {
        status,
        body: body.join("\n"),
        full_label: "Full output",
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_human_status_with_untracked() {
        let text = "On branch feature/auth-cache\n\
\tmodified:   src/auth/session.ts\n\
\tmodified:   package.json\n\
Untracked files:\n\
\tsrc/auth/token-cache.ts\n";
        let s = parse(text);
        assert_eq!(s.branch.as_deref(), Some("feature/auth-cache"));
        assert_eq!(s.modified, 2);
        assert_eq!(s.untracked, 1);
    }

    #[test]
    fn parses_space_indented_human_status() {
        // What the reducer actually sees after tabs are expanded to spaces.
        let text = "On branch main\n    modified:   a.ts\n    modified:   b.ts\nUntracked files:\n    c.ts\n";
        let s = parse(text);
        assert_eq!(s.modified, 2);
        assert_eq!(s.untracked, 1);
    }

    #[test]
    fn parses_porcelain_status() {
        let text = "## main...origin/main\n M src/a.ts\n?? new.ts\nA  added.ts\n D gone.ts\n";
        let s = parse(text);
        assert_eq!(s.branch.as_deref(), Some("main"));
        assert_eq!(s.modified, 1);
        assert_eq!(s.untracked, 1);
        assert_eq!(s.added, 1);
        assert_eq!(s.deleted, 1);
    }
}
