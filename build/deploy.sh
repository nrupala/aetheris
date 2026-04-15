#!/bin/bash
# Aetheris Build & Deploy Script
set -e

BUILD_DIR="/opt/aetheris"
echo "========================================="
echo "AETHERIS BUILD & DEPLOY"
echo "========================================="

# Step 1: Copy files to /opt/aetheris
echo "[1/6] Copying files to $BUILD_DIR..."
if [ ! -d "$BUILD_DIR" ]; then
    sudo mkdir -p "$BUILD_DIR"
fi

# Copy from current build directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
sudo cp -r "$SCRIPT_DIR"/* "$BUILD_DIR/" 2>/dev/null || cp -r "$SCRIPT_DIR"/* "$BUILD_DIR/"

# Make scripts executable
echo "[2/6] Setting permissions..."
chmod +x "$BUILD_DIR/scripts/*.sh" 2>/dev/null || true
chmod +x "$BUILD_DIR/tests/*.sh" 2>/dev/null || true

# Step 2: Install Rust
echo "[3/6] Checking Rust..."
if ! command -v cargo >/dev/null 2>&1; then
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

# Step 3: Build Rust binary
echo "[4/6] Building Rust binary..."
cd "$BUILD_DIR/core"
rustup target add x86_64-unknown-linux-musl 2>/dev/null || true
cargo build --release --target x86_64-unknown-linux-musl 2>&1 | tail -5

# Step 4: Build Docker images
echo "[5/6] Building Docker containers..."
cd "$BUILD_DIR"
docker compose build

# Step 5: Pull AI models (optional, takes time)
echo "[6/6] Setup complete!"
echo ""
echo "Next steps:"
echo "  1. Configure ZFS: sudo $BUILD_DIR/scripts/vault_setup.sh"
echo "  2. Start services: cd $BUILD_DIR && docker compose up -d"
echo "  3. Pull AI models: docker exec aetheris_ai ollama pull nomic-embed-text"
echo "  4. Run UAT: cd $BUILD_DIR/tests && bash run_uat.sh"
