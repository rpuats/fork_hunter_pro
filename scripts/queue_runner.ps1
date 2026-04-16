param(
    [ValidateSet('run', 'status')]
    [string]$Action = 'run',

    [string]$QueuePath = 'artifacts/queue/example-queue.json',

    [string]$RunRoot = 'artifacts/queue/runs',

    [string]$RunId,

    [int]$MaxParallel,

    [int]$PollIntervalMs = 500
)

$ErrorActionPreference = 'Stop'

$pythonArgs = @(
    './scripts/queue_runner.py',
    '--action', $Action,
    '--queue-path', $QueuePath,
    '--run-root', $RunRoot,
    '--poll-interval-ms', $PollIntervalMs
)

if ($RunId) {
    $pythonArgs += @('--run-id', $RunId)
}

if ($PSBoundParameters.ContainsKey('MaxParallel')) {
    $pythonArgs += @('--max-parallel', $MaxParallel)
}

& python @pythonArgs
$exitCode = $LASTEXITCODE
exit $exitCode
