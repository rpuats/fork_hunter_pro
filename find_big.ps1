$scanDirs = @(
    "C:\Users\Administrator\AppData\Local",
    "C:\Users\Administrator\AppData\Roaming",
    "C:\Users\Administrator\Documents",
    "C:\Users\Administrator\Downloads",
    "C:\Windows\System32\config"
)

$bignames = @()

foreach ($dir in $scanDirs) {
    if (Test-Path $dir) {
        $items = Get-ChildItem $dir -Directory -ErrorAction SilentlyContinue | ForEach-Object {
            $size = (Get-ChildItem $_.FullName -Recurse -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum
            [PSCustomObject]@{Dir=$_.Name; GB=[math]::Round($size/1GB,2)}
        } | Sort-Object GB -Descending | Select-Object -First 5
        $bignames += $items
    }
}

$bignames | Sort-Object GB -Descending | Select-Object -First 20