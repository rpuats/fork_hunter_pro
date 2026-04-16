param(
    [string[]]$Slugs = @(),
    [string]$ArtifactRoot = "artifacts/nightly/runtime",
    [string]$LatestSnapshot = "runtime_parser_diagnostics_latest.json"
)

$ErrorActionPreference = "Stop"

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

if ($Slugs.Count -gt 0) {
    $cargoArgs += $Slugs
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
