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

function Test-PythonAvailable {
  return (Test-CommandAvailable -CommandName 'py') -or (Test-CommandAvailable -CommandName 'python')
}

function Assert-CommandAvailable {
  param(
    [Parameter(Mandatory=$true)][string]$CommandName,
    [Parameter(Mandatory=$true)][string]$InstallHint
  )

  if (-not (Test-CommandAvailable -CommandName $CommandName)) {
    throw "Required command '$CommandName' was not found. $InstallHint"
  }
}

function Invoke-Python {
  param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Args)

  if (Test-CommandAvailable -CommandName 'py') {
    & py @Args
    return
  }

  if (Test-CommandAvailable -CommandName 'python') {
    & python @Args
    return
  }

  throw 'Python launcher not found. Install Python or rerun with -SkipPython.'
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

if ((-not $SkipPython) -and ((Test-Path 'requirements.txt') -or (Test-Path 'pyproject.toml'))) {
  if (-not (Test-PythonAvailable)) {
    throw 'Python is required for the legacy tooling in this repo. Install Python 3.10+ or rerun with -SkipPython.'
  }

  if (Test-Path 'requirements.txt') {
    Write-Host 'Installing Python requirements (legacy tooling/reference scripts)...'
    Invoke-Python -m pip install -r requirements.txt
  }

  if (-not $Quick) {
    Write-Host 'Collecting legacy Python tests...'
    Invoke-Python -m pytest --collect-only -q
  }
}

if ((-not $SkipRust) -and (Test-Path 'Cargo.toml')) {
  Assert-CommandAvailable -CommandName 'cargo' -InstallHint 'Install Rust via rustup or rerun with -SkipRust.'

  Write-Host 'Running cargo check for workspace...'
  cargo check --workspace

  if (-not $Quick) {
    Write-Host 'Running focused Rust tests...'
    cargo test -p shared -p engine -p parsers -p scanner -p persistence --quiet
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
Write-Host 'Validated layers:'
Write-Host ("  - Rust workspace: {0}" -f ((Test-Path 'Cargo.toml') -and (-not $SkipRust)))
Write-Host ("  - Legacy Python: {0}" -f (((Test-Path 'requirements.txt') -or (Test-Path 'pyproject.toml')) -and (-not $SkipPython)))
Write-Host 'Next steps:'
Write-Host '  1) Review .env and fill secrets only if needed'
Write-Host '  2) .\worktrees.ps1'
Write-Host '  3) New-AgentWorktree -Name rust-core -Bootstrap'
Write-Host '  4) Read DEV_SETUP.md and OPENCLAW_WORKFLOW.md'
