use prometheus::{opts, register_counter, register_gauge, Counter, Gauge};
use lazy_static::lazy_static;

lazy_static! {
    pub static ref SECURITY_VIOLATIONS: Counter = register_counter!(
        opts!("aetheris_security_violations_total", "Total blocked peer requests.")
    ).unwrap();

    pub static ref VAULT_USAGE_BYTES: Gauge = register_gauge!(
        opts!("aetheris_vault_usage_bytes", "Current encrypted storage usage.")
    ).unwrap();

    pub static ref FILES_INDEXED: Counter = register_counter!(
        opts!("aetheris_files_indexed_total", "Total files indexed.")
    ).unwrap();

    pub static ref SEARCH_QUERIES: Counter = register_counter!(
        opts!("aetheris_search_queries_total", "Total semantic queries.")
    ).unwrap();
}

pub fn metrics_handler() -> String {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();
    let mut buffer = Vec::new();
    encoder.encode(&prometheus::gather(), &mut buffer).ok();
    String::from_utf8(buffer).unwrap_or_default()
}
