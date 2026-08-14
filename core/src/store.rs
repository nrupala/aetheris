//! # Aetheris Persistence Spine (Track 2 - Persistence Foundation)
//!
//! A single SQLite store with idempotent migrations (`_migrations` ledger),
//! WAL journal mode, foreign keys, and a busy timeout - solving the top pain
//! point: persistence everywhere (tasks, conversations, messages, a
//! namespaced long-term memory, a versioned skill registry, and watcher bans).
//!
//! Design reuses the verified harness patterns (CR-0005 SQLite spine + CR-0006
//! idempotent migrations) adapted to Aetheris. No new dependencies (`rusqlite`
//! is already a bundled dependency used by the vector store).
//!
//! Integration seam: this module is complete and unit-tested in isolation.
//! Wiring into AppState + request handlers is the follow-up (done in session
//! with box deploy + verification), so this module carries `allow(dead_code)`
//! until that wiring lands.
#![allow(dead_code)]

use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::{Arc, Mutex};

pub type StoreResult<T> = Result<T, String>;

const MIGRATIONS: &[(&str, &str)] = &[
    (
        "0001_tasks_conversations",
        r#"
        CREATE TABLE IF NOT EXISTS tasks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            request_id TEXT,
            agent_id TEXT NOT NULL DEFAULT '',
            role TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'pending',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            completed_at TEXT,
            output TEXT,
            error TEXT
        );
        CREATE TABLE IF NOT EXISTS conversations (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL DEFAULT '',
            user_email TEXT NOT NULL DEFAULT 'unknown',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id TEXT NOT NULL REFERENCES conversations(id),
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
    "#,
    ),
    (
        "0002_memory_skills_watcher",
        r#"
        CREATE TABLE IF NOT EXISTS memory (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            namespace TEXT NOT NULL,
            entity_key TEXT NOT NULL,
            content TEXT NOT NULL,
            kind TEXT NOT NULL DEFAULT 'note',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(namespace, entity_key)
        );
        CREATE TABLE IF NOT EXISTS skills (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            version TEXT NOT NULL DEFAULT '1.0.0',
            description TEXT NOT NULL DEFAULT '',
            definition_json TEXT NOT NULL DEFAULT '{}',
            source TEXT NOT NULL DEFAULT 'local',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS watcher_bans (
            peer_id TEXT PRIMARY KEY,
            failures INTEGER NOT NULL DEFAULT 0,
            last_seen_ms INTEGER NOT NULL DEFAULT 0,
            banned_until_ms INTEGER NOT NULL DEFAULT 0
        );
    "#,
    ),
];

#[derive(Debug, Clone)]
pub struct TaskRow {
    pub id: i64,
    pub request_id: Option<String>,
    pub agent_id: String,
    pub role: String,
    pub status: String,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub output: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConversationRow {
    pub id: String,
    pub title: String,
    pub user_email: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct MessageRow {
    pub id: i64,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct SkillRow {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub definition_json: String,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct BanRow {
    pub peer_id: String,
    pub failures: i64,
    pub last_seen_ms: i64,
    pub banned_until_ms: i64,
}

#[derive(Clone)]
pub struct Store {
    conn: Arc<Mutex<Connection>>,
}

#[allow(dead_code)]
impl Store {
    pub fn open(path: &Path) -> StoreResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut conn = Connection::open(path).map_err(|e| e.to_string())?;
        Self::configure(&conn)?;
        Self::migrate(&mut conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn in_memory() -> StoreResult<Self> {
        let mut conn = Connection::open_in_memory().map_err(|e| e.to_string())?;
        Self::configure(&conn)?;
        Self::migrate(&mut conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn configure(conn: &Connection) -> StoreResult<()> {
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|e| e.to_string())?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(|e| e.to_string())?;
        conn.pragma_update(None, "busy_timeout", 5000)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn migrate(conn: &mut Connection) -> StoreResult<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS _migrations (
                name TEXT PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .map_err(|e| e.to_string())?;
        let applied: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT name FROM _migrations")
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |r| r.get::<_, String>(0))
                .map_err(|e| e.to_string())?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?
        };
        for (name, sql) in MIGRATIONS {
            if applied.iter().any(|n| n == name) {
                continue;
            }
            let tx = conn.transaction().map_err(|e| e.to_string())?;
            tx.execute_batch(sql).map_err(|e| e.to_string())?;
            tx.execute("INSERT INTO _migrations (name) VALUES (?1)", [*name])
                .map_err(|e| e.to_string())?;
            tx.commit().map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn create_task(
        &self,
        request_id: Option<&str>,
        agent_id: &str,
        role: &str,
        status: &str,
    ) -> StoreResult<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO tasks (request_id, agent_id, role, status) VALUES (?1, ?2, ?3, ?4)",
            params![request_id, agent_id, role, status],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }

    pub fn update_task(
        &self,
        id: i64,
        status: &str,
        output: Option<&str>,
        error: Option<&str>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE tasks SET status = ?1, output = ?2, error = ?3,
                    completed_at = CASE WHEN ?4 = 1 THEN datetime('now') ELSE completed_at END
             WHERE id = ?5",
            params![
                status,
                output,
                error,
                (status == "completed" || status == "failed") as i64,
                id
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_task(&self, id: i64) -> StoreResult<Option<TaskRow>> {
        let conn = self.conn.lock().unwrap();
        let row = conn.query_row(
            "SELECT id, request_id, agent_id, role, status, created_at, completed_at, output, error
             FROM tasks WHERE id = ?1",
            [id],
            |r| Ok(TaskRow {
                id: r.get(0)?, request_id: r.get(1)?, agent_id: r.get(2)?,
                role: r.get(3)?, status: r.get(4)?, created_at: r.get(5)?,
                completed_at: r.get(6)?, output: r.get(7)?, error: r.get(8)?,
            }),
        ).optional().map_err(|e| e.to_string())?;
        Ok(row)
    }

    pub fn list_tasks(&self, limit: i64) -> StoreResult<Vec<TaskRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, request_id, agent_id, role, status, created_at, completed_at, output, error
             FROM tasks ORDER BY id DESC LIMIT ?1",
        ).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([limit], |r| {
                Ok(TaskRow {
                    id: r.get(0)?,
                    request_id: r.get(1)?,
                    agent_id: r.get(2)?,
                    role: r.get(3)?,
                    status: r.get(4)?,
                    created_at: r.get(5)?,
                    completed_at: r.get(6)?,
                    output: r.get(7)?,
                    error: r.get(8)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    pub fn create_conversation(&self, id: &str, title: &str, user_email: &str) -> StoreResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO conversations (id, title, user_email) VALUES (?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET title = excluded.title, updated_at = datetime('now')",
            params![id, title, user_email],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_conversation(&self, id: &str) -> StoreResult<Option<ConversationRow>> {
        let conn = self.conn.lock().unwrap();
        let row = conn.query_row(
            "SELECT id, title, user_email, created_at, updated_at FROM conversations WHERE id = ?1",
            [id],
            |r| Ok(ConversationRow {
                id: r.get(0)?, title: r.get(1)?, user_email: r.get(2)?,
                created_at: r.get(3)?, updated_at: r.get(4)?,
            }),
        ).optional().map_err(|e| e.to_string())?;
        Ok(row)
    }

    pub fn list_conversations(&self, limit: i64) -> StoreResult<Vec<ConversationRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, user_email, created_at, updated_at FROM conversations ORDER BY updated_at DESC LIMIT ?1",
        ).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([limit], |r| {
                Ok(ConversationRow {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    user_email: r.get(2)?,
                    created_at: r.get(3)?,
                    updated_at: r.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    pub fn append_message(
        &self,
        conversation_id: &str,
        role: &str,
        content: &str,
    ) -> StoreResult<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO messages (conversation_id, role, content) VALUES (?1, ?2, ?3)",
            params![conversation_id, role, content],
        )
        .map_err(|e| e.to_string())?;
        let id = conn.last_insert_rowid();
        conn.execute(
            "UPDATE conversations SET updated_at = datetime('now') WHERE id = ?1",
            [conversation_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(id)
    }

    pub fn list_messages(&self, conversation_id: &str, limit: i64) -> StoreResult<Vec<MessageRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, role, content, created_at FROM messages WHERE conversation_id = ?1 ORDER BY id ASC LIMIT ?2",
        ).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![conversation_id, limit], |r| {
                Ok(MessageRow {
                    id: r.get(0)?,
                    role: r.get(1)?,
                    content: r.get(2)?,
                    created_at: r.get(3)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    pub fn set_memory(
        &self,
        namespace: &str,
        key: &str,
        content: &str,
        kind: &str,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO memory (namespace, entity_key, content, kind) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(namespace, entity_key) DO UPDATE SET content = excluded.content,
                 kind = excluded.kind, updated_at = datetime('now')",
            params![namespace, key, content, kind],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_memory(&self, namespace: &str, key: &str) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let v = conn
            .query_row(
                "SELECT content FROM memory WHERE namespace = ?1 AND entity_key = ?2",
                params![namespace, key],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        Ok(v)
    }

    pub fn list_memory(&self, namespace: &str) -> StoreResult<Vec<(String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT entity_key, content FROM memory WHERE namespace = ?1 ORDER BY updated_at DESC",
        ).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([namespace], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    pub fn delete_memory(&self, namespace: &str, key: &str) -> StoreResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM memory WHERE namespace = ?1 AND entity_key = ?2",
            params![namespace, key],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn put_skill(
        &self,
        id: &str,
        name: &str,
        version: &str,
        description: &str,
        definition_json: &str,
        source: &str,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO skills (id, name, version, description, definition_json, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, version = excluded.version,
                 description = excluded.description, definition_json = excluded.definition_json,
                 source = excluded.source, updated_at = datetime('now')",
            params![id, name, version, description, definition_json, source],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_skill(&self, id: &str) -> StoreResult<Option<SkillRow>> {
        let conn = self.conn.lock().unwrap();
        let row = conn.query_row(
            "SELECT id, name, version, description, definition_json, source FROM skills WHERE id = ?1",
            [id],
            |r| Ok(SkillRow {
                id: r.get(0)?, name: r.get(1)?, version: r.get(2)?,
                description: r.get(3)?, definition_json: r.get(4)?, source: r.get(5)?,
            }),
        ).optional().map_err(|e| e.to_string())?;
        Ok(row)
    }

    pub fn list_skills(&self) -> StoreResult<Vec<SkillRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, version, description, definition_json, source FROM skills ORDER BY updated_at DESC",
        ).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok(SkillRow {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    version: r.get(2)?,
                    description: r.get(3)?,
                    definition_json: r.get(4)?,
                    source: r.get(5)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    pub fn upsert_peer(
        &self,
        peer_id: &str,
        failures: i64,
        last_seen_ms: i64,
        banned_until_ms: i64,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO watcher_bans (peer_id, failures, last_seen_ms, banned_until_ms)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(peer_id) DO UPDATE SET failures = excluded.failures,
                 last_seen_ms = excluded.last_seen_ms, banned_until_ms = excluded.banned_until_ms",
            params![peer_id, failures, last_seen_ms, banned_until_ms],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_peer(&self, peer_id: &str) -> StoreResult<Option<BanRow>> {
        let conn = self.conn.lock().unwrap();
        let row = conn.query_row(
            "SELECT peer_id, failures, last_seen_ms, banned_until_ms FROM watcher_bans WHERE peer_id = ?1",
            [peer_id],
            |r| Ok(BanRow {
                peer_id: r.get(0)?, failures: r.get(1)?, last_seen_ms: r.get(2)?, banned_until_ms: r.get(3)?,
            }),
        ).optional().map_err(|e| e.to_string())?;
        Ok(row)
    }

    pub fn list_banned(&self, now_ms: i64) -> StoreResult<Vec<BanRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT peer_id, failures, last_seen_ms, banned_until_ms FROM watcher_bans WHERE banned_until_ms > ?1",
        ).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([now_ms], |r| {
                Ok(BanRow {
                    peer_id: r.get(0)?,
                    failures: r.get(1)?,
                    last_seen_ms: r.get(2)?,
                    banned_until_ms: r.get(3)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_apply_once_and_tables_exist() {
        let s = Store::in_memory().unwrap();
        let conn = s.conn.lock().unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM _migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, MIGRATIONS.len() as i64);
    }

    #[test]
    fn task_lifecycle() {
        let s = Store::in_memory().unwrap();
        let id = s
            .create_task(Some("req-1"), "planner-a", "planner", "pending")
            .unwrap();
        assert!(id > 0);
        s.update_task(id, "completed", Some("done"), None).unwrap();
        let t = s.get_task(id).unwrap().unwrap();
        assert_eq!(t.status, "completed");
        assert_eq!(t.output.as_deref(), Some("done"));
        assert!(t.completed_at.is_some());
        assert_eq!(s.list_tasks(10).unwrap().len(), 1);
    }

    #[test]
    fn conversation_with_messages_persists() {
        let s = Store::in_memory().unwrap();
        s.create_conversation("conv-1", "RAG", "nrupalakolkar@gmail.com")
            .unwrap();
        s.append_message("conv-1", "user", "hello").unwrap();
        s.append_message("conv-1", "assistant", "hi").unwrap();
        let msgs = s.list_messages("conv-1", 10).unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, "user");
        assert_eq!(msgs[1].content, "hi");
        let convs = s.list_conversations(10).unwrap();
        assert_eq!(convs.len(), 1);
        assert_eq!(convs[0].user_email, "nrupalakolkar@gmail.com");
    }

    #[test]
    fn namespaced_memory_upsert_delete() {
        let s = Store::in_memory().unwrap();
        s.set_memory("user", "name", "Milo", "note").unwrap();
        s.set_memory("user", "name", "Milo N.", "note").unwrap();
        assert_eq!(
            s.get_memory("user", "name").unwrap().as_deref(),
            Some("Milo N.")
        );
        assert_eq!(s.list_memory("user").unwrap().len(), 1);
        s.set_memory("user", "city", "Toronto", "note").unwrap();
        assert_eq!(s.list_memory("user").unwrap().len(), 2);
        s.delete_memory("user", "city").unwrap();
        assert!(s.get_memory("user", "city").unwrap().is_none());
        assert!(s.get_memory("project-a", "name").unwrap().is_none());
    }

    #[test]
    fn skill_registry_reuse() {
        let s = Store::in_memory().unwrap();
        let def = r#"{"prompt":"x","tools":["bash"]}"#;
        s.put_skill(
            "sk-1",
            "rag-query",
            "1.0.0",
            "RAG query skill",
            def,
            "local",
        )
        .unwrap();
        s.put_skill(
            "sk-1",
            "rag-query",
            "1.1.0",
            "RAG query skill v2",
            def,
            "local",
        )
        .unwrap();
        let sk = s.get_skill("sk-1").unwrap().unwrap();
        assert_eq!(sk.version, "1.1.0");
        assert_eq!(s.list_skills().unwrap().len(), 1);
    }

    #[test]
    fn watcher_bans_persist_and_query_active() {
        let s = Store::in_memory().unwrap();
        s.upsert_peer("peer-1", 5, 1000, 200_000).unwrap();
        s.upsert_peer("peer-2", 5, 1000, 0).unwrap();
        assert_eq!(s.list_banned(1_500).unwrap().len(), 1);
        assert!(s.get_peer("peer-1").unwrap().is_some());
    }
}
