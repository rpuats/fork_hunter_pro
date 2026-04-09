param(
  [string]$BaseBranch = 'master'
)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $projectRoot

$wtRoot = Join-Path $projectRoot '.worktrees'
New-Item -ItemType Directory -Force -Path $wtRoot | Out-Null

function Test-GitHeadReady {
  git rev-parse --verify HEAD *> $null
  return ($LASTEXITCODE -eq 0)
}

function Get-AgentWorktreePath {
  param([Parameter(Mandatory=$true)][string]$Name)
  Join-Path $wtRoot $Name
}

function New-AgentWorktree {
  param(
    [Parameter(Mandatory=$true)][string]$Name,
    [string]$Branch,
    [switch]$Bootstrap
  )

  if (-not (Test-GitHeadReady)) {
    throw 'Cannot create worktrees before the repo has at least one commit. Create the initial commit first.'
  }

  if (-not $Branch) { $Branch = "agent/$Name" }
  $target = Get-AgentWorktreePath -Name $Name

  if (Test-Path $target) {
    throw "Worktree already exists: $target"
  }

  git worktree add -b $Branch $target $BaseBranch
  Write-Host "Created worktree: $target ($Branch)"

  if ($Bootstrap) {
    Initialize-AgentWorkspace -Name $Name
  }
}

function Initialize-AgentWorkspace {
  param(
    [Parameter(Mandatory=$true)][string]$Name,
    [string]$Task = 'Describe the scoped task for this worker here.'
  )

  $target = Get-AgentWorktreePath -Name $Name
  if (-not (Test-Path $target)) {
    throw "Worktree not found: $target"
  }

  $taskFile = Join-Path $target 'AGENT_TASK.md'
  $outputFile = Join-Path $target 'agent-output.md'
  $envExample = Join-Path $target '.env.example'
  $envFile = Join-Path $target '.env'

  $taskBody = @(
    '# Agent Task',
    '',
    '## Worktree',
    "- Name: $Name",
    "- Path: $target",
    '',
    '## Scope',
    $Task,
    '',
    '## Repo reality',
    '- Mainline: Rust workspace under crates/',
    '- Legacy/reference: Python scripts and tests in repo root',
    '',
    '## Guardrails',
    '- Stay inside the assigned scope/crates/docs',
    '- Prefer Rust workspace changes first',
    '- Do not do destructive cleanup of legacy Python',
    '- Validate only the slice you changed',
    '- Leave a short summary in agent-output.md'
  ) -join [Environment]::NewLine

  Set-Content -Path $taskFile -Value $taskBody -Encoding UTF8

  if (-not (Test-Path $outputFile)) {
    $outputBody = @(
      '# Agent Output',
      '',
      '## Done',
      '- ',
      '',
      '## Validation',
      '- ',
      '',
      '## Risks / follow-ups',
      '- '
    ) -join [Environment]::NewLine

    Set-Content -Path $outputFile -Value $outputBody -Encoding UTF8
  }

  if ((Test-Path $envExample) -and (-not (Test-Path $envFile))) {
    Copy-Item $envExample $envFile
  }

  Write-Host "Initialized agent workspace: $target"
}

function Sync-AgentWorktree {
  param([Parameter(Mandatory=$true)][string]$Name)
  $target = Get-AgentWorktreePath -Name $Name
  if (-not (Test-Path $target)) {
    throw "Worktree not found: $target"
  }

  git -C $target status --short --branch
}

function Remove-AgentWorktree {
  param(
    [Parameter(Mandatory=$true)][string]$Name,
    [switch]$Force
  )

  $target = Get-AgentWorktreePath -Name $Name
  if (-not (Test-Path $target)) {
    throw "Worktree not found: $target"
  }

  if ($Force) {
    git worktree remove --force $target
  } else {
    git worktree remove $target
  }

  Write-Host "Removed worktree: $target"
}

function List-AgentWorktrees {
  git worktree list
}

function Set-ForkHunterTaskPresets {
  $presets = @{
    'rust-core' = 'Work only in crates/shared, crates/engine, crates/scanner, crates/persistence. Focus on runtime core, performance, and correctness. Avoid parsers/api/bot unless explicitly required.'
    'parsers' = 'Work only in crates/parsers plus tightly related parser fixtures/tests. Focus on bookmaker coverage, parser_factory, normalization, and parser reliability.'
    'api-bot' = 'Work only in crates/api, crates/bot, crates/fork_hunter_bin, and tightly related runtime wiring. Focus on endpoints, websocket flow, and bot/runtime glue.'
    'integration' = 'Do cross-crate validation only after workers land changes. Focus on cargo check/test, smoke runs, docs sync, and final integration notes.'
    'legacy-python' = 'Work only in root Python scripts/tests/tooling. Treat Python as legacy/reference. Do not do broad cleanup; only migration support, behavior comparison, or narrow fixes.'
  }

  foreach ($name in $presets.Keys) {
    $target = Get-AgentWorktreePath -Name $name
    if (Test-Path $target) {
      Initialize-AgentWorkspace -Name $name -Task $presets[$name]
    }
  }
}

function New-ForkHunterSwarm {
  param([switch]$Bootstrap)

  $roles = @('rust-core', 'parsers', 'api-bot', 'integration', 'legacy-python')

  foreach ($role in $roles) {
    $target = Get-AgentWorktreePath -Name $role
    if (-not (Test-Path $target)) {
      New-AgentWorktree -Name $role -Bootstrap:$Bootstrap
    } elseif ($Bootstrap) {
      Initialize-AgentWorkspace -Name $role
    }
  }

  Set-ForkHunterTaskPresets
}

function Show-ForkHunterSwarm {
  $rows = @(
    [pscustomobject]@{ Name = 'rust-core'; Scope = 'shared/engine/scanner/persistence'; Validation = 'cargo test -p shared -p engine -p scanner -p persistence' },
    [pscustomobject]@{ Name = 'parsers'; Scope = 'parsers + parser fixtures/tests'; Validation = 'cargo test -p parsers' },
    [pscustomobject]@{ Name = 'api-bot'; Scope = 'api/bot/bin'; Validation = 'cargo check -p api -p bot -p fork_hunter_bin' },
    [pscustomobject]@{ Name = 'integration'; Scope = 'workspace validation/docs'; Validation = 'cargo check --workspace' },
    [pscustomobject]@{ Name = 'legacy-python'; Scope = 'root Python legacy/reference'; Validation = 'py -m pytest --collect-only -q' }
  )

  $rows | Format-Table -AutoSize
}

Write-Host 'Available commands:'
Write-Host '  New-AgentWorktree -Name rust-core [-Bootstrap]'
Write-Host '  Initialize-AgentWorkspace -Name rust-core -Task "..."'
Write-Host '  Sync-AgentWorktree -Name rust-core'
Write-Host '  List-AgentWorktrees'
Write-Host '  Remove-AgentWorktree -Name rust-core'
Write-Host '  New-ForkHunterSwarm [-Bootstrap]'
Write-Host '  Set-ForkHunterTaskPresets'
Write-Host '  Show-ForkHunterSwarm'
Write-Host ''
Write-Host 'Suggested worker names:'
Write-Host '  rust-core, parsers, api-bot, integration, legacy-python'
