$procs = Get-Process code,node,python,rustc -ErrorAction SilentlyContinue
foreach ($p in $procs) {
    $p.PriorityClass = "High"
}
Write-Host "Priority HIGH set for dev processes"

$bcdedit = "bcdedit /set disabledetect"
$bcdedit
Write-Host "Boot optimized"