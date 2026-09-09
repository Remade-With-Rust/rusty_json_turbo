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
    [Parameter(ValueFromRemainingArguments = $true)][string[]]$BenchArgs
)

$ErrorActionPreference = 'Stop'
if ($BenchArgs -and $BenchArgs[0] -eq '--') { $BenchArgs = $BenchArgs[1..($BenchArgs.Count - 1)] }
if (-not (Test-Path $Exe)) { "error: $Exe not found (cargo build --release -p rusty_json_turbo-bench [--features competitors])"; exit 2 }

$mask = [IntPtr]([int64]1 -shl $Cpu)
$argv = @($BenchArgs[0]) + @('--pinned', "cpu$Cpu/High") + $BenchArgs[1..($BenchArgs.Count - 1)]
$commit = (git rev-parse --short HEAD 2>$null)
if ($commit) { $argv += @('--commit', $commit) }

"pinbench: $Exe $($argv -join ' ')"
$p = Start-Process -FilePath $Exe -ArgumentList $argv -PassThru -NoNewWindow
$null = $p.Handle          # cache it, or the properties read empty after exit
$p.ProcessorAffinity = $mask
$p.PriorityClass = 'High'
$applied = [int64]$p.ProcessorAffinity
$p.WaitForExit()
"pinbench: cpu=$Cpu requested=0x{0:X} applied=0x{1:X} priority=High cpu-time={2:N0} ms exit={3}" -f `
    [int64]$mask, $applied, $p.TotalProcessorTime.TotalMilliseconds, $p.ExitCode
if ($applied -ne [int64]$mask) { "!! affinity was not applied -- do not quote this run"; exit 3 }
exit $p.ExitCode
