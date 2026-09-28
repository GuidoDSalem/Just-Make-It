#!/usr/bin/env python3
"""Genera la canción de demostración (examples/musica/demo-118bpm.mp3), hecha por código.

Obra propia, dedicada al dominio público (CC0): se puede usar sin atribución.
Batería, bajo y acordes a 118 BPM, 16 compases con una intro de 2. Sólo usa la biblioteca
estándar; el MP3 lo codifica ffmpeg.

    python3 scripts/demo_song.py && ffmpeg -y -i /tmp/demo.wav -b:a 128k examples/musica/demo-118bpm.mp3
"""
import math, random, struct, sys, wave

SR = 22050
BPM = 118
BEAT = 60 / BPM
BARS = 16
N = int(SR * BEAT * 4 * BARS + SR * 1.5)
out = [0.0] * N
rnd = random.Random(7)
noise = [rnd.uniform(-1, 1) for _ in range(SR)]


def add(t0, dur, fn, amp=1.0):
    i0 = int(t0 * SR)
    for j in range(int(dur * SR)):
        if i0 + j >= N:
            break
        out[i0 + j] += amp * fn(j / SR, j)


def kick(t, j):
    return math.sin(2 * math.pi * (50 + 90 * math.exp(-t * 30)) * t) * math.exp(-t * 9)


def snare(t, j):
    return (0.7 * noise[j % SR] + 0.3 * math.sin(2 * math.pi * 190 * t)) * math.exp(-t * 18)


def hat(t, j):
    return noise[(j * 7) % SR] * math.exp(-t * 70)


def note(freq, decay):
    def f(t, j):
        return (math.sin(2 * math.pi * freq * t) + 0.3 * math.sin(4 * math.pi * freq * t)) * math.exp(-t * decay)
    return f


# Am  F  C  G  (dos compases cada uno)
CHORDS = [(57, 60, 64), (53, 57, 60), (48, 52, 55), (55, 59, 62)]
hz = lambda m: 440 * 2 ** ((m - 69) / 12)

for bar in range(BARS):
    t_bar = bar * 4 * BEAT
    chord = CHORDS[(bar // 2) % 4]
    for n in chord:  # acordes: un golpe largo por compás
        add(t_bar, 4 * BEAT, note(hz(n), 1.2), 0.07)
    if bar < 2:  # intro: sólo acordes y hi-hat
        for e in range(8):
            add(t_bar + e * BEAT / 2, 0.1, hat, 0.08)
        continue
    for b in range(4):
        t = t_bar + b * BEAT
        add(t, 0.45, kick, 0.9 if b % 2 == 0 else 0.0)
        if b % 2 == 1:
            add(t, 0.25, snare, 0.35)
        add(t, 0.1, hat, 0.12)
        add(t + BEAT / 2, 0.1, hat, 0.08)
        add(t, BEAT * 0.9, note(hz(chord[0] - 24), 3.0), 0.35)  # bajo en negras

peak = max(abs(x) for x in out) or 1
with wave.open(sys.argv[1] if len(sys.argv) > 1 else "/tmp/demo.wav", "wb") as w:
    w.setnchannels(1)
    w.setsampwidth(2)
    w.setframerate(SR)
    w.writeframes(b"".join(struct.pack("<h", int(32000 * 0.9 * x / peak)) for x in out))
