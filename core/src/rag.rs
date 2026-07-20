use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagConfig {
    pub chunk_size: usize,
    pub chunk_overlap: usize,
    pub top_k: usize,
    pub query_model: String,
    pub reasoning_enabled: bool,
    pub embed_models: Vec<String>,
    pub reranker_model: String,
    pub reranker_enabled: bool,
    pub timeout_secs: u64,
}

impl Default for RagConfig {
    fn default() -> Self {
        Self {
            chunk_size: 512,
            chunk_overlap: 64,
            top_k: 5,
            query_model: "phi4-mini".to_string(),
            reasoning_enabled: false,
            embed_models: vec!["nomic-embed-text".to_string()],
            reranker_model: "bge-reranker-v2-m3".to_string(),
            reranker_enabled: true,
            timeout_secs: 300,
        }
    }
}

impl RagConfig {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path.join("rag_config.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) {
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
        Self {
            chunk_size,
            chunk_overlap,
        }
    }

    fn count_tokens(text: &str) -> usize {
        text.len() / 4
    }

    fn char_trim_end(s: &str, n: usize) -> String {
        s.chars()
            .rev()
            .take(n)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    fn char_trim_start(s: &str, n: usize) -> String {
        s.chars().skip(n).collect::<String>()
    }

    fn split_by_paragraphs(text: &str) -> Vec<String> {
        let text = text.replace("\r\n", "\n").replace("\r", "\n");
        text.split("\n\n")
            .map(|p| p.trim())
            .filter(|p| !p.is_empty())
            .map(|p| p.to_string())
            .collect()
    }

    fn is_code_source(source: &str) -> bool {
        let lower = source.to_lowercase();
        lower.ends_with(".rs")
            || lower.ends_with(".py")
            || lower.ends_with(".js")
            || lower.ends_with(".ts")
            || lower.ends_with(".go")
            || lower.ends_with(".java")
            || lower.ends_with(".c")
            || lower.ends_with(".cpp")
            || lower.ends_with(".h")
            || lower.ends_with(".hpp")
            || lower.ends_with(".cc")
            || lower.ends_with(".cxx")
            || lower.ends_with(".kt")
            || lower.ends_with(".kts")
            || lower.ends_with(".swift")
            || lower.ends_with(".rb")
            || lower.ends_with(".php")
            || lower.ends_with(".lua")
            || lower.ends_with(".dart")
            || lower.ends_with(".r")
            || lower.ends_with(".m")
            || lower.ends_with(".sh")
            || lower.ends_with(".bash")
            || lower.ends_with(".zsh")
            || lower.ends_with(".ada")
            || lower.ends_with(".adb")
            || lower.ends_with(".ads")
            || lower.ends_with(".sql")
            || lower.ends_with(".toml")
            || lower.ends_with(".yaml")
            || lower.ends_with(".yml")
            || lower.ends_with(".json")
            || lower.ends_with(".css")
            || lower.ends_with(".scss")
            || lower.ends_with(".html")
            || lower.ends_with(".htm")
            || lower.ends_with(".svelte")
            || lower.ends_with(".vue")
            || lower.ends_with(".erl")
            || lower.ends_with(".ex")
            || lower.ends_with(".exs")
            || lower.ends_with(".hs")
            || lower.ends_with(".lhs")
            || lower.ends_with(".clj")
            || lower.ends_with(".cljs")
            || lower.ends_with(".zig")
            || lower.ends_with(".nim")
            || lower.ends_with(".cr")
            || lower.ends_with(".scala")
            || lower.ends_with(".ml")
            || lower.ends_with(".mli")
            || lower.ends_with(".fs")
            || lower.ends_with(".fsx")
    }

    fn is_definition_boundary(line: &str) -> bool {
        let t = line.trim();
        t.starts_with("fn ")
            || t.starts_with("pub fn")
            || t.starts_with("pub(crate) fn")
            || t.starts_with("pub(super) fn")
            || t.starts_with("pub unsafe fn")
            || t.starts_with("unsafe fn")
            || t.starts_with("def ")
            || t.starts_with("async def ")
            || t.starts_with("class ")
            || t.starts_with("public class ")
            || t.starts_with("private class ")
            || t.starts_with("protected class ")
            || t.starts_with("struct ")
            || t.starts_with("pub struct ")
            || t.starts_with("impl ")
            || t.starts_with("pub impl ")
            || t.starts_with("trait ")
            || t.starts_with("pub trait ")
            || t.starts_with("enum ")
            || t.starts_with("pub enum ")
            || t.starts_with("interface ")
            || t.starts_with("type ")
            || t.starts_with("func ")
            || t.starts_with("function ")
            || t.starts_with("sub ")
            || t.starts_with("public function ")
            || t.starts_with("private function ")
            || t.starts_with("public static function ")
            || t.starts_with("async fn")
            || t.starts_with("export function")
            || t.starts_with("export async function")
            || t.starts_with("defn ")
            || t.starts_with("CREATE ")
            || t.starts_with("ALTER ")
            || t.starts_with("DROP ")
            || t.starts_with("SELECT ")
            || t.starts_with("INSERT ")
            || t.starts_with("UPDATE ")
            || t.starts_with("DELETE ")
            || t.starts_with("pub type")
            || t.starts_with("pub enum")
            || t.starts_with("macro_rules!")
            || t.starts_with("#[derive")
            || t.starts_with("#[") && t.contains("]")
    }

    fn split_code_segments(text: &str) -> Vec<(usize, String)> {
        let lines: Vec<&str> = text.lines().collect();
        let mut segments: Vec<(usize, String)> = Vec::new();
        let mut start_line = 0usize;
        for (i, line) in lines.iter().enumerate() {
            if i > 0 && Self::is_definition_boundary(line) {
                let segment = lines[start_line..i].join("\n");
                if !segment.trim().is_empty() {
                    segments.push((start_line, segment));
                }
                start_line = i;
            }
        }
        let remaining = lines[start_line..].join("\n");
        if !remaining.trim().is_empty() {
            segments.push((start_line, remaining));
        }
        if segments.is_empty() {
            segments.push((0, text.to_string()));
        }
        segments
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

        if Self::is_code_source(source) {
            return self.chunk_code(text, source);
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
                    } else if !sent_buffer.is_empty() {
                        chunks.push(Chunk {
                            text: sent_buffer.clone(),
                            index: chunks.len(),
                            source: source.to_string(),
                            token_count: sent_tokens,
                        });
                        let overlap = Self::char_trim_end(
                            &sent_buffer,
                            self.chunk_overlap.min(sent_buffer.chars().count()),
                        );
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
                                word_tokens =
                                    word_buffer.iter().map(|w| Self::count_tokens(w)).sum();
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
                let overlap = Self::char_trim_end(
                    &current_text,
                    self.chunk_overlap.min(current_text.chars().count()),
                );
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

                let skip = self.chunk_size.min(current_text.chars().count());
                current_text = Self::char_trim_start(&current_text, skip)
                    .trim_start()
                    .to_string();
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

    fn chunk_code(&self, text: &str, source: &str) -> Vec<Chunk> {
        let segments = Self::split_code_segments(text);
        let mut chunks: Vec<Chunk> = Vec::new();
        let mut current = String::new();
        let mut current_tokens = 0;
        let half_size = self.chunk_size * 2;

        for (_line, segment) in &segments {
            let seg_tokens = Self::count_tokens(segment);
            if current_tokens + seg_tokens <= half_size {
                if !current.is_empty() {
                    current.push_str("\n\n");
                }
                current.push_str(segment);
                current_tokens += seg_tokens;
                continue;
            }
            if !current.is_empty() {
                chunks.push(Chunk {
                    text: current.clone(),
                    index: chunks.len(),
                    source: source.to_string(),
                    token_count: current_tokens,
                });
            }
            current = segment.clone();
            current_tokens = seg_tokens;
        }

        if !current.is_empty() {
            chunks.push(Chunk {
                text: current,
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
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create db dir: {}", e))?;
        }
        let conn =
            rusqlite::Connection::open(&path).map_err(|e| format!("Failed to open db: {}", e))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
            .map_err(|e| format!("Failed to set pragmas: {}", e))?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")
            .map_err(|e| format!("Failed pragma foreign_keys: {}", e))?;
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
            CREATE INDEX IF NOT EXISTS idx_chunks_source_index ON chunks(source, chunk_index);",
        )
        .map_err(|e| format!("Failed to init schema: {}", e))?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            db_path: path,
        })
    }

    pub fn add_chunks(
        &self,
        chunks: &[Chunk],
        embeddings: &[Vec<f32>],
    ) -> Result<Vec<i64>, String> {
        if chunks.len() != embeddings.len() {
            return Err(format!(
                "Mismatch: {} chunks vs {} embeddings",
                chunks.len(),
                embeddings.len()
            ));
        }

        let conn = self.conn.lock().unwrap();
        conn.execute("BEGIN IMMEDIATE", [])
            .map_err(|e| format!("Failed to begin transaction: {}", e))?;
        let mut ids = Vec::new();

        for (chunk, embedding) in chunks.iter().zip(embeddings.iter()) {
            let norm: f32 = embedding.iter().map(|v| v * v).sum::<f32>().sqrt();
            let normalized: Vec<f32> = if norm > 0.0 {
                embedding.iter().map(|v| v / norm).collect()
            } else {
                embedding.clone()
            };

            let vector_bytes: Vec<u8> = normalized.iter().flat_map(|v| v.to_le_bytes()).collect();
            let dimension = normalized.len() as i64;

            conn.execute(
                "INSERT INTO chunks (text, source, chunk_index, token_count) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![chunk.text, chunk.source, chunk.index as i64, chunk.token_count as i64],
            ).map_err(|e| format!("Failed to insert chunk: {}", e))?;

            let chunk_id = conn.last_insert_rowid();

            conn.execute(
                "INSERT INTO embeddings (chunk_id, vector, dimension) VALUES (?1, ?2, ?3)",
                rusqlite::params![chunk_id, vector_bytes, dimension],
            )
            .map_err(|e| format!("Failed to insert embedding: {}", e))?;

            ids.push(chunk_id);
        }

        conn.execute("COMMIT", [])
            .map_err(|e| format!("Failed to commit transaction: {}", e))?;
        Ok(ids)
    }

    pub fn search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> Result<Vec<SearchResult>, String> {
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

        let rows = stmt
            .query_map([], |row| {
                let chunk_id: i64 = row.get(0)?;
                let vector_bytes: Vec<u8> = row.get(1)?;
                let _dimension: i64 = row.get(2)?;
                let text: String = row.get(3)?;
                let source: String = row.get(4)?;
                let chunk_index: i64 = row.get(5)?;
                let token_count: i64 = row.get(6)?;
                Ok((
                    chunk_id,
                    vector_bytes,
                    text,
                    source,
                    chunk_index as usize,
                    token_count as usize,
                ))
            })
            .map_err(|e| format!("Failed to query embeddings: {}", e))?;

        let mut results: Vec<SearchResult> = Vec::new();
        for row in rows.flatten() {
            let (chunk_id, vector_bytes, text, source, chunk_index, token_count) = row;
            let stored: Vec<f32> = vector_bytes
                .chunks(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect();

            let score: f64 = query_norm
                .iter()
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

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(top_k);
        Ok(results)
    }

    pub fn sources(&self) -> Result<Vec<SourceInfo>, String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT source, COUNT(*) as chunk_count, MIN(created_at) as first_seen, MAX(created_at) as last_seen
             FROM chunks GROUP BY source ORDER BY chunk_count DESC"
        ).map_err(|e| format!("Failed to prepare sources query: {}", e))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(SourceInfo {
                    source: row.get(0)?,
                    chunk_count: row.get(1)?,
                    first_seen: row.get::<_, String>(2).unwrap_or_default(),
                    last_seen: row.get::<_, String>(3).unwrap_or_default(),
                })
            })
            .map_err(|e| format!("Failed to query sources: {}", e))?;

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
            .query_row("SELECT COUNT(DISTINCT source) FROM chunks", [], |row| {
                row.get(0)
            })
            .map_err(|e| format!("Failed to count sources: {}", e))?;

        let dimension: i64 = conn
            .query_row("SELECT dimension FROM embeddings LIMIT 1", [], |row| {
                row.get(0)
            })
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
            .execute(
                "DELETE FROM chunks WHERE source = ?1",
                rusqlite::params![source],
            )
            .map_err(|e| format!("Failed to delete source: {}", e))?;
        Ok(count)
    }
}
