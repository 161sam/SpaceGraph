#!/usr/bin/env python3
"""Synthesize SpaceGraph's UI sound effects as small 16-bit mono WAVs.

Reproducible, dependency-free (stdlib `wave` only). Re-run after editing to
regenerate the committed assets:

    python3 crates/spacegraph-viewer/assets/audio/gen_sounds.py

Sounds (played by `render::audio`, behind the `audio` cargo feature):
  blip.wav    — node pick / selection feedback (short, soft)
  scan.wav    — scan-pulse sweep (G)
  alert.wav   — new alert appears (two-tone klaxon)
  mission.wav — incident resolved (ascending chime)
"""
import math
import struct
import wave

SR = 22050  # sample rate (Hz) — plenty for short UI blips, keeps files tiny


def _env(i, n, attack=0.005, release=0.05):
    """Linear attack / release envelope in [0,1] to avoid click artefacts."""
    t = i / SR
    dur = n / SR
    a = min(1.0, t / attack) if attack > 0 else 1.0
    r = min(1.0, (dur - t) / release) if release > 0 else 1.0
    return max(0.0, min(a, r))


def _write(name, samples):
    peak = max(1e-9, max(abs(s) for s in samples))
    norm = 0.85 / peak  # leave headroom, no clipping
    with wave.open(name, "w") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(SR)
        frames = bytearray()
        for s in samples:
            v = int(max(-1.0, min(1.0, s * norm)) * 32767)
            frames += struct.pack("<h", v)
        w.writeframes(bytes(frames))
    print(f"wrote {name} ({len(samples)} samples, {len(samples)/SR:.2f}s)")


def sine(freq, dur, decay=0.0):
    n = int(SR * dur)
    out = []
    for i in range(n):
        t = i / SR
        amp = math.exp(-decay * t) if decay else 1.0
        out.append(math.sin(2 * math.pi * freq * t) * amp * _env(i, n))
    return out


def blip():
    return sine(1318.0, 0.08, decay=18.0)


def scan():
    # Descending sweep 1600 -> 380 Hz (sonar "whoosh").
    n = int(SR * 0.45)
    out = []
    phase = 0.0
    for i in range(n):
        frac = i / n
        freq = 1600.0 * (1.0 - frac) + 380.0 * frac
        phase += 2 * math.pi * freq / SR
        out.append(math.sin(phase) * _env(i, n, attack=0.01, release=0.12))
    return out


def alert():
    # Two-tone klaxon alternating every 0.12 s with a slight tremolo.
    dur = 0.72
    n = int(SR * dur)
    out = []
    for i in range(n):
        t = i / SR
        freq = 660.0 if int(t / 0.12) % 2 == 0 else 440.0
        # Square-ish: fundamental + odd harmonic for an urgent edge.
        s = math.sin(2 * math.pi * freq * t) + 0.3 * math.sin(2 * math.pi * 3 * freq * t)
        trem = 0.85 + 0.15 * math.sin(2 * math.pi * 7.0 * t)
        out.append(s * trem * _env(i, n, attack=0.004, release=0.06))
    return out


def mission():
    # Ascending major triad arpeggio C5-E5-G5-C6 → "resolved" chime.
    notes = [523.25, 659.25, 783.99, 1046.50]
    step = 0.13
    out = []
    for k, f in enumerate(notes):
        seg = sine(f, step, decay=6.0)
        out.extend(seg)
    return out


if __name__ == "__main__":
    import os

    here = os.path.dirname(os.path.abspath(__file__))
    _write(os.path.join(here, "blip.wav"), blip())
    _write(os.path.join(here, "scan.wav"), scan())
    _write(os.path.join(here, "alert.wav"), alert())
    _write(os.path.join(here, "mission.wav"), mission())
