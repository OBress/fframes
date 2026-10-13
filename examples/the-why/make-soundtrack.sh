#!/bin/sh
# Makes the sound of the film with the ElevenLabs API: the narration, the score and the sound
# effects of SCRIPT.md, into dynamic_media/play/. Files that exist are kept, so delete a file
# to make it again.
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
if [ -z "$key" ]; then
    echo "No API key: set ELEVENLABS_API_KEY or write it to ~/.config/fframes/elevenlabs_key" >&2
    exit 1
fi
voice="${VOICE_ID:-nPczCjzI2devNBz1zQrb}"
api=https://api.elevenlabs.io/v1

# post <endpoint> <file> <json>: writes the audio of the answer to <file>.
post() {
    if [ -s "$2" ]; then
        echo "keep $2"
        return
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
    post "text-to-speech/$voice?output_format=mp3_44100_192" "$out/$1.mp3" "{
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
say voice_quote "If you have your why... you can bear almost any how."

# ---- The score: one piece, composed to the cut (the durations are the shots of SCRIPT.md).
post "music?output_format=mp3_48000_192" "$out/score.mp3" '{
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
    post "sound-generation?output_format=mp3_44100_192" "$out/$1.mp3" "{
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

echo "done: $out"
