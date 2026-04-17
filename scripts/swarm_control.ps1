param(
    [ValidateSet('ensure-state', 'status', 'claim-next', 'complete', 'add-task')]
    [string]$Action = 'status',

    [string]$Lane,
    [string]$TaskId,
    [int]$Priority = 10,
    [string]$Title,
    [string]$Objective,
    [string[]]$DoneCriteria,
    [string]$Note
)

$ErrorActionPreference = 'Stop'

$pythonArgs = @('./scripts/swarm_control.py', $Action)

if ($Lane) {
    $pythonArgs += @('--lane', $Lane)
}

if ($TaskId) {
    if ($Action -eq 'complete') {
        $pythonArgs += @('--task-id', $TaskId)
    } elseif ($Action -eq 'add-task') {
        $pythonArgs += @('--task-id', $TaskId)
    }
}

if ($PSBoundParameters.ContainsKey('Priority')) {
    $pythonArgs += @('--priority', $Priority)
}

if ($Title) {
    $pythonArgs += @('--title', $Title)
}

if ($Objective) {
    $pythonArgs += @('--objective', $Objective)
}

if ($DoneCriteria) {
    $pythonArgs += @('--done-criteria')
    $pythonArgs += $DoneCriteria
}

if ($Note) {
    $pythonArgs += @('--note', $Note)
}

& python @pythonArgs
$exitCode = $LASTEXITCODE
exit $exitCode
