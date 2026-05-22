# Oracle Cloud VM Setup - Always Free Tier

## Prerequisites
- Oracle Cloud account (Free Tier)
- Cloudflare account with domain
- Terraform installed locally (optional)

## Step 1: Create VM in Oracle Cloud Console

### Console Method (Easiest)

1. Go to https://cloud.oracle.com/
2. Navigate to **Compute > Instances**
3. Click **Create Instance**
4. Configure:
   - **Name**: `aetheris-ai-inference`
   - **Compartment**: Create `aetheris-ai` or use root
   - **Availability Domain**: Pick any with "Always Free Eligible"
   - **Image**: Ubuntu 22.04 (ARM64)
   - **Shape**: `VM.Standard.A1.Flex`
     - OCPUs: 4
     - Memory: 24 GB
   - **Networking**: Create new VCN or use existing
   - **Public IP**: None (tunnel only) or assign for initial setup
   - **SSH Key**: Upload your public key
5. Click **Create**

### Terraform Method (Recommended for reproducibility)

```bash
cd LLMVM

# Edit variables
cp terraform.tfvars.example terraform.tfvars
nano terraform.tfvars

# Initialize and apply
terraform init
terraform plan
terraform apply
```

## Step 2: SSH into VM

```bash
# If you assigned a public IP
ssh -i ~/.ssh/aetheris_ai_key ubuntu@<PUBLIC_IP>

# If no public IP, use OCI Bastion or cloud-init only
```

## Step 3: Run Setup Script

The cloud-init runs automatically on first boot. To verify:

```bash
# Check setup log
tail -f /var/log/aetheris-ai-setup.log

# Check services
systemctl status lmstudio
systemctl status cloudflared

# Test local API
curl http://127.0.0.1:1234/v1/models
```

## Step 4: Configure Cloudflare Tunnel

1. Go to https://one.dash.cloudflare.com/
2. Navigate to **Networks > Tunnels**
3. Click **Create Tunnel**
4. Choose **Cloudflared**
5. Name: `aetheris-ai-tunnel`
6. Copy the tunnel token
7. Add Public Hostname:
   - **Subdomain**: `ai`
   - **Domain**: your-domain.com
   - **Service**: HTTP
   - **URL**: `localhost:1234`
8. Under **Additional Application Settings > HTTP Headers**:
   - Add `Authorization` header requirement for API key protection

## Step 5: Verify Remote Access

From your local machine:

```bash
curl https://ai.your-domain.com/v1/models
```

Should return:

```json
{
  "data": [
    {"id": "microsoft/phi-4-reasoning-plus", ...},
    {"id": "nvidia/nemotron-3-nano-4b", ...}
  ]
}
```

## Step 6: Connect Aetheris Core

Update your Aetheris Core environment:

```env
# .env or compose.emulation.yaml
AI_ENDPOINT=https://ai.your-domain.com
AI_API_KEY=your-cloudflare-api-key
AI_MODEL=microsoft/phi-4-reasoning-plus
```

## Monitoring

```bash
# VM resource usage
htop

# LMStudio logs
journalctl -u lmstudio -f

# Cloudflare tunnel logs
journalctl -u cloudflared -f

# Memory pressure
free -h
cat /proc/swaps

# Disk usage
df -h
```

## Troubleshooting

### LMStudio won't start
```bash
# Check logs
journalctl -u lmstudio -n 50

# Manual start
su - aetheris -c "lms server start --port 1234"
```

### Model not loaded
```bash
su - aetheris -c "lms list"
su - aetheris -c "lms load microsoft/phi-4-reasoning-plus"
```

### Tunnel not working
```bash
systemctl status cloudflared
cloudflared tunnel info aetheris-ai-tunnel
```

### Out of memory
```bash
# Check swap
free -h

# Reduce model size or increase swap
sudo fallocate -l 32G /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile
```
