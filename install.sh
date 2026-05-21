#!/usr/bin/env bash
set -euo pipefail

# Aetheris — Platform-Neutral Installer
# Supports: Linux (x86_64, aarch64), macOS (x86_64, arm64), Windows (via WSL/Git Bash)

GITHUB_REPO="nrupala/aetheris"
INSTALL_DIR="${AETHERIS_INSTALL:-/usr/local/bin}"
VERSION="${AETHERIS_VERSION:-latest}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info()  { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

detect_platform() {
    local os arch
    os="$(uname -s | tr '[:upper:]' '[:lower:]')"
    arch="$(uname -m)"

    case "$os" in
        linux)  os="linux" ;;
        darwin) os="darwin" ;;
        mingw*|cygwin*|msys*) os="windows" ;;
        *) log_error "Unsupported OS: $os"; exit 1 ;;
    esac

    case "$arch" in
        x86_64|amd64)  arch="x86_64" ;;
        aarch64|arm64) arch="aarch64" ;;
        *) log_error "Unsupported architecture: $arch"; exit 1 ;;
    esac

    echo "$os $arch"
}

resolve_version() {
    if [ "$VERSION" = "latest" ]; then
        VERSION="$(curl -fsSL "https://api.github.com/repos/${GITHUB_REPO}/releases/latest" | grep '"tag_name"' | head -1 | sed 's/.*"v\([^"]*\)".*/\1/')"
    fi
    log_info "Installing Aetheris v${VERSION}"
}

download_binary() {
    local os="$1" arch="$2"
    local binary_name="aetheris-core"
    local download_url

    if [ "$os" = "windows" ]; then
        binary_name="aetheris-core.exe"
    fi

    # Try GitHub releases first
    download_url="https://github.com/${GITHUB_REPO}/releases/download/v${VERSION}/${binary_name}-${arch}-${os}"

    if curl -fsSL --head "$download_url" > /dev/null 2>&1; then
        log_info "Downloading pre-built binary..."
        curl -fsSL -o "$INSTALL_DIR/${binary_name}" "$download_url"
        chmod +x "$INSTALL_DIR/${binary_name}"
        log_info "Binary installed to ${INSTALL_DIR}/${binary_name}"
        return 0
    fi

    log_warn "Pre-built binary not found. Building from source..."
    build_from_source
}

build_from_source() {
    local binary_name="aetheris-core"

    if ! command -v cargo &> /dev/null; then
        log_info "Installing Rust toolchain..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
    fi

    log_info "Cloning repository..."
    local tmp_dir
    tmp_dir="$(mktemp -d)"
    git clone --depth 1 --branch "v${VERSION}" "https://github.com/${GITHUB_REPO}.git" "$tmp_dir/aetheris" 2>/dev/null || \
    git clone --depth 1 "https://github.com/${GITHUB_REPO}.git" "$tmp_dir/aetheris"

    log_info "Building Aetheris Core..."
    (cd "$tmp_dir/aetheris/core" && cargo build --release)

    log_info "Installing binary..."
    cp "$tmp_dir/aetheris/core/target/release/${binary_name}" "$INSTALL_DIR/${binary_name}"
    chmod +x "$INSTALL_DIR/${binary_name}"
    rm -rf "$tmp_dir"
    log_info "Built and installed to ${INSTALL_DIR}/${binary_name}"
}

install_compose() {
    log_info "Setting up Docker Compose..."

    if ! command -v docker &> /dev/null; then
        log_warn "Docker not found. Install Docker: https://docs.docker.com/get-docker/"
        return
    fi

    local repo_dir="${AETHERIS_REPO:-$HOME/aetheris}"
    if [ ! -d "$repo_dir" ]; then
        log_info "Cloning repository to ${repo_dir}..."
        git clone "https://github.com/${GITHUB_REPO}.git" "$repo_dir"
    fi

    log_info "Starting services..."
    (cd "$repo_dir" && docker compose up -d)
    log_info "Services started. Check status: curl http://localhost:8080/status"
}

verify_install() {
    local binary_name="aetheris-core"
    if [ "$(uname -s | tr '[:upper:]' '[:lower:]')" = "windows" ] || [[ "$OSTYPE" == "msys"* ]]; then
        binary_name="aetheris-core.exe"
    fi

    if command -v "$binary_name" &> /dev/null || [ -x "$INSTALL_DIR/${binary_name}" ]; then
        log_info "Verification: $($INSTALL_DIR/${binary_name} --version 2>&1 || echo 'binary present')"
        log_info "Aetheris installed successfully!"
    else
        log_error "Installation verification failed"
        exit 1
    fi
}

main() {
    log_info "Aetheris Installer"
    log_info "=================="

    local platform
    platform="$(detect_platform)"
    local os arch
    os="${platform%% *}"
    arch="${platform##* }"

    log_info "Detected: ${os}/${arch}"

    resolve_version
    download_binary "$os" "$arch"
    verify_install

    if command -v docker &> /dev/null; then
        echo ""
        log_info "Docker detected. Run 'cd aetheris && docker compose up -d' to start services."
    fi

    echo ""
    log_info "Documentation: https://github.com/${GITHUB_REPO}"
}

main "$@"
