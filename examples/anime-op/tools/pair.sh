#!/usr/bin/env bash
# Reference (left) and our render (right) at the given times, half size, stacked.
# usage: tools/pair.sh ours.mp4 out.png 27.8 30.8 ...
set -euo pipefail
ours=$1; out=$2; shift 2
tmp=$(mktemp -d)
rows=()
for t in "$@"; do
  ffmpeg -hide_banner -loglevel error -y -ss "$t" -i dynamic/ref/ref.mp4 -frames:v 1 -vf scale=960:-2 "$tmp/r_$t.png"
  ffmpeg -hide_banner -loglevel error -y -ss "$t" -i "$ours" -frames:v 1 -vf scale=960:-2 "$tmp/o_$t.png"
  magick "$tmp/r_$t.png" "$tmp/o_$t.png" +append "$tmp/p_$t.png"
  rows+=("$tmp/p_$t.png")
done
magick "${rows[@]}" -append "$out"
rm -rf "$tmp"
