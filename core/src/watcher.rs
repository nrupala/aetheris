use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Instant, Duration};

const MAX_FAILURES: u32 = 5;
const BAN_DURATION: Duration = Duration::from_secs(3600);

pub struct SecurityWatcher {
    violations: Mutex<HashMap<String, (u32, Instant)>>,
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
            println!(
                "SECURITY: Peer ID banned for {} failures.",
                entry.0
            );
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
