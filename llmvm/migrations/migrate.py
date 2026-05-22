#!/usr/bin/env python3
"""
Aetheris RAG — SQLite Migration Runner

Usage:
    python migrate.py apply          # Apply all pending migrations
    python migrate.py status         # Show migration status
    python migrate.py rollback       # Rollback last migration
"""

import sqlite3
import os
import sys
import re

DB_PATH = os.environ.get("RAG_DB_PATH", "./rag_data/vectors.db")
MIGRATIONS_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "migrations")

def get_db():
    os.makedirs(os.path.dirname(DB_PATH), exist_ok=True)
    conn = sqlite3.connect(DB_PATH)
    conn.execute("CREATE TABLE IF NOT EXISTS _migrations (version INTEGER PRIMARY KEY, applied_at DATETIME DEFAULT CURRENT_TIMESTAMP)")
    conn.commit()
    return conn

def get_applied_versions(conn):
    cursor = conn.execute("SELECT version FROM _migrations ORDER BY version")
    return [row[0] for row in cursor.fetchall()]

def get_pending_migrations(applied):
    files = sorted([f for f in os.listdir(MIGRATIONS_DIR) if f.endswith(".sql")])
    pending = []
    for f in files:
        version = int(re.match(r"(\d+)", f).group(1))
        if version not in applied:
            pending.append((version, f))
    return pending

def apply(conn, version, filename):
    filepath = os.path.join(MIGRATIONS_DIR, filename)
    with open(filepath, "r") as f:
        sql = f.read()
    conn.executescript(sql)
    conn.execute("INSERT INTO _migrations (version) VALUES (?)", (version,))
    conn.commit()
    print(f"  [OK] Applied {filename} (v{version})")

def cmd_apply():
    conn = get_db()
    applied = get_applied_versions(conn)
    pending = get_pending_migrations(applied)
    if not pending:
        print("No pending migrations.")
        return
    print(f"Applying {len(pending)} migration(s)...")
    for version, filename in pending:
        apply(conn, version, filename)
    conn.close()

def cmd_status():
    conn = get_db()
    applied = get_applied_versions(conn)
    all_files = sorted([f for f in os.listdir(MIGRATIONS_DIR) if f.endswith(".sql")])
    print(f"Database: {DB_PATH}")
    print(f"Migrations: {len(all_files)} total, {len(applied)} applied, {len(all_files) - len(applied)} pending")
    print()
    for f in all_files:
        version = int(re.match(r"(\d+)", f).group(1))
        status = "APPLIED" if version in applied else "PENDING"
        print(f"  [{status}] v{version:03d} - {f}")
    conn.close()

def cmd_rollback():
    conn = get_db()
    applied = get_applied_versions(conn)
    if not applied:
        print("No migrations to rollback.")
        return
    last_version = applied[-1]
    conn.execute("DELETE FROM _migrations WHERE version = ?", (last_version,))
    conn.commit()
    print(f"Rolled back migration v{last_version:03d}")
    conn.close()

if __name__ == "__main__":
    cmd = sys.argv[1] if len(sys.argv) > 1 else "status"
    if cmd == "apply":
        cmd_apply()
    elif cmd == "status":
        cmd_status()
    elif cmd == "rollback":
        cmd_rollback()
    else:
        print(f"Unknown command: {cmd}")
        print("Usage: python migrate.py [apply|status|rollback]")
        sys.exit(1)
