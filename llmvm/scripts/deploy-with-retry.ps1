#!/usr/bin/env pwsh
# Aetheris LLMVM — Terraform Apply with Retry
# Handles "Out of host capacity" errors with exponential backoff

param(
    [int]$MaxRetries = 10,
    [int]$InitialDelaySec = 30
)

$tf = "C:\Users\HomeUser\AppData\Local\Microsoft\WinGet\Packages\Hashicorp.Terraform_Microsoft.Winget.Source_8wekyb3d8bbwe\terraform.exe"
$workDir = "C:\Users\HomeUser\Downloads\LLMVM"

Write-Host "=== Aetheris LLMVM Deployment ===" -ForegroundColor Cyan
Write-Host "Max retries: $MaxRetries | Initial delay: ${InitialDelaySec}s" -ForegroundColor Cyan
Write-Host ""

$attempt = 0
$delay = $InitialDelaySec

while ($attempt -lt $MaxRetries) {
    $attempt++
    Write-Host "[Attempt $attempt/$MaxRetries] Applying..." -ForegroundColor Yellow
    
    Set-Location $workDir
    $output = & $tf apply -auto-approve 2>&1
    $output | Out-String | Write-Host
    
    $outputStr = $output | Out-String
    
    if ($LASTEXITCODE -eq 0 -or $outputStr -match "Apply complete") {
        Write-Host "`n=== DEPLOYMENT SUCCESSFUL ===" -ForegroundColor Green
        & $tf output 2>&1
        exit 0
    }
    
    if ($outputStr -match "Out of host capacity") {
        Write-Host "`nOut of host capacity. Waiting ${delay}s before retry..." -ForegroundColor Red
        Start-Sleep -Seconds $delay
        $delay = [Math]::Min($delay * 2, 300)  # Cap at 5 minutes
    }
    else {
        Write-Host "`nNon-retryable error. Stopping." -ForegroundColor Red
        exit 1
    }
}

Write-Host "`n=== EXHAUSTED ALL RETRIES ===" -ForegroundColor Red
