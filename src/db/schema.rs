use rusqlite::Connection;
use crate::error::BotError;

pub fn init_tables(conn: &Connection) -> Result<(), BotError> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA busy_timeout = 5000;

         CREATE TABLE IF NOT EXISTS settings (
             key TEXT PRIMARY KEY,
             value TEXT
         );

         CREATE TABLE IF NOT EXISTS topics (
             topic_id INTEGER PRIMARY KEY,
             session_id TEXT,
             auto_pr BOOLEAN DEFAULT 0,
             state TEXT DEFAULT 'open',
             pinned_message_id INTEGER
         );",
    )?;
    Ok(())
}
