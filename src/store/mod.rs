//! Local SQLite storage: schema, row models, and the connection facade.

pub mod db;
pub mod logs;
pub mod models;
pub mod schema;
pub mod session;

pub use db::{Db, StatsAgg};
pub use logs::{write_logs, StoredLogs};
pub use models::{RunRecord, SessionRecord};
