#!/bin/bash
set -e

echo "=== Installing Rust ==="
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustc --version
cargo --version

echo ""
echo "=== Installing Python packages ==="
pip3 install --upgrade pip
pip3 install requests numpy sqlite3 tiktoken fastapi uvicorn httpx

echo ""
echo "=== Installing opencode (if available) ==="
# opencode is a CLI tool - install from GitHub if binary available
if command -v curl &> /dev/null; then
    echo "opencode CLI setup..."
    # Check if opencode has a release binary
    LATEST=$(curl -s https://api.github.com/repos/opencode-ai/opencode/releases/latest | grep tag_name | cut -d'"' -f4)
    if [ -n "$LATEST" ]; then
        curl -sSL "https://github.com/opencode-ai/opencode/releases/download/${LATEST}/opencode-linux-amd64" -o /usr/local/bin/opencode
        chmod +x /usr/local/bin/opencode
        echo "opencode installed: $(opencode --version 2>/dev/null || echo 'binary installed')"
    fi
fi

echo ""
echo "=== Setup complete ==="
echo "Available tools:"
echo "  rustc: $(rustc --version)"
echo "  cargo: $(cargo --version)"
echo "  python3: $(python3 --version)"
echo "  pip3: $(pip3 --version)"
echo "  code-server: ready at :8443"
