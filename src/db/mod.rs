pub mod schema;

use std::path::Path;
use std::sync::{Arc, Mutex};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use crate::error::BotError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopicRecord {
    pub topic_id: i32,
    pub session_id: Option<String>,
    pub auto_pr: bool,
    pub state: String,
    pub pinned_message_id: Option<i32>,
}

#[derive(Clone, Debug)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self, BotError> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let conn = Connection::open(path)?;
        schema::init_tables(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, BotError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        let res = stmt
            .query_row(params![key], |row| row.get(0))
            .optional()?;
        Ok(res)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), BotError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn create_topic(&self, topic_id: i32) -> Result<(), BotError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO topics (topic_id) VALUES (?1) ON CONFLICT(topic_id) DO NOTHING",
            params![topic_id],
        )?;
        Ok(())
    }

    pub fn get_topic(&self, topic_id: i32) -> Result<Option<TopicRecord>, BotError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT topic_id, session_id, auto_pr, state, pinned_message_id FROM topics WHERE topic_id = ?1",
        )?;
        let res = stmt
            .query_row(params![topic_id], |row| {
                let auto_pr_int: i64 = row.get(2)?;
                Ok(TopicRecord {
                    topic_id: row.get(0)?,
                    session_id: row.get(1)?,
                    auto_pr: auto_pr_int != 0,
                    state: row.get(3)?,
                    pinned_message_id: row.get(4)?,
                })
            })
            .optional()?;
        Ok(res)
    }

    pub fn get_topic_by_session(&self, session_id: &str) -> Result<Option<TopicRecord>, BotError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT topic_id, session_id, auto_pr, state, pinned_message_id FROM topics WHERE session_id = ?1",
        )?;
        let res = stmt
            .query_row(params![session_id], |row| {
                let auto_pr_int: i64 = row.get(2)?;
                Ok(TopicRecord {
                    topic_id: row.get(0)?,
                    session_id: row.get(1)?,
                    auto_pr: auto_pr_int != 0,
                    state: row.get(3)?,
                    pinned_message_id: row.get(4)?,
                })
            })
            .optional()?;
        Ok(res)
    }

    pub fn update_topic_session(&self, topic_id: i32, session_id: &str) -> Result<(), BotError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE topics SET session_id = ?1 WHERE topic_id = ?2",
            params![session_id, topic_id],
        )?;
        Ok(())
    }

    pub fn update_topic_auto_pr(&self, topic_id: i32, auto_pr: bool) -> Result<(), BotError> {
        let conn = self.conn.lock().unwrap();
        let val = if auto_pr { 1 } else { 0 };
        conn.execute(
            "UPDATE topics SET auto_pr = ?1 WHERE topic_id = ?2",
            params![val, topic_id],
        )?;
        Ok(())
    }

    pub fn update_topic_state(&self, topic_id: i32, state: &str) -> Result<(), BotError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE topics SET state = ?1 WHERE topic_id = ?2",
            params![state, topic_id],
        )?;
        Ok(())
    }

    pub fn update_topic_pinned_message(
        &self,
        topic_id: i32,
        message_id: i32,
    ) -> Result<(), BotError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE topics SET pinned_message_id = ?1 WHERE topic_id = ?2",
            params![message_id, topic_id],
        )?;
        Ok(())
    }

    pub fn delete_topic(&self, topic_id: i32) -> Result<(), BotError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM topics WHERE topic_id = ?1", params![topic_id])?;
        Ok(())
    }

    pub fn get_all_active_topics(&self) -> Result<Vec<TopicRecord>, BotError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT topic_id, session_id, auto_pr, state, pinned_message_id FROM topics WHERE session_id IS NOT NULL AND state = 'open'",
        )?;
        let rows = stmt.query_map([], |row| {
            let auto_pr_int: i64 = row.get(2)?;
            Ok(TopicRecord {
                topic_id: row.get(0)?,
                session_id: row.get(1)?,
                auto_pr: auto_pr_int != 0,
                state: row.get(3)?,
                pinned_message_id: row.get(4)?,
            })
        })?;

        let mut topics = Vec::new();
        for r in rows {
            topics.push(r?);
        }
        Ok(topics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_settings_and_topics() {
        let conn = Connection::open_in_memory().unwrap();
        schema::init_tables(&conn).unwrap();
        let db = Db {
            conn: Arc::new(Mutex::new(conn)),
        };

        // Settings test
        assert_eq!(db.get_setting("owner_id").unwrap(), None);
        db.set_setting("owner_id", "12345").unwrap();
        assert_eq!(db.get_setting("owner_id").unwrap(), Some("12345".to_string()));

        // Upsert test
        db.set_setting("owner_id", "67890").unwrap();
        assert_eq!(db.get_setting("owner_id").unwrap(), Some("67890".to_string()));

        // Topics test
        db.create_topic(101).unwrap();
        let topic = db.get_topic(101).unwrap().expect("topic should exist");
        assert_eq!(topic.topic_id, 101);
        assert_eq!(topic.state, "open");
        assert!(!topic.auto_pr);
        assert_eq!(topic.session_id, None);

        // Update session & auto_pr
        db.update_topic_session(101, "sessions/sess-1").unwrap();
        db.update_topic_auto_pr(101, true).unwrap();
        db.update_topic_pinned_message(101, 999).unwrap();

        let updated = db.get_topic(101).unwrap().unwrap();
        assert_eq!(updated.session_id.as_deref(), Some("sessions/sess-1"));
        assert!(updated.auto_pr);
        assert_eq!(updated.pinned_message_id, Some(999));

        // Get by session
        let by_sess = db.get_topic_by_session("sessions/sess-1").unwrap().unwrap();
        assert_eq!(by_sess.topic_id, 101);

        // Active topics query
        let active = db.get_all_active_topics().unwrap();
        assert_eq!(active.len(), 1);

        // Close topic
        db.update_topic_state(101, "closed").unwrap();
        let active_after_close = db.get_all_active_topics().unwrap();
        assert_eq!(active_after_close.len(), 0);

        // Delete topic
        db.delete_topic(101).unwrap();
        assert_eq!(db.get_topic(101).unwrap(), None);
    }
}
