use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::File;
use tokio_util::io::ReaderStream;

#[derive(Clone)]
pub struct SyncState {
    pub vault_path: PathBuf,
}

#[allow(dead_code)]
pub async fn download(
    State(state): State<Arc<SyncState>>,
    Path(filename): Path<String>,
) -> impl IntoResponse {
    let path = state.vault_path.join(&filename);
    if !path.starts_with(&state.vault_path) {
        return (StatusCode::FORBIDDEN, "Access Denied").into_response();
    }
    match File::open(&path).await {
        Ok(file) => {
            let stream = ReaderStream::new(file);
            let body = Body::from_stream(stream);
            Response::builder()
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .header(header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", filename))
                .body(body)
                .unwrap_or_else(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Stream Error").into_response())
        }
        Err(_) => (StatusCode::NOT_FOUND, "File not found").into_response(),
    }
}

#[allow(dead_code)]
pub async fn upload(
    State(state): State<Arc<SyncState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    while let Ok(Some(field)) = multipart.next_field().await {
        let filename = field.file_name().unwrap_or("unnamed").to_string();
        let data = match field.bytes().await {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };
        let path = state.vault_path.join(&filename);
        if tokio::fs::write(&path, data).await.is_ok() {
            return (StatusCode::OK, format!("Uploaded {}", filename)).into_response();
        }
    }
    (StatusCode::BAD_REQUEST, "Upload failed").into_response()
}

#[allow(dead_code)]
pub fn sync_router(vault_path: PathBuf) -> Router {
    let state = Arc::new(SyncState { vault_path });
    Router::new()
        .route("/download/{*filename}", get(download))
        .route("/upload", post(upload))
        .with_state(state)
}
