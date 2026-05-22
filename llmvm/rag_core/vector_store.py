"""
Vector Store — SQLite + NumPy.
No Milvus. No Pinecone. Just a database and vector math.

Storage format:
- chunks table: text, source, metadata
- embeddings table: packed float32 binary blobs (efficient)

For <100K chunks, brute-force cosine similarity is fast enough (~50ms).
"""

import sqlite3
import struct
import json
import os
from datetime import datetime
from typing import List, Optional, Tuple
from dataclasses import dataclass
import numpy as np

from .config import RAGConfig, config
from .chunker import Chunk


@dataclass
class SearchResult:
    chunk_id: int
    text: str
    source: str
    score: float
    metadata: dict
    chunk_index: int


class VectorStore:
    """SQLite-backed vector store with NumPy cosine similarity."""

    def __init__(self, db_path: Optional[str] = None, cfg: Optional[RAGConfig] = None):
        self.cfg = cfg or config
        self.db_path = db_path or self.cfg.db_path

        # Ensure directory exists
        os.makedirs(os.path.dirname(self.db_path) or ".", exist_ok=True)

        self._conn = sqlite3.connect(self.db_path)
        self._conn.execute("PRAGMA journal_mode=WAL")  # Better concurrent performance
        self._conn.execute("PRAGMA synchronous=NORMAL")
        self._init_schema()

    def _init_schema(self):
        """Create tables if they don't exist."""
        self._conn.executescript("""
            CREATE TABLE IF NOT EXISTS chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                text TEXT NOT NULL,
                source TEXT NOT NULL,
                chunk_index INTEGER DEFAULT 0,
                token_count INTEGER DEFAULT 0,
                metadata TEXT DEFAULT '{}',
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS embeddings (
                chunk_id INTEGER PRIMARY KEY REFERENCES chunks(id) ON DELETE CASCADE,
                vector BLOB NOT NULL,
                dimension INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_chunks_source ON chunks(source);
            CREATE INDEX IF NOT EXISTS idx_chunks_created ON chunks(created_at);
            CREATE INDEX IF NOT EXISTS idx_chunks_source_index ON chunks(source, chunk_index);
        """)
        self._conn.commit()

    def add(self, chunks: List[Chunk], embeddings: List[np.ndarray]) -> List[int]:
        """
        Store chunks and their embeddings.
        Returns list of chunk IDs.
        """
        if len(chunks) != len(embeddings):
            raise ValueError(f"Mismatch: {len(chunks)} chunks vs {len(embeddings)} embeddings")

        ids = []
        cursor = self._conn.cursor()

        try:
            for chunk, embedding in zip(chunks, embeddings):
                # Normalize embedding for cosine similarity
                norm = np.linalg.norm(embedding)
                if norm > 0:
                    embedding = embedding / norm

                # Pack as float32 bytes
                vector_bytes = embedding.astype(np.float32).tobytes()
                dimension = len(embedding)

                cursor.execute(
                    "INSERT INTO chunks (text, source, chunk_index, token_count, metadata) VALUES (?, ?, ?, ?, ?)",
                    (chunk.text, chunk.source, chunk.index, chunk.token_count, json.dumps(chunk.metadata))
                )
                chunk_id = cursor.lastrowid

                cursor.execute(
                    "INSERT INTO embeddings (chunk_id, vector, dimension) VALUES (?, ?, ?)",
                    (chunk_id, vector_bytes, dimension)
                )
                ids.append(chunk_id)

            self._conn.commit()
        except Exception:
            self._conn.rollback()
            raise

        return ids

    def search(self, query_embedding: np.ndarray, top_k: int = 5, threshold: float = 0.0) -> List[SearchResult]:
        """
        Cosine similarity search.
        Since embeddings are pre-normalized, dot product = cosine similarity.
        """
        # Normalize query
        norm = np.linalg.norm(query_embedding)
        if norm > 0:
            query_embedding = query_embedding / norm

        query_vec = query_embedding.astype(np.float32).tobytes()

        # Fetch all embeddings (for <100K this is fast)
        cursor = self._conn.execute("""
            SELECT e.chunk_id, e.vector, e.dimension,
                   c.text, c.source, c.chunk_index, c.metadata
            FROM embeddings e
            JOIN chunks c ON c.id = e.chunk_id
        """)

        results = []
        for row in cursor:
            chunk_id, vector_bytes, dimension, text, source, chunk_index, metadata = row
            stored_vec = np.frombuffer(vector_bytes, dtype=np.float32)

            # Dot product = cosine similarity (both normalized)
            score = float(np.dot(query_embedding, stored_vec))

            if score >= threshold:
                results.append(SearchResult(
                    chunk_id=chunk_id,
                    text=text,
                    source=source,
                    score=score,
                    metadata=json.loads(metadata) if metadata else {},
                    chunk_index=chunk_index
                ))

        # Sort by score descending
        results.sort(key=lambda r: r.score, reverse=True)
        return results[:top_k]

    def search_by_source(self, query_embedding: np.ndarray, source: str,
                         top_k: int = 5, threshold: float = 0.0) -> List[SearchResult]:
        """Search within a specific source document."""
        all_results = self.search(query_embedding, top_k=top_k * 3, threshold=threshold)
        filtered = [r for r in all_results if r.source == source]
        return filtered[:top_k]

    def delete_source(self, source: str) -> int:
        """Delete all chunks from a source. Returns count deleted."""
        cursor = self._conn.execute("DELETE FROM chunks WHERE source = ?", (source,))
        count = cursor.rowcount
        self._conn.commit()
        return count

    def list_sources(self) -> List[dict]:
        """List all indexed sources with chunk counts."""
        cursor = self._conn.execute("""
            SELECT source, COUNT(*) as chunk_count, MIN(created_at) as first_seen, MAX(created_at) as last_seen
            FROM chunks
            GROUP BY source
            ORDER BY chunk_count DESC
        """)
        return [
            {"source": row[0], "chunks": row[1], "first_seen": row[2], "last_seen": row[3]}
            for row in cursor.fetchall()
        ]

    def stats(self) -> dict:
        """Store statistics."""
        cursor = self._conn.execute("SELECT COUNT(*) FROM chunks")
        total_chunks = cursor.fetchone()[0]

        cursor = self._conn.execute("SELECT COUNT(DISTINCT source) FROM chunks")
        total_sources = cursor.fetchone()[0]

        cursor = self._conn.execute("SELECT dimension FROM embeddings LIMIT 1")
        row = cursor.fetchone()
        dimension = row[0] if row else 0

        cursor = self._conn.execute("SELECT SUM(token_count) FROM chunks")
        row = cursor.fetchone()
        total_tokens = row[0] or 0

        return {
            "total_chunks": total_chunks,
            "total_sources": total_sources,
            "embedding_dimension": dimension,
            "total_tokens": total_tokens,
            "db_path": self.db_path,
            "db_size_mb": round(os.path.getsize(self.db_path) / (1024 * 1024), 2)
        }

    def close(self):
        self._conn.close()

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self.close()
