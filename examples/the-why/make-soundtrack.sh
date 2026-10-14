#!/bin/sh
# Makes the sound of the film with the ElevenLabs API: the narration, the score and the sound
# effects of SCRIPT.md, into dynamic_media/play/. Then it masters the raw takes with ffmpeg
# into the sounds that `fn audio` in src/lib.rs plays (the last part of this file). Files
# that exist are kept, so delete a file to make it again (REMASTER=1 makes all masters again).
#
# The API key comes from $ELEVENLABS_API_KEY or from ~/.config/fframes/elevenlabs_key.
# VOICE_ID chooses the narrator (default: Brian, a deep, calm voice).
set -eu

cd "$(dirname "$0")"
out=dynamic_media/play
mkdir -p "$out"

key="${ELEVENLABS_API_KEY:-}"
if [ -z "$key" ] && [ -f "$HOME/.config/fframes/elevenlabs_key" ]; then
    key="$(cat "$HOME/.config/fframes/elevenlabs_key")"
fi
voice="${VOICE_ID:-nPczCjzI2devNBz1zQrb}"
api=https://api.elevenlabs.io/v1

# The takes of the film are in sound/ in the repository: the generation gives other takes
# every time, so the film keeps the ones it was mixed with. Missing takes are generated.
for take in sound/*.mp3; do
    [ -s "$out/$(basename "$take")" ] || cp "$take" "$out/"
done

# post <endpoint> <file> <json>: writes the audio of the answer to <file>.
post() {
    if [ -s "$2" ]; then
        echo "keep $2"
        return
    fi
    if [ -z "$key" ]; then
        echo "No API key to make $2: set ELEVENLABS_API_KEY or write it to ~/.config/fframes/elevenlabs_key" >&2
        exit 1
    fi
    echo "make $2"
    status=$(curl -sS -o "$2.part" -w '%{http_code}' -X POST "$api/$1" \
        -H "xi-api-key: $key" -H "Content-Type: application/json" -d "$3")
    if [ "$status" != "200" ]; then
        echo "error $status for $2:" >&2
        cat "$2.part" >&2
        echo >&2
        rm -f "$2.part"
        exit 1
    fi
    mv "$2.part" "$2"
}

# ---- The narration: a calm, warm voice, close to the microphone, unhurried.
say() {
    post "text-to-speech/$voice?output_format=mp3_44100_128" "$out/$1.mp3" "{
        \"text\": \"$2\",
        \"model_id\": \"eleven_v3\",
        \"voice_settings\": {\"stability\": 0.5, \"similarity_boost\": 0.8, \"style\": 0.15, \"speed\": 0.92}
    }"
}
say voice_how "We taught the machines how."
say voice_better "Now they do it better than we ever did."
say voice_anything "They can build anything."
say voice_worth "They just don't know what is worth building."
say voice_theirs "That part was never theirs to take."
# the quote comes in two lines, in time with the two lines on the picture
say voice_quote_a "If you have your why,"
say voice_quote_b "you can bear almost any how."

# ---- The score: one piece, composed to the cut (the durations are the shots of SCRIPT.md).
post "music?output_format=mp3_44100_128" "$out/score.mp3" '{
    "model_id": "music_v2_5",
    "composition_plan": {"chunks": [
        {"text": "[Cursor] {low drone} {a ticking clock}", "duration_ms": 7000,
         "positive_styles": ["cinematic film score", "dark ambient", "low sub drone", "ticking clock", "sparse", "minimal", "great production quality"],
         "negative_styles": ["vocals", "drums"]},
        {"text": "[Factory] {mechanical pulse} {metallic percussion}", "duration_ms": 22000,
         "positive_styles": ["mechanical pulse", "metallic industrial percussion", "cold analog synths", "precise rising arpeggio", "relentless", "hybrid orchestral electronic"],
         "negative_styles": ["vocals", "warm", "happy"]},
        {"text": "[City] {hollow piano}", "duration_ms": 13000,
         "positive_styles": ["hollow felt piano", "wide reverb", "the pulse drops away", "empty", "melancholic", "spacious"],
         "negative_styles": ["vocals", "drums"]},
        {"text": "[Walk] {wind} {one sustained string note}", "duration_ms": 10000,
         "positive_styles": ["almost silence", "one sustained cello note", "airy texture", "lonely", "very quiet"],
         "negative_styles": ["vocals", "drums", "melody"]},
        {"text": "[Line] {silence} {one warm note}", "duration_ms": 6500,
         "positive_styles": ["near silence", "a single warm sustained note at the end", "intimate", "hopeful"],
         "negative_styles": ["vocals", "drums", "rhythm"]},
        {"text": "[Wave] {strings swell} {heartbeat}", "duration_ms": 8500,
         "positive_styles": ["warm strings swell", "low heartbeat pulse", "building", "emotional", "rising"],
         "negative_styles": ["vocals", "synth arpeggio"]},
        {"text": "[Why] {full build}", "duration_ms": 15000,
         "positive_styles": ["full orchestral build", "soaring strings", "noble brass", "epic cinematic drums", "hopeful", "triumphant"],
         "negative_styles": ["vocals", "electronic drops"]},
        {"text": "[Sunrise] {sustained warm chord}", "duration_ms": 10000,
         "positive_styles": ["the peak resolves", "sustained warm major chord", "glowing strings", "calm", "under narration"],
         "negative_styles": ["vocals", "drums"]},
        {"text": "[Title] {solo piano}", "duration_ms": 6000,
         "positive_styles": ["single solo piano line", "gentle", "ending", "fades out"],
         "negative_styles": ["vocals", "drums", "strings"]}
    ]}
}'

# ---- The sound effects.
sound() {
    post "sound-generation?output_format=mp3_44100_128" "$out/$1.mp3" "{
        \"text\": \"$2\", \"duration_seconds\": $3, \"prompt_influence\": 0.6
    }"
}
sound sfx_factory "Huge automated factory hall: hundreds of robotic arms whirring in rhythm, servo motors, distant welding crackle, metallic clanks, deep machinery hum" 22
sound sfx_arc "Close electric welding arc ignites with a sharp crackle and sizzle" 2
sound sfx_servo "A single robotic arm turns its head: precise servo motor whir that rises and stops" 2.5
sound sfx_hud "Soft futuristic interface boots up: quiet digital beeps and a scanning tone" 2
sound sfx_dive "A tiny mechanical click, then a deep whoosh that dives down" 3
sound sfx_city "Night city without people: electric hum, distant cranes, soft traffic whoosh, no voices" 13
sound sfx_wind "Cold desert wind at night, soft gusts, sand drifting" 16
sound sfx_steps "Slow footsteps of one person on dry sand and gravel" 8
sound sfx_hum "A warm glowing line of light hums softly, a gentle tonal resonance" 5
sound sfx_streak "A fast streak of light races away into the distance, airy whoosh" 2
sound sfx_wave "Hundreds of robotic arms stop one after another: a cascade of servos winding down" 6
sound sfx_cables "Steel bridge cables tighten and creak over a canyon" 4
sound sfx_glass "Glass panels click into place one after another, airy chimes" 4
sound sfx_rocket "A rocket ignites on the pad and lifts off: deep rumble rising to a roar" 6
sound sfx_birds "Sparse morning birds in a quiet desert at sunrise" 9
sound sfx_keys "A person types slowly on a quiet mechanical keyboard: a few soft keystrokes in a silent room" 2.5
sound sfx_flood "A digital stream of thousands of keystrokes that speeds up and rises into a whoosh" 3
sound sfx_flash "A deep cinematic whoosh rises into a bright flash" 1.5
sound sfx_hud_type "Quiet electronic blips of letters typed on a robot display" 1.2
sound sfx_crane "A tower crane motor whines and its steel cable creaks far away at night" 5
sound sfx_village "A small village at dusk far away: one dog barks, a door closes, soft wind" 5
sound sfx_ignite "A glowing line ignites with a soft warm whoom" 2
sound sfx_sun "A bright airy whoosh of light swells and fades into silence" 2.5
sound sfx_boom "A low cinematic boom with a long soft tail" 5

# ---- The second layer of the sound design: single steps placed on the footfalls of the
# walk, the details of the builds, and the low hits under the cuts.
for i in 1 2 3 4; do
    sound "sfx_step_$i" "One single slow footstep of a man on dry desert sand and fine gravel, close, dry, variation $i" 0.8
done
sound sfx_scrape "A fingertip drags slowly through dry dust on hard cracked ground, a soft gritty scrape, very close" 2.5
sound sfx_embers "Glowing embers crackle and sizzle softly in the dust, small warm sparks, close and intimate" 4
sound sfx_screens "Thousands of computer screens hum in a huge dark room: electrical buzz and a faint high whine" 3
sound sfx_chip "A robot gripper places a tiny chip on a circuit board: a precise click and a short servo whir" 1.5
sound sfx_swell "A deep warm swell of light: a resonant hum rises and a large glass wall vibrates softly" 3
sound sfx_foghorn "A distant foghorn on the San Francisco bay at dusk: one long low tone far away with echo" 5
sound sfx_spin "Steel cable spinning wheels run along a suspension bridge: an airy mechanical whir far away" 3
sound sfx_deck "Huge steel bridge deck sections lock into place one after another far away: deep metallic clanks with echo" 3
sound sfx_drones "A swarm of small drones hovers and flies past: soft electric buzzing that moves" 4
sound sfx_bloom "Water mist sprinklers hiss softly and plants unfold in a greenhouse: gentle organic rustle" 3
sound sfx_deluge "A massive water deluge roars at a rocket launch pad and steam billows out: a huge hiss" 4
sound sfx_sub "A deep cinematic sub bass hit, soft and huge, short tail" 2.5
sound sfx_breeze "A soft desert morning breeze, very gentle and calm, faint sand drifting" 9
# The wind above is only a low rumble: nothing in it masks the steps. This is the air of it.
sound sfx_air "Cold night wind over a wide flat desert plateau: a steady airy whoosh with slow gentle gusts, fine sand hissing along the ground, open and lonely, no rumble, no microphone buffeting, no voices" 22


# ---- Mastering. The takes above are raw: their levels differ by 50 dB, some are mono and
# some carry hiss and clicks. Each sound of the mix is made from them here, cleaned, made
# wide where it helps, and normalised to the same loudness, REF. So a gain in `fn audio` is
# the level of the sound relative to the others. 32-bit float WAV keeps the peaks of quiet
# takes that go above 0 dBFS after the normalisation; the gains of the mix bring them down.
REF=-24

# loudness <file>: the integrated loudness in LUFS. The padding lets a sound shorter than
# the 400 ms block of the meter be measured.
loudness() {
    ffmpeg -nostats -i "$1" -af "apad=pad_dur=0.5,ebur128" -f null - 2>&1 |
        awk '$1 == "I:" { v = $2 } END { print v }'
}

# The rumble below 25 Hz only costs headroom.
clean="highpass=f=25"

# master <name> <filter graph from [0:a] (and [1:a]...) to [a]> <takes...>: makes
# $out/<name>.wav.
master() {
    dst="$out/$1.wav"
    graph=$2
    shift 2
    if [ -s "$dst" ] && [ -z "${REMASTER:-}" ]; then
        echo "keep $dst"
        return
    fi
    echo "master $dst"
    inputs=""
    for take in "$@"; do inputs="$inputs -i $out/$take"; done
    # shellcheck disable=SC2086
    ffmpeg -v error -y $inputs -filter_complex "$graph" -map '[a]' -c:a pcm_f32le "$dst.part.wav"
    gain=$(awk -v ref="$REF" -v level="$(loudness "$dst.part.wav")" 'BEGIN { print ref - level }')
    ffmpeg -v error -y -i "$dst.part.wav" -af "volume=${gain}dB" -c:a pcm_f32le "$dst"
    rm "$dst.part.wav"
}

# plain <name>: the take as it is.
plain() {
    master "$1" "[0:a]$clean[a]" "$1.mp3"
}
# wider <name> <factor>: a stereo take with its side (L - R) raised by <factor>.
wider() {
    master "$1" "[0:a]$clean,extrastereo=m=$2[a]" "$1.mp3"
}
# spread <name> <side gain dB>: a mono take made wide. The side is the mid through 30 ms of
# white noise: it sounds like the mid but does not correlate with it, and it cancels in a
# mono downmix. The gain sets its level (each take needs its own).
spread() {
    master "$1" "[0:a]$clean,pan=mono|c0=0.5*c0+0.5*c1,asplit[m][d];
        anoisesrc=d=0.03:c=white:seed=3:r=44100,afade=t=out:d=0.03:curve=exp[ir];
        [d][ir]afir=irnorm=-1:irgain=1,highpass=f=150,volume=$2dB[s];
        [m][s]amerge=inputs=2,pan=stereo|c0=c0+c1|c1=c0-c1[a]" "$1.mp3"
}

# The narration: no rumble, and its peaks 3 dB lower (a soft compressor, then a limiter), so
# the master limiter of the mix does not touch the voice.
for name in voice_how voice_better voice_anything voice_worth voice_theirs voice_quote_a \
    voice_quote_b; do
    master "$name" "[0:a]highpass=f=70,
        acompressor=threshold=0.1:ratio=2.5:attack=3:release=80:knee=4,
        alimiter=limit=0.35:attack=2:release=40:level=0[a]" "$name.mp3"
done

# The score: its loudest peaks 3 to 4 dB lower, so the climax with the launch stays under
# the ceiling of the master limiter (-1 dBTP) without the limiter.
master score "[0:a]$clean,alimiter=limit=0.63:attack=5:release=120:level=0[a]" score.mp3

# The beds of the places, wide.
# The wind take is the buffeting of a microphone more than wind: cut its lowest rumble and
# hold down its thumps.
master sfx_wind "[0:a]highpass=f=50,highpass=f=50,
    acompressor=threshold=0.05:ratio=4:attack=2:release=150:knee=6,
    alimiter=limit=0.07:attack=3:release=150:level=0[a]" sfx_wind.mp3
master sfx_air "[0:a]highpass=f=120,highpass=f=120,equalizer=f=700:t=o:w=2:g=12,equalizer=f=2500:t=o:w=1.5:g=5[a]" sfx_air.mp3
wider sfx_factory 1.15
plain sfx_city
plain sfx_breeze
spread sfx_screens -15
spread sfx_birds -2
spread sfx_village -15
# Big events, a little wide.
spread sfx_rocket -10
spread sfx_wave -23
spread sfx_swell -9
spread sfx_embers -23
spread sfx_bloom -21
spread sfx_flood -24
spread sfx_foghorn -16
spread sfx_deck -11
for name in sfx_arc sfx_boom sfx_cables sfx_chip sfx_crane sfx_deluge sfx_dive sfx_drones \
    sfx_flash sfx_glass sfx_hud sfx_hud_type sfx_hum sfx_ignite sfx_keys sfx_servo sfx_spin \
    sfx_sub sfx_sun; do
    plain "$name"
done
# The whoosh of light plays four times: two of them pitched, so it does not repeat.
plain sfx_streak
master sfx_streak_low "[0:a]$clean,asetrate=44100*0.8,aresample=44100[a]" sfx_streak.mp3
master sfx_streak_high "[0:a]$clean,asetrate=44100*1.12,aresample=44100[a]" sfx_streak.mp3
# Other machines of the montage, from the same takes: the rotors of the wind farm, the gantry
# of the school, the rollout of the rocket, and the water that runs down the canal.
pitched() {
    master "$1" "[0:a]$clean,asetrate=44100*$3,aresample=44100$4[a]" "$2.mp3"
}
pitched sfx_spin_low sfx_spin 0.7 ""
pitched sfx_servo_high sfx_servo 1.15 ""
pitched sfx_servo_low sfx_servo 0.85 ""
pitched sfx_water sfx_deluge 0.8 ",lowpass=f=2500"

# The footfalls of the walk: single steps cut from the take of the slow walk (the four short
# step takes hold more hiss than step). A soft crunch of dry ground: no rumble, no hiss above
# 4 kHz, no clicks, softer attacks. Mono, the mix pans them.
foot="highpass=f=80,highpass=f=80,lowpass=f=3800,lowpass=f=3800,adeclick,
    acompressor=threshold=0.06:ratio=3:attack=0.2:release=60:knee=4,
    afade=t=in:d=0.004,afade=t=out:st=0.2:d=0.3:curve=exp"
# step <name> <start in the take> <rate>: one footfall; a rate below 1 makes it lower and
# slower, a heavier step.
step() {
    master "$1" "[0:a]atrim=start=$2:duration=0.5,asetpts=N/SR/TB,pan=mono|c0=0.5*c0+0.5*c1,
        asetrate=44100*$3,aresample=44100,$foot[a]" sfx_steps.mp3
}
n=1
for start in 0.527 1.280 2.030 2.778 3.533 4.282 5.031 5.786 6.530 7.287; do
    step "sfx_foot_$n" "$start" 1
    n=$((n + 1))
done
step sfx_foot_11 2.778 0.89
step sfx_foot_12 4.282 1.07
step sfx_foot_13 5.786 0.94
# The knee into the dust: the heaviest footfall, lower and slower, over the soft shift of
# sand of a step take.
master sfx_knee "[0:a]atrim=start=2.030:duration=0.5,asetpts=N/SR/TB,pan=mono|c0=0.5*c0+0.5*c1,
    asetrate=44100*0.75,aresample=44100,$foot,lowpass=f=2500[k];
    [1:a]pan=mono|c0=0.5*c0+0.5*c1,highpass=f=120,lowpass=f=3000,lowpass=f=3000,
    afade=t=out:st=0.3:d=0.5,volume=0.5[s];
    [k][s]amix=inputs=2:duration=longest:normalize=0[a]" sfx_steps.mp3 sfx_step_1.mp3
# The finger in the dust: the scrape without its clicks and its hiss.
master sfx_finger "[0:a]pan=mono|c0=0.5*c0+0.5*c1,adeclick,highpass=f=150,
    lowpass=f=6000,lowpass=f=6000[a]" sfx_scrape.mp3

echo "done: $out"
