#!/usr/bin/env bash
# Reference (top) over our render (bottom), one pair per second, 10 per row.
# usage: tools/compare.sh ours.mp4 start_seconds count out.png
set -euo pipefail
ours=$1; start=$2; count=$3; out=$4
tmp=$(mktemp -d)
ffmpeg -hide_banner -loglevel error -y -ss "$start" -t "$count" -i dynamic/ref/ref.mp4 \
  -vf "fps=1,scale=240:-2,drawtext=fontfile=/System/Library/Fonts/Menlo.ttc:text='%{eif\:t+$start\:d}':x=3:y=3:fontsize=14:fontcolor=white:box=1:boxcolor=black@0.7" "$tmp/ref_%03d.png"
ffmpeg -hide_banner -loglevel error -y -ss "$start" -t "$count" -i "$ours" -vf "fps=1,scale=240:-2" "$tmp/our_%03d.png"
n=$(ls "$tmp"/ref_*.png | wc -l)
for k in $(seq 1 "$n"); do
  f=$(printf "%03d" "$k")
  [ -f "$tmp/our_$f.png" ] || cp "$tmp/ref_$f.png" "$tmp/our_$f.png"
  magick "$tmp/ref_$f.png" "$tmp/our_$f.png" -append -bordercolor "#222" -border 2 "$tmp/pair_$f.png"
done
rows=()
for r in $(seq 0 $(( (n - 1) / 10 ))); do
  files=()
  for c in $(seq 1 10); do
    k=$(( r * 10 + c )); [ "$k" -le "$n" ] || break
    files+=("$tmp/pair_$(printf "%03d" "$k").png")
  done
  magick "${files[@]}" +append "$tmp/row_$r.png"
  rows+=("$tmp/row_$r.png")
done
magick "${rows[@]}" -append "$out"
rm -rf "$tmp"
