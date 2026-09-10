# tools/biasvs.ps1 -- what did WE change, net of instantiation bias?
#
# THE PROBLEM THIS SOLVES. `rjson-bench bench` compares ours against upstream
# inside one process, and that number is not the gain: at M0, when our source
# was BYTE-IDENTICAL to upstream, the same comparison read anywhere from 0.819x
# to 1.094x depending on the cell. Two separate crate instantiations get laid
# out and inlined differently, and for some cells that is worth 18%. Quoting
# ours-vs-upstream as a gain silently folds that bias in.
#
# THE INSTRUMENT. Two binaries: a REFERENCE whose library code we did not
# change, and the CURRENT one. Each reports ours-vs-upstream for itself, and
# each has its own upstream arm measured in the same window as its own ours
# arm. So each binary yields a ratio that is already normalised against
# upstream, and box drift between the two runs cancels:
#
#     bias    = reference_ours / reference_upstream    (what layout alone gives)
#     current = current_ours   / current_upstream      (what we report today)
#     NET     = bias / current                         (what WE actually changed)
#
# The two binaries are interleaved round by round with the leader alternating,
# so drift cannot settle on either one. Every ratio is a median over rounds.
#
# Both binaries must expose the same cells, which is why the reference is
# chosen per scenario: M0 (`b3df966`) for S1-S3, where our source was
# identical to upstream, and a later commit for S4, which did not exist at M0.

param(
    [Parameter(Mandatory = $true)][string]$RefExe,
    [Parameter(Mandatory = $true)][string]$CurExe,
    [string]$RefLabel = "reference",
    [int]$Cpu = 2,
    [int]$Rounds = 9,
    [int]$Pairs = 9,
    [int]$WindowMs = 200,
    [string]$Cells = "--all",
    [string]$Out = ""
)

$ErrorActionPreference = 'Stop'
foreach ($e in @($RefExe, $CurExe)) {
    if (-not (Test-Path $e)) { "error: $e not found"; exit 2 }
}
$mask = [IntPtr]([int64]1 -shl $Cpu)
$tmp = Join-Path $env:TEMP ("biasvs-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $tmp | Out-Null

$lines = New-Object System.Collections.Generic.List[string]
function Emit([string]$s) { $lines.Add($s); Write-Host $s }

# One `bench` run, pinned, returning cell -> ours/upstream ratio.
function Invoke-Bench([string]$exe, [string]$tag) {
    $log = Join-Path $tmp "$tag.txt"
    $argv = @('bench') + ($Cells -split '\s+' | Where-Object { $_ }) +
            @('--pairs', $Pairs, '--window-ms', $WindowMs, '--pinned', "cpu$Cpu/High")
    $p = Start-Process -FilePath $exe -ArgumentList $argv -PassThru -NoNewWindow -RedirectStandardOutput $log
    $null = $p.Handle
    $p.ProcessorAffinity = $mask
    $p.PriorityClass = 'High'
    $applied = [int64]$p.ProcessorAffinity
    $p.WaitForExit()
    if ($applied -ne [int64]$mask) { throw "affinity not applied to $exe" }
    if ($p.ExitCode -ne 0) { throw "$exe exited $($p.ExitCode) -- see $log" }
    $out = @{}
    foreach ($line in Get-Content $log) {
        # | file | column | ours **N MB/s** | upstream N MB/s | **R.RRRx** [..] | w/p, z = z |
        if ($line -match '^\|\s*([\w\-]+)\s*\|\s*([\w\-]+)\s*\|.*\|\s*\*\*([\d.]+)x\*\*') {
            $out["$($matches[1])/$($matches[2])"] = [double]$matches[3]
        }
    }
    if ($out.Count -eq 0) { throw "$exe produced no parsable rows -- see $log" }
    $out
}

Emit "biasvs: reference=$RefExe ($RefLabel)"
Emit "biasvs: current=$CurExe"
Emit "biasvs: cpu=$Cpu rounds=$Rounds pairs=$Pairs window=$WindowMs ms cells='$Cells'"

$refRatios = @{}
$curRatios = @{}
for ($r = 0; $r -lt $Rounds; $r++) {
    if ($r % 2 -eq 0) {
        $a = Invoke-Bench $RefExe "ref$r"; $b = Invoke-Bench $CurExe "cur$r"
    } else {
        $b = Invoke-Bench $CurExe "cur$r"; $a = Invoke-Bench $RefExe "ref$r"
    }
    foreach ($cell in $a.Keys) {
        if (-not $refRatios.ContainsKey($cell)) {
            $refRatios[$cell] = New-Object System.Collections.Generic.List[double]
            $curRatios[$cell] = New-Object System.Collections.Generic.List[double]
        }
        $refRatios[$cell].Add($a[$cell])
        if ($b.ContainsKey($cell)) { $curRatios[$cell].Add($b[$cell]) }
    }
    Write-Host ("  round {0,2} lead={1}" -f ($r + 1), $(if ($r % 2 -eq 0) { "ref" } else { "cur" }))
}

function Med($xs) { $s = @($xs | Sort-Object); $s[[int]([math]::Floor($s.Count / 2))] }

Emit ""
Emit "| file | column | bias ($RefLabel) | reported now | NET (what we changed) |"
Emit "|---|---|---:|---:|---:|"
foreach ($cell in ($refRatios.Keys | Sort-Object)) {
    if ($curRatios[$cell].Count -eq 0) { continue }
    $bias = Med $refRatios[$cell]
    $cur = Med $curRatios[$cell]
    $net = $bias / $cur
    $parts = $cell -split '/'
    # NET > 1 means we made it faster than the reference did, net of bias.
    Emit ("| {0} | {1} | {2:N3}x | {3:N3}x | **{4:N3}x** |" -f $parts[0], $parts[1], $bias, $cur, $net)
}

Emit ""
Emit ("method: TWO BINARIES interleaved round by round with the leader alternated, each pinned to " +
      "cpu$Cpu/High; each binary runs `rjson-bench bench`, which measures its OWN ours arm against " +
      "its OWN upstream arm in the same window, so each round yields a ratio already normalised " +
      "against upstream and drift between runs cancels; rounds=$Rounds, pairs=$Pairs per run, " +
      "window=$WindowMs ms; statistic=median over rounds of each binary's median paired ratio. " +
      "bias = the reference's ours/upstream (at M0 the two sources were byte-identical, so this " +
      "is layout and instantiation alone); NET = bias / current, i.e. what our code changed with " +
      "the per-cell bias divided out. NET > 1 is a real gain. machine=$env:PROCESSOR_IDENTIFIER")

Remove-Item -Recurse -Force $tmp
if ($Out) {
    $dir = Split-Path -Parent $Out
    if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir | Out-Null }
    $lines | Out-File -FilePath $Out -Encoding utf8
    "biasvs: wrote $Out"
}
