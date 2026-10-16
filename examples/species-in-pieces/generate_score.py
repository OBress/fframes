"""An original, unhurried 196-second score. No borrowed recordings or samples.

Soft struck strings and sustained open chords leave space for the original
four-second animal cycles. Regenerate with Python 3 and NumPy.
"""
from pathlib import Path
import wave
import numpy as np

RATE = 24_000
INTRO = 8
DURATION = INTRO + 180 + 8
rng = np.random.default_rng(180)
mix = np.zeros((RATE * DURATION, 2), dtype=np.float64)


def add(signal, start, pan=0.):
    offset = round(start * RATE)
    if offset >= len(mix):
        return
    count = min(len(signal), len(mix) - offset)
    angle = (pan + 1) * np.pi / 4
    mix[offset:offset + count, 0] += signal[:count] * np.cos(angle)
    mix[offset:offset + count, 1] += signal[:count] * np.sin(angle)


def piano(note, start, level=.12, pan=0.):
    t = np.arange(8 * RATE) / RATE
    f = 440 * 2 ** ((note - 69) / 12)
    signal = np.zeros_like(t)
    for harmonic, amount, decay in [(1, 1., .65), (2, .36, 1.), (3, .13, 1.8), (4, .045, 2.8)]:
        phase = 2 * np.pi * f * harmonic * (1 + harmonic * .00015) * t
        signal += np.sin(phase) * amount * np.exp(-t * decay)
    signal *= (1 - np.exp(-t * 100)) * level
    add(signal, start, pan)
    for delay, strength in [(.37, .18), (.73, .1), (1.19, .06)]:
        add(signal * strength, start + delay, -pan)


def chord(notes, start, duration=23.):
    t = np.arange(round(duration * RATE)) / RATE
    env = np.sin(np.pi * np.minimum(t / 3., 1) / 2) ** 2
    env *= np.sin(np.pi * np.minimum((duration - t) / 5., 1) / 2) ** 2
    for i, note in enumerate(notes):
        f = 440 * 2 ** ((note - 69) / 12)
        signal = np.sin(2 * np.pi * f * t) + .3 * np.sin(2 * np.pi * f * 1.0015 * t)
        signal *= env * .028 * (0.92 + .08 * np.sin(t * .45 + i))
        add(signal, start, (i - 1.5) * .35)


# A quiet opening phrase joins the first animal's chord without a splice.
chord([50, 57, 62, 66], 0., INTRO + 3.)
piano(74, .65, .055, -.15)
piano(69, 2.8, .045, .15)

harmony = [[50, 57, 62, 66], [47, 54, 62, 66], [43, 55, 62, 69], [45, 57, 64, 71], [50, 57, 64, 69], [47, 59, 66, 73], [43, 55, 62, 66], [45, 57, 62, 69], [50, 57, 62, 66]]
for block, notes in enumerate(harmony):
    start = INTRO + block * 20.
    chord(notes, start)
    # A sparse, evolving phrase; no percussion or edit-speed metronome.
    phrase = [(0., notes[0] + 12), (3., notes[2] + 12), (6.5, notes[3] + 12), (10., notes[1] + 12), (13., notes[2] + 12), (17., notes[3] + (0 if block % 2 else 12))]
    for j, (offset, note) in enumerate(phrase):
        piano(note, start + offset, .12 if j in [0, 3] else .085, (j % 3 - 1) * .35)
    piano(notes[0], start + .15, .10)
    piano(notes[0], start + 10.15, .075)

# Resolve the last animal into the assembled eighteen without changing scene pace.
chord([50, 57, 62, 66], INTRO + 177., 11.)
piano(62, INTRO + 180.35, .10)
piano(74, INTRO + 182.4, .06)
piano(50, INTRO + 183., .055)

# A continuous, very quiet bed of filtered air prevents dead silence between notes.
for start in range(0, DURATION, 10):
    count = 12 * RATE
    raw = rng.normal(size=count)
    air = np.convolve(raw, np.ones(180) / 180, mode='same')
    env = np.sin(np.linspace(0, np.pi, count)) ** 2
    add(air * env * .03, start, -.2)

mix *= .78 / max(np.max(np.abs(mix)), 1e-6)
mix[:RATE*2] *= np.linspace(0, 1, RATE*2)[:, None]
mix[-RATE*4:] *= np.linspace(1, 0, RATE*4)[:, None]
output = Path(__file__).parent / 'media' / 'pieces.wav'
with wave.open(str(output), 'wb') as wav:
    wav.setnchannels(2)
    wav.setsampwidth(2)
    wav.setframerate(RATE)
    wav.writeframes((mix * 32767).astype('<i2').tobytes())
print(f'{output}: {DURATION}s, original stereo score')
