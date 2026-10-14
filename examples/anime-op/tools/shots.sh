#!/usr/bin/env bash
# For each time: reference over ours (480 px wide), N columns per row.
# usage: tools/shots.sh ours.mp4 out.png cols t1 t2 ...
set -euo pipefail
ours=$1; out=$2; cols=$3; shift 3
tmp=$(mktemp -d)
i=0
for t in "$@"; do
  ffmpeg -hide_banner -loglevel error -y -ss "$t" -i dynamic/ref/ref.mp4 -frames:v 1 -vf scale=480:-2 "$tmp/r.png"
  ffmpeg -hide_banner -loglevel error -y -ss "$t" -i "$ours" -frames:v 1 -vf scale=480:-2 "$tmp/o.png"
  magick "$tmp/r.png" "$tmp/o.png" -append -bordercolor "#222" -border 2 "$tmp/p_$(printf %03d $i).png"
  i=$((i+1))
done
rows=()
n=$i
for r in $(seq 0 $(( (n - 1) / cols ))); do
  files=()
  for c in $(seq 0 $((cols-1))); do
    k=$(( r * cols + c )); [ "$k" -lt "$n" ] || break
    files+=("$tmp/p_$(printf %03d $k).png")
  done
  magick "${files[@]}" +append "$tmp/row_$r.png"
  rows+=("$tmp/row_$r.png")
done
magick "${rows[@]}" -append "$out"
rm -rf "$tmp"
