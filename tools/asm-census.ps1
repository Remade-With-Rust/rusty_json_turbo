# tools/asm-census.ps1 -- EMITTED-ASM CENSUS of the shipped library.
#
# WHAT THIS IS. A counter, not a clock. It reads the assembly rustc actually
# emitted for the `serde_json` lib target and counts four things:
#
#   1. calls/jumps into the panic machinery (bounds checks, slice-index fails,
#      unwrap/expect fails, core::panicking::*),
#   2. calls to memcpy / memmove / memset, split by whether the length argument
#      was a compile-time immediate or a runtime register,
#   3. packed / SIMD instructions (evidence the compiler auto-vectorised), and
#   4. the largest emitted functions by instruction count.
#
# WHAT THIS IS NOT. It never claims a timing. A static instruction count is not
# a duration: an instruction inside a cold error block costs nothing and one
# inside a per-byte loop costs everything. Every number here PRICES A BRICK
# BEFORE IT IS BUILT -- it says whether there is anything there to remove. The
# verdict on whether removing it was worth it belongs to the bench harness and
# the runtime census counters, never to this file.
#
# HOW THE ASM IS PRODUCED (the method this tool reports at the end):
#
#   cargo rustc --release --lib -- --emit asm
#       -C llvm-args=-x86-asm-syntax=intel -C debuginfo=1
#
# `-C debuginfo=1` is not cosmetic: the release profile carries no debug info,
# and WITHOUT it there are no line-table directives in the .s, so every count
# is whole-crate with no per-source-file attribution. On x86_64-pc-windows-msvc
# rustc emits CodeView line tables (`.cv_file` / `.cv_func_id` /
# `.cv_inline_site_id` / `.cv_loc`), not DWARF `.file` / `.loc`; this tool reads
# both and says in the method line which it found.
#
# ATTRIBUTION. Because the whole crate is one codegen unit, de.rs, ser.rs and
# read.rs land in ONE .s. Each instruction is attributed two ways:
#
#   own    -- the innermost inline frame's source file. "Code written in this
#             file", after inlining moved it around.
#   owner  -- the innermost frame in the inline chain that is one of the three
#             files of interest. A bounds check whose innermost frame is
#             core/src/slice/index.rs but whose caller chain passes through
#             read.rs is read.rs's bounds check, and `owner` says so.
#
# Usage:
#   powershell -NoProfile -File tools/asm-census.ps1
#   powershell -NoProfile -File tools/asm-census.ps1 -Out docs/ASM-CENSUS.md -Top 25
#   powershell -NoProfile -File tools/asm-census.ps1 -Asm path\to\some.s
#
# House constraints honoured: Windows PowerShell 5.1 -- no pipeline chain
# operators, no ternary, no null-coalescing, no -AsHashtable; if/else
# throughout. ASCII-only output, deterministic ordering.

param(
    # Path to the .s to read. Omitted: auto-discover the newest .s under
    # target/**/ that carries line tables.
    [string]$Asm = "",
    # Optional markdown output file. The report also always goes to stdout.
    [string]$Out = "",
    # How many of the largest functions to list.
    [int]$Top = 25,
    # Skip `rustfilt`; use the built-in length-prefix path sketch instead.
    [switch]$NoDemangle
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Off

$RepoRoot = Split-Path -Parent $PSScriptRoot
if (-not $RepoRoot) { $RepoRoot = (Get-Location).Path }

# The invocation this tool documents. Reported verbatim in the method line so a
# reader can reproduce the .s these numbers came from.
$MethodCmd = 'cargo rustc --release --lib -- --emit asm -C llvm-args=-x86-asm-syntax=intel -C debuginfo=1'

### helpers ###################################################################

function ConvertTo-Ascii([string]$s) {
    if ($null -eq $s) { return "" }
    $sb = New-Object System.Text.StringBuilder
    foreach ($ch in $s.ToCharArray()) {
        $code = [int]$ch
        if ($code -ge 32 -and $code -lt 127) { [void]$sb.Append($ch) }
        elseif ($code -eq 10) { [void]$sb.Append($ch) }
        elseif ($code -eq 13) { [void]$sb.Append($ch) }
        elseif ($code -eq 9) { [void]$sb.Append(' ') }
        else { [void]$sb.Append('?') }
    }
    return $sb.ToString()
}

function Add-Count($tbl, $key, [int]$n) {
    if ($tbl.ContainsKey($key)) { $tbl[$key] = $tbl[$key] + $n }
    else { $tbl[$key] = $n }
}

function Get-Count($tbl, $key) {
    if ($tbl.ContainsKey($key)) { return [int]$tbl[$key] }
    return 0
}

# Fallback demangler: rustc's v0 and legacy manglings are both sequences of
# <decimal-length><identifier>. Walking those gives a readable path sketch.
# Not a real demangler; labelled as a sketch wherever it is used.
function Get-PathSketch([string]$sym) {
    $parts = @()
    $i = 0
    $n = $sym.Length
    while ($i -lt $n) {
        $c = $sym[$i]
        if ($c -ge '0' -and $c -le '9') {
            $j = $i
            while ($j -lt $n -and $sym[$j] -ge '0' -and $sym[$j] -le '9') { $j++ }
            $len = 0
            $ok = [int]::TryParse($sym.Substring($i, $j - $i), [ref]$len)
            if ($ok -and $len -gt 0 -and ($j + $len) -le $n) {
                $cand = $sym.Substring($j, $len)
                if ($cand -match '^[A-Za-z_][A-Za-z0-9_]*$') {
                    $parts += $cand
                    $i = $j + $len
                    continue
                }
            }
            $i = $j
            continue
        }
        $i++
    }
    if ($parts.Count -eq 0) { return $sym }
    return ($parts -join '::')
}

# Display form for a source path: the last two components, so the many `mod.rs`
# and `index.rs` files in core cannot be confused with each other.
function Get-ShortPath([string]$p) {
    if ($p -eq "") { return "" }
    $norm = $p.Replace('\', '/')
    $segs = @($norm.Split('/') | Where-Object { $_ -ne "" })
    # Three segments when the middle one is `src`, so a dependency's
    # `itoa-1.0.15/src/lib.rs` cannot be read as this crate's own `src/lib.rs`.
    if ($segs.Count -ge 3 -and $segs[$segs.Count - 2] -eq 'src') {
        return ($segs[$segs.Count - 3] + '/src/' + $segs[$segs.Count - 1])
    }
    if ($segs.Count -ge 2) { return ($segs[$segs.Count - 2] + '/' + $segs[$segs.Count - 1]) }
    if ($segs.Count -eq 1) { return $segs[0] }
    return $p
}

function Get-Truncated([string]$s, [int]$max) {
    if ($s.Length -le $max) { return $s }
    return $s.Substring(0, $max - 3) + '...'
}

function Format-MdCell([string]$s) {
    return (ConvertTo-Ascii $s).Replace('|', '\|')
}

### 1. locate the .s #########################################################

$discoveryNote = ""
if ($Asm -eq "") {
    $searchRoot = Join-Path $RepoRoot 'target'
    if (-not (Test-Path $searchRoot)) {
        Write-Error "no target/ directory under $RepoRoot; run: $MethodCmd"
        exit 2
    }
    $cands = @(Get-ChildItem -Path $searchRoot -Recurse -Filter *.s -File -ErrorAction SilentlyContinue |
        Sort-Object -Property @{Expression = 'LastWriteTimeUtc'; Descending = $true}, @{Expression = 'FullName'; Descending = $false})
    if ($cands.Count -eq 0) {
        Write-Error "no .s found under $searchRoot; run: $MethodCmd"
        exit 2
    }
    $picked = $null
    foreach ($c in $cands) {
        $hit = Select-String -Path $c.FullName -Pattern '^\s*\.(cv_loc|loc)\s' -List -ErrorAction SilentlyContinue
        if ($hit) { $picked = $c; break }
    }
    if ($null -eq $picked) {
        $picked = $cands[0]
        $discoveryNote = "WARNING: no candidate .s carried line tables, so per-file attribution is unavailable. Rebuild with -C debuginfo=1."
    }
    $Asm = $picked.FullName
    if ($cands.Count -gt 1) {
        $discoveryNote = ($discoveryNote + " Auto-discovery picked the newest line-table-carrying .s of " + $cands.Count + " under target/.").Trim()
    }
}

if (-not (Test-Path $Asm)) {
    Write-Error "asm file not found: $Asm"
    exit 2
}
$asmItem = Get-Item $Asm
$asmSize = [int64]$asmItem.Length
if ($asmSize -lt 65536) {
    $discoveryNote = ($discoveryNote + " WARNING: " + $asmItem.Name + " is only " + $asmSize + " bytes -- suspiciously small for this crate; check the build really emitted asm for the lib.").Trim()
}
$asmSha = (Get-FileHash -Path $Asm -Algorithm SHA256).Hash

### 2. read + parse ##########################################################

$lines = [System.IO.File]::ReadAllLines($Asm)

$isIntel = $false
foreach ($l in $lines) { if ($l -match '\.intel_syntax') { $isIntel = $true; break } }

$fileOfId    = @{}   # cv/dwarf file id -> source path
$fileSum     = @{}   # source path -> "<kind>:<hex>" checksum recorded by the compiler
$siteParent  = @{}   # inline site id -> parent frame id
$siteFile    = @{}   # inline site id -> file id of the CALL SITE (parent frame)
$rootFuncIds = @{}   # cv_func_id ids
$cvLocCount  = 0
$dwLocCount  = 0

# Files of interest, resolved against the repo root so a dependency that also
# ships a src/de.rs cannot be mistaken for ours.
$interest = New-Object System.Collections.Specialized.OrderedDictionary
foreach ($nm in @('de.rs', 'ser.rs', 'read.rs')) {
    $full = (Join-Path (Join-Path $RepoRoot 'src') $nm)
    $interest[$nm] = $full.ToLowerInvariant()
}
$bucketNames = @('de.rs', 'ser.rs', 'read.rs', 'crate-other', 'rust-std', 'dependency', 'unattributed')
$rootLower = $RepoRoot.ToLowerInvariant()

$bucketOfPath = @{}
function Get-Bucket([string]$path) {
    if ($path -eq "") { return 'unattributed' }
    if ($bucketOfPath.ContainsKey($path)) { return $bucketOfPath[$path] }
    $lp = $path.ToLowerInvariant()
    $b = $null
    foreach ($k in $interest.Keys) {
        if ($lp -eq $interest[$k]) { $b = $k; break }
    }
    if ($null -eq $b) {
        if ($lp.StartsWith($rootLower + '\') -or $lp.StartsWith($rootLower + '/')) { $b = 'crate-other' }
        elseif ($lp -like '*\lib\rustlib\src\rust\library\*') { $b = 'rust-std' }
        elseif ($lp -like '*/lib/rustlib/src/rust/library/*') { $b = 'rust-std' }
        else { $b = 'dependency' }
    }
    $bucketOfPath[$path] = $b
    return $b
}

# Pass 1: directives that must be known before any instruction is classified.
foreach ($raw in $lines) {
    $t = $raw.TrimStart()
    if ($t.Length -eq 0) { continue }
    if ($t[0] -ne '.') { continue }
    if ($t.StartsWith('.cv_file')) {
        # .cv_file <id> "<path>" "<checksum-hex>" <kind>   -- kind 1 = MD5, 3 = SHA256
        $m = [regex]::Match($t, '^\.cv_file\s+(\d+)\s+"((?:[^"\\]|\\.)*)"(?:\s+"([0-9A-Fa-f]*)"\s+(\d+))?')
        if ($m.Success) {
            $id = [int]$m.Groups[1].Value
            $p = $m.Groups[2].Value.Replace('\\', '\')
            if (-not $fileOfId.ContainsKey($id)) { $fileOfId[$id] = $p }
            if ($m.Groups[3].Success -and $m.Groups[3].Value -ne "") {
                if (-not $fileSum.ContainsKey($p)) {
                    $fileSum[$p] = $m.Groups[4].Value + ':' + $m.Groups[3].Value.ToUpperInvariant()
                }
            }
        }
        continue
    }
    if ($t.StartsWith('.file')) {
        # DWARF: .file <id> "<dir>" "<name>"  or  .file <id> "<path>"
        $m = [regex]::Match($t, '^\.file\s+(\d+)\s+"((?:[^"\\]|\\.)*)"(?:\s+"((?:[^"\\]|\\.)*)")?')
        if ($m.Success) {
            $id = [int]$m.Groups[1].Value
            $a = $m.Groups[2].Value.Replace('\\', '\')
            $b2 = $m.Groups[3].Value
            if ($b2) { $p = (Join-Path $a ($b2.Replace('\\', '\'))) } else { $p = $a }
            if (-not $fileOfId.ContainsKey($id)) { $fileOfId[$id] = $p }
        }
        continue
    }
    if ($t.StartsWith('.cv_func_id')) {
        $m = [regex]::Match($t, '^\.cv_func_id\s+(\d+)')
        if ($m.Success) { $rootFuncIds[[int]$m.Groups[1].Value] = $true }
        continue
    }
    if ($t.StartsWith('.cv_inline_site_id')) {
        $m = [regex]::Match($t, '^\.cv_inline_site_id\s+(\d+)\s+within\s+(\d+)\s+inlined_at\s+(\d+)\s+(\d+)')
        if ($m.Success) {
            $sid = [int]$m.Groups[1].Value
            $siteParent[$sid] = [int]$m.Groups[2].Value
            $siteFile[$sid] = [int]$m.Groups[3].Value
        }
        continue
    }
}

# Inline-chain resolution, memoised on (frame id, innermost file id).
$chainCache = @{}
function Get-Chain([int]$frameId, [int]$innerFileId) {
    $key = "$frameId/$innerFileId"
    if ($chainCache.ContainsKey($key)) { return $chainCache[$key] }
    $seen = @{}
    $ordered = New-Object System.Collections.ArrayList
    if ($fileOfId.ContainsKey($innerFileId)) {
        $p = $fileOfId[$innerFileId]
        [void]$ordered.Add($p)
        $seen[$p] = $true
    }
    $id = $frameId
    $guard = 0
    while ($siteParent.ContainsKey($id)) {
        $guard++
        if ($guard -gt 512) { break }
        $fid = $siteFile[$id]
        if ($fileOfId.ContainsKey($fid)) {
            $p = $fileOfId[$fid]
            if (-not $seen.ContainsKey($p)) { [void]$ordered.Add($p); $seen[$p] = $true }
        }
        $id = $siteParent[$id]
    }
    $arr = $ordered.ToArray()
    $chainCache[$key] = $arr
    return $arr
}

function Get-Owner($chain) {
    foreach ($p in $chain) {
        $b = Get-Bucket $p
        if ($b -eq 'de.rs' -or $b -eq 'ser.rs' -or $b -eq 'read.rs') { return $b }
    }
    if ($chain.Count -gt 0) { return (Get-Bucket $chain[0]) }
    return 'unattributed'
}

### 3. what we count #########################################################

# Panic machinery. Each row is matched against the mangled call target with the
# length-prefix boundary form `<digits><name>` so that `Unexpected` cannot be
# mistaken for `expect_failed`. The mission plan named eight symbols; rustc has
# since renamed some of them, so the retired names stay in the table and read 0
# -- that is itself a finding about the toolchain, not a gap in the tool.
$panicPatterns = New-Object System.Collections.Specialized.OrderedDictionary
$panicPatterns['core::panicking::panic_bounds_check']            = 'panic_bounds_check'
$panicPatterns['core::slice::index::slice_index_fail']           = 'slice_index_fail'
$panicPatterns['core::slice::index::slice_index_len_fail']       = 'slice_index_len_fail'
$panicPatterns['core::slice::index::slice_start_index_len_fail'] = 'slice_start_index_len_fail'
$panicPatterns['core::slice::index::slice_end_index_len_fail']   = 'slice_end_index_len_fail'
$panicPatterns['core::str::slice_error_fail']                    = 'slice_error_fail'
$panicPatterns['core::panicking::panic_misaligned_pointer_dereference'] = 'panic_misaligned_pointer_dereference'
$panicPatterns['core::{option,result}::unwrap_failed']           = 'unwrap_failed'
$panicPatterns['core::panicking::expect_failed']                 = 'expect_failed'
$panicPatterns['core::panicking::panic_cannot_unwind']           = 'panic_cannot_unwind'
$panicPatterns['core::panicking::panic']                         = 'panic'
$panicPatterns['core::panicking::panic_fmt']                     = 'panic_fmt'
$panicPatterns['core::panicking::panic_nounwind']                = 'panic_nounwind'
$panicPatterns['core::panicking::panic_nounwind_fmt']            = 'panic_nounwind_fmt'
$panicPatterns['core::panicking::panic_const_* (any)']           = 'panic_const_'

# A mangled path item is `<length><name>`, and the digits are preceded by
# IDENTIFIER characters (`...4core9panicking18panic_bounds_check`), so the
# leading guard must be "not a digit" -- anchoring on a non-identifier char
# matches nothing at all. The TRAILING guard is what stops `10Unexpected` being
# read as `expect_failed` and `9panicking` as `panic`.
$panicRegex = @{}
foreach ($k in $panicPatterns.Keys) {
    $nm = $panicPatterns[$k]
    if ($nm.EndsWith('_')) {
        $panicRegex[$k] = [regex]("(^|[^0-9])[0-9]+" + [regex]::Escape($nm))
    } else {
        $panicRegex[$k] = [regex]("(^|[^0-9])[0-9]+" + [regex]::Escape($nm) + "($|[^A-Za-z0-9_])")
    }
}
$panickingModRegex = [regex]'[0-9]+core[0-9]+panicking'

# Packed / SIMD classes, matched on the mnemonic. The first group is the set the
# mission plan named -- instructions that appear only when the compiler has
# auto-vectorised arithmetic or a comparison. The second group is 16-byte BLOCK
# MOVES and scalar-float xmm traffic, reported apart because `movups`/`movaps`
# traffic is an inlined struct or buffer copy and `punpckldq`+`movapd` is an
# integer-to-double conversion; counting either as SIMD would let this report
# claim auto-vectorisation that is not there.
$packedClasses = New-Object System.Collections.Specialized.OrderedDictionary
$packedClasses['movdq* (movdqa/movdqu)'] = '^movdq'
$packedClasses['punpck*']                = '^punpck'
$packedClasses['pcmpeq*']                = '^pcmpeq'
$packedClasses['pmovmsk*']               = '^pmovmsk'
$packedClasses['pshuf*']                 = '^pshuf'
$packedClasses['por']                    = '^por$'
$packedClasses['pand / pandn']           = '^pandn?$'
$packedClasses['vp* (AVX/AVX2 integer)'] = '^vp'
$packedClasses['vmov* (AVX move)']       = '^vmov'

$blockMoveClasses = New-Object System.Collections.Specialized.OrderedDictionary
$blockMoveClasses['movups / movaps (16-byte block move)'] = '^mov(up|ap)s$'
$blockMoveClasses['movupd / movapd']                      = '^mov(up|ap)d$'
$blockMoveClasses['pxor / xorps / xorpd (register zeroing)'] = '^(pxor|xorps|xorpd)$'
$blockMoveClasses['movd / movq (xmm <-> gpr)']            = '^mov[dq]$'
$blockMoveClasses['movss / movsd (SCALAR float, not packed)'] = '^movs[sd]$'

$packedRegex = @{}
foreach ($k in $packedClasses.Keys) { $packedRegex[$k] = [regex]$packedClasses[$k] }
$blockRegex = @{}
foreach ($k in $blockMoveClasses.Keys) { $blockRegex[$k] = [regex]$blockMoveClasses[$k] }

$memRegex = [regex]'(^|[^A-Za-z0-9_])(memcpy|memmove|memset)($|[^A-Za-z0-9_])'

### 4. main pass #############################################################

$totalInstr   = 0
$totalLines   = $lines.Count
$ownInstr     = @{}   # bucket -> instrs whose innermost frame is that file
$ownerInstr   = @{}   # bucket -> instrs owned by that file after the chain walk
$fnInstr      = @{}   # symbol -> instruction count (EH funclets folded in)
$funcletCount = 0
$sehProcCount = 0
$udCount      = 0

$panicCounts = @{}   # "row|bucket" -> n ; "row|TOTAL" -> n
$panicSites  = New-Object System.Collections.ArrayList
$memCounts   = @{}   # "callee|lenclass|bucket" -> n
$memSites    = New-Object System.Collections.ArrayList
$packCounts  = @{}   # "class|bucket" -> n
$blockCounts = @{}   # "class|bucket" -> n

$curFn     = ""
$curFrame  = -1
$curFileId = -1
$curLine   = 0
$recent    = New-Object System.Collections.ArrayList   # sliding window of instructions

$funcletRe = [regex]'^\?[A-Za-z]+\$\d+@\?0\?(.+)@[0-9A-Za-z]+$'
$movRe     = [regex]'^([a-z0-9]+)\s+%?([a-z0-9]+)\s*,\s*(.+)$'

for ($li = 0; $li -lt $lines.Count; $li++) {
    $raw = $lines[$li]
    if ($raw.Length -eq 0) { continue }
    $t = $raw.TrimStart()
    if ($t.Length -eq 0) { continue }
    $c0 = $t[0]

    if ($c0 -eq '.') {
        if ($t.StartsWith('.cv_loc')) {
            $m = [regex]::Match($t, '^\.cv_loc\s+(\d+)\s+(\d+)\s+(\d+)')
            if ($m.Success) {
                $curFrame = [int]$m.Groups[1].Value
                $curFileId = [int]$m.Groups[2].Value
                $curLine = [int]$m.Groups[3].Value
                $cvLocCount++
            }
        }
        elseif ($t.StartsWith('.loc')) {
            $m = [regex]::Match($t, '^\.loc\s+(\d+)\s+(\d+)')
            if ($m.Success) {
                $curFrame = -1
                $curFileId = [int]$m.Groups[1].Value
                $curLine = [int]$m.Groups[2].Value
                $dwLocCount++
            }
        }
        elseif ($t.StartsWith('.seh_proc')) {
            $sehProcCount++
            $nm = $t.Substring(9).Trim()
            if ($nm.StartsWith('"')) { $nm = $nm.Trim('"') }
            $fm = $funcletRe.Match($nm)
            if ($fm.Success) {
                $curFn = $fm.Groups[1].Value
                $funcletCount++
            } else {
                $curFn = $nm
            }
            $curFrame = -1
            $curFileId = -1
            $curLine = 0
            $recent.Clear()
        }
        elseif ($t.StartsWith('.seh_endproc')) {
            $curFn = ""
            $curFrame = -1
            $curFileId = -1
            $curLine = 0
            $recent.Clear()
        }
        continue
    }
    if ($c0 -eq '#') { continue }
    if ($raw[0] -ne ' ' -and $raw[0] -ne "`t") { continue }   # label at column 0

    # ---- this line is an instruction ----
    $totalInstr++
    $sp = $t.IndexOfAny([char[]]@(' ', "`t"))
    if ($sp -lt 0) { $mnem = $t } else { $mnem = $t.Substring(0, $sp) }
    $mnem = $mnem.ToLowerInvariant()
    $rest = ""
    if ($sp -ge 0) { $rest = $t.Substring($sp).Trim() }
    $hashPos = $rest.IndexOf('#')
    if ($hashPos -ge 0) { $rest = $rest.Substring(0, $hashPos).Trim() }

    if ($curFn -ne "") { Add-Count $fnInstr $curFn 1 }

    # @() is load-bearing: a function returning a ONE-element array has it
    # unrolled to a bare string, and $chain[0] would then be the first CHARACTER
    # of the path. Re-wrapping restores array semantics for every chain length.
    if ($curFileId -ge 0) { $chain = @(Get-Chain $curFrame $curFileId) } else { $chain = @() }
    if ($chain.Count -gt 0) { $ownB = Get-Bucket $chain[0] } else { $ownB = 'unattributed' }
    $owner = Get-Owner $chain
    Add-Count $ownInstr $ownB 1
    Add-Count $ownerInstr $owner 1

    if ($mnem -eq 'ud2' -or $mnem -eq 'int3') { $udCount++ }

    # --- packed / SIMD ---
    foreach ($k in $packedClasses.Keys) {
        if ($packedRegex[$k].IsMatch($mnem)) {
            Add-Count $packCounts ($k + '|TOTAL') 1
            Add-Count $packCounts ($k + '|' + $owner) 1
            break
        }
    }
    foreach ($k in $blockMoveClasses.Keys) {
        if ($blockRegex[$k].IsMatch($mnem)) {
            Add-Count $blockCounts ($k + '|TOTAL') 1
            Add-Count $blockCounts ($k + '|' + $owner) 1
            break
        }
    }

    # --- calls / jumps ---
    $isCall = ($mnem -eq 'call' -or $mnem -eq 'callq' -or $mnem -eq 'jmp' -or $mnem -eq 'jmpq')
    if ($isCall -and $rest.Length -gt 0) {
        $tgt = $rest
        if ($tgt.StartsWith('*')) { $tgt = $tgt.Substring(1) }
        if ($tgt.StartsWith('%')) { $tgt = $tgt.Substring(1) }
        $isSymbol = ($tgt -match '^[A-Za-z_$@.?][A-Za-z0-9_$@.?]*$')
        if ($isSymbol -and (-not $tgt.StartsWith('.L'))) {

            # panic machinery
            $matchedRow = $null
            foreach ($k in $panicPatterns.Keys) {
                if ($panicRegex[$k].IsMatch($tgt)) { $matchedRow = $k; break }
            }
            if (($null -eq $matchedRow) -and $panickingModRegex.IsMatch($tgt)) {
                $matchedRow = 'core::panicking::* (other)'
            }
            if ($null -ne $matchedRow) {
                Add-Count $panicCounts ($matchedRow + '|TOTAL') 1
                Add-Count $panicCounts ($matchedRow + '|' + $owner) 1
                $srcLoc = 'unattributed'
                if ($chain.Count -gt 0) { $srcLoc = (Get-ShortPath $chain[0]) + ':' + $curLine }
                [void]$panicSites.Add([pscustomobject]@{
                    Row = $matchedRow; Owner = $owner; Src = $srcLoc; Fn = $curFn
                })
            }

            # memcpy / memmove / memset
            $mm = $memRegex.Match($tgt)
            if ($mm.Success) {
                $which = $mm.Groups[2].Value
                # The length is the 3rd integer argument:
                #   MSVC x64:  memcpy(rcx = dst, rdx = src, r8 = len)
                #   SysV:      memcpy(rdi = dst, rsi = src, rdx = len)
                if ($isIntel) { $lenRegs = @('r8', 'r8d', 'r8w', 'r8b') }
                else { $lenRegs = @('rdx', 'edx', 'dx', 'dl') }
                $cls = 'unknown'
                for ($ri = $recent.Count - 1; $ri -ge 0; $ri--) {
                    $pm2 = $movRe.Match($recent[$ri])
                    if (-not $pm2.Success) { continue }
                    $pop = $pm2.Groups[1].Value
                    $pdst = $pm2.Groups[2].Value.ToLowerInvariant()
                    $psrc = $pm2.Groups[3].Value.Trim()
                    if ($lenRegs -notcontains $pdst) { continue }
                    if ($pop -eq 'xor' -and $psrc.TrimStart('%') -eq $pdst) { $cls = 'const'; break }
                    if ($pop -eq 'mov' -or $pop -eq 'movl' -or $pop -eq 'movq') {
                        if ($psrc -match '^\$?-?(0x[0-9a-fA-F]+|[0-9]+)$') { $cls = 'const' }
                        else { $cls = 'runtime' }
                        break
                    }
                    $cls = 'runtime'
                    break
                }
                Add-Count $memCounts ($which + '|TOTAL|TOTAL') 1
                Add-Count $memCounts ($which + '|' + $cls + '|TOTAL') 1
                Add-Count $memCounts ($which + '|TOTAL|' + $owner) 1
                $srcLoc = 'unattributed'
                if ($chain.Count -gt 0) { $srcLoc = (Get-ShortPath $chain[0]) + ':' + $curLine }
                [void]$memSites.Add([pscustomobject]@{
                    Which = $which; Len = $cls; Owner = $owner; Src = $srcLoc; Fn = $curFn
                })
            }
        }
    }

    [void]$recent.Add(($mnem + ' ' + $rest))
    if ($recent.Count -gt 16) { $recent.RemoveAt(0) }
}

### 5. demangle ##############################################################

$demangleTool = 'the built-in path sketch (length-prefix walk, not a real demangler)'
$symbols = @($fnInstr.Keys | Sort-Object)
$demangled = @{}
if (-not $NoDemangle) {
    $rf = Get-Command rustfilt -ErrorAction SilentlyContinue
    if ($rf) {
        $tmpIn = Join-Path $env:TEMP ('asm-census-' + [guid]::NewGuid().ToString('N') + '.txt')
        [System.IO.File]::WriteAllLines($tmpIn, $symbols)
        $res = @(Get-Content -LiteralPath $tmpIn | & $rf.Source)
        Remove-Item -LiteralPath $tmpIn -Force -ErrorAction SilentlyContinue
        if ($res.Count -eq $symbols.Count) {
            for ($i = 0; $i -lt $symbols.Count; $i++) { $demangled[$symbols[$i]] = $res[$i] }
            $demangleTool = 'rustfilt (' + $rf.Source + ')'
        }
    }
}
foreach ($s in $symbols) {
    if (-not $demangled.ContainsKey($s)) { $demangled[$s] = (Get-PathSketch $s) }
}

### 6. render ################################################################

$sb = New-Object System.Text.StringBuilder
function W([string]$s) { [void]$sb.AppendLine((ConvertTo-Ascii $s)) }

$rustcVv = ""
try { $rustcVv = ((& rustc -vV 2>&1) -join "`n") } catch { $rustcVv = "rustc -vV failed" }
$hostTriple = "unknown"
$rustcRelease = "unknown"
$llvmVer = "unknown"
foreach ($ln in ($rustcVv -split "`n")) {
    $lt = $ln.Trim()
    if ($lt.StartsWith('host:')) { $hostTriple = $lt.Substring(5).Trim() }
    if ($lt.StartsWith('release:')) { $rustcRelease = $lt.Substring(8).Trim() }
    if ($lt.StartsWith('LLVM version:')) { $llvmVer = $lt.Substring(13).Trim() }
}
$rustcFirst = (($rustcVv -split "`n")[0]).Trim()

# Target triple as evidenced by the asm path (target/<triple>/release/...).
$pathTriple = ""
$normAsm = $Asm.Replace('/', '\')
$pmT = [regex]::Match($normAsm, '\\target\\([A-Za-z0-9_\-\.]+)\\release\\')
if ($pmT.Success) { $pathTriple = $pmT.Groups[1].Value }

if ($isIntel) { $syntax = 'Intel (.intel_syntax directive present)' }
else { $syntax = 'AT&T (no .intel_syntax directive)' }
if ($cvLocCount -gt 0) { $lineTables = 'CodeView: ' + $cvLocCount + ' .cv_loc directives, ' + $siteParent.Count + ' inline sites' }
elseif ($dwLocCount -gt 0) { $lineTables = 'DWARF: ' + $dwLocCount + ' .loc directives (no inline-frame chain)' }
else { $lineTables = 'NONE -- per-file attribution unavailable' }
if ($isIntel) { $lenRegName = 'r8, the 3rd integer argument on x86_64-pc-windows-msvc' }
else { $lenRegName = 'rdx, the 3rd integer argument on SysV' }

W '# Emitted-asm census -- rusty_json_turbo (lib `serde_json`)'
W ''
W 'This census PRICES BRICKS BEFORE THEY ARE BUILT. It reads the assembly rustc'
W 'actually emitted for the library and counts what is there, so that a brick is'
W 'opened or closed on evidence instead of on the expectation that a lever exists.'
W 'It answers the three questions the mission plan asks of the `.s`:'
W ''
W '- **(a) Is there a bounds-check tax worth removing?** The house expectation,'
W '  from h264, rav1e and dds, is that it is about zero. A near-zero result is a'
W '  real finding that CLOSES a brick, not a failed measurement.'
W '- **(b) Are there runtime-length `memcpy` calls on the hot parse/serialize'
W '  paths that a brick could remove?** A source grep finds only the copies'
W '  someone wrote; the `.s` finds the ones the optimiser introduced and the ones'
W '  it failed to fold.'
W '- **(c) Has the compiler already auto-vectorised anything?** If it has, a'
W '  hand-written SIMD twin is competing with the compiler and its claim shrinks'
W '  accordingly; if it has not, the twin is competing with scalar code.'
W ''
W 'Generated by `tools/asm-census.ps1`. It counts instructions; it never measures'
W 'time. See the method line at the end.'
W ''
if ($discoveryNote -ne "") { W ('> ' + $discoveryNote); W '' }

W '## 0. Totals'
W ''
W '| quantity | value |'
W '| --- | --- |'
W ('| asm lines read | ' + $totalLines + ' |')
W ('| instructions counted | ' + $totalInstr + ' |')
W ('| `.seh_proc` regions | ' + $sehProcCount + ', of which ' + $funcletCount + ' are MSVC EH cleanup funclets folded into their parent |')
W ('| distinct functions after folding | ' + $fnInstr.Count + ' |')
W ('| asm syntax | ' + $syntax + ' |')
W ('| line tables | ' + $lineTables + ' |')
W ('| source files referenced | ' + $fileOfId.Count + ' |')
W ('| `ud2` / `int3` (unreachable / abort markers) | ' + $udCount + ' |')
W ''

# Source freshness. The compiler records a checksum of every source file in the
# `.cv_file` directive, so the .s can prove which bytes it was built from. The
# stale-binary trap in the house playbook is the commonest way a census lies:
# every line number below is meaningless if the file moved under it.
$freshRows = New-Object System.Collections.ArrayList
$freshStale = 0
$freshChecked = 0
foreach ($p in ($fileOfId.Values | Sort-Object -Unique)) {
    $b = Get-Bucket $p
    if ($b -eq 'rust-std' -or $b -eq 'dependency') { continue }
    $rec = 'not recorded'
    $now = 'n/a'
    $verdict = 'unknown'
    if ($fileSum.ContainsKey($p)) {
        $parts = $fileSum[$p].Split(':')
        $kind = $parts[0]
        $rec = $parts[1]
        $alg = ''
        if ($kind -eq '3') { $alg = 'SHA256' }
        elseif ($kind -eq '1') { $alg = 'MD5' }
        elseif ($kind -eq '2') { $alg = 'SHA1' }
        if ($alg -ne '' -and (Test-Path -LiteralPath $p)) {
            $now = (Get-FileHash -LiteralPath $p -Algorithm $alg).Hash.ToUpperInvariant()
            $freshChecked++
            if ($now -eq $rec) { $verdict = 'MATCHES the build' }
            else { $verdict = 'CHANGED since the build'; $freshStale++ }
        }
        elseif ($alg -eq '') { $verdict = 'checksum kind ' + $kind + ' unsupported' }
        else { $verdict = 'file no longer on disk' }
    }
    [void]$freshRows.Add([pscustomobject]@{
        Path = (Get-ShortPath $p); Bucket = $b; Rec = $rec; Now = $now; Verdict = $verdict
    })
}

W '## 0b. Source freshness (the stale-binary check)'
W ''
W 'Every line number in this report is only as good as the source the asm was'
W 'built from. The compiler stamps a checksum of each source file into the'
W '`.cv_file` directive, so the `.s` can be held against the working tree. Only'
W 'this crate''s own files are checked; std and dependency sources are pinned by'
W 'the toolchain and lockfile.'
W ''
W '| source file | bucket | verdict |'
W '| --- | --- | --- |'
foreach ($r in ($freshRows | Sort-Object -Property Bucket, Path)) {
    W ('| `' + (Format-MdCell $r.Path) + '` | `' + $r.Bucket + '` | ' + (Format-MdCell $r.Verdict) + ' |')
}
W ''
if ($freshStale -gt 0) {
    W ('**' + $freshStale + ' of ' + $freshChecked + ' checked source files have CHANGED since this asm was emitted.**')
    W 'The counts below are still exactly what the compiler emitted, but the line'
    W 'numbers in sections 2b and 3b point into the older text. Re-run the build'
    W 'and this tool to bring them back into agreement.'
} else {
    W ('All ' + $freshChecked + ' checked source files match the bytes this asm was built from.')
}
W ''

W '## 1. Instructions by source file'
W ''
W 'Two columns, because inlining moves code. `own` counts instructions whose'
W 'INNERMOST inline frame is that file -- code written in that file, wherever it'
W 'ended up. `owner` walks the inline chain outward and credits the innermost of'
W 'the three files of interest, so a bounds check inlined from'
W '`core::slice::index` into a `read.rs` loop is counted against `read.rs`. Each'
W 'instruction is counted exactly once in each column.'
W ''
W '| source | own instrs | own % | owner instrs | owner % |'
W '| --- | ---: | ---: | ---: | ---: |'
foreach ($b in $bucketNames) {
    $o = Get-Count $ownInstr $b
    $w = Get-Count $ownerInstr $b
    $op = 0
    $wp = 0
    if ($totalInstr -gt 0) {
        $op = [math]::Round(100.0 * $o / $totalInstr, 2)
        $wp = [math]::Round(100.0 * $w / $totalInstr, 2)
    }
    W ('| `' + $b + '` | ' + $o + ' | ' + $op + '% | ' + $w + ' | ' + $wp + '% |')
}
W ('| **total** | **' + $totalInstr + '** | 100% | **' + $totalInstr + '** | 100% |')
W ''

# Breakdown columns for sections 2-4. `crate other` is its own column rather
# than being swept into `external`: a copy inside `value/ser.rs` or `map.rs` is
# still this crate's copy, and hiding it next to alloc's would mis-price a brick.
$bcols = @('de.rs', 'ser.rs', 'read.rs', 'crate-other')
$bcolHeader = '| de.rs | ser.rs | read.rs | crate other | external | total sites |'
$bcolHeaderOps = '| de.rs | ser.rs | read.rs | crate other | external | total ops |'
$bcolRule = '| ---: | ---: | ---: | ---: | ---: | ---: |'
function Format-Row([string]$label, $tbl, [string]$prefix) {
    $tot = Get-Count $tbl ($prefix + '|TOTAL')
    $cells = ''
    $named = 0
    foreach ($b in $bcols) {
        $v = Get-Count $tbl ($prefix + '|' + $b)
        $named = $named + $v
        $cells = $cells + ' ' + $v + ' |'
    }
    $other = $tot - $named
    return ('| ' + $label + ' |' + $cells + ' ' + $other + ' | **' + $tot + '** |')
}

W '## 2. Bounds-check / panic branches'
W ''
W 'Direct `call`/`jmp` sites whose target is panic machinery. Counted per SITE,'
W 'not per dynamic execution: a site parked in a cold error block costs nothing at'
W 'run time, so this table bounds the LEVER, not the tax. The mission plan named'
W 'eight symbols; the toolchain has since renamed part of that set (a single'
W '`slice_index_fail` now replaces the three `slice_*_index_len_fail` helpers), so'
W 'the retired names are kept as rows and read 0.'
W ''
W ('| panic symbol ' + $bcolHeader)
W ('| --- ' + $bcolRule)
$panicRowKeys = @()
foreach ($k in $panicPatterns.Keys) { $panicRowKeys += $k }
$panicRowKeys += 'core::panicking::* (other)'
$panicGrand = 0
$panicPerCol = @{}
foreach ($k in $panicRowKeys) {
    W (Format-Row ('`' + $k + '`') $panicCounts $k)
    $panicGrand = $panicGrand + (Get-Count $panicCounts ($k + '|TOTAL'))
    foreach ($b in $bcols) { Add-Count $panicPerCol $b (Get-Count $panicCounts ($k + '|' + $b)) }
}
$panicNamed = 0
$panicCells = ''
foreach ($b in $bcols) {
    $v = Get-Count $panicPerCol $b
    $panicNamed = $panicNamed + $v
    $panicCells = $panicCells + ' **' + $v + '** |'
}
W ('| **all panic sites** |' + $panicCells + ' **' + ($panicGrand - $panicNamed) + '** | **' + $panicGrand + '** |')
W ''

if ($panicSites.Count -gt 0) {
    W '### 2b. Every panic site, with its innermost source line and enclosing function'
    W ''
    W '| # | owner | innermost src | symbol | enclosing function |'
    W '| ---: | --- | --- | --- | --- |'
    $orderedP = @($panicSites | Sort-Object -Property Owner, Src, Row, Fn)
    $i = 0
    foreach ($s in $orderedP) {
        $i++
        if ($i -gt 120) {
            W ('| ... | ' + ($orderedP.Count - 120) + ' further sites not listed | | | |')
            break
        }
        $dn = $demangled[$s.Fn]
        if (-not $dn) { $dn = $s.Fn }
        W ('| ' + $i + ' | ' + (Format-MdCell $s.Owner) + ' | ' + (Format-MdCell $s.Src) + ' | `' + (Format-MdCell $s.Row) + '` | `' + (Format-MdCell (Get-Truncated $dn 90)) + '` |')
    }
    W ''
}

W '## 3. memcpy / memmove / memset calls'
W ''
W 'A `call memcpy` that survives optimisation is a copy whose length the optimiser'
W 'could not fold away. The length column is read off the length ARGUMENT REGISTER'
W ('(' + $lenRegName + ') by walking back up to 16')
W 'instructions for the last write to it: an immediate operand is `const`, any'
W 'other operand is `runtime`, and `unknown` means no write to that register was'
W 'found inside the window. So `const` is a LOWER bound and `runtime` an UPPER'
W 'bound -- `mov r8, rax` is reported runtime even when `rax` happens to hold a'
W 'constant established further back. Copies the optimiser inlined into `movups`'
W 'pairs are not calls and are not in this table; see section 4.'
W ''
W '| callee | const len | runtime len | unknown | total |'
W '| --- | ---: | ---: | ---: | ---: |'
$memTot = 0
foreach ($w2 in @('memcpy', 'memmove', 'memset')) {
    $c = Get-Count $memCounts ($w2 + '|const|TOTAL')
    $r = Get-Count $memCounts ($w2 + '|runtime|TOTAL')
    $u = Get-Count $memCounts ($w2 + '|unknown|TOTAL')
    $tt = Get-Count $memCounts ($w2 + '|TOTAL|TOTAL')
    $memTot = $memTot + $tt
    W ('| `' + $w2 + '` | ' + $c + ' | ' + $r + ' | ' + $u + ' | **' + $tt + '** |')
}
W ('| **total** | | | | **' + $memTot + '** |')
W ''
W ('| callee ' + $bcolHeader)
W ('| --- ' + $bcolRule)
foreach ($w2 in @('memcpy', 'memmove', 'memset')) {
    W (Format-Row ('`' + $w2 + '`') $memCounts ($w2 + '|TOTAL'))
}
W ''

if ($memSites.Count -gt 0) {
    W '### 3b. Every mem* call site'
    W ''
    W '| # | callee | len | owner | innermost src | enclosing function |'
    W '| ---: | --- | --- | --- | --- | --- |'
    $orderedM = @($memSites | Sort-Object -Property Owner, Which, Len, Src, Fn)
    $i = 0
    foreach ($s in $orderedM) {
        $i++
        if ($i -gt 120) {
            W ('| ... | ' + ($orderedM.Count - 120) + ' further sites not listed | | | | |')
            break
        }
        $dn = $demangled[$s.Fn]
        if (-not $dn) { $dn = $s.Fn }
        W ('| ' + $i + ' | `' + $s.Which + '` | ' + $s.Len + ' | ' + (Format-MdCell $s.Owner) + ' | ' + (Format-MdCell $s.Src) + ' | `' + (Format-MdCell (Get-Truncated $dn 80)) + '` |')
    }
    W ''
}

W '## 4. Packed / SIMD instructions'
W ''
W 'The first table is the set the mission plan named -- instructions that appear'
W 'only when the compiler has auto-vectorised arithmetic or a comparison. The'
W 'second table is deliberately separate: 16-byte block moves and scalar-float xmm'
W 'traffic use the same register file but are an inlined struct or buffer copy, or'
W 'an integer-to-double conversion, not vectorised work. Merging the two would let'
W 'this report claim auto-vectorisation that is not there.'
W ''
W ('| packed op class ' + $bcolHeaderOps)
W ('| --- ' + $bcolRule)
$packGrand = 0
foreach ($k in $packedClasses.Keys) {
    W (Format-Row ('`' + $k + '`') $packCounts $k)
    $packGrand = $packGrand + (Get-Count $packCounts ($k + '|TOTAL'))
}
W ('| **total packed ops** | | | | | | **' + $packGrand + '** |')
W ''
W ('| xmm traffic that is NOT auto-vectorisation ' + $bcolHeaderOps)
W ('| --- ' + $bcolRule)
$blockGrand = 0
foreach ($k in $blockMoveClasses.Keys) {
    W (Format-Row ('`' + $k + '`') $blockCounts $k)
    $blockGrand = $blockGrand + (Get-Count $blockCounts ($k + '|TOTAL'))
}
W ('| **total** | | | | | | **' + $blockGrand + '** |')
W ''

W ('## 5. Top ' + $Top + ' functions by emitted instruction count')
W ''
W 'MSVC EH cleanup funclets (`?dtor$N@?0?<parent>@4HA`) are folded into the parent'
W ('function they unwind. Names come from ' + $demangleTool + '.')
W ''
W '| # | instrs | % of crate | demangled function |'
W '| ---: | ---: | ---: | --- |'
$sorted = @($fnInstr.GetEnumerator() | Sort-Object -Property @{Expression = 'Value'; Descending = $true}, @{Expression = 'Key'; Descending = $false})
$rank = 0
$topSum = 0
foreach ($e in $sorted) {
    $rank++
    if ($rank -gt $Top) { break }
    $topSum = $topSum + $e.Value
    $dn = $demangled[$e.Key]
    if (-not $dn) { $dn = $e.Key }
    $pc = 0
    if ($totalInstr -gt 0) { $pc = [math]::Round(100.0 * $e.Value / $totalInstr, 2) }
    W ('| ' + $rank + ' | ' + $e.Value + ' | ' + $pc + '% | `' + (Format-MdCell (Get-Truncated $dn 150)) + '` |')
}
W ''
$topPc = 0
if ($totalInstr -gt 0) { $topPc = [math]::Round(100.0 * $topSum / $totalInstr, 2) }
W ('These ' + [math]::Min($Top, $sorted.Count) + ' functions are ' + $topSum + ' of ' + $totalInstr + ' emitted instructions (' + $topPc + '% of the crate).')
W ''

### method ###################################################################

W '## Method'
W ''
W 'Every number above carries the method that produced it.'
W ''
W '| field | value |'
W '| --- | --- |'
W ('| rustc invocation | `' + $MethodCmd + '` |')
W ('| toolchain | `' + $rustcFirst + '` |')
W ('| rustc release | `' + $rustcRelease + '`, LLVM `' + $llvmVer + '` |')
W ('| host triple | `' + $hostTriple + '` |')
if ($pathTriple -ne "") {
    W ('| target triple | `' + $pathTriple + '` (read off the asm path) |')
} else {
    W ('| target triple | `' + $hostTriple + '` (no `--target` given, so the asm path carries no triple component) |')
}
W ('| asm file | `' + $Asm + '` |')
W ('| asm size | ' + $asmSize + ' bytes, ' + $totalLines + ' lines |')
W ('| asm sha256 | `' + $asmSha + '` |')
W ('| asm mtime (UTC) | ' + $asmItem.LastWriteTimeUtc.ToString('yyyy-MM-ddTHH:mm:ssZ') + ' |')
W ('| asm syntax detected | ' + $syntax + ' |')
W ('| line tables detected | ' + $lineTables + ' |')
W ('| demangler | ' + $demangleTool + ' |')
W ('| tool | `tools/asm-census.ps1` -Top ' + $Top + ' |')
W ''
W 'THIS INSTRUMENT COUNTS INSTRUCTIONS. IT DOES NOT MEASURE TIME. No number in'
W 'this report may be quoted as, converted into, or used to imply a duration or a'
W 'speedup. A static count says whether a lever EXISTS; only the bench harness'
W '(`rjson-bench`, `tools/pinvs.ps1`) and the runtime census counters say whether'
W 'pulling it paid.'
W ''
W 'Two standing caveats on coverage:'
W ''
W '1. `--emit asm` on the LIB target emits only the code this crate itself'
W '   instantiates. Generic functions monomorphised in a CONSUMER -- every'
W '   `#[derive(Deserialize)]` struct path -- are not in this `.s` at all. What is'
W '   here is the crate-internal instantiation set: the `Value` DOM paths,'
W '   `SliceRead` / `StrRead`, the `Formatter` impls, and everything non-generic.'
W '2. `codegen-units = 1` and `lto = "thin"` are set for the release profile. Asm'
W '   is emitted per codegen unit BEFORE link-time optimisation, so the finally'
W '   linked object can differ. These counts are pre-LTO.'

### output ###################################################################

$text = ConvertTo-Ascii ($sb.ToString())
Write-Output $text

if ($Out -ne "") {
    $outDir = Split-Path -Parent $Out
    if ($outDir -and (-not (Test-Path $outDir))) { New-Item -ItemType Directory -Path $outDir -Force | Out-Null }
    # UTF-8 with NO byte-order mark. PowerShell 5.1's `-Encoding utf8` prepends
    # EF BB BF, which are bytes above 0x7F, and the repo's ASCII-only lint would
    # then fail on the very file this tool just wrote. The payload is already
    # ASCII-only (ConvertTo-Ascii above), so no-BOM UTF-8 is byte-for-byte ASCII.
    $enc = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Out, $text, $enc)
    Write-Output ""
    Write-Output ("wrote " + $Out)
}
