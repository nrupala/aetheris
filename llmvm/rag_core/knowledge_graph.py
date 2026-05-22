"""
Personal Knowledge Graph — User context layer between LLM and user.

Stores:
- User profile, preferences, and interaction patterns
- Entities extracted from documents (concepts, tools, projects, etc.)
- Relationships between entities
- Conversation history for personalization
- Decision history with context

Every RAG query is enriched with personal graph context so the AI
responds as if it knows the user — because it does.
"""

import sqlite3
import json
import os
import time
from datetime import datetime
from typing import List, Optional, Dict, Tuple
from dataclasses import dataclass, field

from .config import RAGConfig, config


@dataclass
class Entity:
    id: Optional[int]
    name: str
    entity_type: str  # concept, tool, project, technology, etc.
    description: str = ""
    source: str = ""
    importance: float = 1.0
    created_at: str = ""
    metadata: dict = field(default_factory=dict)


@dataclass
class Relation:
    id: Optional[int]
    source_entity: str  # entity name
    target_entity: str  # entity name
    relation_type: str  # depends_on, uses, created_by, etc.
    weight: float = 1.0
    context: str = ""
    created_at: str = ""


@dataclass
class Interaction:
    query: str
    timestamp: str
    topics: List[str] = field(default_factory=list)
    files_accessed: List[str] = field(default_factory=list)
    result_summary: str = ""


class KnowledgeGraph:
    """SQLite-backed personal knowledge graph."""

    def __init__(self, db_path: Optional[str] = None, cfg: Optional[RAGConfig] = None):
        self.cfg = cfg or config
        self.db_path = db_path or self.cfg.graph_db_path
        os.makedirs(os.path.dirname(self.db_path) or ".", exist_ok=True)
        self._conn = sqlite3.connect(self.db_path)
        self._conn.execute("PRAGMA journal_mode=WAL")
        self._conn.execute("PRAGMA foreign_keys=ON")
        self._init_schema()

    def _init_schema(self):
        self._conn.executescript("""
            -- User profile: who you are, what you care about
            CREATE TABLE IF NOT EXISTS user_profile (
                key TEXT PRIMARY KEY,
                value TEXT,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            -- Interaction history: what you've asked, when, about what
            CREATE TABLE IF NOT EXISTS interactions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                query TEXT NOT NULL,
                timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                topics TEXT DEFAULT '[]',
                files_accessed TEXT DEFAULT '[]',
                result_summary TEXT DEFAULT ''
            );

            -- Entities: concepts, tools, projects, technologies, etc.
            CREATE TABLE IF NOT EXISTS entities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT UNIQUE NOT NULL,
                entity_type TEXT NOT NULL,
                description TEXT DEFAULT '',
                source TEXT DEFAULT '',
                importance REAL DEFAULT 1.0,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                metadata TEXT DEFAULT '{}'
            );

            -- Relations: how entities connect
            CREATE TABLE IF NOT EXISTS relations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_name TEXT NOT NULL REFERENCES entities(name) ON DELETE CASCADE,
                target_name TEXT NOT NULL REFERENCES entities(name) ON DELETE CASCADE,
                relation_type TEXT NOT NULL,
                weight REAL DEFAULT 1.0,
                context TEXT DEFAULT '',
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(source_name, target_name, relation_type)
            );

            -- Decision history: why you made choices
            CREATE TABLE IF NOT EXISTS decisions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                decision TEXT NOT NULL,
                reason TEXT,
                context TEXT,
                alternatives TEXT DEFAULT '[]',
                timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            -- Document context: what each document is about
            CREATE TABLE IF NOT EXISTS document_context (
                source TEXT PRIMARY KEY,
                summary TEXT,
                key_concepts TEXT DEFAULT '[]',
                related_entities TEXT DEFAULT '[]',
                ingested_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_entities_type ON entities(entity_type);
            CREATE INDEX IF NOT EXISTS idx_entities_importance ON entities(importance DESC);
            CREATE INDEX IF NOT EXISTS idx_relations_source ON relations(source_name);
            CREATE INDEX IF NOT EXISTS idx_relations_target ON relations(target_name);
            CREATE INDEX IF NOT EXISTS idx_interactions_timestamp ON interactions(timestamp);
        """)
        self._conn.commit()

    # --- User Profile ---

    def set_profile(self, key: str, value: str):
        """Set a user profile value."""
        self._conn.execute(
            "INSERT OR REPLACE INTO user_profile (key, value, updated_at) VALUES (?, ?, ?)",
            (key, value, datetime.utcnow().isoformat())
        )
        self._conn.commit()

    def get_profile(self, key: str) -> Optional[str]:
        val = self._conn.execute(
            "SELECT value FROM user_profile WHERE key = ?", (key,)
        ).fetchone()
        return val[0] if val else None

    def get_full_profile(self) -> Dict:
        rows = self._conn.execute("SELECT key, value FROM user_profile").fetchall()
        return {k: v for k, v in rows}

    # --- Interactions ---

    def record_interaction(self, query: str, topics: List[str] = None,
                           files_accessed: List[str] = None, result_summary: str = ""):
        """Record a user query for personalization."""
        self._conn.execute(
            "INSERT INTO interactions (query, topics, files_accessed, result_summary) VALUES (?, ?, ?, ?)",
            (query, json.dumps(topics or []), json.dumps(files_accessed or []), result_summary)
        )
        self._conn.commit()

        # Boost importance of entities mentioned in this query
        if topics:
            for topic in topics:
                self._conn.execute(
                    "UPDATE entities SET importance = importance + 0.1 WHERE name = ?",
                    (topic,)
                )
        self._conn.commit()

    def get_recent_interactions(self, limit: int = 20) -> List[Dict]:
        rows = self._conn.execute(
            "SELECT query, timestamp, topics, files_accessed, result_summary "
            "FROM interactions ORDER BY timestamp DESC LIMIT ?",
            (limit,)
        ).fetchall()
        return [
            {
                "query": r[0],
                "timestamp": r[1],
                "topics": json.loads(r[2]),
                "files_accessed": json.loads(r[3]),
                "result_summary": r[4]
            }
            for r in rows
        ]

    def get_query_patterns(self) -> Dict:
        """Analyze what the user asks about most."""
        rows = self._conn.execute(
            "SELECT topics FROM interactions WHERE topics != '[]'"
        ).fetchall()

        topic_counts: Dict[str, int] = {}
        for row in rows:
            for topic in json.loads(row[0]):
                topic_counts[topic] = topic_counts.get(topic, 0) + 1

        return {
            "total_queries": len(rows),
            "top_topics": sorted(topic_counts.items(), key=lambda x: x[1], reverse=True)[:10],
            "topic_distribution": topic_counts
        }

    # --- Entities ---

    def add_entity(self, name: str, entity_type: str, description: str = "",
                   source: str = "", importance: float = 1.0,
                   metadata: dict = None) -> int:
        """Add an entity to the graph."""
        cursor = self._conn.execute(
            """INSERT OR IGNORE INTO entities (name, entity_type, description, source, importance, metadata)
               VALUES (?, ?, ?, ?, ?, ?)""",
            (name, entity_type, description, source, importance,
             json.dumps(metadata or {}))
        )
        self._conn.commit()
        return cursor.lastrowid or 0

    def get_entity(self, name: str) -> Optional[Dict]:
        row = self._conn.execute(
            "SELECT name, entity_type, description, source, importance, created_at, metadata "
            "FROM entities WHERE name = ?", (name,)
        ).fetchone()
        if not row:
            return None
        return {
            "name": row[0],
            "type": row[1],
            "description": row[2],
            "source": row[3],
            "importance": row[4],
            "created_at": row[5],
            "metadata": json.loads(row[6])
        }

    def list_entities(self, entity_type: str = None, min_importance: float = 0.0,
                      limit: int = 100) -> List[Dict]:
        query = "SELECT name, entity_type, description, importance, source, created_at FROM entities WHERE 1=1"
        params = []
        if entity_type:
            query += " AND entity_type = ?"
            params.append(entity_type)
        if min_importance > 0:
            query += " AND importance >= ?"
            params.append(min_importance)
        query += " ORDER BY importance DESC LIMIT ?"
        params.append(limit)

        rows = self._conn.execute(query, params).fetchall()
        return [
            {
                "name": r[0], "type": r[1], "description": r[2],
                "importance": r[3], "source": r[4], "created_at": r[5]
            }
            for r in rows
        ]

    def delete_entity(self, name: str) -> int:
        """Delete an entity and its relations."""
        self._conn.execute("DELETE FROM relations WHERE source_name = ? OR target_name = ?", (name, name))
        cursor = self._conn.execute("DELETE FROM entities WHERE name = ?", (name,))
        self._conn.commit()
        return cursor.rowcount

    # --- Relations ---

    def add_relation(self, source: str, target: str, relation_type: str,
                     weight: float = 1.0, context: str = ""):
        """Connect two entities."""
        self._conn.execute(
            """INSERT OR REPLACE INTO relations (source_name, target_name, relation_type, weight, context)
               VALUES (?, ?, ?, ?, ?)""",
            (source, target, relation_type, weight, context)
        )
        self._conn.commit()

    def get_relations(self, entity_name: str = None) -> List[Dict]:
        query = "SELECT source_name, target_name, relation_type, weight, context FROM relations WHERE 1=1"
        params = []
        if entity_name:
            query += " AND (source_name = ? OR target_name = ?)"
            params.extend([entity_name, entity_name])
        query += " ORDER BY weight DESC"

        rows = self._conn.execute(query, params).fetchall()
        return [
            {
                "source": r[0], "target": r[1], "type": r[2],
                "weight": r[3], "context": r[4]
            }
            for r in rows
        ]

    def get_connections(self, entity_name: str, depth: int = 2) -> Dict:
        """Get all connected entities up to `depth` hops."""
        visited = set()
        frontier = [entity_name]
        connections = []

        for d in range(depth):
            next_frontier = []
            for name in frontier:
                if name in visited:
                    continue
                visited.add(name)

                rows = self._conn.execute(
                    """SELECT target_name, relation_type, weight, context
                       FROM relations WHERE source_name = ?""",
                    (name,)
                ).fetchall()

                for target, rtype, weight, ctx in rows:
                    connections.append({
                        "from": name, "to": target,
                        "relation": rtype, "weight": weight,
                        "context": ctx, "depth": d
                    })
                    if target not in visited:
                        next_frontier.append(target)

            frontier = next_frontier

        return {"center": entity_name, "connections": connections, "total_nodes": len(visited)}

    # --- Decisions ---

    def record_decision(self, decision: str, reason: str = "",
                        context: str = "", alternatives: List[str] = None):
        """Record a decision with context."""
        self._conn.execute(
            "INSERT INTO decisions (decision, reason, context, alternatives) VALUES (?, ?, ?, ?)",
            (decision, reason, context, json.dumps(alternatives or []))
        )
        self._conn.commit()

    def get_decisions(self, limit: int = 20) -> List[Dict]:
        rows = self._conn.execute(
            "SELECT decision, reason, context, alternatives, timestamp "
            "FROM decisions ORDER BY timestamp DESC LIMIT ?",
            (limit,)
        ).fetchall()
        return [
            {
                "decision": r[0], "reason": r[1], "context": r[2],
                "alternatives": json.loads(r[3]), "timestamp": r[4]
            }
            for r in rows
        ]

    # --- Document Context ---

    def set_document_context(self, source: str, summary: str = "",
                              key_concepts: List[str] = None,
                              related_entities: List[str] = None):
        self._conn.execute(
            """INSERT OR REPLACE INTO document_context
               (source, summary, key_concepts, related_entities)
               VALUES (?, ?, ?, ?)""",
            (source, summary,
             json.dumps(key_concepts or []),
             json.dumps(related_entities or []))
        )
        self._conn.commit()

    def get_document_context(self, source: str) -> Optional[Dict]:
        row = self._conn.execute(
            "SELECT source, summary, key_concepts, related_entities, ingested_at "
            "FROM document_context WHERE source = ?", (source,)
        ).fetchone()
        if not row:
            return None
        return {
            "source": row[0], "summary": row[1],
            "key_concepts": json.loads(row[2]),
            "related_entities": json.loads(row[3]),
            "ingested_at": row[4]
        }

    # --- Personal Context for RAG ---

    def get_personal_context(self, query: str) -> str:
        """
        Build a personal context block to inject into RAG system prompt.
        Includes: user profile, relevant past queries, related entities,
        recent decisions, and important concepts.
        """
        parts = []

        # User profile
        profile = self.get_full_profile()
        if profile:
            profile_str = "\n".join(f"- {k}: {v}" for k, v in profile.items())
            parts.append(f"## User Profile\n{profile_str}")

        # Related entities from query
        query_lower = query.lower()
        entities = self._conn.execute(
            "SELECT name, entity_type, importance FROM entities ORDER BY importance DESC LIMIT 50"
        ).fetchall()

        relevant = []
        for name, etype, imp in entities:
            if name.lower() in query_lower or any(w in name.lower() for w in query_lower.split() if len(w) > 3):
                relevant.append(f"- {name} ({etype}, importance: {imp:.1f})")

        if relevant:
            parts.append("## Relevant Concepts\n" + "\n".join(relevant))

        # Recent interactions on similar topics
        interactions = self.get_recent_interactions(limit=5)
        if interactions:
            recent = "\n".join(
                f"- Q: {i['query']}" for i in interactions
                if any(w in i["query"].lower() for w in query_lower.split() if len(w) > 3)
            )
            if recent:
                parts.append(f"## Your Recent Related Questions\n{recent}")

        # Recent decisions
        decisions = self.get_decisions(limit=3)
        if decisions:
            dec_str = "\n".join(f"- {d['decision']}: {d['reason']}" for d in decisions)
            parts.append(f"## Recent Decisions\n{dec_str}")

        if not parts:
            return ""

        return "\n\n".join(parts)

    # --- Import / Export ---

    def export_graph(self) -> Dict:
        """Export entire graph as JSON."""
        return {
            "exported_at": datetime.utcnow().isoformat(),
            "user_profile": self.get_full_profile(),
            "entities": self.list_entities(limit=10000),
            "relations": self.get_relations(),
            "decisions": self.get_decisions(limit=1000),
            "document_context": {
                row[0]: {
                    "summary": row[1],
                    "key_concepts": json.loads(row[2]),
                    "related_entities": json.loads(row[3])
                }
                for row in self._conn.execute(
                    "SELECT source, summary, key_concepts, related_entities FROM document_context"
                ).fetchall()
            },
            "interactions": self.get_recent_interactions(limit=10000),
        }

    def import_graph(self, data: Dict, merge: bool = True):
        """Import graph from JSON. If merge=False, clear existing first."""
        if not merge:
            self.reset()

        # Profile
        for key, value in data.get("user_profile", {}).items():
            self.set_profile(key, value)

        # Entities
        for e in data.get("entities", []):
            self.add_entity(
                e["name"], e["type"], e.get("description", ""),
                e.get("source", ""), e.get("importance", 1.0),
                e.get("metadata")
            )

        # Relations
        for r in data.get("relations", []):
            self.add_relation(r["source"], r["target"], r["type"],
                              r.get("weight", 1.0), r.get("context", ""))

        # Decisions
        for d in data.get("decisions", []):
            self.record_decision(d["decision"], d.get("reason", ""),
                                 d.get("context", ""), d.get("alternatives"))

        # Document context
        for source, ctx in data.get("document_context", {}).items():
            self.set_document_context(source, ctx.get("summary", ""),
                                       ctx.get("key_concepts", []),
                                       ctx.get("related_entities", []))

    # --- Reset ---

    def reset(self, keep_entities: bool = False, keep_interactions: bool = False):
        """Reset graph data."""
        if not keep_interactions:
            self._conn.execute("DELETE FROM interactions")
        if not keep_entities:
            self._conn.execute("DELETE FROM relations")
            self._conn.execute("DELETE FROM entities")
        self._conn.execute("DELETE FROM decisions")
        self._conn.execute("DELETE FROM document_context")
        if not keep_entities and not keep_interactions:
            self._conn.execute("DELETE FROM user_profile")
        self._conn.commit()

    # --- Stats ---

    def stats(self) -> Dict:
        entities = self._conn.execute("SELECT COUNT(*) FROM entities").fetchone()[0]
        relations = self._conn.execute("SELECT COUNT(*) FROM relations").fetchone()[0]
        interactions = self._conn.execute("SELECT COUNT(*) FROM interactions").fetchone()[0]
        decisions = self._conn.execute("SELECT COUNT(*) FROM decisions").fetchone()[0]
        docs = self._conn.execute("SELECT COUNT(*) FROM document_context").fetchone()[0]

        type_breakdown = {}
        for row in self._conn.execute(
            "SELECT entity_type, COUNT(*) FROM entities GROUP BY entity_type"
        ).fetchall():
            type_breakdown[row[0]] = row[1]

        return {
            "entities": entities,
            "relations": relations,
            "interactions": interactions,
            "decisions": decisions,
            "documents_with_context": docs,
            "entity_types": type_breakdown,
            "db_path": self.db_path,
            "db_size_mb": round(os.path.getsize(self.db_path) / (1024 * 1024), 2)
        }

    def close(self):
        self._conn.close()

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self.close()
