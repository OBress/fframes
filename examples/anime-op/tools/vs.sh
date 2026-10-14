#!/usr/bin/env bash
# Reference frame (left) next to a freshly rendered frame of ours (right), half size, one row
# per time. usage: tools/vs.sh out.png 27.8 30.8 ...
set -euo pipefail
out=$1; shift
tmp=$(mktemp -d)
times=$(printf "%ss," "$@"); times=${times%,}
../../target/release/anime-op frame "$times" -o "$tmp/ours" >/dev/null 2>&1
rows=()
for t in "$@"; do
  ffmpeg -hide_banner -loglevel error -y -ss "$t" -i dynamic/ref/ref.mp4 -frames:v 1 -vf scale=960:-2 "$tmp/r_$t.png"
  magick "$tmp/ours/${t}s.png" -resize 960x "$tmp/o_$t.png"
  magick "$tmp/r_$t.png" "$tmp/o_$t.png" +append "$tmp/p_$t.png"
  rows+=("$tmp/p_$t.png")
done
magick "${rows[@]}" -append "$out"
rm -rf "$tmp"
