#!/usr/bin/env bash
# Fail if any .rs file under the given roots contains a non-ASCII byte.
# Usage: scripts/check-ascii-rs.sh [path ...]
# Default: scan ./src and ./crates if present, else .
set -euo pipefail

roots=("$@")
if [[ ${#roots[@]} -eq 0 ]]; then
  if [[ -d src ]]; then roots+=(src); fi
  if [[ -d crates ]]; then roots+=(crates); fi
  if [[ ${#roots[@]} -eq 0 ]]; then roots=(.); fi
fi

offenders=0
while IFS= read -r -d '' file; do
  if LC_ALL=C grep -n '[^[:ascii:]]' "$file" >/dev/null 2>&1; then
    echo "non-ASCII in $file:"
    LC_ALL=C grep -n '[^[:ascii:]]' "$file" | head -n 20
    offenders=$((offenders + 1))
  fi
done < <(find "${roots[@]}" -type f -name '*.rs' -print0)

if [[ "$offenders" -gt 0 ]]; then
  echo
  echo "error: $offenders .rs file(s) contain non-ASCII bytes."
  echo "Route glyphs through thoth::symbols and keep source as \\\\u{…} escapes."
  exit 1
fi

echo "ok: all scanned .rs files are ASCII"
