#!/bin/sh
# Makes dynamic_media/play/billboards.mp4: 20 tiles of 640 x 480 in a 5 x 4 grid, one per screen
# of Times Square. The two screens seen most in the flight (measured: tile 14 has the most
# area over the pass, tile 1 the largest view) play made_of_motion.mp4 and ThePrimeagen. The
# other tiles are cameos: 10 s from the most watched video of a programming channel, from its
# official YouTube channel (cameos/list.txt: tile|video id|channel). A tile without a cameo plays a part of made-of-motion. Every
# tile loops without a seam, so the whole file loops too. The clips are fetched once with
# yt-dlp (from PATH, or through uv); each full video is deleted after its 10 s are cut.
set -eu
film_tile=14
prime_tile=1
cd "$(dirname "$0")/dynamic_media/play"
src=made_of_motion.mp4
length=$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$src")
mkdir -p cameos
# yt-dlp from PATH, or through uv
if command -v yt-dlp >/dev/null 2>&1; then
    ytdlp="yt-dlp"
elif command -v uvx >/dev/null 2>&1; then
    ytdlp="uvx -q yt-dlp"
elif command -v uv >/dev/null 2>&1; then
    ytdlp="uv tool run -q yt-dlp"
else
    ytdlp=""
fi
# Homebrew's ffmpeg has libx264, some Linux builds only libopenh264
if ffmpeg -hide_banner -encoders 2>/dev/null | grep -q libx264; then
    h264="-c:v libx264 -crf 16 -preset fast"
else
    h264="-c:v libopenh264 -b:v 8M"
fi

# cut <video id> <start s> <out>: 10 s of the video, cropped to 4:3
cut() {
    [ -s "$3" ] && return
    if [ -z "$ytdlp" ]; then
        echo "no yt-dlp for $3: install it (brew install yt-dlp) or uv" >&2
        return 1
    fi
    $ytdlp --no-progress -f "bv*[height<=720][ext=mp4]/bv*[height<=720]" \
        -o cameos/full.mp4 "https://www.youtube.com/watch?v=$1" >/dev/null
    ffmpeg -v error -y -ss "$2" -t 10.2 -i cameos/full.mp4 -an \
        -vf "crop=ih*4/3:ih,scale=960:720" $h264 "$3"
    rm -f cameos/full.mp4
}
cut w7i4amO_zaE 2 primeagen.mp4
if [ -f cameos/list.txt ]; then
    while IFS='|' read -r tile id _; do
        cut "$id" 140 "cameos/t$tile.mp4" || echo "no cameo for tile $tile"
    done <cameos/list.txt
fi

inputs="-i $src -i $src"
filters="[0:v][1:v]concat=n=2:v=1:a=0,fps=24,scale=640:480,setsar=1,split=20"
for k in $(seq 0 19); do filters="$filters[s$k]"; done
filters="$filters;"
pads=""
layout=""
next=2
for k in $(seq 0 19); do
    clip=""
    [ -s "cameos/t$k.mp4" ] && clip="cameos/t$k.mp4"
    [ "$k" -eq "$prime_tile" ] && clip=primeagen.mp4
    if [ -n "$clip" ] && [ "$k" -ne "$film_tile" ]; then
        inputs="$inputs -i $clip -i $clip"
        filters="$filters[s$k]nullsink;[$next:v][$((next + 1)):v]concat=n=2:v=1:a=0,fps=24,scale=640:480,setsar=1,trim=duration=$length,setpts=PTS-STARTPTS[t$k];"
        next=$((next + 2))
    else
        start=$(echo "$length * $k / 20" | bc -l)
        filters="$filters[s$k]trim=start=$start:duration=$length,setpts=PTS-STARTPTS[t$k];"
    fi
    pads="$pads[t$k]"
    layout="$layout$((k % 5 * 640))_$((k / 5 * 480))|"
done
# shellcheck disable=SC2086
ffmpeg -v error -y $inputs -filter_complex "${filters}${pads}xstack=inputs=20:layout=${layout%|}[v]" \
    -map "[v]" -an $h264 -pix_fmt yuv420p -t "$length" billboards.mp4
echo "made billboards.mp4"
