#!/usr/bin/env bash
# Downloads what the video needs but the repository does not ship:
#   dynamic/audio/song.wav  the reference opening's audio (for this experiment only)
#   dynamic/ref/ref.mp4     the reference opening (for side by side comparisons)
#   dynamic/mocap/*.bvh     Bandai Namco Research Motion Dataset 1 (CC BY-NC 4.0)
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p dynamic/audio dynamic/ref dynamic/mocap

if [ ! -f dynamic/ref/ref.mp4 ]; then
  yt-dlp -f "bv*[height<=1080][ext=mp4]+ba[ext=m4a]/b" --merge-output-format mp4 \
    -o "dynamic/ref/ref.%(ext)s" "https://www.youtube.com/watch?v=9rVKos-oGnQ"
fi
if [ ! -f dynamic/audio/song.wav ]; then
  ffmpeg -y -i dynamic/ref/ref.mp4 -vn -ac 2 -ar 44100 -c:a pcm_s16le dynamic/audio/song.wav
fi

base=https://raw.githubusercontent.com/BandaiNamcoResearchInc/Bandai-Namco-Research-Motiondataset/master/dataset/Bandai-Namco-Research-Motiondataset-1/data
for clip in walk_normal walk_proud walk_chimpira walk_happy walk_active walk_masculinity walk_musical \
  walk_not-confident run_active run_normal dash_active dash_normal byebye_happy byebye_active bye_proud \
  bow_proud guide_normal call_normal respond_normal kick_normal punch_normal slash_normal \
  dance-long_normal dance-short_normal; do
  f="dataset-1_${clip}_001.bvh"
  [ -f "dynamic/mocap/$f" ] || curl -sfL "$base/$f" -o "dynamic/mocap/$f"
done
echo "ok: $(ls dynamic/mocap | wc -l | tr -d ' ') clips"
