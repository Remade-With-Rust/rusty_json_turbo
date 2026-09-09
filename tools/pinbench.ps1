# tools/pinbench.ps1 -- run rjson-bench pinned to one core at High priority.
#
# The measurement core's shape (codec-measurement, rusty_xml bench/pinvs.ps1):
# pin to ONE logical CPU that is not core 0, High priority, and let the bench
# do the in-process ABBA pairing. On this box (i7-14650HX, 8P+8E) logical CPUs
# 0-15 are the P-core threads; the default pin is CPU 2.
#
#   powershell -File tools/pinbench.ps1 -- bench --all --pairs 20
#   powershell -File tools/pinbench.ps1 -Cpu 4 -- null --all
#   powershell -File tools/pinbench.ps1 -Exe target\release\rjson-bench.exe -- bench --cell twitter,dom-parse
#
# The wrapper passes `--pinned cpu<N>/High` so the bench echoes the pin into its
# method line, and the bench sleeps a settle interval before the first number so
# the affinity is in force when it matters. The affinity actually applied is
# read back and printed at the end; if it does not match, do not quote the run.

param(
    [int]$Cpu = 2,
    [string]$Exe = "target\release\rjson-bench.exe",
    # Where the bench's stdout goes. A child started with -NoNewWindow does not
    # reliably follow a redirected parent stdout, so the file is explicit.
    [string]$Out = "",
    [Parameter(ValueFromRemainingArguments = $true)][string[]]$BenchArgs
)

$ErrorActionPreference = 'Stop'
if ($BenchArgs -and $BenchArgs[0] -eq '--') { $BenchArgs = @($BenchArgs | Select-Object -Skip 1) }
if (-not $BenchArgs) { "usage: pinbench.ps1 [-Cpu N] [-Exe path] [-Out file] -- <bench|null> args..."; exit 2 }
if (-not (Test-Path $Exe)) { "error: $Exe not found (cargo build --release -p rusty_json_turbo-bench [--features competitors])"; exit 2 }

$mask = [IntPtr]([int64]1 -shl $Cpu)
$rest = @($BenchArgs | Select-Object -Skip 1)
$argv = @($BenchArgs[0]) + @('--pinned', "cpu$Cpu/High") + $rest
$commit = (git rev-parse --short HEAD 2>$null)
if ($commit) { $argv += @('--commit', $commit) }
$dirty = (git status --porcelain 2>$null)
if ($dirty) { $argv += @('--commit', "$commit-dirty") }

"pinbench: $Exe $($argv -join ' ')"
if ($Out) {
    $dir = Split-Path -Parent $Out
    if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir | Out-Null }
    $p = Start-Process -FilePath $Exe -ArgumentList $argv -PassThru -NoNewWindow -RedirectStandardOutput $Out
} else {
    $p = Start-Process -FilePath $Exe -ArgumentList $argv -PassThru -NoNewWindow
}
$null = $p.Handle          # cache it, or the properties read empty after exit
$p.ProcessorAffinity = $mask
$p.PriorityClass = 'High'
$applied = [int64]$p.ProcessorAffinity
$p.WaitForExit()
"pinbench: cpu=$Cpu requested=0x{0:X} applied=0x{1:X} priority=High cpu-time={2:N0} ms exit={3}" -f `
    [int64]$mask, $applied, $p.TotalProcessorTime.TotalMilliseconds, $p.ExitCode
if ($applied -ne [int64]$mask) { "!! affinity was not applied -- do not quote this run"; exit 3 }
exit $p.ExitCode
