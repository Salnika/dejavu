//! SQLite schema (spec §15). `overhead_ms` is added beyond the spec DDL to back
//! the average-overhead quality metric (§17.6 / §19.2).

pub const SCHEMA_VERSION: i64 = 1;

pub const SCHEMA_V1: &str = r#"
CREATE TABLE IF NOT EXISTS runs (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  repo_root TEXT NOT NULL,
  cwd TEXT NOT NULL,
  shim_name TEXT NOT NULL,
  argv_json TEXT NOT NULL,
  command_original TEXT NOT NULL,
  command_family TEXT NOT NULL,
  command_key TEXT NOT NULL,
  classification TEXT NOT NULL,
  exit_code INTEGER NOT NULL,
  duration_ms INTEGER NOT NULL,
  overhead_ms INTEGER NOT NULL DEFAULT 0,
  stdout_path TEXT,
  stderr_path TEXT,
  normalized_path TEXT,
  raw_stdout_bytes INTEGER NOT NULL,
  raw_stderr_bytes INTEGER NOT NULL,
  raw_total_bytes INTEGER NOT NULL,
  emitted_bytes INTEGER NOT NULL,
  estimated_raw_tokens INTEGER NOT NULL,
  estimated_emitted_tokens INTEGER NOT NULL,
  estimated_saved_tokens INTEGER NOT NULL,
  normalized_hash TEXT,
  stdout_hash TEXT,
  stderr_hash TEXT,
  git_head TEXT,
  git_worktree_hash TEXT,
  comparison_base_run_id TEXT,
  comparison_result TEXT NOT NULL,
  summary TEXT,
  full_output_requested INTEGER NOT NULL DEFAULT 0,
  internal_error TEXT
);

CREATE TABLE IF NOT EXISTS sessions (
  id TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  ended_at TEXT,
  repo_root TEXT NOT NULL,
  agent_command TEXT NOT NULL,
  raw_tokens_total INTEGER NOT NULL DEFAULT 0,
  emitted_tokens_total INTEGER NOT NULL DEFAULT 0,
  saved_tokens_total INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_runs_command_key ON runs(repo_root, command_key, created_at);
CREATE INDEX IF NOT EXISTS idx_runs_session ON runs(session_id, created_at);
CREATE INDEX IF NOT EXISTS idx_runs_family ON runs(command_family, created_at);
"#;
