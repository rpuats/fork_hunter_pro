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
    '## Guardrails',
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

Write-Host 'Available commands:'
Write-Host '  New-AgentWorktree -Name rust-core [-Bootstrap]'
Write-Host '  Initialize-AgentWorkspace -Name rust-core -Task "..."'
Write-Host '  Sync-AgentWorktree -Name rust-core'
Write-Host '  List-AgentWorktrees'
Write-Host '  Remove-AgentWorktree -Name rust-core'
