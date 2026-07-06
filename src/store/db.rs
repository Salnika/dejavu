//! SQLite connection wrapper: open, migrate, insert, and query.
//!
//! Concurrency: multiple `dejavu run` processes (e.g. a parallel agent or
//! `make -j`) open the same DB. WAL + a 5s busy timeout absorb contention; the
//! runtime treats any write failure as a reason to fall back to raw passthrough
//! rather than change the command result.

use super::models::{RunRecord, SessionRecord};
use super::schema;
use crate::error::StoreError;
use rusqlite::{named_params, params, Connection, OptionalExtension};
use std::path::Path;
use std::time::Duration;

pub struct Db {
    pub conn: Connection,
}

/// Aggregate stats over a repo's runs (spec §17.6).
#[derive(Debug, Clone, Default)]
pub struct StatsAgg {
    pub runs_captured: i64,
    pub optimized: i64,
    pub unchanged: i64,
    pub small_delta: i64,
    pub large_delta: i64,
    pub passthrough: i64,
    pub raw_tokens: i64,
    pub emitted_tokens: i64,
    pub saved_tokens: i64,
    pub full_output_requested: i64,
    pub internal_error: i64,
    pub avg_overhead_ms: f64,
}

impl Db {
    /// Read-only open for reporting over OTHER repos' databases (`stats --all`):
    /// no migration, no WAL side files, works on read-only caches, and never
    /// touches a DB that a live agent session is writing.
    pub fn open_read_only(path: &Path) -> Result<Db, StoreError> {
        let conn = Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        conn.busy_timeout(Duration::from_millis(5000))?;
        Ok(Db { conn })
    }

    pub fn open(path: &Path) -> Result<Db, StoreError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        // execute_batch tolerates the result row that `PRAGMA journal_mode`
        // returns; pragma_update would error on it.
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;\
             PRAGMA synchronous=NORMAL;\
             PRAGMA foreign_keys=ON;",
        )?;
        conn.busy_timeout(Duration::from_millis(5000))?;
        let db = Db { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<(), StoreError> {
        let version: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version < schema::SCHEMA_VERSION {
            self.conn.execute_batch(schema::SCHEMA_V1)?;
            self.conn
                .execute_batch(&format!("PRAGMA user_version={};", schema::SCHEMA_VERSION))?;
        }
        Ok(())
    }

    pub fn insert_run(&self, r: &RunRecord) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO runs (
                id, session_id, created_at, repo_root, cwd, shim_name, argv_json,
                command_original, command_family, command_key, classification,
                exit_code, duration_ms, overhead_ms, stdout_path, stderr_path,
                normalized_path, raw_stdout_bytes, raw_stderr_bytes, raw_total_bytes,
                emitted_bytes, estimated_raw_tokens, estimated_emitted_tokens,
                estimated_saved_tokens, normalized_hash, stdout_hash, stderr_hash,
                git_head, git_worktree_hash, comparison_base_run_id, comparison_result,
                summary, full_output_requested, internal_error
            ) VALUES (
                :id, :session_id, :created_at, :repo_root, :cwd, :shim_name, :argv_json,
                :command_original, :command_family, :command_key, :classification,
                :exit_code, :duration_ms, :overhead_ms, :stdout_path, :stderr_path,
                :normalized_path, :raw_stdout_bytes, :raw_stderr_bytes, :raw_total_bytes,
                :emitted_bytes, :estimated_raw_tokens, :estimated_emitted_tokens,
                :estimated_saved_tokens, :normalized_hash, :stdout_hash, :stderr_hash,
                :git_head, :git_worktree_hash, :comparison_base_run_id, :comparison_result,
                :summary, :full_output_requested, :internal_error
            )",
            named_params! {
                ":id": r.id,
                ":session_id": r.session_id,
                ":created_at": r.created_at,
                ":repo_root": r.repo_root,
                ":cwd": r.cwd,
                ":shim_name": r.shim_name,
                ":argv_json": r.argv_json,
                ":command_original": r.command_original,
                ":command_family": r.command_family,
                ":command_key": r.command_key,
                ":classification": r.classification,
                ":exit_code": r.exit_code,
                ":duration_ms": r.duration_ms,
                ":overhead_ms": r.overhead_ms,
                ":stdout_path": r.stdout_path,
                ":stderr_path": r.stderr_path,
                ":normalized_path": r.normalized_path,
                ":raw_stdout_bytes": r.raw_stdout_bytes,
                ":raw_stderr_bytes": r.raw_stderr_bytes,
                ":raw_total_bytes": r.raw_total_bytes,
                ":emitted_bytes": r.emitted_bytes,
                ":estimated_raw_tokens": r.estimated_raw_tokens,
                ":estimated_emitted_tokens": r.estimated_emitted_tokens,
                ":estimated_saved_tokens": r.estimated_saved_tokens,
                ":normalized_hash": r.normalized_hash,
                ":stdout_hash": r.stdout_hash,
                ":stderr_hash": r.stderr_hash,
                ":git_head": r.git_head,
                ":git_worktree_hash": r.git_worktree_hash,
                ":comparison_base_run_id": r.comparison_base_run_id,
                ":comparison_result": r.comparison_result,
                ":summary": r.summary,
                ":full_output_requested": r.full_output_requested,
                ":internal_error": r.internal_error,
            },
        )?;
        Ok(())
    }

    /// Resolve a run by exact id, then by unique short-id prefix (newest wins).
    pub fn get_run(&self, target: &str) -> Result<Option<RunRecord>, StoreError> {
        if let Some(run) = self
            .conn
            .query_row("SELECT * FROM runs WHERE id = ?1", [target], |row| {
                RunRecord::from_row(row)
            })
            .optional()?
        {
            return Ok(Some(run));
        }
        let pattern = format!("{target}%");
        let run = self
            .conn
            .query_row(
                "SELECT * FROM runs WHERE id LIKE ?1 ORDER BY created_at DESC, rowid DESC LIMIT 1",
                [pattern],
                RunRecord::from_row,
            )
            .optional()?;
        Ok(run)
    }

    /// How many runs share a short-id prefix (for ambiguity detection).
    pub fn count_prefix(&self, prefix: &str) -> Result<i64, StoreError> {
        let pattern = format!("{prefix}%");
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM runs WHERE id LIKE ?1",
            [pattern],
            |r| r.get(0),
        )?;
        Ok(count)
    }

    pub fn latest_run(&self, repo_root: &str) -> Result<Option<RunRecord>, StoreError> {
        let run = self
            .conn
            .query_row(
                "SELECT * FROM runs WHERE repo_root = ?1 ORDER BY created_at DESC, rowid DESC LIMIT 1",
                [repo_root],
                RunRecord::from_row,
            )
            .optional()?;
        Ok(run)
    }

    /// The most recent comparable prior run (spec §12, hybrid decision #3):
    /// matched on `(repo_root, cwd, command_family, command_key)`. Git state is
    /// NOT part of the match.
    pub fn find_comparable_prior(
        &self,
        repo_root: &str,
        cwd: &str,
        command_family: &str,
        command_key: &str,
        before_created_at: &str,
    ) -> Result<Option<RunRecord>, StoreError> {
        let run = self
            .conn
            .query_row(
                "SELECT * FROM runs
                 WHERE repo_root = :repo_root AND cwd = :cwd
                   AND command_family = :family AND command_key = :key
                   AND created_at < :before
                   AND classification != 'internal_error'
                 ORDER BY created_at DESC, rowid DESC LIMIT 1",
                named_params! {
                    ":repo_root": repo_root,
                    ":cwd": cwd,
                    ":family": command_family,
                    ":key": command_key,
                    ":before": before_created_at,
                },
                RunRecord::from_row,
            )
            .optional()?;
        Ok(run)
    }

    pub fn upsert_session_start(&self, s: &SessionRecord) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO sessions (id, created_at, ended_at, repo_root, agent_command,
                 raw_tokens_total, emitted_tokens_total, saved_tokens_total)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO NOTHING",
            params![
                s.id,
                s.created_at,
                s.ended_at,
                s.repo_root,
                s.agent_command,
                s.raw_tokens_total,
                s.emitted_tokens_total,
                s.saved_tokens_total,
            ],
        )?;
        Ok(())
    }

    pub fn accumulate_session_tokens(
        &self,
        session_id: &str,
        raw: i64,
        emitted: i64,
        saved: i64,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE sessions SET
                raw_tokens_total = raw_tokens_total + ?2,
                emitted_tokens_total = emitted_tokens_total + ?3,
                saved_tokens_total = saved_tokens_total + ?4
             WHERE id = ?1",
            params![session_id, raw, emitted, saved],
        )?;
        Ok(())
    }

    pub fn mark_full_output_requested(&self, run_id: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE runs SET full_output_requested = 1
             WHERE id = ?1 AND full_output_requested = 0",
            [run_id],
        )?;
        Ok(())
    }

    /// Runs older than `cutoff` (RFC3339), for log-file cleanup.
    pub fn runs_before(&self, repo_root: &str, cutoff: &str) -> Result<Vec<RunRecord>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM runs WHERE repo_root = ?1 AND created_at < ?2 ORDER BY created_at",
        )?;
        let rows = stmt.query_map(params![repo_root, cutoff], RunRecord::from_row)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Delete run rows older than `cutoff`. Returns the number deleted.
    pub fn delete_runs_before(&self, repo_root: &str, cutoff: &str) -> Result<usize, StoreError> {
        let n = self.conn.execute(
            "DELETE FROM runs WHERE repo_root = ?1 AND created_at < ?2",
            params![repo_root, cutoff],
        )?;
        Ok(n)
    }

    /// Aggregate run stats; `repo_root: None` aggregates the whole database.
    pub fn aggregate_stats(&self, repo_root: Option<&str>) -> Result<StatsAgg, StoreError> {
        let agg = self.conn.query_row(
            "SELECT
                COUNT(*),
                COALESCE(SUM(CASE WHEN classification IN
                    ('first_seen','unchanged','small_delta','large_delta') THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN classification='unchanged' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN classification='small_delta' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN classification='large_delta' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN classification='passthrough' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(estimated_raw_tokens), 0),
                COALESCE(SUM(estimated_emitted_tokens), 0),
                COALESCE(SUM(estimated_saved_tokens), 0),
                COALESCE(SUM(full_output_requested), 0),
                COALESCE(SUM(CASE WHEN classification='internal_error' THEN 1 ELSE 0 END), 0),
                COALESCE(AVG(overhead_ms), 0.0)
             FROM runs WHERE (?1 IS NULL OR repo_root = ?1)",
            params![repo_root],
            |row| {
                Ok(StatsAgg {
                    runs_captured: row.get(0)?,
                    optimized: row.get(1)?,
                    unchanged: row.get(2)?,
                    small_delta: row.get(3)?,
                    large_delta: row.get(4)?,
                    passthrough: row.get(5)?,
                    raw_tokens: row.get(6)?,
                    emitted_tokens: row.get(7)?,
                    saved_tokens: row.get(8)?,
                    full_output_requested: row.get(9)?,
                    internal_error: row.get(10)?,
                    avg_overhead_ms: row.get(11)?,
                })
            },
        )?;
        Ok(agg)
    }

    /// Top-N `(display, saved_tokens)` grouped by command, most savings first.
    /// Top saving commands; `limit: None` returns them all (SQLite `LIMIT -1`).
    pub fn top_savings(
        &self,
        repo_root: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<(String, i64)>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT command_original, SUM(estimated_saved_tokens) AS saved
             FROM runs
             WHERE (?1 IS NULL OR repo_root = ?1) AND estimated_saved_tokens > 0
             GROUP BY command_key
             ORDER BY saved DESC LIMIT ?2",
        )?;
        let limit = limit.map(|l| l as i64).unwrap_or(-1);
        let rows = stmt.query_map(params![repo_root, limit], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Distinct repo roots recorded in this database (normally one per cache).
    pub fn repo_roots(&self) -> Result<Vec<String>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT repo_root FROM runs ORDER BY repo_root")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}
