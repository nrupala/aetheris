use lazy_static::lazy_static;
use prometheus::{opts, register_counter, register_gauge, Counter, Gauge};

lazy_static! {
    pub static ref SECURITY_VIOLATIONS: Counter = register_counter!(opts!(
        "aetheris_security_violations_total",
        "Total blocked peer requests."
    ))
    .unwrap();
    pub static ref VAULT_USAGE_BYTES: Gauge = register_gauge!(opts!(
        "aetheris_vault_usage_bytes",
        "Current encrypted storage usage."
    ))
    .unwrap();
    pub static ref FILES_INDEXED: Counter = register_counter!(opts!(
        "aetheris_files_indexed_total",
        "Total files indexed."
    ))
    .unwrap();
    pub static ref SEARCH_QUERIES: Counter = register_counter!(opts!(
        "aetheris_search_queries_total",
        "Total semantic queries."
    ))
    .unwrap();
}

pub fn metrics_handler() -> String {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();
    let mut buffer = Vec::new();
    encoder.encode(&prometheus::gather(), &mut buffer).ok();
    String::from_utf8(buffer).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_handler_returns_string() {
        // Force lazy_static initialization
        let _ = SEARCH_QUERIES.get();
        let output = metrics_handler();
        assert!(output.contains("# HELP"));
        assert!(output.contains("# TYPE"));
    }

    #[test]
    fn test_metrics_handler_contains_registered_metrics() {
        let _ = SEARCH_QUERIES.get();
        let _ = FILES_INDEXED.get();
        let _ = SECURITY_VIOLATIONS.get();
        let _ = VAULT_USAGE_BYTES.get();
        let output = metrics_handler();
        assert!(output.contains("aetheris_security_violations_total"));
        assert!(output.contains("aetheris_vault_usage_bytes"));
        assert!(output.contains("aetheris_search_queries_total"));
    }

    #[test]
    fn test_gauge_set_and_get() {
        VAULT_USAGE_BYTES.set(2048.0);
        assert_eq!(VAULT_USAGE_BYTES.get(), 2048.0);
    }

    #[test]
    fn test_gauge_increment() {
        VAULT_USAGE_BYTES.set(5000.0);
        let before = VAULT_USAGE_BYTES.get();
        VAULT_USAGE_BYTES.inc();
        let after = VAULT_USAGE_BYTES.get();
        assert_eq!(after, before + 1.0);
    }

    #[test]
    fn test_counter_increment() {
        let before = SECURITY_VIOLATIONS.get();
        SECURITY_VIOLATIONS.inc();
        let after = SECURITY_VIOLATIONS.get();
        assert_eq!(after, before + 1.0);
    }

    #[test]
    fn test_metrics_handler_is_valid_prometheus_format() {
        let output = metrics_handler();
        for line in output.lines() {
            if line.starts_with("# HELP") || line.starts_with("# TYPE") || line.is_empty() {
                continue;
            }
            assert!(line.contains(" "), "Metric line missing value: {}", line);
        }
    }

    #[tokio::test]
    async fn test_concurrent_metric_updates() {
        let mut handles = vec![];
        for _ in 0..100 {
            handles.push(tokio::spawn(async {
                SEARCH_QUERIES.inc();
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
    }
}
