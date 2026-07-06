//! `git diff` summary (spec §16.8): files changed + per-file +/- hunk counts.
//! Always renders the hunk summary of the current diff (no diff-of-diffs).

use super::SpecialOutput;
use crate::reduce::Classification;

struct FileDiff {
    name: String,
    added: usize,
    removed: usize,
    binary: bool,
}

fn parse(normalized: &str) -> Vec<FileDiff> {
    let mut files: Vec<FileDiff> = Vec::new();
    for line in normalized.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            let name = rest
                .split_whitespace()
                .next_back()
                .and_then(|s| s.strip_prefix("b/").or(Some(s)))
                .unwrap_or(rest)
                .to_string();
            files.push(FileDiff {
                name,
                added: 0,
                removed: 0,
                binary: false,
            });
        } else if line.starts_with("Binary files ") {
            if let Some(f) = files.last_mut() {
                f.binary = true;
            }
        } else if let Some(f) = files.last_mut() {
            if line.starts_with("+++") || line.starts_with("---") {
                continue;
            }
            if line.starts_with('+') {
                f.added += 1;
            } else if line.starts_with('-') {
                f.removed += 1;
            }
        }
    }
    files
}

pub fn reduce(
    normalized: &str,
    _prior: Option<&str>,
    class: Classification,
    prev_short: Option<&str>,
    _max_lines: usize,
) -> Option<SpecialOutput> {
    let files = parse(normalized);
    if files.is_empty() {
        // Empty diff or an unexpected shape → generic.
        return None;
    }
    let count_line = format!("{} files changed:", files.len());
    let prev = prev_short.unwrap_or("?");

    let mut body = vec![count_line.clone()];
    for f in &files {
        body.push(format!("- {}", f.name));
    }
    body.push("Hunk summary:".to_string());
    for f in &files {
        if f.binary {
            body.push(format!("- {}: binary", f.name));
        } else {
            body.push(format!("- {}: +{} -{}", f.name, f.added, f.removed));
        }
    }

    let status = match class {
        Classification::Unchanged => format!("git diff unchanged since run {prev}."),
        Classification::FirstSeen => "git diff summary.".to_string(),
        _ => format!("git diff changed since run {prev}."),
    };
    let body = if matches!(class, Classification::Unchanged) {
        format!("{} files changed.", files.len())
    } else {
        body.join("\n")
    };

    Some(SpecialOutput {
        status,
        body,
        full_label: "Full diff",
        summary: count_line,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_files_and_hunks() {
        let diff = "diff --git a/src/session.ts b/src/session.ts\n\
--- a/src/session.ts\n+++ b/src/session.ts\n@@ -1,3 +1,4 @@\n line\n-old\n+new1\n+new2\n\
diff --git a/package.json b/package.json\n--- a/package.json\n+++ b/package.json\n@@ -1 +1 @@\n-x\n+y\n";
        let files = parse(diff);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].name, "src/session.ts");
        assert_eq!(files[0].added, 2);
        assert_eq!(files[0].removed, 1);
        assert_eq!(files[1].added, 1);
        assert_eq!(files[1].removed, 1);
    }
}
