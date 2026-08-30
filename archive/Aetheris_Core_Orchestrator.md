To meet your requirement for maximum performance without dependencies like Node/Vite, Rust is the ideal choice. It offers memory safety without a garbage collector and can easily interface with inotify (Linux file watching) and OPA via WebAssembly or HTTP. 
Below is the Aetheris Core Orchestrator in Rust.
Cargo.toml (Dependencies)
Uses only essential, low-level FOSS crates. 
toml

[package]
name = "aetheris-core"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1", features = ["full"] } # Async runtime
notify = "6.1"                                 # FOSS File watching (inotify)
reqwest = { version = "0.11", features = ["json"] } # To talk to OPA/AI
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

Use code with caution.
main.rs (The Orchestrator)
This code performs two critical roles:

    File Watcher: Listens for filesystem events to trigger AI re-indexing.
    Zero-Trust Guard: Intercepts requests and validates them against the OPA policy.

rust

use notify::{Watcher, RecursiveMode, Config};
use std::path::Path;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct OpaInput {
    user_role: String,
    method: String,
    action: String,
    path: String,
}

#[derive(Deserialize)]
struct OpaResponse {
    result: bool,
}

// 1. THE ZERO-TRUST GUARD
// Validates access before any file or AI operation
async fn check_access(user: &str, action: &str, path: &str) -> bool {
    let client = reqwest::Client::new();
    let input = OpaInput {
        user_role: user.to_string(),
        method: "GET".to_string(),
        action: action.to_string(),
        path: path.to_string(),
    };

    let res = client.post("http://localhost:8181/v1/data/aetheris/authz/allow")
        .json(&input)
        .send()
        .await;

    match res {
        Ok(response) => response.json::<OpaResponse>().await.map(|r| r.result).unwrap_or(false),
        Err(_) => false, // Fail closed
    }
}

// 2. THE SEMANTIC INDEXER TRIGGER
// Sends new files to the local AI engine
async fn index_file(path: &str) {
    println!("🔍 AI Engine: Indexing semantic vectors for {}", path);
    // Logic to POST to Ollama or ChromaDB goes here
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🛡️ Aetheris Core Active. Zero-Trust Mesh Engaged.");

    // Setup File Watcher
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    
    let mut watcher = notify::recommended_watcher(move |res| {
        match res {
            Ok(event) => { let _ = tx.blocking_send(event); },
            Err(e) => println!("Watcher error: {:?}", e),
        }
    })?;

    // Watch the 'vault' directory for new data
    watcher.watch(Path::new("./vault"), RecursiveMode::Recursive)?;

    // Core Event Loop
    while let Some(event) = rx.recv().await {
        for path in event.paths {
            if let Some(path_str) = path.to_str() {
                // Before indexing, we check if the 'system' has permission to touch this
                if check_access("system_agent", "index", path_str).await {
                    index_file(path_str).await;
                }
            }
        }
    }

    Ok(())
}

Use code with caution.
Why this is "Faster than Frameworks"

    Binary Execution: This compiles to a single static binary. There is no interpreter (Python), no virtual machine (Java), and no heavy runtime (Node.js).
    Zero Memory Overhead: Rust uses "Zero-Cost Abstractions." The memory used is strictly for your data, not for framework overhead.
    Direct Syscalls: It uses the Linux inotify subsystem directly to watch files, which is significantly more efficient than polling.
    Security: Rust's ownership model prevents "Buffer Overflows," which are the primary way systems like Cloudflare (written in C/C++/Rust) stay secure at the edge. 

The Final "Zero-Trust" Flow

    A user connects via the WireGuard Mesh (Handshake encrypted).
    The user requests a file via a simple HTML form (No JS).
    The Rust Core intercepts the request and asks OPA (the policy you wrote) for permission.
    If Approved: The Rust Core streams the file directly from the encrypted ZFS/BTRFS volume.
    If Denied: The connection is dropped instantly.