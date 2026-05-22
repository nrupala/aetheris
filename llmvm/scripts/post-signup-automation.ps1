#!/usr/bin/env pwsh
# Aetheris LLMVM — Post-Signup Automation
# Run after OCI account is created and API key is configured
param(
    [ValidateSet("chicago", "jovanovac")]
    [string]$Region = "chicago",

    [switch]$DeployOnly,
    [switch]$SkipDeploy
)

$tf = "C:\Users\HomeUser\AppData\Local\Microsoft\WinGet\Packages\Hashicorp.Terraform_Microsoft.Winget.Source_8wekyb3d8bbwe\terraform.exe"
$workDir = "C:\Users\HomeUser\Downloads\LLMVM"

Write-Host "=== Aetheris LLMVM Post-Signup Automation ===" -ForegroundColor Cyan
Write-Host "Target region: $Region"
Write-Host ""

# Step 1: Validate API key
Write-Host "[Step 1/6] Validating OCI API key..." -ForegroundColor Yellow
$ociConfig = "$env:USERPROFILE\.oci\config"
if (-not (Test-Path $ociConfig)) {
    Write-Host "  ERROR: OCI config not found at $ociConfig" -ForegroundColor Red
    Write-Host "  Fix: Run the API key setup in OCI Console and save the config." -ForegroundColor Red
    exit 1
}
Write-Host "  OCI config found." -ForegroundColor Green

# Step 2: Select tfvars
$tfvarsFile = "terraform.tfvars.$Region"
if (-not (Test-Path (Join-Path $workDir $tfvarsFile))) {
    Write-Host "  ERROR: $tfvarsFile not found." -ForegroundColor Red
    Write-Host "  Fix: Copy terraform.tfvars.$Region to terraform.tfvars and fill in values." -ForegroundColor Red
    exit 1
}

# Step 3: Copy tfvars to active config
Write-Host "`n[Step 2/6] Activating $tfvarsFile..." -ForegroundColor Yellow
Copy-Item (Join-Path $workDir $tfvarsFile) (Join-Path $workDir "terraform.tfvars") -Force
Write-Host "  Copied to terraform.tfvars" -ForegroundColor Green

# Step 4: Initialize Terraform
Write-Host "`n[Step 3/6] Initializing Terraform..." -ForegroundColor Yellow
Set-Location $workDir
$output = & $tf init -upgrade 2>&1 | Out-String
Write-Host $output

if ($LASTEXITCODE -ne 0) {
    Write-Host "  ERROR: Terraform init failed." -ForegroundColor Red
    Write-Host "  Check your OCI credentials and region in terraform.tfvars" -ForegroundColor Red
    exit 1
}
Write-Host "  Terraform initialized." -ForegroundColor Green

# Step 5: Plan
Write-Host "`n[Step 4/6] Planning deployment..." -ForegroundColor Yellow
$output = & $tf plan 2>&1 | Out-String
Write-Host $output

if ($output -match "Error:") {
    Write-Host "  ERROR: Plan failed." -ForegroundColor Red
    if ($output -match "NotAuthenticated") {
        Write-Host "  Your API key is not valid for this region. Check fingerprint and key." -ForegroundColor Red
    }
    if ($output -match "Out of host capacity") {
        Write-Host "  ARM capacity unavailable. Run deploy-with-retry.ps1 instead." -ForegroundColor Red
    }
    exit 1
}
Write-Host "  Plan successful. Ready to deploy." -ForegroundColor Green

if ($SkipDeploy) {
    Write-Host "`n  Skipping deployment. Run manually: terraform apply -auto-approve"
    exit 0
}

# Step 6: Deploy
Write-Host "`n[Step 5/6] Deploying infrastructure..." -ForegroundColor Yellow
$output = & $tf apply -auto-approve 2>&1 | Out-String
Write-Host $output

if ($output -match "Out of host capacity") {
    Write-Host "`n  ARM capacity unavailable. Switching to retry mode..." -ForegroundColor Red
    & "$workDir\scripts\deploy-with-retry.ps1"
    exit $LASTEXITCODE
}

if ($output -match "Apply complete|1 added") {
    Write-Host "`n  Deployment successful!" -ForegroundColor Green
} else {
    Write-Host "`n  Deployment may have issues. Check output above." -ForegroundColor Red
    exit 1
}

# Step 7: Capture outputs
Write-Host "`n[Step 6/6] Capturing instance details..." -ForegroundColor Yellow
& $tf output -json 2>&1 | Out-File -FilePath (Join-Path $workDir ".actions\instance-info-$Region.json") -Force
& $tf output 2>&1

Write-Host "`n=== DEPLOYMENT COMPLETE ===" -ForegroundColor Green
Write-Host "Instance details saved to .actions\instance-info-$Region.json"
Write-Host ""
Write-Host "Next steps:"
Write-Host "1. SSH: $( & $tf output ssh_command 2>$null )"
Write-Host "2. Run: scripts\setup-vm.ps1 <instance_ip>"
