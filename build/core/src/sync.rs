use axum::{
    extract::{Multipart, Path, State},
    body::Body,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use tokio::fs::File;
use tokio_util::io::ReaderStream;
use std::path::PathBuf;
use std::sync::Arc;

struct AppState {
    vault_path: PathBuf,
}

async fn download_file(
    State(state): State<Arc<AppState>>,
    Path(filename): Path<String>,
) -> impl IntoResponse {
    let path = state.vault_path.join(&filename);

    if !path.starts_with(&state.vault_path) {
        return (StatusCode::FORBIDDEN, "Access Denied").into_response();
    }

    let file = match File::open(&path).await {
        Ok(file) => file,
        Err(_) => return (StatusCode::NOT_FOUND, "File not found").into_response(),
    };

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", filename))
        .body(body)
        .unwrap()
}

async fn upload_file(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.file_name().unwrap_or("unknown").to_string();
        let path = state.vault_path.join(&name);

        let data = field.bytes().await.unwrap();
        tokio::fs::write(&path, data).await.unwrap();
        
        println!("Synchronized: {}", name);
    }
    StatusCode::OK
}

#[tokio::main]
async fn main() {
    let state = Arc::new(AppState {
        vault_path: PathBuf::from("./vault"),
    });

    let app = Router::new()
        .route("/download/:filename", get(download_file))
        .route("/upload", post(upload_file))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
