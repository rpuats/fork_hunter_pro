param(
    [Parameter(Mandatory = $true)]
    [string]$Name,

    [int]$DelaySec = 1,

    [ValidateSet('success', 'fail')]
    [string]$Mode = 'success'
)

$ErrorActionPreference = 'Stop'

Write-Output ("job={0} starting delay={1}s mode={2}" -f $Name, $DelaySec, $Mode)
Start-Sleep -Seconds $DelaySec

if ($Mode -eq 'fail') {
    Write-Error ("job={0} requested failure" -f $Name)
    exit 7
}

Write-Output ("job={0} completed" -f $Name)
exit 0
