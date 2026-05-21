use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const MAX_FAILURES: u32 = 5;
const BAN_DURATION: Duration = Duration::from_secs(3600);

pub struct SecurityWatcher {
    violations: Mutex<HashMap<String, (u32, Instant)>>,
}

impl Default for SecurityWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl SecurityWatcher {
    pub fn new() -> Self {
        Self {
            violations: Mutex::new(HashMap::new()),
        }
    }

    pub fn record_failure(&self, peer_id: String) {
        let mut map = self.violations.lock().unwrap_or_else(|e| e.into_inner());
        let entry = map.entry(peer_id).or_insert((0, Instant::now()));
        entry.0 += 1;
        entry.1 = Instant::now();

        if entry.0 >= MAX_FAILURES {
            println!("SECURITY: Peer ID banned for {} failures.", entry.0);
        }
    }

    pub fn is_banned(&self, peer_id: &str) -> bool {
        let map = self.violations.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((count, last_seen)) = map.get(peer_id) {
            if *count >= MAX_FAILURES && last_seen.elapsed() < BAN_DURATION {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_watcher_is_empty() {
        let watcher = SecurityWatcher::new();
        assert!(!watcher.is_banned("peer-1"));
    }

    #[test]
    fn test_single_failure_does_not_ban() {
        let watcher = SecurityWatcher::new();
        watcher.record_failure("peer-1".to_string());
        assert!(!watcher.is_banned("peer-1"));
    }

    #[test]
    fn test_five_failures_bans_peer() {
        let watcher = SecurityWatcher::new();
        for _ in 0..5 {
            watcher.record_failure("peer-1".to_string());
        }
        assert!(watcher.is_banned("peer-1"));
    }

    #[test]
    fn test_four_failures_does_not_ban() {
        let watcher = SecurityWatcher::new();
        for _ in 0..4 {
            watcher.record_failure("peer-1".to_string());
        }
        assert!(!watcher.is_banned("peer-1"));
    }

    #[test]
    fn test_different_peers_independent() {
        let watcher = SecurityWatcher::new();
        for _ in 0..5 {
            watcher.record_failure("peer-1".to_string());
        }
        watcher.record_failure("peer-2".to_string());
        assert!(watcher.is_banned("peer-1"));
        assert!(!watcher.is_banned("peer-2"));
    }

    #[test]
    fn test_unknown_peer_not_banned() {
        let watcher = SecurityWatcher::new();
        assert!(!watcher.is_banned("nonexistent"));
    }

    #[test]
    fn test_ban_persists_after_additional_failures() {
        let watcher = SecurityWatcher::new();
        for _ in 0..10 {
            watcher.record_failure("peer-1".to_string());
        }
        assert!(watcher.is_banned("peer-1"));
    }

    #[test]
    fn test_empty_peer_id_handled() {
        let watcher = SecurityWatcher::new();
        for _ in 0..5 {
            watcher.record_failure("".to_string());
        }
        assert!(watcher.is_banned(""));
    }

    #[tokio::test]
    async fn test_concurrent_failures() {
        let watcher = Arc::new(SecurityWatcher::new());
        let mut handles = vec![];

        for _ in 0..10 {
            let w = watcher.clone();
            handles.push(tokio::spawn(async move {
                w.record_failure("concurrent-peer".to_string());
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        assert!(watcher.is_banned("concurrent-peer"));
    }

    use std::sync::Arc;
}
