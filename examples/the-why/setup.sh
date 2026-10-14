#!/bin/sh
# Builds everything THE WHY needs at run time into dynamic_media/, on macOS or Linux, from a
# fresh clone. Steps that are done already are skipped, so it is safe to run again.
#
#   1. PBR textures and skies (Poly Haven)                 fetch-assets.sh
#   2. The Golden Gate: terrain, buildings, city lights    fetch-sf.sh   (USGS, DataSF, OSM)
#   3. Manhattan, the boroughs and New Jersey              fetch-nyc.sh  (NYC 3D Building Model, ~1 GB)
#   4. made_of_motion.mp4: renders examples/made-of-motion
#   5. The score, voices and sound effects                 make-soundtrack.sh (ElevenLabs)
#   6. The Times Square billboards                         make-billboards.sh (yt-dlp)
#
# Needs: cargo, python3, ffmpeg, curl, bc, and uv (for yt-dlp). On macOS:
#   brew install ffmpeg python uv
# Step 5 masters the takes of the film in sound/ (no API key needed). A missing take is generated
# with ElevenLabs ($ELEVENLABS_API_KEY or ~/.config/fframes/elevenlabs_key).
set -eu
cd "$(dirname "$0")"

missing=""
for tool in cargo python3 ffmpeg ffprobe curl bc uvx; do
    command -v "$tool" >/dev/null 2>&1 || missing="$missing $tool"
done
if [ -n "$missing" ]; then
    echo "missing:$missing (macOS: brew install ffmpeg python uv; cargo from rustup.rs)" >&2
    exit 1
fi

step() { printf '\n==> %s\n' "$1"; }

step "1/6 textures and skies"
./fetch-assets.sh

step "2/6 the Golden Gate"
./fetch-sf.sh

step "3/6 Manhattan"
if [ -s dynamic_media/nyc/manhattan.bin ]; then
    echo "keep dynamic_media/nyc/manhattan.bin"
else
    ./fetch-nyc.sh
fi

step "4/6 made-of-motion"
mkdir -p dynamic_media/play
if [ -s dynamic_media/play/made_of_motion.mp4 ]; then
    echo "keep dynamic_media/play/made_of_motion.mp4"
else
    # 1440 x 1080 at 2/3 scale: the 960 x 720 of the screens
    (cd ../made-of-motion &&
        cargo run --release -p made-of-motion -- render --scale 0.6666667 \
            -o ../the-why/dynamic_media/play/made_of_motion.mp4)
fi

step "5/6 sound"
# the takes of the film come from sound/; this masters them, no API key needed
./make-soundtrack.sh

step "6/6 billboards"
./make-billboards.sh

printf '\ndone. Render the film:\n'
printf '  cargo run --release -p the-why-example --bin the-why -- render -o the-why-4k.mp4\n'
printf '  (half size for a quick check: render --scale 0.5 -o the-why-1080p.mp4)\n'
