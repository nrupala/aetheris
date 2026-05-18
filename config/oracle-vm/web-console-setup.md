# Web Console Setup Commands

## Quick Login Command
```bash
# Open Oracle Cloud Montreal console
start https://console.ca-montreal-1.oraclecloud.com
```

## Instance Configuration Summary

### Basic Details
- **Name**: `aetheris-ai-server`
- **Region**: `Canada Southeast (Montreal)`
- **Availability Domain**: `AD-1`
- **Capacity**: `On-demand`

### Image 
- **OS**: `Canonical Ubuntu`
- **Version**: `22.04 LTS Minimal`
- **Architecture**: `ARM64 (aarch64)`

### Shape
- **Shape series**: `VM.Standard.A1.Flex`
- **OCPU count**: `4`
- **Memory**: `24 GB`

### Networking
- **VCN**: `Create new virtual cloud network`
- **Subnet**: `Create new public subnet`
- **Public IP**: `Assign a public IPv4 address`

### SSH Keys
- **Key type**: `Generate a key pair for me`
- **Key name**: `aetheris-ai-key`
- **IMPORTANT**: Download and save the private key!

### Boot Volume
- **Size**: `100 GB`
- **Performance**: `Balanced`
- **Encryption**: `Encrypt using Oracle-managed keys`

## After Creation

1. **Note the public IP address**
2. **Test SSH connection**:
   ```bash
   ssh -i aetheris-ai-key ubuntu@<PUBLIC_IP>
   ```

3. **Upload and run setup**:
   ```bash
   # From your local machine:
   scp -i aetheris-ai-key setup.sh ubuntu@<PUBLIC_IP>:~
   scp -i aetheris-ai-key free-tier-check.sh ubuntu@<PUBLIC_IP>:~
   
   # On the server:
   chmod +x setup.sh free-tier-check.sh
   ./free-tier-check.sh
   export CLOUDFLARE_TUNNEL_TOKEN="your-actual-token"
   ./setup.sh
   ```