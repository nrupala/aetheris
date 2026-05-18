# PowerShell Connection Test for Oracle Cloud Instance

Write-Host "🔧 Testing Oracle Cloud Instance Connectivity" -ForegroundColor Green
Write-Host ""

# Get public IP from user
$PUBLIC_IP = Read-Host "Enter the public IP address of your Oracle instance"

# Test SSH connection
Write-Host "Testing SSH connection to $PUBLIC_IP..." -ForegroundColor Yellow

try {
    $result = ssh -i aetheris-ai-key -o ConnectTimeout=10 -o BatchMode=yes ubuntu@$PUBLIC_IP "echo 'SSH connection successful!'" 2>$null
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✅ SSH connection successful" -ForegroundColor Green
        
        # Upload setup files
        Write-Host "Uploading setup files..." -ForegroundColor Yellow
        
        scp -i aetheris-ai-key setup.sh ubuntu@${PUBLIC_IP}:~
        scp -i aetheris-ai-key free-tier-check.sh ubuntu@${PUBLIC_IP}:~
        
        Write-Host "✅ Files uploaded successfully" -ForegroundColor Green
        Write-Host ""
        Write-Host "Next steps on the server:" -ForegroundColor Cyan
        Write-Host "1. chmod +x setup.sh free-tier-check.sh"
        Write-Host "2. ./free-tier-check.sh" 
        Write-Host "3. export CLOUDFLARE_TUNNEL_TOKEN='your-token-here'"
        Write-Host "4. ./setup.sh"
    } else {
        Write-Host "❌ SSH connection failed" -ForegroundColor Red
        Write-Host "Check:" -ForegroundColor Yellow
        Write-Host "- Instance is running"
        Write-Host "- Public IP is correct" 
        Write-Host "- SSH key permissions"
        Write-Host "- Network connectivity"
    }
}
catch {
    Write-Host "❌ Error testing connection: $_" -ForegroundColor Red
}