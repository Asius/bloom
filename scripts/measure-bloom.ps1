param(
    [ValidateRange(2, 300)][int]$Seconds = 10,
    [ValidateRange(1, 30)][int]$Interval = 2
)

# Read-only, opt-in measurement. Output contains totals only, no paths or titles.
$ErrorActionPreference = 'Stop'
function Read-BloomUsage {
    $inventory = @(Get-CimInstance Win32_Process)
    $included = [System.Collections.Generic.HashSet[int]]::new()
    foreach ($entry in $inventory) {
        if ($entry.Name -ieq 'bloom.exe') { [void]$included.Add([int]$entry.ProcessId) }
    }
    do {
        $added = $false
        foreach ($entry in $inventory) {
            if ($included.Contains([int]$entry.ParentProcessId) -and $entry.Name -ieq 'msedgewebview2.exe') {
                $added = $included.Add([int]$entry.ProcessId) -or $added
            }
        }
    } while ($added)
    $cpu = @{}
    [long]$privateBytes = 0
    foreach ($entry in $inventory) {
        if (-not $included.Contains([int]$entry.ProcessId)) { continue }
        try {
            $process = Get-Process -Id $entry.ProcessId -ErrorAction Stop
            # Include start time to avoid attributing a recycled PID to an old process.
            $key = '{0}:{1}' -f $process.Id, $process.StartTime.ToUniversalTime().Ticks
            $cpu[$key] = $process.TotalProcessorTime.TotalSeconds
            $privateBytes += $process.PrivateMemorySize64
        } catch { continue } # A process can exit between the two read-only calls.
    }
    [pscustomobject]@{ Cpu = $cpu; PrivateBytes = $privateBytes; Time = [datetime]::UtcNow }
}

$previous = Read-BloomUsage
if ($previous.Cpu.Count -eq 0) { throw 'Bloom is not running. Start the installed app first.' }
$deadline = [datetime]::UtcNow.AddSeconds($Seconds)
do {
    Start-Sleep -Seconds $Interval
    $current = Read-BloomUsage
    [double]$cpuSeconds = 0
    foreach ($key in $current.Cpu.Keys) {
        if ($previous.Cpu.ContainsKey($key)) {
            $cpuSeconds += [math]::Max(0, $current.Cpu[$key] - $previous.Cpu[$key])
        }
    }
    [pscustomobject]@{
        Processes = $current.Cpu.Count
        CpuPercent = [math]::Round(100 * $cpuSeconds / [math]::Max(0.001, ($current.Time - $previous.Time).TotalSeconds) / [Environment]::ProcessorCount, 2)
        PrivateMiB = [math]::Round($current.PrivateBytes / 1MB, 1)
    }
    $previous = $current
} while ([datetime]::UtcNow -lt $deadline)
