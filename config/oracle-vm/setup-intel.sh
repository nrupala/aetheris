#!/bin/bash
# Oracle Cloud VM - Aetheris Remote AI Inference Server (Intel/PAYG)
# Instance: VM.Standard3.Flex (1 OCPU, 16GB RAM - x86_64 Architecture)
# OS: Ubuntu 24.04 Minimal
# Purpose: Serve Ollama models securely via Cloudflare Tunnel
# Cost: PAYG (Covered by $300 credits)

set -euo pipefail

# ============================================
# CONFIGURATION
# ============================================
OLLAMA_VERSION="latest"
CLOUDFLARE_TUNNEL_TOKEN="${CLOUDFLARE_TUNNEL_TOKEN:?Set CLOUDFLARE_TUNNEL_TOKEN env var}"
CLOUDFLARE_SUBDOMAIN="${CLOUDFLARE_SUBDOMAIN:-aetheris-ai}"
CLOUDFLARE_DOMAIN="${CLOUDFLARE_DOMAIN:-your-domain.com}"
AI_API_KEY="${AI_API_KEY:-$(openssl rand -hex 32)}"
MODEL="${1:-qwen2.5:7b}"

echo "===================================================="
echo "  Aetheris Remote AI Server - Oracle Cloud Intel"
echo "  Instance: VM.Standard3.Flex (1OCPU/16GB x86_64)"
echo "===================================================="

# ============================================
# 1. SYSTEM PREPARATION
# ============================================
echo "[1/6] Updating system..."
apt-get update && apt-get upgrade -y

# Install dependencies
apt-get install -y \
    curl \
    wget \
    htop \
    sysstat \
    net-tools \
    build-essential \
    zram-config \
    jq

# ============================================
# 2. MEMORY OPTIMIZATION
# ============================================
echo "[2/6] Configuring memory optimization..."

# Create 8GB swap
fallocate -l 8G /swapfile
chmod 600 /swapfile
mkswap /swapfile
swapon /swapfile
echo '/swapfile none swap sw 0 0' >> /etc/fstab

# Configure ZRAM for additional compressed memory
echo "vm.swappiness=10" >> /etc/sysctl.conf
echo "vm.vfs_cache_pressure=50" >> /etc/sysctl.conf
sysctl -p

# ============================================
# 3. OLLAMA INSTALLATION
# ============================================
echo "[3/6] Installing Ollama..."
curl -fsSL https://ollama.com/install.sh | sh

# Add Ollama to PATH if needed (usually installed in /usr/local/bin)
ollama --version

# Configure Ollama to listen on localhost only
echo "[Service]" > /etc/systemd/system/ollama.service.d/override.conf
echo "Environment='OLLAMA_HOST=127.0.0.1:11434'" >> /etc/systemd/system/ollama.service.d/override.conf
systemctl daemon-reload
systemctl restart ollama

# Download model
echo "[4/6] Downloading model: $MODEL..."
ollama pull "$MODEL"

# ============================================
# 5. CLOUDFLARE TUNNEL
# ============================================
echo "[5/6] Setting up Cloudflare Tunnel..."

# Install cloudflared
curl -fsSL https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64 \
    -o /usr/local/bin/cloudflared
chmod +x /usr/local/bin/cloudflared
cloudflared --version

# Create systemd service for cloudflared
cat > /etc/systemd/system/cloudflared.service << EOF
[Unit]
Description=Cloudflare Tunnel
After=network.target

[Service]
Type=simple
User=root
ExecStart=/usr/local/bin/cloudflared tunnel --no-autoupdate run --token $CLOUDFLARE_TUNNEL_TOKEN
Restart=always
RestartSec=5
LimitNOFILE=50000

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable cloudflared
systemctl start cloudflared

echo ""
echo "Cloudflare Tunnel Status:"
systemctl status cloudflared --no-pager

# ============================================
# 6. SECURITY HARDENING
# ============================================
echo "[6/6] Hardening security..."

# Configure firewall
ufw default deny incoming
ufw allow OpenSSH
ufw allow from 173.245.48.0/20 to any port 11434 proto tcp
ufw allow from 103.21.244.0/22 to any port 11434 proto tcp
ufw allow from 103.22.200.0/22 to any port 11434 proto tcp
ufw allow from 103.31.4.0/22 to any port 11434 proto tcp
ufw allow from 141.101.64.0/18 to any port 11434 proto tcp
ufw allow from 108.162.192.0/18 to any port 11434 proto tcp
ufw allow from 190.93.240.0/20 to any port 11434 proto tcp
ufw allow from 188.114.96.0/20 to any port 11434 proto tcp
ufw allow from 197.234.240.0/22 to any port 11434 proto tcp
ufw allow from 198.41.128.0/17 to any port 11434 proto tcp
ufw allow from 162.158.0.0/15 to any port 11434 proto tcp
ufw allow from 104.16.0.0/13 to any port 11434 proto tcp
ufw allow from 104.24.0.0/14 to any port 11434 proto tcp
ufw allow from 172.64.0.0/13 to any port 11434 proto tcp
ufw allow from 131.0.72.0/22 to any port 11434 proto tcp
ufw enable

# Create API key file for reference
mkdir -p /etc/aetheris
echo "AI_API_KEY=$AI_API_KEY" > /etc/aetheris/.ai_credentials
chmod 600 /etc/aetheris/.ai_credentials

# ============================================
# VERIFICATION
# ============================================
echo ""
echo "===================================================="
echo "  Aetheris Remote AI Server - Setup Complete"
echo "===================================================="
echo ""
echo "Endpoint: https://$CLOUDFLARE_SUBDOMAIN.$CLOUDFLARE_DOMAIN"
echo "API Key:  $AI_API_KEY"
echo "Model:    $MODEL"
echo ""
echo "Test from Aetheris Core:"
echo "  curl -X POST https://$CLOUDFLARE_SUBDOMAIN.$CLOUDFLARE_DOMAIN/v1/chat/completions \\"
echo "    -H 'Authorization: Bearer $AI_API_KEY' \\"
echo "    -H 'Content-Type: application/json' \\"
echo "    -d '{\"model\":\"$MODEL\",\"messages\":[{\"role\":\"user\",\"content\":\"Hello\"}]}'"
echo ""
echo "Monitor logs:"
echo "  journalctl -u ollama -f"
echo "  systemctl status cloudflared"
echo ""