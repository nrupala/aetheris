use std::time::Duration;

pub fn http_client() -> reqwest::Client {
    http_client_with_timeout(Duration::from_secs(30))
}

pub fn http_client_with_timeout(timeout: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .expect("Failed to create HTTP client")
}

pub fn model_timeout(model: &str) -> Duration {
    let model = model.to_lowercase();
    // Parse "b" suffix for billion-parameter models
    let param_b = model
        .split(|c: char| !c.is_alphanumeric())
        .filter_map(|part| {
            let num_part = part.trim_end_matches('b');
            if num_part.len() < part.len() {
                num_part.parse::<f64>().ok()
            } else {
                None
            }
        })
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let params_b = param_b.unwrap_or(0.0);

    if params_b >= 14.0 {
        Duration::from_secs(600) // 10 min for 14B+ on ARM cold start
    } else if params_b >= 7.0 {
        Duration::from_secs(300) // 5 min for 7-8B
    } else if params_b >= 3.0 {
        Duration::from_secs(180) // 3 min for 3B
    } else if params_b >= 1.0 {
        Duration::from_secs(120) // 2 min for 1.5B
    } else {
        // Fallback: check for size keywords in name
        if model.contains("large") {
            Duration::from_secs(300)
        } else if model.contains("mini") || model.contains("small") || model.contains("tiny") {
            Duration::from_secs(90)
        } else {
            Duration::from_secs(120)
        }
    }
}
