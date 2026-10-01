import math
import os
import random
import struct
import wave

RATE = 48000
OUT = os.path.dirname(os.path.abspath(__file__))


def at(ms):
    return int(RATE * ms / 1000)


# attack and release in ms, the tone held flat between them.
def pad(freq, ms, gain, attack, release, harm=((1, 1.0),)):
    n = at(ms)
    a, r = max(1, at(attack)), max(1, at(release))
    out = []
    phase = 0.0
    for i in range(n):
        phase += 2 * math.pi * freq / RATE
        env = min(1.0, i / a) * min(1.0, (n - i) / r) ** 2
        out.append(gain * env * sum(h * math.sin(k * phase) for k, h in harm))
    return out


def air(ms, gain, attack, release, seed=1):
    rnd = random.Random(seed)
    n = at(ms)
    a, r = max(1, at(attack)), max(1, at(release))
    out = []
    lp = hp = 0.0
    for i in range(n):
        lp += 0.12 * (rnd.uniform(-1, 1) - lp)
        hp += 0.02 * (lp - hp)
        out.append(gain * min(1.0, i / a) * min(1.0, (n - i) / r) ** 2 * (lp - hp) * 4)
    return out


def mix(*parts):
    n = max(at(p[0]) + len(p[1]) for p in parts)
    buf = [0.0] * n
    for offset, samples in parts:
        o = at(offset)
        for i, s in enumerate(samples):
            buf[o + i] += s
    return buf


def write(name, samples):
    peak = max(1e-06, *(abs(s) for s in samples))
    k = 0.85 / peak if peak > 0.85 else 1.0
    with wave.open(os.path.join(OUT, name + ".wav"), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(b"".join(struct.pack("<h", int(max(-1, min(1, s * k)) * 32767)) for s in samples))


# Only boot.wav is made here: the other sounds came with the Reprise port.
SOUNDS = {
    "boot": mix(
        (0, pad(98, 1700, 0.10, 500, 900, harm=((1, 1.0), (2, 0.35), (3, 0.1)))),
        (60, pad(147, 1640, 0.07, 520, 900, harm=((1, 1.0), (2, 0.25)))),
        (0, air(1500, 0.05, 600, 800, seed=3)),
        (420, pad(587, 1200, 0.035, 300, 900)),
        (620, pad(880, 1000, 0.025, 260, 800)),
    ),
}

if __name__ == "__main__":
    for name, samples in SOUNDS.items():
        write(name, samples)
        print(name, f"{len(samples) / RATE * 1000:.0f} ms")
