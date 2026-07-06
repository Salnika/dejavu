//! Session lifecycle helpers.

use super::Db;
use crate::error::StoreError;
use rusqlite::params;

impl Db {
    /// Mark a session ended (idempotent — only sets `ended_at` once).
    pub fn end_session(&self, session_id: &str, ended_at: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE sessions SET ended_at = ?2 WHERE id = ?1 AND ended_at IS NULL",
            params![session_id, ended_at],
        )?;
        Ok(())
    }
}
