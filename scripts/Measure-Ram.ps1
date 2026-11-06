param(
    [Parameter(Mandatory = $true)][int]$RootPid,
    [ValidateRange(1, 60)][int]$Samples = 5,
    [ValidateRange(1, 60)][int]$IntervalSeconds = 2,
    [string]$OutputPath
)
$ErrorActionPreference = 'Stop'
$measurements = @()
for ($sample = 0; $sample -lt $Samples; $sample++) {
    $all = @(Get-CimInstance Win32_Process)
    $root = $all | Where-Object ProcessId -eq $RootPid
    if (-not $root) { throw "Root process $RootPid is not running" }
    $ids = [System.Collections.Generic.HashSet[int]]::new()
    [void]$ids.Add($RootPid)
    do {
        $added = $false
        foreach ($item in $all) {
            if ($ids.Contains([int]$item.ParentProcessId) -and $ids.Add([int]$item.ProcessId)) { $added = $true }
        }
    } while ($added)
    $privateWorkingSets = @{}
    Get-CimInstance Win32_PerfRawData_PerfProc_Process | ForEach-Object {
        $privateWorkingSets[[int]$_.IDProcess] = [long]$_.WorkingSetPrivate
    }
    $rows = @()
    foreach ($item in $all) {
        if (-not $ids.Contains([int]$item.ProcessId)) { continue }
        try { $process = Get-Process -Id $item.ProcessId -ErrorAction Stop } catch { continue }
        $group = if ($item.ProcessId -eq $RootPid) { 'app' }
            elseif ($item.Name -eq 'msedgewebview2.exe') { 'webview' }
            elseif ($item.Name -eq 'omp.exe') { 'omp' }
            else { 'helpers' }
        $rows += [pscustomobject]@{
            pid = [int]$item.ProcessId
            parent_pid = [int]$item.ParentProcessId
            name = $item.Name
            group = $group
            working_set_bytes = [long]$process.WorkingSet64
            private_working_set_bytes = [long]$privateWorkingSets[[int]$item.ProcessId]
            private_commit_bytes = [long]$process.PrivateMemorySize64
        }
    }
    $measurements += [pscustomobject]@{
        timestamp = (Get-Date).ToUniversalTime().ToString('o')
        root_pid = $RootPid
        total_working_set_bytes = [long](($rows | Measure-Object working_set_bytes -Sum).Sum)
        total_private_working_set_bytes = [long](($rows | Measure-Object private_working_set_bytes -Sum).Sum)
        total_private_commit_bytes = [long](($rows | Measure-Object private_commit_bytes -Sum).Sum)
        processes = $rows
    }
    if ($sample + 1 -lt $Samples) { Start-Sleep -Seconds $IntervalSeconds }
}
$result = [pscustomobject]@{
    method = 'Windows process tree; working set includes shared pages per process; private working set excludes shared pages; private commit is committed virtual memory, not resident RAM. Measurement PowerShell process excluded.'
    samples = $measurements
}
$json = $result | ConvertTo-Json -Depth 8
if ($OutputPath) { $json | Set-Content -LiteralPath $OutputPath -Encoding UTF8 }
$json
