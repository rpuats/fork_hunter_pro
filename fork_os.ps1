#!/usr/bin/env powershell
<#
    🚀 FORK-OS MASTER CONTROL
    Запускает и управляет всеми агентами Fork-OS
#>

param(
    [string]$Action = "start",
    [string]$Mode = "full"
)

function Write-Header {
    param([string]$Text)
    Write-Host ""
    Write-Host "=" -NoNewline -ForegroundColor Green
    Write-Host ("=" * 78) -ForegroundColor Green
    Write-Host $Text -ForegroundColor Cyan -BackgroundColor Black
    Write-Host "=" -NoNewline -ForegroundColor Green
    Write-Host ("=" * 78) -ForegroundColor Green
    Write-Host ""
}

function Write-Status {
    param([string]$Text, [string]$Status = "info")
    
    $emoji = @{
        "info" = "ℹ️ "
        "success" = "✅ "
        "error" = "❌ "
        "warning" = "⚠️  "
        "start" = "▶️  "
        "stop" = "⏹️  "
    }[$Status]
    
    Write-Host "$emoji $Text"
}

function Start-AllAgents {
    Write-Header "🚀 FORK-OS PARALLEL AGENTS - STARTING ALL SYSTEMS"
    
    Write-Status "Checking prerequisites..." "info"
    
    # Check Rust installation
    if (!(Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-Status "Rust not found! Install from https://rustup.rs/" "error"
        exit 1
    }
    Write-Status "✓ Rust/Cargo found" "success"
    
    # Check Python installation
    if (!(Get-Command python -ErrorAction SilentlyContinue)) {
        Write-Status "Python not found! Install Python 3.8+" "error"
        exit 1
    }
    Write-Status "✓ Python found" "success"
    
    # Build project
    Write-Status "Building Rust project (this may take a few minutes)..." "info"
    Push-Location
    cd $PSScriptRoot
    
    cargo build --release 2>&1 | tail -20
    
    if ($LASTEXITCODE -ne 0) {
        Write-Status "Build failed! Check cargo errors above." "error"
        Pop-Location
        exit 1
    }
    Write-Status "✓ Build successful" "success"
    
    Pop-Location
    
    # Start agents in parallel
    Write-Header "🎯 STARTING AGENTS IN PARALLEL"
    
    $agents = @(
        @{ name = "🚀 ORCHESTRATOR"; script = "python run_all_agents.py" },
        @{ name = "🔍 PROBLEM DEBUGGER"; script = "python debug_problem_bks.py" },
        @{ name = "⚡ PARSER OPTIMIZER"; script = "python optimize_parsers.py" },
        @{ name = "💡 IDEAS GENERATOR"; script = "python generate_ideas.py" }
    )
    
    $processes = @()
    
    foreach ($agent in $agents) {
        Write-Status "Starting: $($agent.name)" "start"
        
        $pinfo = New-Object System.Diagnostics.ProcessStartInfo
        $pinfo.FileName = "powershell.exe"
        $pinfo.Arguments = "-NoExit -Command `"cd '$PSScriptRoot'; $($agent.script)`""
        $pinfo.UseShellExecute = $true
        $pinfo.WindowStyle = [System.Diagnostics.ProcessWindowStyle]::Normal
        
        $process = [System.Diagnostics.Process]::Start($pinfo)
        $processes += @{ name = $agent.name; process = $process; pid = $process.Id }
        
        Write-Status "$($agent.name) started (PID: $($process.Id))" "success"
        Start-Sleep -Seconds 2
    }
    
    Write-Header "✅ ALL AGENTS STARTED"
    
    Write-Host ""
    Write-Host "📊 Running Agents:" -ForegroundColor Cyan
    foreach ($p in $processes) {
        Write-Host "  • $($p.name) [PID: $($p.pid)]"
    }
    
    Write-Host ""
    Write-Host "🌐 API Access:" -ForegroundColor Cyan
    Write-Host "  • Health: http://localhost:8080/api/v1/health"
    Write-Host "  • Metrics: http://localhost:8080/api/v1/metrics"
    Write-Host "  • Surebets: http://localhost:8080/api/v1/surebets"
    Write-Host "  • WebSocket: ws://localhost:8080/ws"
    
    Write-Host ""
    Write-Host "📁 Agent Results:" -ForegroundColor Cyan
    Write-Host "  • Parser Performance: $(Get-Item -Path "agent_results" -ErrorAction SilentlyContinue | Select -ExpandProperty FullName)"
    Write-Host "  • Generated Ideas: generated_ideas.jsonl"
    Write-Host "  • Parser Metrics: parser_performance.json"
    Write-Host "  • Debug Results: debug_results.log"
    
    Write-Host ""
    Write-Host "⏹️  To stop all agents: Run 'Stop-AllAgents' or Ctrl+C in each window" -ForegroundColor Yellow
    
    # Monitor processes
    Write-Host ""
    Write-Status "Monitoring agents..." "info"
    
    $stillRunning = $true
    while ($stillRunning) {
        $running = 0
        foreach ($p in $processes) {
            if (!$p.process.HasExited) {
                $running++
            }
        }
        
        if ($running -eq 0) {
            Write-Status "All agents have exited" "warning"
            $stillRunning = $false
        } else {
            $timestamp = (Get-Date -Format "HH:mm:ss")
            Write-Host "[$timestamp] Agents running: $running/$($processes.Count)" -ForegroundColor Gray
        }
        
        Start-Sleep -Seconds 30
    }
}

function Stop-AllAgents {
    Write-Header "⏹️  STOPPING ALL AGENTS"
    
    Write-Status "Stopping Python agents..." "info"
    
    Get-Process -Name "python" -ErrorAction SilentlyContinue | 
        Where-Object { $_.CommandLine -match "(run_all_agents|debug_problem_bks|optimize_parsers|generate_ideas)" } |
        ForEach-Object {
            Write-Status "Stopping $($_.ProcessName) [PID: $($_.Id)]" "stop"
            Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
        }
    
    Write-Host ""
    Write-Status "All agents stopped" "success"
}

function Show-Status {
    Write-Header "📊 AGENTS STATUS"
    
    Write-Host "Running Processes:" -ForegroundColor Cyan
    Get-Process -Name "cargo", "python" -ErrorAction SilentlyContinue |
        Where-Object { $_.CommandLine -match "fork-hunter|run_all_agents|debug_problem_bks|optimize_parsers|generate_ideas" } |
        ForEach-Object {
            $uptime = (Get-Date) - $_.StartTime
            Write-Host "  • $($_.ProcessName) [PID: $($_.Id), Running: $($uptime.Hours)h$($uptime.Minutes)m$($uptime.Seconds)s]"
        }
    
    Write-Host ""
    Write-Host "API Status:" -ForegroundColor Cyan
    try {
        $health = Invoke-RestMethod -Uri "http://localhost:8080/api/v1/health" -ErrorAction SilentlyContinue
        Write-Host "  ✅ API is healthy: $($health.status)"
    } catch {
        Write-Host "  ❌ API is not responding"
    }
    
    Write-Host ""
    Write-Host "Result Files:" -ForegroundColor Cyan
    if (Test-Path "agent_results") {
        $files = Get-ChildItem "agent_results" -Filter "*.json" | Measure-Object
        Write-Host "  📁 Agent results: $($files.Count) files"
    }
    if (Test-Path "generated_ideas.jsonl") {
        $lines = @(Get-Content "generated_ideas.jsonl" -ErrorAction SilentlyContinue)
        Write-Host "  💡 Generated ideas: $($lines.Count) lines"
    }
    if (Test-Path "parser_performance.json") {
        Write-Host "  📊 Parser metrics available"
    }
}

function Show-Help {
    Write-Header "FORK-OS MASTER CONTROL - HELP"
    
    Write-Host ""
    Write-Host "Usage: .\fork_os.ps1 [ACTION] [MODE]" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "Actions:" -ForegroundColor Yellow
    Write-Host "  start     - Start all agents (default)" -ForegroundColor White
    Write-Host "  stop      - Stop all running agents" -ForegroundColor White
    Write-Host "  status    - Show status of all agents" -ForegroundColor White
    Write-Host "  help      - Show this help message" -ForegroundColor White
    Write-Host ""
    Write-Host "Examples:" -ForegroundColor Yellow
    Write-Host "  .\fork_os.ps1 start" -ForegroundColor Gray
    Write-Host "  .\fork_os.ps1 status" -ForegroundColor Gray
    Write-Host "  .\fork_os.ps1 stop" -ForegroundColor Gray
}

# Main dispatch
switch ($Action.ToLower()) {
    "start" {
        Start-AllAgents
    }
    "stop" {
        Stop-AllAgents
    }
    "status" {
        Show-Status
    }
    "help" {
        Show-Help
    }
    default {
        Write-Status "Unknown action: $Action" "error"
        Show-Help
        exit 1
    }
}
