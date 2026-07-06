//! Line-level diff statistics and compact unified diffs (spec §12.4).

use similar::{ChangeTag, TextDiff};

pub struct DiffStats {
    pub changed_lines: usize,
    pub ratio: f64,
}

/// `changed_lines = inserts + deletes`; `ratio = changed / max(prev, curr)`
/// (decision #6). Both-empty → ratio 0.0.
pub fn diff_stats(prev: &str, curr: &str) -> DiffStats {
    let diff = TextDiff::from_lines(prev, curr);
    let mut changed = 0usize;
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert | ChangeTag::Delete => changed += 1,
            ChangeTag::Equal => {}
        }
    }
    let prev_lines = prev.lines().count();
    let curr_lines = curr.lines().count();
    let denom = prev_lines.max(curr_lines).max(1);
    DiffStats {
        changed_lines: changed,
        ratio: changed as f64 / denom as f64,
    }
}

/// A compact unified diff, capped to `max_lines` output lines.
pub fn unified_diff(prev: &str, curr: &str, context: usize, max_lines: usize) -> String {
    let diff = TextDiff::from_lines(prev, curr);
    let mut out: Vec<String> = Vec::new();
    'outer: for group in diff.grouped_ops(context) {
        for op in group {
            for change in diff.iter_changes(&op) {
                let sign = match change.tag() {
                    ChangeTag::Delete => '-',
                    ChangeTag::Insert => '+',
                    ChangeTag::Equal => ' ',
                };
                let value = change.value();
                let value = value.strip_suffix('\n').unwrap_or(value);
                out.push(format!("{sign}{value}"));
                if out.len() >= max_lines {
                    out.push("... (diff truncated)".to_string());
                    break 'outer;
                }
            }
        }
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_has_no_changes() {
        let stats = diff_stats("a\nb\nc\n", "a\nb\nc\n");
        assert_eq!(stats.changed_lines, 0);
        assert_eq!(stats.ratio, 0.0);
    }

    #[test]
    fn one_line_change_is_small() {
        let stats = diff_stats("a\nb\nc\n", "a\nB\nc\n");
        // one delete + one insert
        assert_eq!(stats.changed_lines, 2);
        assert!(stats.ratio < 0.8);
    }

    #[test]
    fn unified_diff_shows_the_change() {
        let d = unified_diff("a\nb\nc\n", "a\nB\nc\n", 1, 100);
        assert!(d.contains("-b"));
        assert!(d.contains("+B"));
    }

    #[test]
    fn unified_diff_caps_lines() {
        let prev: String = (0..100).map(|i| format!("line {i}\n")).collect();
        let curr: String = (0..100).map(|i| format!("changed {i}\n")).collect();
        let d = unified_diff(&prev, &curr, 3, 10);
        assert!(d.lines().count() <= 11);
        assert!(d.contains("truncated"));
    }
}
