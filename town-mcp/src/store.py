"""SQLite-backed chunk store with brute-force cosine search via numpy.

Phase 1 keeps it dependency-light: embeddings are stored as float32 BLOBs and
scored in-process. The corpus is small, so brute force is instant. sqlite-vec is
the documented scale-up path (see README) when the corpus grows large.
"""
from __future__ import annotations
import os
import sqlite3
from typing import List
import numpy as np

SCHEMA = """
CREATE TABLE IF NOT EXISTS docs (
  id      INTEGER PRIMARY KEY,
  path    TEXT UNIQUE NOT NULL,
  title   TEXT
);
CREATE TABLE IF NOT EXISTS chunks (
  id        INTEGER PRIMARY KEY,
  doc_id    INTEGER NOT NULL REFERENCES docs(id) ON DELETE CASCADE,
  ord       INTEGER NOT NULL,
  text      TEXT NOT NULL,
  dim       INTEGER NOT NULL,
  embedding BLOB NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_chunks_doc ON chunks(doc_id);
"""


def _to_blob(vec) -> bytes:
    return np.asarray(vec, dtype=np.float32).tobytes()


def _from_blob(blob: bytes) -> np.ndarray:
    return np.frombuffer(blob, dtype=np.float32)


class Store:
    def __init__(self, db_path: str):
        os.makedirs(os.path.dirname(os.path.abspath(db_path)) or ".", exist_ok=True)
        self.db = sqlite3.connect(db_path)
        self.db.execute("PRAGMA foreign_keys=ON")
        self.db.executescript(SCHEMA)
        self.db.commit()

    def clear(self):
        self.db.execute("DELETE FROM chunks")
        self.db.execute("DELETE FROM docs")
        self.db.commit()

    def upsert_document(self, path: str, title: str, chunks: List[str], embeddings: List[List[float]]):
        cur = self.db.cursor()
        cur.execute("DELETE FROM chunks WHERE doc_id IN (SELECT id FROM docs WHERE path=?)", (path,))
        cur.execute("DELETE FROM docs WHERE path=?", (path,))
        cur.execute("INSERT INTO docs(path, title) VALUES(?, ?)", (path, title))
        doc_id = cur.lastrowid
        for i, (text, emb) in enumerate(zip(chunks, embeddings)):
            arr = np.asarray(emb, dtype=np.float32)
            cur.execute(
                "INSERT INTO chunks(doc_id, ord, text, dim, embedding) VALUES(?,?,?,?,?)",
                (doc_id, i, text, int(arr.shape[0]), _to_blob(arr)),
            )
        self.db.commit()

    def _load(self):
        rows = self.db.execute(
            "SELECT c.text, c.embedding, d.path, d.title, c.ord "
            "FROM chunks c JOIN docs d ON d.id=c.doc_id"
        ).fetchall()
        if not rows:
            return [], None
        mat = np.vstack([_from_blob(r[1]) for r in rows])
        return rows, mat

    def search(self, query_vec, k: int = 5) -> List[dict]:
        rows, mat = self._load()
        if mat is None:
            return []
        q = np.asarray(query_vec, dtype=np.float32)
        qn = q / (np.linalg.norm(q) + 1e-8)
        mn = mat / (np.linalg.norm(mat, axis=1, keepdims=True) + 1e-8)
        sims = mn @ qn
        k = max(1, min(k, len(rows)))
        idx = np.argsort(-sims)[:k]
        out = []
        for i in idx:
            r = rows[int(i)]
            out.append({"score": float(sims[int(i)]), "text": r[0],
                        "path": r[2], "title": r[3], "ord": r[4]})
        return out

    def stats(self) -> dict:
        d = self.db.execute("SELECT COUNT(*) FROM docs").fetchone()[0]
        c = self.db.execute("SELECT COUNT(*) FROM chunks").fetchone()[0]
        return {"docs": d, "chunks": c}
