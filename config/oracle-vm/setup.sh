#!/bin/bash
# Oracle Cloud VM - Aetheris Remote AI Inference Server (FREE TIER)
# Instance: VM.Standard.A1.Flex (4 OCPU, 24GB RAM - ARM Architecture)
# OS: Ubuntu 22.04 LTS ARM64
# Purpose: Serve LMStudio models securely via Cloudflare Tunnel
# Cost: 100% FREE within Oracle Cloud Free Tier

set -euo pipefail

# ============================================
# CONFIGURATION
# ============================================
LMSTUDIO_VERSION="latest"
CLOUDFLARE_TUNNEL_TOKEN="${CLOUDFLARE_TUNNEL_TOKEN:?Set CLOUDFLARE_TUNNEL_TOKEN env var}"
CLOUDFLARE_SUBDOMAIN="${CLOUDFLARE_SUBDOMAIN:-aetheris-ai}"
CLOUDFLARE_DOMAIN="${CLOUDFLARE_DOMAIN:-your-domain.com}"
AI_API_KEY="${AI_API_KEY:-$(openssl rand -hex 32)}"
MODEL="${1:-nvidia/nemotron-3-nano-4b}"

echo "===================================================="
echo "  Aetheris Remote AI Server - Oracle Cloud FREE TIER"
echo "  Instance: VM.Standard.A1.Flex (4OCPU/24GB ARM)"
echo "  Cost: $0.00/month (within free tier)"
echo "===================================================="

# ============================================
# 1. SYSTEM PREPARATION
# ============================================
echo "[1/7] Updating system..."
apt-get update && apt-get upgrade -y

# Install dependencies
apt-get install -y \
    curl \
    wget \
    htop \
    sysstat \
    net-tools \
    build-essential \
    libvulkan1 \
    vulkan-tools \
    zram-config

# ============================================
# 2. MEMORY OPTIMIZATION
# ============================================
echo "[2/7] Configuring memory optimization..."

# Create 8GB swap (A1.Flex has 24GB RAM, less swap needed)
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
# 3. LMSTUDIO SERVER INSTALLATION
# ============================================
echo "[3/7] Installing LMStudio Server..."

# Download LMStudio CLI for ARM64
curl -fsSL https://install.lmstudio.ai | bash -s -- --headless --arch arm64

# Add LMStudio to PATH
echo 'export PATH="$HOME/.cache/lm-studio/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc

# Verify installation
lms --version

# ============================================
# 4. MODEL DOWNLOAD & PRELOAD
# ============================================
echo "[4/7] Downloading model: $MODEL..."
lms download "$MODEL" 2>/dev/null || echo "Model already exists or downloading..."

# Pre-load model into memory
echo "Preloading model into memory..."
lms load "$MODEL"

# ============================================
# 5. LMSTUDIO SERVER CONFIGURATION
# ============================================
echo "[5/7] Starting LMStudio Server..."

# Start server on localhost only (Cloudflare tunnel will expose it)
# The API key is enforced at the Cloudflare level
nohup lms server start --port 1234 --host 127.0.0.1 > /var/log/lmstudio.log 2>&1 &

# Wait for server to be ready
echo "Waiting for LMStudio server..."
for i in {1..30}; do
    if curl -s http://127.0.0.1:1234/v1/models > /dev/null 2>&1; then
        echo "LMStudio server is ready!"
        break
    fi
    sleep 2
done

# Verify server
curl -s http://127.0.0.1:1234/v1/models | python3 -m json.tool

# ============================================
# 6. CLOUDFLARE TUNNEL
# ============================================
echo "[6/7] Setting up Cloudflare Tunnel..."

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
# 7. SECURITY HARDENING
# ============================================
echo "[7/7] Hardening security..."

# Configure firewall (only allow Cloudflare IPs)
ufw default deny incoming
ufw allow OpenSSH
ufw allow from 173.245.48.0/20 to any port 1234 proto tcp
ufw allow from 103.21.244.0/22 to any port 1234 proto tcp
ufw allow from 103.22.200.0/22 to any port 1234 proto tcp
ufw allow from 103.31.4.0/22 to any port 1234 proto tcp
ufw allow from 141.101.64.0/18 to any port 1234 proto tcp
ufw allow from 108.162.192.0/18 to any port 1234 proto tcp
ufw allow from 190.93.240.0/20 to any port 1234 proto tcp
ufw allow from 188.114.96.0/20 to any port 1234 proto tcp
ufw allow from 197.234.240.0/22 to any port 1234 proto tcp
ufw allow from 198.41.128.0/17 to any port 1234 proto tcp
ufw allow from 162.158.0.0/15 to any port 1234 proto tcp
ufw allow from 104.16.0.0/13 to any port 1234 proto tcp
ufw allow from 104.24.0.0/14 to any port 1234 proto tcp
ufw allow from 172.64.0.0/13 to any port 1234 proto tcp
ufw allow from 131.0.72.0/22 to any port 1234 proto tcp
ufw enable

# Create API key file for reference
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
echo "  tail -f /var/log/lmstudio.log"
echo "  systemctl status cloudflared"
echo ""
