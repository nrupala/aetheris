-- Migration 002: WAL mode and performance optimizations
-- Enables Write-Ahead Logging for better concurrent access
-- Adds metadata indexing and search optimization columns

PRAGMA journal_mode=WAL;
PRAGMA synchronous=NORMAL;
PRAGMA cache_size=-2000;
PRAGMA mmap_size=268435456;

ALTER TABLE sources ADD COLUMN IF NOT EXISTS chunk_count INTEGER DEFAULT 0;
ALTER TABLE sources ADD COLUMN IF NOT EXISTS token_count INTEGER DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_chunks_token_count ON chunks(token_count);
CREATE INDEX IF NOT EXISTS idx_sources_chunk_count ON sources(chunk_count);
