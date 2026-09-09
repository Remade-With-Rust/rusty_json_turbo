# tools/pinvs.ps1 -- paired A/B between two BINARIES, pinned, ABBA-interleaved.
#
# `rjson-bench bench` compares two arms inside one process. Some things cannot
# be compared that way, and the global allocator is the first of them: a program
# has exactly one, so "system vs rusty_alloc" is two binaries, not two arms.
#
# This runner alternates the two executables pair by pair (ABBA), pins each to
# one core at High priority, and pairs the per-cell times each one reports for
# ITSELF (`rjson-bench solo` prints RESULT lines). Reading each arm's internal
# duration is what keeps process launch out of the number -- a per-invocation
# overhead inflates the shorter arm by a larger fraction, which is how a harness
# manufactures an effect (codec-measurement section 5).
#
#   powershell -File tools/pinvs.ps1 -ExeA a.exe -ExeB b.exe -LabelA system `
#              -LabelB rusty_alloc -Pairs 20 -Bench "solo --all --arm ours"
#
# Pass the SAME exe as A and B to take the process-level null arm: that floor,
# not zero, is what an effect has to clear.

param(
    [Parameter(Mandatory = $true)][string]$ExeA,
    [Parameter(Mandatory = $true)][string]$ExeB,
    [string]$LabelA = "A",
    [string]$LabelB = "B",
    [int]$Cpu = 2,
    [int]$Pairs = 20,
    [string]$Bench = "solo --all --arm ours",
    [string]$Out = "",
    # `NAME=VALUE` set only for that arm. Pass the SAME exe as A and B with
    # different env to A/B two implementations inside ONE binary: same code
    # layout, one knob between them. That is strictly better than comparing two
    # builds, where the layout difference alone has measured +-13%.
    [string]$EnvA = "",
    [string]$EnvB = ""
)

$ErrorActionPreference = 'Stop'
foreach ($e in @($ExeA, $ExeB)) {
    if (-not (Test-Path $e)) { "error: $e not found"; exit 2 }
}
$mask = [IntPtr]([int64]1 -shl $Cpu)
$benchArgs = @($Bench -split '\s+' | Where-Object { $_ })
$tmp = Join-Path $env:TEMP ("pinvs-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $tmp | Out-Null

function Invoke-Solo([string]$exe, [string]$tag, [string]$envSpec) {
    $log = Join-Path $tmp "$tag.txt"
    $argv = $benchArgs + @('--pinned', "cpu$Cpu/High")
    $restore = $null
    if ($envSpec) {
        $kv = $envSpec -split '=', 2
        $restore = @{ Name = $kv[0]; Old = [Environment]::GetEnvironmentVariable($kv[0]) }
        [Environment]::SetEnvironmentVariable($kv[0], $kv[1])
    }
    try {
        $p = Start-Process -FilePath $exe -ArgumentList $argv -PassThru -NoNewWindow -RedirectStandardOutput $log
    } finally {
        if ($restore) { [Environment]::SetEnvironmentVariable($restore.Name, $restore.Old) }
    }
    $null = $p.Handle
    $p.ProcessorAffinity = $mask
    $p.PriorityClass = 'High'
    $applied = [int64]$p.ProcessorAffinity
    $p.WaitForExit()
    if ($applied -ne [int64]$mask) { throw "affinity not applied to $exe (got 0x{0:X})" -f $applied }
    if ($p.ExitCode -ne 0) { throw "$exe exited $($p.ExitCode) -- see $log" }
    $rows = @{}
    $binary = ""
    foreach ($line in Get-Content $log) {
        if ($line -like 'BINARY|*') { $binary = $line }
        if ($line -like 'RESULT|*') {
            $f = $line -split '\|'
            # RESULT|file|column|arm|min_ns|median_ns|iters|bytes
            $rows["$($f[1])/$($f[2])"] = [pscustomobject]@{
                MinNs = [double]$f[4]; Bytes = [int64]$f[7]; Iters = [int]$f[6]
            }
        }
    }
    if ($rows.Count -eq 0) { throw "$exe produced no RESULT lines -- is the verb 'solo'? see $log" }
    [pscustomobject]@{ Rows = $rows; Binary = $binary }
}

function Get-Sha([string]$p) { (Get-FileHash -Algorithm SHA256 -Path $p).Hash.Substring(0, 16) }

$lines = New-Object System.Collections.Generic.List[string]
function Emit([string]$s) { $lines.Add($s); Write-Host $s }

Emit "pinvs: $LabelA=$ExeA ($(Get-Sha $ExeA))"
Emit "pinvs: $LabelB=$ExeB ($(Get-Sha $ExeB))"
if ((Get-Sha $ExeA) -eq (Get-Sha $ExeB)) { Emit "pinvs: NULL ARM -- identical binaries, this is the floor" }
Emit "pinvs: cpu=$Cpu priority=High pairs=$Pairs bench='$Bench'"

# cell -> list of paired ratios, plus the per-arm times behind them
$ratios = @{}
$timesA = @{}
$timesB = @{}
$bytes = @{}
$binaryA = ""
$binaryB = ""

for ($i = 0; $i -lt $Pairs; $i++) {
    # ABBA: alternate which binary leads, so drift cannot settle on one arm.
    if ($i % 2 -eq 0) {
        $ra = Invoke-Solo $ExeA "a$i" $EnvA; $rb = Invoke-Solo $ExeB "b$i" $EnvB
    } else {
        $rb = Invoke-Solo $ExeB "b$i" $EnvB; $ra = Invoke-Solo $ExeA "a$i" $EnvA
    }
    if (-not $binaryA) { $binaryA = $ra.Binary; $binaryB = $rb.Binary }
    foreach ($cell in $ra.Rows.Keys) {
        if (-not $rb.Rows.ContainsKey($cell)) { throw "cell $cell missing from $LabelB" }
        $a = $ra.Rows[$cell]; $b = $rb.Rows[$cell]
        # Work parity: the two binaries must have done the same job.
        if ($a.Bytes -ne $b.Bytes) { throw "work parity: $cell bytes differ ($($a.Bytes) vs $($b.Bytes))" }
        if (-not $ratios.ContainsKey($cell)) {
            $ratios[$cell] = New-Object System.Collections.Generic.List[double]
            $timesA[$cell] = New-Object System.Collections.Generic.List[double]
            $timesB[$cell] = New-Object System.Collections.Generic.List[double]
            $bytes[$cell] = $a.Bytes
        }
        $ratios[$cell].Add($a.MinNs / $b.MinNs)
        $timesA[$cell].Add($a.MinNs)
        $timesB[$cell].Add($b.MinNs)
    }
    $lead = if ($i % 2 -eq 0) { $LabelA } else { $LabelB }
    Write-Host ("  pair {0,2} lead={1}" -f ($i + 1), $lead)
}

function Med($xs) { $s = @($xs | Sort-Object); $s[[int]([math]::Floor($s.Count / 2))] }

Emit ""
Emit "| file | column | $LabelA | $LabelB | $LabelA/$LabelB time (median [min, max]) | wins, z |"
Emit "|---|---|---:|---:|---:|---:|"
foreach ($cell in ($ratios.Keys | Sort-Object)) {
    $r = $ratios[$cell]
    $n = $r.Count
    $wins = @($r | Where-Object { $_ -lt 1.0 }).Count
    $z = ($wins - $n / 2.0) / (0.5 * [math]::Sqrt($n))
    $medR = Med $r
    $lo = ($r | Measure-Object -Minimum).Minimum
    $hi = ($r | Measure-Object -Maximum).Maximum
    $mbsA = $bytes[$cell] / (Med $timesA[$cell]) * 1e3
    $mbsB = $bytes[$cell] / (Med $timesB[$cell]) * 1e3
    $parts = $cell -split '/'
    Emit ("| {0} | {1} | {2} **{3:N0} MB/s** | {4} {5:N0} MB/s | **{6:N3}x** [{7:N3}, {8:N3}] | {9}/{10}, z = {11:+0.00;-0.00} |" -f `
        $parts[0], $parts[1], $LabelA, $mbsA, $LabelB, $mbsB, $medR, $lo, $hi, $wins, $n, $z)
}
Emit ""
Emit ("method: PROCESS-level paired A/B between two binaries (a global allocator is one per program, " +
      "so it cannot be an in-process arm); each binary reports its own per-cell time via `rjson-bench solo` " +
      "so process launch is outside the number; leading binary alternated per pair (ABBA); pairs=$Pairs; " +
      "statistic=min per-iteration time in each arm-sample window; verdict=median of paired ratios + paired " +
      "wins with z; pinned=cpu$Cpu/High, affinity read back per run; work parity: output/input bytes asserted " +
      "equal per cell per pair; machine=$env:PROCESSOR_IDENTIFIER")
Emit "A: $binaryA"
Emit "B: $binaryB"

Remove-Item -Recurse -Force $tmp
if ($Out) {
    $dir = Split-Path -Parent $Out
    if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir | Out-Null }
    $lines | Out-File -FilePath $Out -Encoding utf8
    "pinvs: wrote $Out"
}
