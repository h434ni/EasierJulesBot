import Database from 'better-sqlite3';
import path from 'path';

const dbPath = path.resolve(process.cwd(), 'bot.db');
export const db = new Database(dbPath);

db.exec(`
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
  );
`);

export const DB = {
  getSetting(key: string): string | null {
    const row = db.prepare('SELECT value FROM settings WHERE key = ?').get(key) as { value: string } | undefined;
    return row ? row.value : null;
  },

  setSetting(key: string, value: string) {
    db.prepare(`
      INSERT INTO settings (key, value) 
      VALUES (?, ?) 
      ON CONFLICT(key) DO UPDATE SET value = excluded.value
    `).run(key, value);
  },

  createTopic(topicId: number) {
    db.prepare('INSERT INTO topics (topic_id) VALUES (?) ON CONFLICT(topic_id) DO NOTHING').run(topicId);
  },

  getTopic(topicId: number): any {
    return db.prepare('SELECT * FROM topics WHERE topic_id = ?').get(topicId);
  },

  getTopicBySession(sessionId: string): any {
    return db.prepare('SELECT * FROM topics WHERE session_id = ?').get(sessionId);
  },

  updateTopicSession(topicId: number, sessionId: string) {
    db.prepare('UPDATE topics SET session_id = ? WHERE topic_id = ?').run(sessionId, topicId);
  },

  updateTopicAutoPr(topicId: number, autoPr: boolean) {
    db.prepare('UPDATE topics SET auto_pr = ? WHERE topic_id = ?').run(autoPr ? 1 : 0, topicId);
  },

  updateTopicState(topicId: number, state: string) {
    db.prepare('UPDATE topics SET state = ? WHERE topic_id = ?').run(state, topicId);
  },

  updateTopicPinnedMessage(topicId: number, messageId: number) {
    db.prepare('UPDATE topics SET pinned_message_id = ? WHERE topic_id = ?').run(messageId, topicId);
  },

  deleteTopic(topicId: number) {
    db.prepare('DELETE FROM topics WHERE topic_id = ?').run(topicId);
  },

  getAllActiveTopics(): any[] {
    return db.prepare("SELECT * FROM topics WHERE session_id IS NOT NULL AND state = 'open'").all();
  }
};
