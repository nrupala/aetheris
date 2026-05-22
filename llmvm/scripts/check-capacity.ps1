#!/usr/bin/env pwsh
# Aetheris LLMVM — Multi-Region Capacity Checker
# Probes OCI regions for ARM (VM.Standard.A1.Flex) capacity
param(
    [string[]]$Regions = @("ca-montreal-1", "ca-toronto-1", "ap-mumbai-1")
)

$tf = "C:\Users\HomeUser\AppData\Local\Microsoft\WinGet\Packages\Hashicorp.Terraform_Microsoft.Winget.Source_8wekyb3d8bbwe\terraform.exe"
$workDir = "C:\Users\HomeUser\Downloads\LLMVM"
$originalRegion = (Get-Content "$workDir\terraform.tfvars" | Select-String 'region').ToString().Split('=')[1].Trim().Trim('"')

Write-Host "=== OCI ARM Capacity Checker ===" -ForegroundColor Cyan
Write-Host "Regions: $($Regions -join ', ')"
Write-Host ""

$results = @()

foreach ($region in $Regions) {
    Write-Host "`n[$region] Checking..." -ForegroundColor Yellow
    
    # Update region in tfvars temporarily
    $tfvars = Get-Content "$workDir\terraform.tfvars"
    $tfvars = $tfvars -replace '^region\s*=.*$', "region              = `"$region`""
    $tfvars | Set-Content "$workDir\terraform.tfvars"
    
    # Init for new region
    & $tf init -upgrade -input=false 2>&1 | Out-Null
    
    # Quick plan to test auth + capacity
    $output = & $tf plan -input=false 2>&1 | Out-String
    
    if ($output -match "Out of host capacity") {
        Write-Host "  [FAIL] Out of host capacity" -ForegroundColor Red
        $results += [PSCustomObject]@{ Region = $region; Status = "NO_CAPACITY"; Note = "500-InternalError" }
    }
    elseif ($output -match "NotAuthenticated") {
        Write-Host "  [FAIL] API key not registered" -ForegroundColor Red
        $results += [PSCustomObject]@{ Region = $region; Status = "NO_AUTH"; Note = "401-NotAuthenticated" }
    }
    elseif ($output -match "Plan:.*1 to add") {
        Write-Host "  [OK] Capacity available! Plan ready." -ForegroundColor Green
        $results += [PSCustomObject]@{ Region = $region; Status = "AVAILABLE"; Note = "Ready to deploy" }
    }
    else {
        Write-Host "  [?]" ($output | Select-String -Pattern "Error:" | Select-Object -First 1) -ForegroundColor Gray
        $results += [PSCustomObject]@{ Region = $region; Status = "UNKNOWN"; Note = "See error output" }
    }
}

# Restore original region
$tfvars = Get-Content "$workDir\terraform.tfvars"
$tfvars = $tfvars -replace '^region\s*=.*$', "region              = `"$originalRegion`""
$tfvars | Set-Content "$workDir\terraform.tfvars"

# Summary table
Write-Host "`n=== Summary ===" -ForegroundColor Cyan
$results | Format-Table -AutoSize

# Return best region
$available = $results | Where-Object { $_.Status -eq "AVAILABLE" }
if ($available) {
    Write-Host "Best region: $($available[0].Region)" -ForegroundColor Green
    exit 0
} else {
    Write-Host "No available regions found." -ForegroundColor Red
    exit 1
}
