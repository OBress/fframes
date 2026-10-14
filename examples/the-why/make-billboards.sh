#!/bin/sh
# Makes dynamic_media/play/billboards.mp4 from made_of_motion.mp4: 20 tiles of 640 x 480 in a
# 5 x 4 grid. Tile k plays the film from k / 20 of its length, so every billboard of the city
# shows another part of it. Each tile loops without a seam, so the whole file loops too.
# Tile 16 (the screen with the pink frame in Times Square) is a cameo instead: ThePrimeagen
# in "0 to LSP: Neovim RC From Scratch", from his official channel (needs uvx and yt-dlp).
set -eu
cd "$(dirname "$0")/dynamic_media/play"
src=made_of_motion.mp4
length=$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$src")
if [ ! -s primeagen.mp4 ]; then
    uvx -q yt-dlp -f "bv*[height<=720][ext=mp4]" -o primeagen_full.mp4 \
        "https://www.youtube.com/watch?v=w7i4amO_zaE"
    # 10 s of him on camera, cut to 4:3 around his face
    ffmpeg -v error -y -ss 2 -t 10.2 -i primeagen_full.mp4 -an -vf "crop=960:720:134:0" \
        -c:v libopenh264 -b:v 8M primeagen.mp4
    rm primeagen_full.mp4
fi
# Tiles are cut from the film twice in a row, by exact frame time; the cameo loops.
filters="[0:v][1:v]concat=n=2:v=1:a=0,fps=24,scale=640:480,setsar=1,split=20"
for k in $(seq 0 19); do filters="$filters[s$k]"; done
filters="$filters;[2:v][3:v]concat=n=2:v=1:a=0,fps=24,scale=640:480,setsar=1,trim=duration=$length,setpts=PTS-STARTPTS[cameo];"
pads=""
layout=""
for k in $(seq 0 19); do
    start=$(echo "$length * $k / 20" | bc -l)
    if [ "$k" -eq 16 ]; then
        filters="$filters[s$k]nullsink;"
        pads="$pads[cameo]"
    else
        filters="$filters[s$k]trim=start=$start:duration=$length,setpts=PTS-STARTPTS[t$k];"
        pads="$pads[t$k]"
    fi
    layout="$layout$((k % 5 * 640))_$((k / 5 * 480))|"
done
ffmpeg -v error -y -i "$src" -i "$src" -i primeagen.mp4 -i primeagen.mp4 \
    -filter_complex "${filters}${pads}xstack=inputs=20:layout=${layout%|}[v]" \
    -map "[v]" -an -c:v libopenh264 -b:v 40M -pix_fmt yuv420p -t "$length" billboards.mp4
echo "made billboards.mp4"
