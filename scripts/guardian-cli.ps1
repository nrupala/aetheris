#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Aetheris Guardian CLI — Query system health, alerts, recommendations
.DESCRIPTION
    Tri-interface guardian accessible via CLI, Browser, or Chat.
    Connects to the Aetheris Core API to query system status.
.EXAMPLE
    .\guardian-cli.ps1 health
    .\guardian-cli.ps1 alerts
    .\guardian-cli.ps1 recommendations
    .\guardian-cli.ps1 versions
    .\guardian-cli.ps1 "ask: is the system running well?"
#>

param(
    [Parameter(Position=0)]
    [string]$Command = "health",

    [Parameter(Position=1)]
    [string]$BaseUrl = "http://localhost:8080"
)

function Write-Banner {
    Write-Host "╔══════════════════════════════════════╗" -ForegroundColor Cyan
    Write-Host "║       Aetheris Guardian CLI          ║" -ForegroundColor Cyan
    Write-Host "╚══════════════════════════════════════╝" -ForegroundColor Cyan
    Write-Host ""
}

function Invoke-GuardianApi {
    param([string]$Endpoint, [string]$Method = "GET", [object]$Body = $null)
    $url = "$BaseUrl$Endpoint"
    try {
        $params = @{
            Uri = $url
            Method = $Method
            ContentType = "application/json"
            UseBasicParsing = $true
        }
        if ($Body) {
            $params.Body = ($Body | ConvertTo-Json -Compress)
        }
        $response = Invoke-RestMethod @params
        return $response
    } catch {
        Write-Host " ERROR: Unable to connect to $url" -ForegroundColor Red
        Write-Host "        $_" -ForegroundColor Red
        return $null
    }
}

function Show-Health {
    Write-Host " SYSTEM HEALTH" -ForegroundColor Green
    Write-Host "─" * 50
    $health = Invoke-GuardianApi -Endpoint "/guardian/health"
    if (-not $health) { return }

    Write-Host " Status:     " -NoNewline
    if ($health.status -eq "healthy") { Write-Host $health.status -ForegroundColor Green }
    elseif ($health.status -eq "degraded") { Write-Host $health.status -ForegroundColor Yellow }
    else { Write-Host $health.status -ForegroundColor Red }

    $uptime = [math]::Floor($health.uptime_seconds / 86400)
    $remain = $health.uptime_seconds % 86400
    $hours = [math]::Floor($remain / 3600)
    $remain = $remain % 3600
    $mins = [math]::Floor($remain / 60)
    $secs = $remain % 60
    Write-Host " Uptime:     ${uptime}d ${hours}h ${mins}m ${secs}s"
    Write-Host " Version:    $($health.version)"
    Write-Host ""

    Write-Host " SERVICES" -ForegroundColor Green
    Write-Host "─" * 50
    foreach ($svc in $health.services) {
        $icon = if ($svc.status -in @("running","connected","online","active")) { "●" } else { "○" }
        $color = if ($svc.status -in @("running","connected","online","active")) { "Green" } elseif ($svc.status -eq "standby") { "Yellow" } else { "Red" }
        Write-Host " $icon $($svc.name.PadRight(20)) $($svc.status.PadRight(15)) $($svc.latency_ms)ms" -ForegroundColor $color
    }

    if ($health.alerts -and $health.alerts.Count -gt 0) {
        Write-Host ""
        Write-Host " ALERTS ($($health.alerts.Count))" -ForegroundColor Red
        Write-Host "─" * 50
        foreach ($alert in $health.alerts) {
            Write-Host " [$($alert.severity)] $($alert.message)" -ForegroundColor Red
        }
    }

    if ($health.recommendations -and $health.recommendations.Count -gt 0) {
        Write-Host ""
        Write-Host " RECOMMENDATIONS ($($health.recommendations.Count))" -ForegroundColor Yellow
        Write-Host "─" * 50
        foreach ($rec in $health.recommendations) {
            Write-Host " [$($rec.priority)] $($rec.title)" -ForegroundColor Yellow
            Write-Host "     $($rec.description)"
        }
    }
}

function Show-Alerts {
    $health = Invoke-GuardianApi -Endpoint "/guardian/health"
    if (-not $health) { return }
    if (-not $health.alerts -or $health.alerts.Count -eq 0) {
        Write-Host " No active alerts. System is running smoothly." -ForegroundColor Green
        return
    }
    Write-Host " ACTIVE ALERTS ($($health.alerts.Count))" -ForegroundColor Red
    Write-Host "─" * 60
    foreach ($alert in $health.alerts) {
        Write-Host " [$($alert.severity.ToUpper())] $($alert.message)" -ForegroundColor Red
        Write-Host "          $($alert.timestamp)" -ForegroundColor DarkGray
    }
}

function Show-Recommendations {
    $health = Invoke-GuardianApi -Endpoint "/guardian/health"
    if (-not $health) { return }
    if (-not $health.recommendations -or $health.recommendations.Count -eq 0) {
        Write-Host " No active recommendations." -ForegroundColor Green
        Write-Host " Tag me with specific areas: memory, latency, storage, security"
        return
    }
    Write-Host " RECOMMENDATIONS" -ForegroundColor Yellow
    Write-Host "─" * 60
    foreach ($rec in $health.recommendations) {
        $pColor = switch ($rec.priority) {
            "high" { "Red" }
            "medium" { "Yellow" }
            default { "Green" }
        }
        Write-Host " [$($rec.priority.ToUpper())] [$($rec.category)] $($rec.title)" -ForegroundColor $pColor
        Write-Host "     $($rec.description)" -ForegroundColor Gray
        Write-Host "     → $($rec.action)" -ForegroundColor Cyan
        Write-Host ""
    }
}

function Show-Versions {
    $versions = Invoke-GuardianApi -Endpoint "/guardian/versions"
    if (-not $versions) { return }
    if (-not $versions.versions -or $versions.versions.Count -eq 0) {
        Write-Host " No Chronicle versions captured yet." -ForegroundColor Yellow
        return
    }
    Write-Host " CHRONICLE VERSIONS ($($versions.versions.Count))" -ForegroundColor Cyan
    Write-Host "─" * 60
    foreach ($v in $versions.versions) {
        Write-Host " $($v.id) | $($v.version_type.PadRight(12)) | $($v.summary) | $($v.compression_ratio)x" -ForegroundColor Cyan
    }
}

function Send-ChatQuery {
    param([string]$Query)
    $body = @{ query = $Query }
    $result = Invoke-GuardianApi -Endpoint "/guardian/query" -Method "POST" -Body $body
    if ($result) {
        Write-Host ""
        Write-Host " Guardian:" -ForegroundColor Green
        Write-Host "─" * 50
        Write-Host $result.answer -ForegroundColor White
    }
}

# ─── Main ─────────────────────────────────────────────────────

Write-Banner

if ($Command -eq "health") { Show-Health }
elseif ($Command -eq "alerts") { Show-Alerts }
elseif ($Command -eq "recommendations" -or $Command -eq "recs") { Show-Recommendations }
elseif ($Command -eq "versions" -or $Command -eq "chronicle") { Show-Versions }
elseif ($Command -eq "help") {
    Write-Host " USAGE" -ForegroundColor Green
    Write-Host "─" * 50
    Write-Host " .\guardian-cli.ps1 health              — Full system health report" -ForegroundColor White
    Write-Host " .\guardian-cli.ps1 alerts              — Active alerts and issues" -ForegroundColor White
    Write-Host " .\guardian-cli.ps1 recommendations     — Performance suggestions" -ForegroundColor White
    Write-Host " .\guardian-cli.ps1 versions            — Chronicle version snapshots" -ForegroundColor White
    Write-Host " .\guardian-cli.ps1 help                — This help" -ForegroundColor White
    Write-Host ""
    Write-Host " Natural language queries sent to /guardian/query:" -ForegroundColor Cyan
    Write-Host " .\guardian-cli.ps1 \"ask: how is the system?\"" -ForegroundColor White
    Write-Host " .\guardian-cli.ps1 \"ask: check memory\"" -ForegroundColor White
}
elseif ($Command -like "ask:*") {
    Send-ChatQuery -Query $Command.Substring(4).Trim()
}
else {
    Send-ChatQuery -Query $Command
}
