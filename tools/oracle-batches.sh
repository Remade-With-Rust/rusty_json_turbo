#!/usr/bin/env bash
# Run the differential oracle over a file list in accounted batches.
# Every batch's exit status is recorded, so a crashed batch is REPORTED rather
# than silently shortening the total -- which is what an unaccounted `xargs`
# pipeline did twice, giving 1,643 then 1,543 documents for the same input.
set -u
list="$1"; bin="$2"; size="${3:-40}"
total=0; toks=0; dm=0; nm=0; batches=0; failed=0
mapfile -t files < "$list"
n=${#files[@]}
for ((i=0; i<n; i+=size)); do
  chunk=("${files[@]:i:size}")
  out=$("$bin" diff-oracle "${chunk[@]}" 2>&1)
  st=$?
  line=$(grep -m1 "^diff-oracle:" <<<"$out")
  batches=$((batches+1))
  if [ $st -ne 0 ] || [ -z "$line" ]; then
    failed=$((failed+1))
    echo "BATCH $batches FAILED (exit $st), files $i..$((i+${#chunk[@]}-1))"
    grep -m3 -E "^error|panic" <<<"$out" | sed 's/^/    /'
    continue
  fi
  read -r d t a b <<<"$(sed -E 's/diff-oracle: ([0-9]+) documents, ([0-9]+) number tokens, ([0-9]+) document mismatches, ([0-9]+) number mismatches/\1 \2 \3 \4/' <<<"$line")"
  total=$((total+d)); toks=$((toks+t)); dm=$((dm+a)); nm=$((nm+b))
done
echo "### ACCOUNTED TOTAL"
echo "    files listed:       $n"
echo "    batches:            $batches (failed: $failed)"
echo "    documents checked:  $total"
echo "    number tokens:      $toks"
echo "    document mismatches:$dm"
echo "    number mismatches:  $nm"
