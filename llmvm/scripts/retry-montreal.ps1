#!/usr/bin/env pwsh
# Aetheris LLMVM — Montreal ARM Capacity Retry Bot
# Retries creating VM.Standard.A1.Flex until capacity opens up
param(
    [int]$MaxAttempts = 1000,
    [int]$DelaySec = 30
)

$tf = "C:\Users\HomeUser\AppData\Local\Microsoft\WinGet\Packages\Hashicorp.Terraform_Microsoft.Winget.Source_8wekyb3d8bbwe\terraform.exe"
$workDir = "C:\Users\HomeUser\Downloads\LLMVM"

Write-Host "=== Montreal ARM Capacity Retry Bot ===" -ForegroundColor Cyan
Write-Host "Region: ca-montreal-1"
Write-Host "Shape: VM.Standard.A1.Flex (4 OCPU / 24GB RAM)"
Write-Host "Max attempts: $MaxAttempts | Delay: ${DelaySec}s"
Write-Host ""

$attempt = 0
$success = $false

while ($attempt -lt $MaxAttempts -and -not $success) {
    $attempt++
    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    Write-Host "[$timestamp] Attempt $attempt/$MaxAttempts" -ForegroundColor Yellow

    Set-Location $workDir
    $output = & $tf apply -auto-approve 2>&1 | Out-String

    if ($output -match "Apply complete|1 added, 0 changed") {
        Write-Host "`n  [SUCCESS] Instance created on attempt $attempt!" -ForegroundColor Green
        & $tf output 2>&1
        $success = $true
        break
    }

    if ($output -match "Out of host capacity") {
        $remaining = $MaxAttempts - $attempt
        Write-Host "  No capacity. Waiting ${DelaySec}s... ($remaining attempts left)" -ForegroundColor DarkYellow
    }
    elseif ($output -match "NotAuthenticated|InvalidParameter") {
        Write-Host "  FATAL: $(& { if ($output -match "NotAuthenticated") { "Auth error" } else { "Config error" } })" -ForegroundColor Red
        Write-Host "  $output" -ForegroundColor DarkGray
        break
    }
    else {
        Write-Host "  Unexpected error. Waiting ${DelaySec}s..." -ForegroundColor Red
        $output | Select-Object -First 5 | ForEach-Object { Write-Host "    $_" -ForegroundColor DarkGray }
    }

    Start-Sleep -Seconds $DelaySec
}

if ($success) {
    Write-Host "`n=== INSTANCE DEPLOYED ===" -ForegroundColor Green
    Write-Host "Run: scripts\setup-vm.ps1 to bootstrap the VM"
} else {
    Write-Host "`n=== RETRY EXHAUSTED ===" -ForegroundColor Red
    Write-Host "ARM capacity not available in Montreal after $attempt attempts."
    Write-Host "Consider: us-chicago-1 or eu-jovanovac-1 for new signup."
}
