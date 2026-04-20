$folders = @(
    "$env:LOCALAPPDATA\CrashDumps",
    "$env:LOCALAPPDATA\D3DSCache",
    "$env:LOCALAPPDATA\IconCache",
    "$env:TEMP"
)

foreach ($f in $folders) {
    if (Test-Path $f) {
        Get-ChildItem $f -Recurse -Force -ErrorAction SilentlyContinue | 
            Where-Object { $_.LastWriteTime -lt (Get-Date).AddDays(-7) } | 
            Remove-Item -Force -ErrorAction SilentlyContinue
    }
}

$logDirs = @(
    "C:\Windows\Logs",
    "C:\Windows\Temp"
)

foreach ($d in $logDirs) {
    Get-ChildItem $d -File -ErrorAction SilentlyContinue | 
        Where-Object { $_.LastWriteTime -lt (Get-Date).AddDays(-7) } | 
        Remove-Item -Force -ErrorAction SilentlyContinue
}

Write-Host "Temp files older than 7 days cleaned"
Write-Host "Checking disk..."
Get-PSDrive C | Select-Object Used,Free