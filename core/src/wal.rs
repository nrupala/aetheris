use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WalEntry {
    FileUpload {
        filename: String,
        size: u64,
    },
    FileDownload {
        filename: String,
    },
    FileDelete {
        filename: String,
    },
    SecurityBan {
        peer_id: String,
        reason: String,
    },
    SecurityUnban {
        peer_id: String,
    },
    AiQuery {
        model: String,
        tokens: u64,
    },
    ConfigChange {
        key: String,
        old_value: String,
        new_value: String,
    },
    Snapshot {
        name: String,
    },
    Custom {
        action: String,
        details: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalRecord {
    pub timestamp: u64,
    pub sequence: u64,
    pub entry: WalEntry,
}

pub struct WriteAheadLog {
    log_path: PathBuf,
    sequence: u64,
}

impl WriteAheadLog {
    pub fn new(log_dir: &str) -> std::io::Result<Self> {
        let log_dir_path = PathBuf::from(log_dir);
        std::fs::create_dir_all(&log_dir_path)?;
        let log_path = log_dir_path.join("wal.log");
        let sequence = Self::count_existing_records(&log_path);
        Ok(Self { log_path, sequence })
    }

    fn count_existing_records(path: &PathBuf) -> u64 {
        if !path.exists() {
            return 0;
        }
        match File::open(path) {
            Ok(file) => {
                let reader = BufReader::new(file);
                reader.lines().count() as u64
            }
            Err(_) => 0,
        }
    }

    pub fn append(&mut self, entry: WalEntry) -> std::io::Result<u64> {
        self.sequence += 1;
        let record = WalRecord {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            sequence: self.sequence,
            entry: entry.clone(),
        };

        let line = serde_json::to_string(&record)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;
        writeln!(file, "{}", line)?;
        file.sync_all()?;

        Ok(record.sequence)
    }

    pub fn replay<F>(&self, mut handler: F) -> std::io::Result<usize>
    where
        F: FnMut(&WalRecord) -> std::io::Result<()>,
    {
        if !self.log_path.exists() {
            return Ok(0);
        }

        let file = File::open(&self.log_path)?;
        let reader = BufReader::new(file);
        let mut count = 0;

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(record) = serde_json::from_str::<WalRecord>(&line) {
                handler(&record)?;
                count += 1;
            }
        }

        Ok(count)
    }

    pub fn truncate(&self) -> std::io::Result<()> {
        if self.log_path.exists() {
            std::fs::remove_file(&self.log_path)?;
        }
        Ok(())
    }

    pub fn record_count(&self) -> u64 {
        self.sequence
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_wal_dir(name: &str) -> String {
        let base = std::env::temp_dir().join("aetheris_wal_tests");
        std::fs::create_dir_all(&base).ok();
        let dir = base.join(name);
        let _ = std::fs::remove_dir_all(&dir);
        dir.to_string_lossy().to_string()
    }

    #[test]
    fn test_wal_creation() {
        let dir = test_wal_dir("creation");
        let wal = WriteAheadLog::new(&dir).unwrap();
        assert_eq!(wal.record_count(), 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_wal_append() {
        let dir = test_wal_dir("append");
        let mut wal = WriteAheadLog::new(&dir).unwrap();
        let seq = wal
            .append(WalEntry::FileUpload {
                filename: "test.txt".to_string(),
                size: 1024,
            })
            .unwrap();
        assert_eq!(seq, 1);
        assert_eq!(wal.record_count(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_wal_multiple_appends() {
        let dir = test_wal_dir("multiple");
        let mut wal = WriteAheadLog::new(&dir).unwrap();
        wal.append(WalEntry::FileUpload {
            filename: "a.txt".to_string(),
            size: 100,
        })
        .unwrap();
        wal.append(WalEntry::FileDownload {
            filename: "a.txt".to_string(),
        })
        .unwrap();
        wal.append(WalEntry::SecurityBan {
            peer_id: "peer-1".to_string(),
            reason: "excess failures".to_string(),
        })
        .unwrap();
        assert_eq!(wal.record_count(), 3);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_wal_replay() {
        let dir = test_wal_dir("replay");
        let mut wal = WriteAheadLog::new(&dir).unwrap();
        wal.append(WalEntry::FileUpload {
            filename: "doc.pdf".to_string(),
            size: 5000,
        })
        .unwrap();
        wal.append(WalEntry::AiQuery {
            model: "test-model".to_string(),
            tokens: 256,
        })
        .unwrap();

        let mut replayed = Vec::new();
        let count = wal
            .replay(|record| {
                replayed.push(record.clone());
                Ok(())
            })
            .unwrap();
        assert_eq!(count, 2);
        assert_eq!(replayed.len(), 2);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_wal_truncate() {
        let dir = test_wal_dir("truncate");
        let mut wal = WriteAheadLog::new(&dir).unwrap();
        wal.append(WalEntry::Custom {
            action: "test".to_string(),
            details: "data".to_string(),
        })
        .unwrap();
        wal.truncate().unwrap();
        assert_eq!(wal.record_count(), 1);
        let count = wal.replay(|_| Ok(())).unwrap();
        assert_eq!(count, 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_wal_persistence_across_instances() {
        let dir = test_wal_dir("persistence");
        {
            let mut wal = WriteAheadLog::new(&dir).unwrap();
            wal.append(WalEntry::FileUpload {
                filename: "persistent.txt".to_string(),
                size: 2048,
            })
            .unwrap();
        }
        {
            let wal = WriteAheadLog::new(&dir).unwrap();
            assert_eq!(wal.record_count(), 1);
            let mut replayed = 0;
            wal.replay(|_| {
                replayed += 1;
                Ok(())
            })
            .unwrap();
            assert_eq!(replayed, 1);
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_wal_all_entry_types() {
        let dir = test_wal_dir("all_types");
        let mut wal = WriteAheadLog::new(&dir).unwrap();
        wal.append(WalEntry::FileUpload {
            filename: "f.txt".to_string(),
            size: 1,
        })
        .unwrap();
        wal.append(WalEntry::FileDownload {
            filename: "f.txt".to_string(),
        })
        .unwrap();
        wal.append(WalEntry::FileDelete {
            filename: "f.txt".to_string(),
        })
        .unwrap();
        wal.append(WalEntry::SecurityBan {
            peer_id: "p".to_string(),
            reason: "r".to_string(),
        })
        .unwrap();
        wal.append(WalEntry::SecurityUnban {
            peer_id: "p".to_string(),
        })
        .unwrap();
        wal.append(WalEntry::AiQuery {
            model: "m".to_string(),
            tokens: 1,
        })
        .unwrap();
        wal.append(WalEntry::ConfigChange {
            key: "k".to_string(),
            old_value: "o".to_string(),
            new_value: "n".to_string(),
        })
        .unwrap();
        wal.append(WalEntry::Snapshot {
            name: "snap1".to_string(),
        })
        .unwrap();
        wal.append(WalEntry::Custom {
            action: "a".to_string(),
            details: "d".to_string(),
        })
        .unwrap();

        let count = wal.replay(|_| Ok(())).unwrap();
        assert_eq!(count, 9);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn test_concurrent_wal_appends() {
        let dir = test_wal_dir("concurrent");
        let wal = std::sync::Arc::new(std::sync::Mutex::new(WriteAheadLog::new(&dir).unwrap()));
        let mut handles = vec![];

        for i in 0..50 {
            let w = wal.clone();
            handles.push(tokio::spawn(async move {
                let mut wal = w.lock().unwrap();
                wal.append(WalEntry::Custom {
                    action: "concurrent".to_string(),
                    details: format!("op-{}", i),
                })
                .unwrap();
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        let wal = wal.lock().unwrap();
        assert_eq!(wal.record_count(), 50);
        std::fs::remove_dir_all(&dir).ok();
    }
}
