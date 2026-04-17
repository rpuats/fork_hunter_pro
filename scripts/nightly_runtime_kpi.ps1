param(
    [string[]]$Slugs = @(),
    [string]$ArtifactRoot = "artifacts/nightly/runtime",
    [string]$LatestSnapshot = "runtime_parser_diagnostics_latest.json"
)

$ErrorActionPreference = "Stop"

function Clear-PoisonedProxyEnv {
    $proxyVars = @(
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "GIT_HTTP_PROXY",
        "GIT_HTTPS_PROXY"
    )
    $poisonedValues = @(
        "http://127.0.0.1:9",
        "https://127.0.0.1:9"
    )

    $cleared = @()
    foreach ($name in $proxyVars) {
        $current = [Environment]::GetEnvironmentVariable($name)
        if ([string]::IsNullOrWhiteSpace($current)) {
            continue
        }

        if ($poisonedValues -contains $current.Trim().ToLowerInvariant()) {
            Set-Item -Path ("Env:{0}" -f $name) -Value ""
            $cleared += $name
        }
    }

    if ($cleared.Count -gt 0) {
        Write-Host ("Cleared poisoned proxy env for nightly runtime KPI: {0}" -f ($cleared -join ", "))
    }
}

function Normalize-Slugs([string[]]$RawSlugs) {
    $normalized = @()
    foreach ($entry in $RawSlugs) {
        if ([string]::IsNullOrWhiteSpace($entry)) {
            continue
        }

        foreach ($slug in ($entry -split ",")) {
            $trimmed = $slug.Trim()
            if (-not [string]::IsNullOrWhiteSpace($trimmed)) {
                $normalized += $trimmed
            }
        }
    }

    return $normalized
}

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent $scriptDir
$artifactDir = Join-Path $repoRoot $ArtifactRoot
$runsDir = Join-Path $artifactDir "runs"
$historyPath = Join-Path $artifactDir "runtime_parser_diagnostics_history.jsonl"
$latestPath = Join-Path $artifactDir $LatestSnapshot
$timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
$snapshotPath = Join-Path $runsDir ("runtime_parser_diagnostics_{0}.json" -f $timestamp)

New-Item -ItemType Directory -Force -Path $artifactDir | Out-Null
New-Item -ItemType Directory -Force -Path $runsDir | Out-Null

Clear-PoisonedProxyEnv
$resolvedSlugs = Normalize-Slugs $Slugs

$cargoArgs = @(
    "run",
    "-p",
    "parsers",
    "--bin",
    "runtime_parser_diagnostics",
    "--",
    "--strict-exit",
    "--json-out",
    $latestPath,
    "--json-out",
    $historyPath,
    "--json-out",
    $snapshotPath
)

if ($resolvedSlugs.Count -gt 0) {
    $cargoArgs += $resolvedSlugs
}

Write-Host "Running nightly runtime KPI diagnostics..."
Write-Host ("Artifacts: latest={0}; history={1}; snapshot={2}" -f $latestPath, $historyPath, $snapshotPath)

Push-Location $repoRoot
try {
    & cargo @cargoArgs
    $exitCode = $LASTEXITCODE
}
finally {
    Pop-Location
}

if ($exitCode -ne 0) {
    exit $exitCode
}
