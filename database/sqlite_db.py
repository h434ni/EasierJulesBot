import aiosqlite
from typing import Optional, Dict, Any, List
from .base import BaseDatabase

class SQLiteDatabase(BaseDatabase):
    def __init__(self, db_path: str = "bot.db"):
        self.db_path = db_path
        self.conn = None

    async def connect(self):
        self.conn = await aiosqlite.connect(self.db_path)
        self.conn.row_factory = aiosqlite.Row
        await self._init_db()

    async def disconnect(self):
        if self.conn:
            await self.conn.close()

    async def _init_db(self):
        await self.conn.execute("""
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT
            )
        """)
        await self.conn.execute("""
            CREATE TABLE IF NOT EXISTS topics (
                topic_id INTEGER PRIMARY KEY,
                session_id TEXT,
                auto_pr BOOLEAN DEFAULT 0,
                state TEXT DEFAULT 'open',
                pinned_message_id INTEGER
            )
        """)
        await self.conn.commit()

    async def get_setting(self, key: str) -> Optional[str]:
        cursor = await self.conn.execute("SELECT value FROM settings WHERE key = ?", (key,))
        row = await cursor.fetchone()
        return row['value'] if row else None

    async def set_setting(self, key: str, value: str):
        await self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            (key, value)
        )
        await self.conn.commit()

    async def create_topic(self, topic_id: int):
        await self.conn.execute(
            "INSERT INTO topics (topic_id) VALUES (?) ON CONFLICT(topic_id) DO NOTHING",
            (topic_id,)
        )
        await self.conn.commit()

    async def get_topic(self, topic_id: int) -> Optional[Dict[str, Any]]:
        cursor = await self.conn.execute("SELECT * FROM topics WHERE topic_id = ?", (topic_id,))
        row = await cursor.fetchone()
        return dict(row) if row else None

    async def get_topic_by_session(self, session_id: str) -> Optional[Dict[str, Any]]:
        cursor = await self.conn.execute("SELECT * FROM topics WHERE session_id = ?", (session_id,))
        row = await cursor.fetchone()
        return dict(row) if row else None

    async def update_topic_session(self, topic_id: int, session_id: str):
        await self.conn.execute("UPDATE topics SET session_id = ? WHERE topic_id = ?", (session_id, topic_id))
        await self.conn.commit()

    async def update_topic_auto_pr(self, topic_id: int, auto_pr: bool):
        await self.conn.execute("UPDATE topics SET auto_pr = ? WHERE topic_id = ?", (auto_pr, topic_id))
        await self.conn.commit()

    async def update_topic_state(self, topic_id: int, state: str):
        await self.conn.execute("UPDATE topics SET state = ? WHERE topic_id = ?", (state, topic_id))
        await self.conn.commit()

    async def update_topic_pinned_message(self, topic_id: int, message_id: int):
        await self.conn.execute("UPDATE topics SET pinned_message_id = ? WHERE topic_id = ?", (message_id, topic_id))
        await self.conn.commit()

    async def delete_topic(self, topic_id: int):
        await self.conn.execute("DELETE FROM topics WHERE topic_id = ?", (topic_id,))
        await self.conn.commit()

    async def get_all_active_topics(self) -> List[Dict[str, Any]]:
        cursor = await self.conn.execute("SELECT * FROM topics WHERE session_id IS NOT NULL AND state = 'open'")
        rows = await cursor.fetchall()
        return [dict(row) for row in rows]
