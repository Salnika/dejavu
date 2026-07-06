//! Shared model for findings-based reducers (tsc/eslint/jest/pytest) and the
//! New/Unchanged/Fixed grouping (spec §16.3). Stability key is `(file, code)` —
//! never the line number, which shifts as code is edited.

use super::SpecialOutput;
use crate::reduce::Classification;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub file: String,
    /// Displayed location (e.g. `84:12` or a test name); not part of the key.
    pub loc: String,
    /// Rule / error code (e.g. `TS2322`, `no-explicit-any`, `FAIL`).
    pub code: String,
    pub detail: Option<String>,
}

impl Finding {
    fn key(&self) -> (&str, &str) {
        (&self.file, &self.code)
    }

    fn display(&self) -> String {
        let loc = if self.loc.is_empty() {
            String::new()
        } else {
            format!(":{}", self.loc)
        };
        match &self.detail {
            Some(d) => format!("- {}{} {}\n  {}", self.file, loc, self.code, d),
            None => format!("- {}{} {}", self.file, loc, self.code),
        }
    }

    fn display_brief(&self) -> String {
        let loc = if self.loc.is_empty() {
            String::new()
        } else {
            format!(":{}", self.loc)
        };
        format!("- {}{} {}", self.file, loc, self.code)
    }
}

pub struct Labels {
    pub noun: &'static str,
    pub fail_status: &'static str,
    pub ok_status: &'static str,
    pub item_plural: &'static str,
}

/// Compose the compact body for a findings-based family across all states.
pub fn compose(
    labels: &Labels,
    current: &[Finding],
    prior: Option<&[Finding]>,
    class: Classification,
    exit_code: i32,
    prev_short: Option<&str>,
    max_lines: usize,
) -> SpecialOutput {
    let count = current.len();
    let prev = prev_short.unwrap_or("?");

    let (status, body) = match class {
        Classification::FirstSeen => {
            let status = if exit_code != 0 {
                labels.fail_status.to_string()
            } else {
                labels.ok_status.to_string()
            };
            (status, list_body(labels, current, prior, max_lines))
        }
        Classification::LargeDelta => (
            format!("{} changed significantly since run {prev}.", labels.noun),
            list_body(labels, current, prior, max_lines),
        ),
        Classification::Unchanged => {
            let mut body = format!("Same {count} {}:", labels.item_plural);
            for f in current.iter().take(max_lines) {
                body.push('\n');
                body.push_str(&f.display_brief());
            }
            (format!("{} unchanged since run {prev}.", labels.noun), body)
        }
        Classification::SmallDelta => (
            format!("{} changed since run {prev}.", labels.noun),
            delta_body(current, prior, max_lines),
        ),
    };

    let summary = list_body(labels, current, prior, 12);
    SpecialOutput {
        status,
        body,
        full_label: "Full output",
        summary,
    }
}

fn list_body(
    labels: &Labels,
    current: &[Finding],
    prior: Option<&[Finding]>,
    max_lines: usize,
) -> String {
    let mut lines = vec![format!("{} {}:", current.len(), labels.item_plural)];

    let (new, _unchanged, fixed) = group(current, prior);
    // On a first run `prior` is None → everything is "new"; just list them all.
    let show_grouping = prior.is_some() && (!new.is_empty() || fixed > 0);

    if show_grouping {
        if !new.is_empty() {
            lines.push("New since previous run:".to_string());
            for f in new.iter().take(max_lines) {
                lines.push(f.display());
            }
        }
        if fixed > 0 {
            lines.push(format!("Fixed: {fixed}"));
        }
    } else {
        for f in current.iter().take(max_lines) {
            lines.push(f.display());
        }
    }
    lines.join("\n")
}

fn delta_body(current: &[Finding], prior: Option<&[Finding]>, max_lines: usize) -> String {
    let (new, unchanged, fixed) = group(current, prior);
    let mut lines = Vec::new();
    if !new.is_empty() {
        lines.push("New:".to_string());
        for f in new.iter().take(max_lines) {
            lines.push(f.display());
        }
    }
    if fixed > 0 {
        lines.push(format!("Fixed: {fixed}"));
    }
    if !unchanged.is_empty() {
        lines.push(format!("Unchanged: {}", unchanged.len()));
    }
    if lines.is_empty() {
        lines.push("No net change in findings.".to_string());
    }
    lines.join("\n")
}

/// Partition current findings against prior by `(file, code)`.
fn group<'a>(
    current: &'a [Finding],
    prior: Option<&'a [Finding]>,
) -> (Vec<&'a Finding>, Vec<&'a Finding>, usize) {
    let prior = match prior {
        Some(p) => p,
        None => return (current.iter().collect(), Vec::new(), 0),
    };
    let prior_keys: std::collections::HashSet<(&str, &str)> =
        prior.iter().map(Finding::key).collect();
    let current_keys: std::collections::HashSet<(&str, &str)> =
        current.iter().map(Finding::key).collect();

    let mut new = Vec::new();
    let mut unchanged = Vec::new();
    for f in current {
        if prior_keys.contains(&f.key()) {
            unchanged.push(f);
        } else {
            new.push(f);
        }
    }
    let fixed = prior
        .iter()
        .filter(|f| !current_keys.contains(&f.key()))
        .count();
    (new, unchanged, fixed)
}
