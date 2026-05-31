use axum::{
    body::Body,
    http::{Method, StatusCode, Uri},
    response::Response,
};
use reqwest::Client;
use std::time::Duration;

pub struct OrchestratorProxy {
    client: Client,
    base_url: String,
}

impl OrchestratorProxy {
    pub fn new(base_url: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .expect("Failed to create HTTP client"),
            base_url,
        }
    }

    pub async fn forward(
        &self,
        method: Method,
        uri: &Uri,
        body: axum::body::Bytes,
    ) -> Response<Body> {
        let path = uri.path();
        let target_path = path.strip_prefix("/").unwrap_or(path);
        let target_url = format!("{}/{}", self.base_url, target_path);

        let req_method = match method {
            Method::GET => reqwest::Method::GET,
            Method::POST => reqwest::Method::POST,
            Method::PUT => reqwest::Method::PUT,
            Method::DELETE => reqwest::Method::DELETE,
            Method::PATCH => reqwest::Method::PATCH,
            _ => reqwest::Method::GET,
        };

        let req = self
            .client
            .request(req_method, &target_url)
            .body(body.to_vec())
            .header("Content-Type", "application/json");

        match req.send().await {
            Ok(resp) => {
                let status_code =
                    StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
                let body_bytes = resp.bytes().await.unwrap_or_default();
                let mut response = Response::new(Body::from(body_bytes));
                *response.status_mut() = status_code;
                response
            }
            Err(e) => Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from(format!(
                    "{{\"error\": \"Orchestrator proxy failed: {}\"}}",
                    e
                )))
                .unwrap(),
        }
    }
}
