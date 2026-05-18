#!/bin/bash
# Connection Test Script for Oracle Cloud Instance

echo "🔧 Testing Oracle Cloud Instance Connectivity"
echo ""

# Get public IP from user
read -p "Enter the public IP address of your Oracle instance: " PUBLIC_IP

# Test SSH connection
echo "Testing SSH connection to $PUBLIC_IP..."
if ssh -i aetheris-ai-key -o ConnectTimeout=10 -o BatchMode=yes ubuntu@$PUBLIC_IP "echo 'SSH connection successful!'"; then
    echo "✅ SSH connection successful"
    
    # Upload setup files
    echo "Uploading setup files..."
    scp -i aetheris-ai-key setup.sh ubuntu@$PUBLIC_IP:~
    scp -i aetheris-ai-key free-tier-check.sh ubuntu@$PUBLIC_IP:~
    
    echo "✅ Files uploaded successfully"
    echo ""
    echo "Next steps on the server:"
    echo "1. chmod +x setup.sh free-tier-check.sh"
    echo "2. ./free-tier-check.sh"
    echo "3. export CLOUDFLARE_TUNNEL_TOKEN=\"your-token-here\""
    echo "4. ./setup.sh"
else
    echo "❌ SSH connection failed"
    echo "Check:"
    echo "- Instance is running"
    echo "- Public IP is correct"
    echo "- SSH key permissions (chmod 600 aetheris-ai-key)"
    echo "- Network connectivity"
fi