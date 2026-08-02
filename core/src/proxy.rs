use axum::{
    body::{Body, Bytes},
    http::{header, Method, StatusCode, Uri},
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

/// OpenAI-compatible reverse proxy to the local AI backend (Ollama).
///
/// The core exposes `/v1/*` so the `ai` web panel can talk a pure
/// OpenAI-compatible API. Requests are forwarded to `{AI_ENDPOINT}/v1/...`
/// (e.g. `http://127.0.0.1:11434/v1/chat/completions`) and the upstream body is
/// streamed back, so `stream: true` (SSE / ndjson) works through the core.
pub struct OpenAiProxy {
    client: Client,
    base_url: String,
}

impl OpenAiProxy {
    pub fn new(base_url: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(600))
                .build()
                .expect("Failed to create HTTP client"),
            base_url,
        }
    }

    pub async fn forward(
        &self,
        method: Method,
        path: &str,
        query: Option<&str>,
        content_type: Option<&str>,
        accept: Option<&str>,
        body: Bytes,
    ) -> Response<Body> {
        let target_url = format!(
            "{}{}{}",
            self.base_url,
            path,
            query.map(|q| format!("?{}", q)).unwrap_or_default()
        );
        let req_method = match method {
            Method::GET => reqwest::Method::GET,
            Method::POST => reqwest::Method::POST,
            Method::PUT => reqwest::Method::PUT,
            Method::DELETE => reqwest::Method::DELETE,
            _ => reqwest::Method::GET,
        };

        let mut rb = self
            .client
            .request(req_method, &target_url)
            .body(body.to_vec());
        if let Some(ct) = content_type {
            rb = rb.header("Content-Type", ct);
        }
        if let Some(ac) = accept {
            rb = rb.header("Accept", ac);
        }

        match rb.send().await {
            Ok(resp) => {
                let status_code =
                    StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
                let content_type = resp
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| header::HeaderValue::from_str(v).ok());
                let mut response = Response::new(Body::from_stream(resp.bytes_stream()));
                *response.status_mut() = status_code;
                if let Some(ct) = content_type {
                    response.headers_mut().insert(header::CONTENT_TYPE, ct);
                }
                response
            }
            Err(e) => Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from(format!(
                    "{{\"error\": \"OpenAI proxy failed: {}\"}}",
                    e
                )))
                .unwrap(),
        }
    }
}
