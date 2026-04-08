param(
  [switch]$SkipPython,
  [switch]$SkipRust,
  [switch]$InstallOptionalTools,
  [switch]$Quick
)

$ErrorActionPreference = 'Stop'

Write-Host '== Fork Hunter Pro bootstrap =='

$projectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $projectRoot

Write-Host "Project root: $projectRoot"

function Test-CommandAvailable {
  param([Parameter(Mandatory=$true)][string]$CommandName)
  return [bool](Get-Command $CommandName -ErrorAction SilentlyContinue)
}

function Ensure-NpmGlobalPackage {
  param(
    [Parameter(Mandatory=$true)][string]$Match,
    [Parameter(Mandatory=$true)][string]$Package
  )

  $globalPackages = npm list -g --depth=0 2>$null | Out-String
  if ($globalPackages -notmatch $Match) {
    Write-Host "Installing npm package: $Package"
    npm install -g $Package
  }
}

function Ensure-CargoTool {
  param([Parameter(Mandatory=$true)][string]$Binary)

  if (-not (Test-CommandAvailable -CommandName $Binary)) {
    Write-Host "Installing cargo tool: $Binary"
    cargo install $Binary
  }
}

if ((Test-Path '.env.example') -and (-not (Test-Path '.env'))) {
  Write-Host 'Creating local .env from .env.example'
  Copy-Item '.env.example' '.env'
}

if ((-not $SkipPython) -and (Test-Path 'requirements.txt')) {
  Write-Host 'Installing Python requirements (legacy tooling)...'
  py -m pip install -r requirements.txt
}

if ((-not $SkipRust) -and (Test-Path 'Cargo.toml')) {
  Write-Host 'Running cargo check...'
  cargo check

  if (-not $Quick) {
    Write-Host 'Running focused Rust tests...'
    cargo test -p shared -p engine -p parsers --quiet
  }
}

if (Test-CommandAvailable -CommandName 'npm') {
  Ensure-NpmGlobalPackage -Match '@openai/codex' -Package '@openai/codex'
  Ensure-NpmGlobalPackage -Match '@anthropic-ai/claude-code' -Package '@anthropic-ai/claude-code'
}

if ($InstallOptionalTools -and (Test-CommandAvailable -CommandName 'cargo')) {
  Ensure-CargoTool -Binary 'cargo-nextest'
  Ensure-CargoTool -Binary 'cargo-watch'
}

Write-Host 'Bootstrap complete.'
Write-Host 'Next steps:'
Write-Host '  1) .\worktrees.ps1'
Write-Host '  2) New-AgentWorktree -Name rust-core -Bootstrap'
Write-Host '  3) Read OPENCLAW_WORKFLOW.md'
