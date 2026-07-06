//! The common compact-output envelope (spec §16.1).

use crate::commands::fmt_int;

pub struct Envelope<'a> {
    pub status: &'a str,
    pub command: &'a str,
    pub exit_code: i32,
    /// An optional emphasis line under the header (e.g. exit-code change).
    pub headline: Option<&'a str>,
    /// An optional note (e.g. git-state annotation).
    pub note: Option<&'a str>,
    pub body: &'a str,
    pub suppressed_tokens: i64,
    pub run_id_short: &'a str,
    pub prev_id_short: Option<&'a str>,
    /// `Full output` / `Full diff` / `Full logs`.
    pub full_label: &'a str,
}

/// Render the envelope to a string. Never leaks internal fields
/// (hashes/command_key/sqlite paths) — only the opaque short ids appear.
pub fn render(env: &Envelope) -> String {
    let mut out = String::new();
    out.push_str(&format!("dejavu: {}\n", env.status));
    out.push_str(&format!("Command: {}\n", env.command));
    out.push_str(&format!("Exit code: {}\n", env.exit_code));
    if let Some(headline) = env.headline {
        out.push_str(headline);
        out.push('\n');
    }
    if let Some(note) = env.note {
        out.push_str(note);
        out.push('\n');
    }
    if !env.body.trim().is_empty() {
        out.push('\n');
        out.push_str(env.body.trim_end());
        out.push('\n');
    }
    out.push_str(&format!(
        "\nSuppressed ~{} estimated tokens.\n",
        fmt_int(env.suppressed_tokens)
    ));
    out.push_str(&format!(
        "{}: dejavu show {} --stdout\n",
        env.full_label, env.run_id_short
    ));
    if let Some(prev) = env.prev_id_short {
        out.push_str(&format!("Previous output: dejavu show {prev} --stdout\n"));
    }

    debug_assert!(
        !out.contains("normalized_hash") && !out.contains("runs.sqlite"),
        "compact output must not leak internal fields"
    );
    out
}
