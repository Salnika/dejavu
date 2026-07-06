//! Docker logs (spec §16.11). Logs append at the tail, so we strip the common
//! *prefix* shared with the previous run and emit only the new trailing lines;
//! when there is no comparable prior, emit a bounded tail.

use super::SpecialOutput;
use crate::reduce::Classification;

const TAIL_LINES: usize = 20;

pub fn reduce(
    normalized: &str,
    prior: Option<&str>,
    class: Classification,
    prev_short: Option<&str>,
    max_lines: usize,
) -> Option<SpecialOutput> {
    let cur: Vec<&str> = normalized.lines().collect();
    let prev = prev_short.unwrap_or("?");
    let tail_cap = max_lines.min(TAIL_LINES.max(max_lines));

    let (status, body) = match (class, prior) {
        (Classification::Unchanged, _) => (
            format!("docker logs unchanged since run {prev}."),
            format!("{} log lines, no new output.", cur.len()),
        ),
        (Classification::SmallDelta | Classification::LargeDelta, Some(prior_text)) => {
            let prev_lines: Vec<&str> = prior_text.lines().collect();
            let common = common_prefix_len(&prev_lines, &cur);
            let new_lines = &cur[common..];
            if new_lines.is_empty() {
                (
                    format!("docker logs unchanged since run {prev}."),
                    format!("{} log lines, no new output.", cur.len()),
                )
            } else {
                let shown: Vec<&str> = new_lines.iter().take(tail_cap).copied().collect();
                (
                    "docker logs delta.".to_string(),
                    format!(
                        "{} new lines since run {prev}.\n\n{}",
                        new_lines.len(),
                        shown.join("\n")
                    ),
                )
            }
        }
        _ => {
            // First seen (or no usable prior): bounded tail.
            let start = cur.len().saturating_sub(tail_cap);
            let shown = &cur[start..];
            (
                "docker logs.".to_string(),
                format!(
                    "{} log lines. Showing last {}:\n\n{}",
                    cur.len(),
                    shown.len(),
                    shown.join("\n")
                ),
            )
        }
    };

    Some(SpecialOutput {
        status,
        body,
        full_label: "Full logs",
        summary: format!("{} log lines", cur.len()),
    })
}

fn common_prefix_len(a: &[&str], b: &[&str]) -> usize {
    a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count()
}
