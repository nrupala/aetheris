use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagConfig {
    pub chunk_size: usize,
    pub chunk_overlap: usize,
    pub top_k: usize,
    pub query_model: String,
    pub reasoning_enabled: bool,
    pub embed_models: Vec<String>,
}

impl Default for RagConfig {
    fn default() -> Self {
        Self {
            chunk_size: 512,
            chunk_overlap: 64,
            top_k: 5,
            query_model: "qwen2.5:14b".to_string(),
            reasoning_enabled: false,
            embed_models: vec![
                "nomic-embed-text".to_string(),
                "phi-4-reasoning-plus-q4_k_m".to_string(),
                "qwen2.5:14b".to_string(),
            ],
        }
    }
}

impl RagConfig {
    pub fn load(path: &PathBuf) -> Self {
        std::fs::read_to_string(path.join("rag_config.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &PathBuf) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path.join("rag_config.json"), &json);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub text: String,
    pub index: usize,
    pub source: String,
    pub token_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub chunk_id: i64,
    pub text: String,
    pub source: String,
    pub score: f64,
    pub chunk_index: usize,
    pub token_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceInfo {
    pub source: String,
    pub chunk_count: i64,
    pub first_seen: String,
    pub last_seen: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoreStats {
    pub total_chunks: i64,
    pub total_sources: i64,
    pub embedding_dimension: i64,
    pub db_path: String,
    pub db_size_mb: f64,
}

pub struct TextChunker {
    pub chunk_size: usize,
    pub chunk_overlap: usize,
}

impl TextChunker {
    pub fn new(chunk_size: usize, chunk_overlap: usize) -> Self {
        Self { chunk_size, chunk_overlap }
    }

    fn count_tokens(text: &str) -> usize {
        text.len() / 4
    }

    fn split_by_paragraphs(text: &str) -> Vec<String> {
        let text = text.replace("\r\n", "\n").replace("\r", "\n");
        text.split("\n\n")
            .map(|p| p.trim())
            .filter(|p| !p.is_empty())
            .map(|p| p.to_string())
            .collect()
    }

    fn split_by_sentences(text: &str) -> Vec<String> {
        let mut sentences = Vec::new();
        let mut current = String::new();
        for c in text.chars() {
            current.push(c);
            if matches!(c, '.' | '!' | '?') {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    sentences.push(trimmed);
                }
                current = String::new();
            }
        }
        let trimmed = current.trim().to_string();
        if !trimmed.is_empty() {
            sentences.push(trimmed);
        }
        sentences
    }

    pub fn chunk(&self, text: &str, source: &str) -> Vec<Chunk> {
        if text.trim().is_empty() {
            return vec![];
        }

        let mut chunks: Vec<Chunk> = Vec::new();
        let paragraphs = Self::split_by_paragraphs(text);
        let mut current_text = String::new();
        let mut current_tokens = 0;

        for para in &paragraphs {
            let para_tokens = Self::count_tokens(para);

            if current_tokens + para_tokens <= self.chunk_size {
                if !current_text.is_empty() {
                    current_text.push_str("\n\n");
                }
                current_text.push_str(para);
                current_tokens += para_tokens;
                continue;
            }

            if para_tokens > self.chunk_size {
                if !current_text.is_empty() {
                    chunks.push(Chunk {
                        text: current_text.clone(),
                        index: chunks.len(),
                        source: source.to_string(),
                        token_count: current_tokens,
                    });
                    current_text = String::new();
                    current_tokens = 0;
                }

                let sentences = Self::split_by_sentences(para);
                let mut sent_buffer = String::new();
                let mut sent_tokens = 0;

                for sentence in &sentences {
                    let s_tokens = Self::count_tokens(sentence);
                    if sent_tokens + s_tokens <= self.chunk_size {
                        if !sent_buffer.is_empty() {
                            sent_buffer.push(' ');
                        }
                        sent_buffer.push_str(sentence);
                        sent_tokens += s_tokens;
                    } else {
                        if !sent_buffer.is_empty() {
                            chunks.push(Chunk {
                                text: sent_buffer.clone(),
                                index: chunks.len(),
                                source: source.to_string(),
                                token_count: sent_tokens,
                            });
                            let overlap_start = sent_buffer.len().saturating_sub(self.chunk_overlap);
                            let overlap = sent_buffer[overlap_start..].to_string();
                            let overlap_tokens = Self::count_tokens(&overlap);
                            sent_buffer = if !overlap.is_empty() {
                                format!("{} {}", overlap, sentence)
                            } else {
                                sentence.clone()
                            };
                            sent_tokens = overlap_tokens + s_tokens;
                        } else {
                            let words: Vec<&str> = sentence.split_whitespace().collect();
                            let mut word_buffer: Vec<&str> = Vec::new();
                            let mut word_tokens = 0;
                            for word in &words {
                                let w_tok = Self::count_tokens(word);
                                if word_tokens + w_tok > self.chunk_size && !word_buffer.is_empty() {
                                    let hard_chunk = word_buffer.join(" ");
                                    chunks.push(Chunk {
                                        text: hard_chunk,
                                        index: chunks.len(),
                                        source: source.to_string(),
                                        token_count: word_tokens,
                                    });
                                    let keep = word_buffer.len().saturating_sub(self.chunk_overlap / 4);
                                    word_buffer = word_buffer[keep..].to_vec();
                                    word_tokens = word_buffer.iter().map(|w| Self::count_tokens(w)).sum();
                                }
                                word_buffer.push(word);
                                word_tokens += w_tok;
                            }
                            if !word_buffer.is_empty() {
                                chunks.push(Chunk {
                                    text: word_buffer.join(" "),
                                    index: chunks.len(),
                                    source: source.to_string(),
                                    token_count: word_tokens,
                                });
                            }
                            sent_buffer = String::new();
                            sent_tokens = 0;
                        }
                    }
                }

                if !sent_buffer.is_empty() {
                    chunks.push(Chunk {
                        text: sent_buffer,
                        index: chunks.len(),
                        source: source.to_string(),
                        token_count: sent_tokens,
                    });
                }
                continue;
            }

            if !current_text.is_empty() {
                let overlap_start = current_text.len().saturating_sub(self.chunk_overlap);
                let overlap = &current_text[overlap_start..];
                current_text = if !overlap.is_empty() {
                    format!("{}\n\n{}", overlap, para)
                } else {
                    format!("{}\n\n{}", current_text, para)
                };
                chunks.push(Chunk {
                    text: current_text.clone(),
                    index: chunks.len(),
                    source: source.to_string(),
                    token_count: self.chunk_size,
                });

                current_text = current_text[self.chunk_size.min(current_text.len())..].trim_start().to_string();
                current_tokens = Self::count_tokens(&current_text);
            }
        }

        let trimmed = current_text.trim().to_string();
        if !trimmed.is_empty() {
            chunks.push(Chunk {
                text: trimmed,
                index: chunks.len(),
                source: source.to_string(),
                token_count: current_tokens,
            });
        }

        chunks
    }
}

#[derive(Clone)]
pub struct VectorStore {
    conn: Arc<Mutex<rusqlite::Connection>>,
    db_path: PathBuf,
}

impl VectorStore {
    pub fn new(db_path: &str) -> Result<Self, String> {
        let path = PathBuf::from(db_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create db dir: {}", e))?;
        }
        let conn = rusqlite::Connection::open(&path)
            .map_err(|e| format!("Failed to open db: {}", e))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
            .map_err(|e| format!("Failed to set pragmas: {}", e))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                text TEXT NOT NULL,
                source TEXT NOT NULL,
                chunk_index INTEGER DEFAULT 0,
                token_count INTEGER DEFAULT 0,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS embeddings (
                chunk_id INTEGER PRIMARY KEY REFERENCES chunks(id) ON DELETE CASCADE,
                vector BLOB NOT NULL,
                dimension INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_chunks_source ON chunks(source);
            CREATE INDEX IF NOT EXISTS idx_chunks_source_index ON chunks(source, chunk_index);"
        ).map_err(|e| format!("Failed to init schema: {}", e))?;

        Ok(Self { conn: Arc::new(Mutex::new(conn)), db_path: path })
    }

    pub fn add_chunks(&self, chunks: &[Chunk], embeddings: &[Vec<f32>]) -> Result<Vec<i64>, String> {
        if chunks.len() != embeddings.len() {
            return Err(format!("Mismatch: {} chunks vs {} embeddings", chunks.len(), embeddings.len()));
        }

        let conn = self.conn.lock().unwrap();
        let mut ids = Vec::new();

        for (chunk, embedding) in chunks.iter().zip(embeddings.iter()) {
            let norm: f32 = embedding.iter().map(|v| v * v).sum::<f32>().sqrt();
            let normalized: Vec<f32> = if norm > 0.0 {
                embedding.iter().map(|v| v / norm).collect()
            } else {
                embedding.clone()
            };

            let vector_bytes: Vec<u8> = normalized.iter()
                .flat_map(|v| v.to_le_bytes())
                .collect();
            let dimension = normalized.len() as i64;

            conn.execute(
                "INSERT INTO chunks (text, source, chunk_index, token_count) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![chunk.text, chunk.source, chunk.index as i64, chunk.token_count as i64],
            ).map_err(|e| format!("Failed to insert chunk: {}", e))?;

            let chunk_id = conn.last_insert_rowid();

            conn.execute(
                "INSERT INTO embeddings (chunk_id, vector, dimension) VALUES (?1, ?2, ?3)",
                rusqlite::params![chunk_id, vector_bytes, dimension],
            ).map_err(|e| format!("Failed to insert embedding: {}", e))?;

            ids.push(chunk_id);
        }

        Ok(ids)
    }

    pub fn search(&self, query_embedding: &[f32], top_k: usize) -> Result<Vec<SearchResult>, String> {
        let norm: f32 = query_embedding.iter().map(|v| v * v).sum::<f32>().sqrt();
        let query_norm: Vec<f32> = if norm > 0.0 {
            query_embedding.iter().map(|v| v / norm).collect()
        } else {
            query_embedding.to_vec()
        };

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT e.chunk_id, e.vector, e.dimension, c.text, c.source, c.chunk_index, c.token_count
             FROM embeddings e JOIN chunks c ON c.id = e.chunk_id"
        ).map_err(|e| format!("Failed to prepare search: {}", e))?;

        let rows = stmt.query_map([], |row| {
            let chunk_id: i64 = row.get(0)?;
            let vector_bytes: Vec<u8> = row.get(1)?;
            let _dimension: i64 = row.get(2)?;
            let text: String = row.get(3)?;
            let source: String = row.get(4)?;
            let chunk_index: i64 = row.get(5)?;
            let token_count: i64 = row.get(6)?;
            Ok((chunk_id, vector_bytes, text, source, chunk_index as usize, token_count as usize))
        }).map_err(|e| format!("Failed to query embeddings: {}", e))?;

        let mut results: Vec<SearchResult> = Vec::new();
        for row in rows.flatten() {
            let (chunk_id, vector_bytes, text, source, chunk_index, token_count) = row;
            let stored: Vec<f32> = vector_bytes.chunks(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect();

            let score: f64 = query_norm.iter()
                .zip(stored.iter())
                .map(|(a, b)| (*a as f64) * (*b as f64))
                .sum();

            results.push(SearchResult {
                chunk_id,
                text,
                source,
                score,
                chunk_index,
                token_count,
            });
        }

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
        Ok(results)
    }

    pub fn sources(&self) -> Result<Vec<SourceInfo>, String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT source, COUNT(*) as chunk_count, MIN(created_at) as first_seen, MAX(created_at) as last_seen
             FROM chunks GROUP BY source ORDER BY chunk_count DESC"
        ).map_err(|e| format!("Failed to prepare sources query: {}", e))?;

        let rows = stmt.query_map([], |row| {
            Ok(SourceInfo {
                source: row.get(0)?,
                chunk_count: row.get(1)?,
                first_seen: row.get::<_, String>(2).unwrap_or_default(),
                last_seen: row.get::<_, String>(3).unwrap_or_default(),
            })
        }).map_err(|e| format!("Failed to query sources: {}", e))?;

        let mut sources: Vec<SourceInfo> = Vec::new();
        for row in rows.flatten() {
            sources.push(row);
        }
        Ok(sources)
    }

    pub fn stats(&self) -> Result<StoreStats, String> {
        let conn = self.conn.lock().unwrap();

        let total_chunks: i64 = conn
            .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))
            .map_err(|e| format!("Failed to count chunks: {}", e))?;

        let total_sources: i64 = conn
            .query_row("SELECT COUNT(DISTINCT source) FROM chunks", [], |row| row.get(0))
            .map_err(|e| format!("Failed to count sources: {}", e))?;

        let dimension: i64 = conn
            .query_row("SELECT dimension FROM embeddings LIMIT 1", [], |row| row.get(0))
            .unwrap_or(0);

        let db_size = std::fs::metadata(&self.db_path)
            .map(|m| m.len() as f64 / (1024.0 * 1024.0))
            .unwrap_or(0.0);

        Ok(StoreStats {
            total_chunks,
            total_sources,
            embedding_dimension: dimension,
            db_path: self.db_path.to_string_lossy().to_string(),
            db_size_mb: (db_size * 100.0).round() / 100.0,
        })
    }

    pub fn delete_source(&self, source: &str) -> Result<usize, String> {
        let conn = self.conn.lock().unwrap();
        let count = conn
            .execute("DELETE FROM chunks WHERE source = ?1", rusqlite::params![source])
            .map_err(|e| format!("Failed to delete source: {}", e))?;
        Ok(count)
    }
}
