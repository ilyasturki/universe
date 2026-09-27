import math
import os
import random
import struct
import wave

RATE = 44100
OUT = os.path.dirname(os.path.abspath(__file__))


def tone(freq, ms, gain=0.5, attack=0.002, curve=2.2, sweep=0.0, harmonics=((1, 1.0),)):
    n = int(RATE * ms / 1000)
    out = []
    phase = 0.0
    for i in range(n):
        t = i / RATE
        f = freq * (1.0 + sweep * (t / (ms / 1000)))
        phase += 2 * math.pi * f / RATE
        env = min(1.0, t / attack) * math.exp(-curve * t / (ms / 1000))
        out.append(gain * env * sum(g * math.sin(phase * k) for k, g in harmonics))
    return out


# Band-passed noise under a rise-and-fall envelope: the Control Center's breath.
def whoosh(ms, gain=0.5, low=500.0, high=2400.0, peak=0.4, seed=5):
    rng = random.Random(seed)
    n = int(RATE * ms / 1000)
    out = []
    lp = bp = 0.0
    for i in range(n):
        t = i / n
        cutoff = low + (high - low) * math.sin(math.pi * min(1.0, t / (2 * peak)))
        a = 1 - math.exp(-2 * math.pi * cutoff / RATE)
        lp += a * (rng.uniform(-1, 1) - lp)
        bp += 0.08 * (lp - bp)
        env = math.sin(math.pi * t / (2 * peak)) if t < peak else math.exp(-4.0 * (t - peak) / (1 - peak))
        out.append(gain * env * (lp - bp) * 3.0)
    return out


def mix(*parts):
    n = max(len(p[1]) + p[0] for p in parts)
    buf = [0.0] * n
    for offset, samples in parts:
        for i, s in enumerate(samples):
            buf[offset + i] += s
    return buf


def write(name, samples):
    peak = max(1e-06, *(abs(s) for s in samples))
    k = 0.85 / peak if peak > 0.85 else 1.0
    with wave.open(os.path.join(OUT, name + ".wav"), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(b"".join(struct.pack("<h", int(max(-1, min(1, s * k)) * 32767)) for s in samples))


def at(ms):
    return int(RATE * ms / 1000)


SOUNDS = {
    "tick": tone(1480, 34, gain=0.26, attack=0.0015, curve=4.2, harmonics=((1, 1.0), (2.01, 0.18))),
    "type": tone(1250, 24, gain=0.2, attack=0.001, curve=4.6),
    "edge": tone(310, 60, gain=0.24, curve=3.6, harmonics=((1, 1.0), (2, 0.2))),
    "ok": mix(
        (0, tone(659, 120, gain=0.36, curve=3.0, harmonics=((1, 1.0), (2, 0.28), (3, 0.08)))),
        (at(18), tone(988, 140, gain=0.2, curve=3.2, harmonics=((1, 1.0), (2, 0.15)))),
    ),
    "select": mix((0, tone(880, 70, gain=0.3, curve=3.2)), (at(36), tone(1319, 90, gain=0.22, curve=3.0))),
    "back": mix((0, tone(740, 90, gain=0.3, curve=3.0, sweep=-0.1)), (at(34), tone(554, 110, gain=0.22, curve=3.0))),
    "open": mix(
        (0, tone(587, 90, gain=0.24, curve=3.0)), (at(40), tone(880, 130, gain=0.2, curve=2.8)), (0, whoosh(220, gain=0.12, low=900, high=3200, peak=0.3))
    ),
    "panel": mix((0, whoosh(420, gain=0.5, low=380, high=2600, peak=0.32)), (at(60), tone(1175, 220, gain=0.08, curve=3.4))),
    "home": mix(
        (0, tone(784, 140, gain=0.28, curve=2.8, harmonics=((1, 1.0), (2, 0.2)))),
        (at(90), tone(1175, 220, gain=0.24, curve=2.6, harmonics=((1, 1.0), (2, 0.15)))),
    ),
    "launch": mix(
        (0, whoosh(700, gain=0.3, low=300, high=2000, peak=0.45)),
        (at(40), tone(523, 420, gain=0.2, curve=2.2, harmonics=((1, 1.0), (2, 0.25)))),
        (at(160), tone(784, 460, gain=0.18, curve=2.0, harmonics=((1, 1.0), (2, 0.2)))),
        (at(300), tone(1047, 520, gain=0.16, curve=1.9)),
    ),
}

if __name__ == "__main__":
    for name, samples in SOUNDS.items():
        write(name, samples)
        print(name, f"{len(samples) / RATE * 1000:.0f} ms")
