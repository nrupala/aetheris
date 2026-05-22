# OCI ARM Capacity Retry Loop - Montreal
# Runs until instance is provisioned or max attempts reached
param(
    [int]$MaxAttempts = 288,      # 288 * 5min = 24 hours
    [int]$IntervalSec = 300       # 5 minutes between attempts
)

$tf = "C:\Users\HomeUser\AppData\Local\Microsoft\WinGet\Packages\Hashicorp.Terraform_Microsoft.Winget.Source_8wekyb3d8bbwe\terraform.exe"
$workDir = "C:\Users\HomeUser\Downloads\LLMVM"
$logFile = "$workDir\scripts\capacity-retry.log"

Write-Host "=== OCI Montreal Capacity Retry Loop ===" -ForegroundColor Cyan
Write-Host "Max attempts: $MaxAttempts | Interval: ${IntervalSec}s" -ForegroundColor Cyan
Write-Host "Logging to: $logFile" -ForegroundColor Cyan

Set-Content $logFile "$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') - Starting capacity retry loop"

for ($i = 1; $i -le $MaxAttempts; $i++) {
    $time = Get-Date -Format 'yyyy-MM-dd HH:mm:ss'
    $msg = "`n[$time] Attempt $i/$MaxAttempts"
    Write-Host $msg -ForegroundColor Yellow
    Add-Content $logFile $msg

    Set-Location $workDir
    $out = & $tf apply -auto-approve 2>&1 | Out-String

    if ($out -match "Apply complete") {
        $success = "SUCCESS! Instance created at $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')"
        Write-Host $success -ForegroundColor Green
        Add-Content $logFile $success
        $ipLine = ($out | Select-String "instance_public_ip").ToString()
        Write-Host $ipLine -ForegroundColor Green
        Add-Content $logFile $ipLine
        break
    } elseif ($out -match "Out of host capacity") {
        Write-Host "  No capacity - next attempt in $($IntervalSec/60)min" -ForegroundColor Gray
        Add-Content $logFile "  Out of host capacity"
    } elseif ($out -match "EOF") {
        Write-Host "  Network error - next attempt in $($IntervalSec/60)min" -ForegroundColor Gray
        Add-Content $logFile "  Network timeout/EOF"
    } else {
        $err = ($out | Select-String "Error:" | Select-Object -First 1)
        Write-Host "  Error: $err" -ForegroundColor Red
        Add-Content $logFile "  Error: $err"
    }

    if ($i -lt $MaxAttempts) {
        Start-Sleep -Seconds $IntervalSec
    }
}

if (-not ($out -match "Apply complete")) {
    $fail = "$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') - All $MaxAttempts attempts exhausted"
    Write-Host $fail -ForegroundColor Red
    Add-Content $logFile $fail
}
