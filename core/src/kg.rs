use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex};

const SENTENCE_STARTERS: &[&str] = &[
    "The", "This", "That", "These", "Those", "What", "Which", "Who", "Whom",
    "Whose", "Where", "When", "Why", "How", "A", "An", "It", "Its", "We",
    "They", "He", "She", "You", "I", "My", "Our", "Your", "His", "Her",
    "Their", "There", "Here", "Some", "Many", "Much", "Each", "Every",
    "Both", "All", "No", "Not", "But", "And", "Or", "If", "Then", "Else",
    "So", "For", "With", "Without", "Because", "Although", "While",
    "However", "Therefore", "Thus", "Hence", "Indeed", "Meanwhile",
    "Nevertheless", "Nonetheless", "Moreover", "Furthermore", "Additionally",
    "Consequently", "Subsequently", "Eventually", "Finally", "Initially",
    "Currently", "Recently", "Previously", "Basically", "Essentially",
    "Importantly", "Specifically", "Generally", "Typically", "Commonly",
    "Notably", "Interestingly", "Unfortunately", "Meanwhile", "Next",
    "First", "Second", "Third", "Last", "Another", "One", "Two", "Three",
];

#[derive(Debug, Clone, Serialize)]
pub struct KGEntity {
    pub name: String,
    pub entity_type: String,
    pub chunk_count: i64,
    pub first_seen: String,
    pub last_seen: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct KGRelation {
    pub source: String,
    pub target: String,
    pub relation_type: String,
    pub strength: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct KGStats {
    pub entities: i64,
    pub relations: i64,
    pub clusters: i64,
    pub central_nodes: Vec<String>,
}

struct WordToken {
    text: String,
    is_sentence_start: bool,
}

pub struct KnowledgeGraph {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl KnowledgeGraph {
    pub fn new(db_path: &Path) -> Result<Self, String> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create kg db directory: {}", e))?;
        }
        let conn = rusqlite::Connection::open(db_path)
            .map_err(|e| format!("Failed to open kg db: {}", e))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
            .map_err(|e| format!("Failed to set kg pragmas: {}", e))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS entities (
                name TEXT PRIMARY KEY,
                entity_type TEXT NOT NULL,
                first_seen TEXT NOT NULL DEFAULT (datetime('now')),
                last_seen TEXT NOT NULL DEFAULT (datetime('now')),
                chunk_count INTEGER DEFAULT 1
            );
            CREATE TABLE IF NOT EXISTS relations (
                source TEXT NOT NULL,
                target TEXT NOT NULL,
                relation_type TEXT NOT NULL DEFAULT 'co_occurs',
                strength INTEGER DEFAULT 1,
                first_seen TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (source, target, relation_type)
            );
            CREATE INDEX IF NOT EXISTS idx_entities_type ON entities(entity_type);
            CREATE INDEX IF NOT EXISTS idx_relations_source ON relations(source);
            CREATE INDEX IF NOT EXISTS idx_relations_target ON relations(target);",
        )
        .map_err(|e| format!("Failed to create kg schema: {}", e))?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn ingest(&self, text: &str, _source: &str) -> Result<(), String> {
        let entities = extract_entities(text);
        if entities.is_empty() {
            return Ok(());
        }

        let conn = self.conn.lock().unwrap();

        for entity in &entities {
            let entity_type = infer_entity_type(entity);
            conn.execute(
                "INSERT INTO entities (name, entity_type, chunk_count)
                 VALUES (?1, ?2, 1)
                 ON CONFLICT(name) DO UPDATE SET
                     last_seen = datetime('now'),
                     chunk_count = chunk_count + 1",
                rusqlite::params![entity, entity_type],
            )
            .map_err(|e| format!("Failed to upsert entity: {}", e))?;
        }

        let n = entities.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let (src, tgt) = if entities[i] < entities[j] {
                    (&entities[i], &entities[j])
                } else {
                    (&entities[j], &entities[i])
                };
                conn.execute(
                    "INSERT INTO relations (source, target, relation_type, strength)
                     VALUES (?1, ?2, 'co_occurs', 1)
                     ON CONFLICT(source, target, relation_type) DO UPDATE SET
                         strength = strength + 1",
                    rusqlite::params![src, tgt],
                )
                .map_err(|e| format!("Failed to upsert relation: {}", e))?;
            }
        }

        Ok(())
    }

    pub fn get_stats(&self) -> Result<KGStats, String> {
        let conn = self.conn.lock().unwrap();

        let entities: i64 = conn
            .query_row("SELECT COUNT(*) FROM entities", [], |row| row.get(0))
            .map_err(|e| format!("Failed to count entities: {}", e))?;

        let relations: i64 = conn
            .query_row("SELECT COUNT(*) FROM relations", [], |row| row.get(0))
            .map_err(|e| format!("Failed to count relations: {}", e))?;

        let mut stmt = conn
            .prepare("SELECT source, target FROM relations")
            .map_err(|e| format!("Failed to prepare relations query: {}", e))?;

        let rows = stmt
            .query_map([], |row| {
                let source: String = row.get(0)?;
                let target: String = row.get(1)?;
                Ok((source, target))
            })
            .map_err(|e| format!("Failed to query relations: {}", e))?;

        let mut graph: HashMap<String, Vec<String>> = HashMap::new();
        for row in rows.flatten() {
            let (s, t) = row;
            graph.entry(s.clone()).or_default().push(t.clone());
            graph.entry(t).or_default().push(s);
        }

        let mut visited: HashSet<String> = HashSet::new();
        let mut component_count = 0i64;

        for node in graph.keys() {
            if visited.contains(node) {
                continue;
            }
            component_count += 1;
            let mut queue = VecDeque::new();
            queue.push_back(node.clone());
            visited.insert(node.clone());
            while let Some(current) = queue.pop_front() {
                if let Some(neighbors) = graph.get(&current) {
                    for neighbor in neighbors {
                        if !visited.contains(neighbor) {
                            visited.insert(neighbor.clone());
                            queue.push_back(neighbor.clone());
                        }
                    }
                }
            }
        }

        let mut degree: Vec<(String, usize)> =
            graph.into_iter().map(|(k, v)| (k, v.len())).collect();
        degree.sort_by(|a, b| b.1.cmp(&a.1));
        let central_nodes: Vec<String> = degree.into_iter().take(10).map(|(n, _)| n).collect();

        Ok(KGStats {
            entities,
            relations,
            clusters: component_count,
            central_nodes,
        })
    }

    pub fn get_entities(
        &self,
        type_filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<KGEntity>, String> {
        let conn = self.conn.lock().unwrap();

        let entities = if let Some(ft) = type_filter {
            let mut stmt = conn
                .prepare(
                    "SELECT name, entity_type, chunk_count, first_seen, last_seen
                     FROM entities WHERE entity_type = ?1
                     ORDER BY chunk_count DESC LIMIT ?2",
                )
                .map_err(|e| format!("Failed to prepare entity query: {}", e))?;

            let rows: Vec<KGEntity> = stmt
                .query_map(rusqlite::params![ft, limit as i64], |row| {
                    Ok(KGEntity {
                        name: row.get(0)?,
                        entity_type: row.get(1)?,
                        chunk_count: row.get(2)?,
                        first_seen: row.get::<_, String>(3).unwrap_or_default(),
                        last_seen: row.get::<_, String>(4).unwrap_or_default(),
                    })
                })
                .map_err(|e| format!("Failed to query entities: {}", e))?
                .filter_map(|r| r.ok())
                .collect();
            rows
        } else {
            let mut stmt = conn
                .prepare(
                    "SELECT name, entity_type, chunk_count, first_seen, last_seen
                     FROM entities ORDER BY chunk_count DESC LIMIT ?1",
                )
                .map_err(|e| format!("Failed to prepare entity query: {}", e))?;

            let rows: Vec<KGEntity> = stmt
                .query_map(rusqlite::params![limit as i64], |row| {
                    Ok(KGEntity {
                        name: row.get(0)?,
                        entity_type: row.get(1)?,
                        chunk_count: row.get(2)?,
                        first_seen: row.get::<_, String>(3).unwrap_or_default(),
                        last_seen: row.get::<_, String>(4).unwrap_or_default(),
                    })
                })
                .map_err(|e| format!("Failed to query entities: {}", e))?
                .filter_map(|r| r.ok())
                .collect();
            rows
        };

        Ok(entities)
    }

    pub fn get_relations(&self, limit: usize) -> Result<Vec<KGRelation>, String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT source, target, relation_type, strength
                 FROM relations ORDER BY strength DESC LIMIT ?1",
            )
            .map_err(|e| format!("Failed to prepare relations query: {}", e))?;

        let relations = stmt
            .query_map(rusqlite::params![limit as i64], |row| {
                Ok(KGRelation {
                    source: row.get(0)?,
                    target: row.get(1)?,
                    relation_type: row.get(2)?,
                    strength: row.get(3)?,
                })
            })
            .map_err(|e| format!("Failed to query relations: {}", e))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(relations)
    }
}

fn tokenize(text: &str) -> Vec<WordToken> {
    let mut tokens = Vec::new();
    let mut buf: Vec<char> = Vec::new();
    let mut next_is_sentence_start = true;

    for c in text.chars() {
        if c.is_alphanumeric() || c == '-' || c == '_' || c == '\'' {
            buf.push(c);
        } else {
            if !buf.is_empty() {
                let word: String = buf.drain(..).collect();
                tokens.push(WordToken {
                    text: word,
                    is_sentence_start: next_is_sentence_start,
                });
                next_is_sentence_start = false;
            }
            if c == '.' || c == '!' || c == '?' {
                next_is_sentence_start = true;
            }
        }
    }
    if !buf.is_empty() {
        tokens.push(WordToken {
            text: buf.into_iter().collect(),
            is_sentence_start: next_is_sentence_start,
        });
    }

    tokens
}

fn is_sentence_starter(word: &str) -> bool {
    SENTENCE_STARTERS.contains(&word)
}

fn infer_entity_type(name: &str) -> &'static str {
    let has_whitespace = name.contains(' ');
    let is_all_upper_digit = name.chars().all(|c| c.is_uppercase() || c.is_ascii_digit());
    let has_numbers_or_dashes =
        name.chars().any(|c| c.is_ascii_digit() || c == '-');

    if !has_whitespace && is_all_upper_digit {
        return "acronym";
    }

    if has_whitespace {
        if has_numbers_or_dashes {
            return "technology";
        }
        return "system";
    }

    if name.ends_with("er") || name.ends_with("or") || name.ends_with("ist") {
        return "person";
    }

    if name.ends_with("tion")
        || name.ends_with("sion")
        || name.ends_with("ment")
        || name.ends_with("ism")
        || name.ends_with("ity")
        || name.ends_with("ogy")
        || name.ends_with("ics")
    {
        return "concept";
    }

    let lower = name.to_lowercase();
    if lower.contains(".com")
        || lower.contains("api")
        || lower.contains("sdk")
        || lower.contains("protocol")
        || lower.contains("framework")
    {
        return "tool";
    }

    if name.len() >= 6 {
        "technology"
    } else {
        "concept"
    }
}

fn extract_entities(text: &str) -> Vec<String> {
    let tokens = tokenize(text);
    let n = tokens.len();
    if n == 0 {
        return Vec::new();
    }

    let mut seen: HashSet<String> = HashSet::new();

    for (i, token) in tokens.iter().enumerate() {
        let word = &token.text;

        let Some(first) = word.chars().next() else {
            continue;
        };
        if !first.is_uppercase() {
            continue;
        }

        if token.is_sentence_start && i > 0 && word.len() <= 3 {
            let is_acronym = word.chars().all(|c| c.is_uppercase() || c.is_ascii_digit());
            if !is_acronym {
                continue;
            }
        }

        if is_sentence_starter(word) {
            continue;
        }

        if word.len() >= 2 {
            seen.insert(word.clone());
        }
    }

    for i in 0..n.saturating_sub(1) {
        let w1 = &tokens[i].text;
        let w2 = &tokens[i + 1].text;

        let Some(f1) = w1.chars().next() else {
            continue;
        };
        let Some(f2) = w2.chars().next() else {
            continue;
        };

        if !f1.is_uppercase() || !f2.is_uppercase() {
            continue;
        }
        if is_sentence_starter(w1) || is_sentence_starter(w2) {
            continue;
        }
        if w1.len() < 2 || w2.len() < 2 {
            continue;
        }

        let pair = format!("{} {}", w1, w2);
        seen.insert(pair);
    }

    for i in 0..n.saturating_sub(2) {
        let w1 = &tokens[i].text;
        let w2 = &tokens[i + 1].text;
        let w3 = &tokens[i + 2].text;

        let Some(f1) = w1.chars().next() else {
            continue;
        };
        let Some(f2) = w2.chars().next() else {
            continue;
        };
        let Some(f3) = w3.chars().next() else {
            continue;
        };

        if !f1.is_uppercase() || !f2.is_uppercase() || !f3.is_uppercase() {
            continue;
        }
        if is_sentence_starter(w1) || is_sentence_starter(w2) || is_sentence_starter(w3) {
            continue;
        }
        if w1.len() < 2 || w2.len() < 2 || w3.len() < 2 {
            continue;
        }

        let triple = format!("{} {} {}", w1, w2, w3);
        seen.insert(triple);
    }

    let mut result: Vec<String> = seen.into_iter().collect();
    result.sort();
    result
}
