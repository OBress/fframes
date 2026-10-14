#!/usr/bin/env bash
# Tiles PNGs (in the given order) into rows of N: tools/sheet.sh out.png N files...
set -euo pipefail
out=$1; n=$2; shift 2
tmp=$(mktemp -d)
i=0; row=0; files=()
for f in "$@"; do
  magick "$f" -resize 320x180 -bordercolor "#222" -border 2 "$tmp/t_$i.png"
  files+=("$tmp/t_$i.png"); i=$((i+1))
  if [ ${#files[@]} -eq "$n" ]; then magick "${files[@]}" +append "$tmp/row_$row.png"; row=$((row+1)); files=(); fi
done
if [ ${#files[@]} -gt 0 ]; then magick "${files[@]}" +append "$tmp/row_$row.png"; row=$((row+1)); fi
rows=(); for r in $(seq 0 $((row-1))); do rows+=("$tmp/row_$r.png"); done
magick "${rows[@]}" -background "#222" -append "$out"
rm -rf "$tmp"
