# corpus/fetch.ps1 -- re-fetch the corpus from its pinned source and verify it.
#
# The files are committed; this script exists so their provenance can be
# re-established from scratch and so a tampered checkout is caught. It reads
# HASHES.txt (path + SHA-256), downloads each path from serde-rs/json-benchmark
# at the pinned commit, and refuses any byte that does not match.
#
#   powershell -File corpus/fetch.ps1            # verify committed files
#   powershell -File corpus/fetch.ps1 -Download  # re-download, then verify

param([switch]$Download)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$pin = '17b13dd'
$base = "https://raw.githubusercontent.com/serde-rs/json-benchmark/$pin/data"

$manifest = Get-Content (Join-Path $root 'HASHES.txt')
$bad = 0
foreach ($line in $manifest) {
    if (-not $line.Trim()) { continue }
    $parts = $line -split '\s+', 2
    $expected = $parts[0].ToLowerInvariant()
    $rel = $parts[1].Trim()
    $dest = Join-Path $root $rel
    if ($Download) {
        $dir = Split-Path -Parent $dest
        if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir | Out-Null }
        Invoke-WebRequest -Uri "$base/$rel" -OutFile $dest -UseBasicParsing
    }
    if (-not (Test-Path $dest)) { "MISSING  $rel"; $bad++; continue }
    $actual = (Get-FileHash -Algorithm SHA256 -Path $dest).Hash.ToLowerInvariant()
    if ($actual -ne $expected) { "MISMATCH $rel`n  expected $expected`n  actual   $actual"; $bad++ }
    else { "ok       $rel" }
}
if ($bad -gt 0) { "error: $bad file(s) failed verification"; exit 1 }
"ok: $($manifest.Count) files verified against HASHES.txt (json-benchmark@$pin)"
