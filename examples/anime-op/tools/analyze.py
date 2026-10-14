"""Beat grid, onsets and loudness envelope of dynamic/song.wav -> dynamic/analysis.json."""
import json, sys
import numpy as np, librosa

y, sr = librosa.load(sys.argv[1] if len(sys.argv) > 1 else "dynamic/song.wav", sr=22050, mono=True)
tempo, beats = librosa.beat.beat_track(y=y, sr=sr, units="time", tightness=400)
onset_env = librosa.onset.onset_strength(y=y, sr=sr)
onsets = librosa.onset.onset_detect(onset_envelope=onset_env, sr=sr, units="time")
rms = librosa.feature.rms(y=y, hop_length=512)[0]
times = librosa.frames_to_time(np.arange(len(rms)), sr=sr, hop_length=512)
# low band (kick) energy
S = np.abs(librosa.stft(y, n_fft=2048, hop_length=512))
freqs = librosa.fft_frequencies(sr=sr, n_fft=2048)
low = S[freqs < 150].sum(axis=0)
low = low / low.max()
print("tempo", tempo, "n beats", len(beats))
d = np.diff(beats)
print("beat interval median", np.median(d), "-> bpm", 60/np.median(d))
print("first beats", np.round(beats[:12], 3))
# loudness per second
for s in range(0, int(times[-1]) + 1, 2):
    m = (times >= s) & (times < s + 2)
    print(f"{s:3d}s rms {rms[m].mean():.3f} low {low[m].mean():.3f}", end=" | " if s % 10 < 8 else "\n")
json.dump({"tempo": float(np.atleast_1d(tempo)[0]), "beats": [float(b) for b in beats],
           "onsets": [float(o) for o in onsets]}, open("dynamic/analysis.json", "w"))
