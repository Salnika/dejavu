//! Row structs for `runs` and `sessions`.

use rusqlite::Row;

/// One captured command run. Mirrors the `runs` table (spec §15.1) plus
/// `overhead_ms`. `classification`/`comparison_result` are stored as strings.
#[derive(Debug, Clone)]
pub struct RunRecord {
    pub id: String,
    pub session_id: String,
    pub created_at: String,
    pub repo_root: String,
    pub cwd: String,
    pub shim_name: String,
    pub argv_json: String,
    pub command_original: String,
    pub command_family: String,
    pub command_key: String,
    pub classification: String,
    pub exit_code: i64,
    pub duration_ms: i64,
    pub overhead_ms: i64,
    pub stdout_path: Option<String>,
    pub stderr_path: Option<String>,
    pub normalized_path: Option<String>,
    pub raw_stdout_bytes: i64,
    pub raw_stderr_bytes: i64,
    pub raw_total_bytes: i64,
    pub emitted_bytes: i64,
    pub estimated_raw_tokens: i64,
    pub estimated_emitted_tokens: i64,
    pub estimated_saved_tokens: i64,
    pub normalized_hash: Option<String>,
    pub stdout_hash: Option<String>,
    pub stderr_hash: Option<String>,
    pub git_head: Option<String>,
    pub git_worktree_hash: Option<String>,
    pub comparison_base_run_id: Option<String>,
    pub comparison_result: String,
    pub summary: Option<String>,
    pub full_output_requested: i64,
    pub internal_error: Option<String>,
}

impl RunRecord {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<RunRecord> {
        Ok(RunRecord {
            id: row.get("id")?,
            session_id: row.get("session_id")?,
            created_at: row.get("created_at")?,
            repo_root: row.get("repo_root")?,
            cwd: row.get("cwd")?,
            shim_name: row.get("shim_name")?,
            argv_json: row.get("argv_json")?,
            command_original: row.get("command_original")?,
            command_family: row.get("command_family")?,
            command_key: row.get("command_key")?,
            classification: row.get("classification")?,
            exit_code: row.get("exit_code")?,
            duration_ms: row.get("duration_ms")?,
            overhead_ms: row.get("overhead_ms")?,
            stdout_path: row.get("stdout_path")?,
            stderr_path: row.get("stderr_path")?,
            normalized_path: row.get("normalized_path")?,
            raw_stdout_bytes: row.get("raw_stdout_bytes")?,
            raw_stderr_bytes: row.get("raw_stderr_bytes")?,
            raw_total_bytes: row.get("raw_total_bytes")?,
            emitted_bytes: row.get("emitted_bytes")?,
            estimated_raw_tokens: row.get("estimated_raw_tokens")?,
            estimated_emitted_tokens: row.get("estimated_emitted_tokens")?,
            estimated_saved_tokens: row.get("estimated_saved_tokens")?,
            normalized_hash: row.get("normalized_hash")?,
            stdout_hash: row.get("stdout_hash")?,
            stderr_hash: row.get("stderr_hash")?,
            git_head: row.get("git_head")?,
            git_worktree_hash: row.get("git_worktree_hash")?,
            comparison_base_run_id: row.get("comparison_base_run_id")?,
            comparison_result: row.get("comparison_result")?,
            summary: row.get("summary")?,
            full_output_requested: row.get("full_output_requested")?,
            internal_error: row.get("internal_error")?,
        })
    }
}

/// One agent session. Mirrors the `sessions` table (spec §15.2).
#[derive(Debug, Clone)]
pub struct SessionRecord {
    pub id: String,
    pub created_at: String,
    pub ended_at: Option<String>,
    pub repo_root: String,
    pub agent_command: String,
    pub raw_tokens_total: i64,
    pub emitted_tokens_total: i64,
    pub saved_tokens_total: i64,
}
