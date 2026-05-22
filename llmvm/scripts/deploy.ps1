#!/usr/bin/env pwsh
# Aetheris LLMVM — Terraform Deploy with Retry Logic
# Handles "Out of host capacity" with exponential backoff
param(
    [int]$MaxRetries = 15,
    [int]$InitialDelaySec = 30
)

$tf = "C:\Users\HomeUser\AppData\Local\Microsoft\WinGet\Packages\Hashicorp.Terraform_Microsoft.Winget.Source_8wekyb3d8bbwe\terraform.exe"
$workDir = "C:\Users\HomeUser\Downloads\LLMVM"

Write-Host "=== Aetheris LLMVM Deploy ===" -ForegroundColor Cyan
Write-Host "Region: $((Get-Content "$workDir\terraform.tfvars" | Select-String 'region').ToString().Split('=')[1].Trim().Trim('"'))" -ForegroundColor Cyan
Write-Host "Max retries: $MaxRetries | Initial delay: ${InitialDelaySec}s"
Write-Host ""

$attempt = 0
$delay = $InitialDelaySec

while ($attempt -lt $MaxRetries) {
    $attempt++
    Write-Host "[$(Get-Date -Format 'HH:mm:ss')] Attempt $attempt/$MaxRetries..." -ForegroundColor Yellow
    
    Set-Location $workDir
    $output = & $tf apply -auto-approve 2>&1 | Out-String
    Write-Host $output
    
    if ($output -match "Apply complete|1 added, 0 changed") {
        Write-Host "`n=== DEPLOYMENT SUCCESSFUL ===" -ForegroundColor Green
        & $tf output 2>&1
        exit 0
    }
    
    if ($output -match "Out of host capacity") {
        Write-Host "`n  No ARM capacity. Waiting ${delay}s..." -ForegroundColor Red
        Start-Sleep -Seconds $delay
        $delay = [Math]::Min($delay * 2, 300)
    }
    else {
        Write-Host "`n  Non-retryable error. Stopping." -ForegroundColor Red
        exit 1
    }
}

Write-Host "`n=== ALL RETRIES EXHAUSTED ===" -ForegroundColor Red
