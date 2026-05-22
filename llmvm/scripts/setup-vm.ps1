#!/usr/bin/env pwsh
# Aetheris LLMVM — VM Bootstrap Script (runs via SSH after deployment)
param(
    [Parameter(Mandatory)]
    [string]$HostIP,

    [string]$KeyPath = "$env:USERPROFILE\.ssh\llmvm_key",
    [string]$TunnelDomain = "nrupalakolkar.com"
)

$sshArgs = "-o StrictHostKeyChecking=no -i $KeyPath"

function Run-SSH {
    param([string]$Cmd)
    Write-Host "  > $Cmd" -ForegroundColor DarkGray
    ssh $sshArgs ubuntu@$HostIP $Cmd 2>&1
}

Write-Host "=== Aetheris LLMVM VM Bootstrap ===" -ForegroundColor Cyan
Write-Host "Target: $HostIP"
Write-Host ""

# Phase 1: System Update
Write-Host "[Phase 1/7] System update..." -ForegroundColor Yellow
Run-SSH "sudo apt-get update && sudo apt-get upgrade -y"

# Phase 2: Swap + Memory
Write-Host "`n[Phase 2/7] Configuring swap (16GB swap + 4GB ZRAM)..." -ForegroundColor Yellow
Run-SSH @"
sudo fallocate -l 16G /swapfile 2>/dev/null || sudo dd if=/dev/zero of=/swapfile bs=1M count=16384
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile
echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab
sudo sysctl -w vm.swappiness=10
echo 'vm.swappiness=10' | sudo tee -a /etc/sysctl.conf
"@

# Phase 3: Docker
Write-Host "`n[Phase 3/7] Installing Docker..." -ForegroundColor Yellow
Run-SSH @"
sudo apt-get install -y ca-certificates curl gnupg lsb-release
sudo install -m 0755 -d /etc/apt/keyrings
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo gpg --dearmor -o /etc/apt/keyrings/docker.gpg
sudo chmod a+r /etc/apt/keyrings/docker.gpg
echo "deb [arch=\$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu \$(lsb_release -cs) stable" | sudo tee /etc/apt/sources.list.d/docker.list
sudo apt-get update
sudo apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
sudo systemctl enable docker && sudo systemctl start docker
sudo usermod -aG docker ubuntu
"@

# Phase 4: Security
Write-Host "`n[Phase 4/7] Hardening security..." -ForegroundColor Yellow
Run-SSH @"
sudo apt-get install -y fail2ban ufw
sudo ufw default deny incoming
sudo ufw default allow outgoing
sudo ufw allow 22/tcp
sudo ufw --force enable
sudo systemctl enable fail2ban && sudo systemctl start fail2ban
sudo sed -i 's/#PermitRootLogin yes/PermitRootLogin no/' /etc/ssh/sshd_config
sudo sed -i 's/#PasswordAuthentication yes/PasswordAuthentication no/' /etc/ssh/sshd_config
sudo sed -i 's/#MaxAuthTries 6/MaxAuthTries 3/' /etc/ssh/sshd_config
sudo systemctl restart sshd
"@

# Phase 5: Directory Structure + Users
Write-Host "`n[Phase 5/7] Setting up directories and users..." -ForegroundColor Yellow
Run-SSH @"
sudo mkdir -p /opt/aetheris /var/lib/aetheris/models /var/lib/aetheris/rag /var/lib/aetheris/backups
sudo useradd -m -s /bin/bash aetheris 2>/dev/null || true
sudo useradd -m -s /bin/bash dev 2>/dev/null || true
sudo useradd -m -s /bin/bash monitor 2>/dev/null || true
sudo chown -R aetheris:aetheris /var/lib/aetheris
sudo usermod -aG docker aetheris
sudo usermod -aG docker dev
"@

# Phase 6: Cloudflare Tunnel
Write-Host "`n[Phase 6/7] Installing Cloudflare Tunnel..." -ForegroundColor Yellow
Run-SSH @"
curl -fsSL https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-arm64 -o /tmp/cloudflared
sudo mv /tmp/cloudflared /usr/local/bin/cloudflared
sudo chmod +x /usr/local/bin/cloudflared
sudo mkdir -p /etc/cloudflared
"@

# Phase 7: code-server
Write-Host "`n[Phase 7/7] Installing code-server..." -ForegroundColor Yellow
Run-SSH @"
curl -fsSL https://code-server.dev/install.sh | sh -s -- --method=standalone --prefix=/opt/code-server
"@

Write-Host "`n=== VM BOOTSTRAP COMPLETE ===" -ForegroundColor Green
Write-Host ""
Write-Host "VM is ready. Next:"
Write-Host "1. SSH: ssh -i $KeyPath ubuntu@$HostIP"
Write-Host "2. Deploy Docker containers: docker compose up -d"
Write-Host "3. Configure Cloudflare Tunnel for $TunnelDomain"
