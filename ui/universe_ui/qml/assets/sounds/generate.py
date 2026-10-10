import math
import os
import random
import struct
import wave

RATE = 48000
OUT = os.path.dirname(os.path.abspath(__file__))


def at(ms):
    return int(RATE * ms / 1000)


# WebAudio's linearRamp then setTargetAtTime: a linear rise over attack, held, then a fall whose time constant is a third of release.
def envelope(ms, gain, attack, release, length):
    a, hold, tau = max(1, at(attack)), max(at(attack), at(ms - release)), max(1.0, at(release) / 3)
    return [gain * (i / a if i < a else 1.0 if i < hold else math.exp(-(i - hold) / tau)) for i in range(length)]


# A partial that rises in 4 ms, then rings down from 8 ms with its own time constant (in seconds).
def ring(freq, gain, tau, length):
    return [
        gain * min(1.0, i / at(4)) * (math.exp(-(i - at(8)) / (tau * RATE)) if i > at(8) else 1.0) * math.sin(2 * math.pi * freq * i / RATE)
        for i in range(length)
    ]


# A held tone of a few harmonics, peak-normalised as WebAudio's PeriodicWave is.
def pad(freq, ms, gain, attack, release, harm=(1.0,)):
    n = at(ms + 2 * release)
    wave_of = [sum(h * math.sin((k + 1) * 2 * math.pi * j / 512) for k, h in enumerate(harm)) for j in range(512)]
    peak = max(abs(v) for v in wave_of)
    env = envelope(ms, gain, attack, release, n)
    return [env[i] * sum(h * math.sin((k + 1) * 2 * math.pi * freq * i / RATE) for k, h in enumerate(harm)) / peak for i in range(n)]


# White noise through WebAudio's band-pass biquad (Q = centre / bandwidth), its centre swept exponentially to sweep over ms.
def air(ms, gain, attack, release, low, high, sweep=None, seed=1):
    rnd = random.Random(seed)
    n = at(ms + 2 * release)
    centre, q = math.sqrt(low * high), math.sqrt(low * high) / (high - low)
    env = envelope(ms, gain, attack, release, n)
    out = []
    x1 = x2 = y1 = y2 = 0.0
    for i in range(n):
        f = centre * (sweep / centre) ** min(1.0, i / at(ms)) if sweep else centre
        w = 2 * math.pi * f / RATE
        alpha = math.sin(w) / (2 * q)
        x = rnd.uniform(-1, 1)
        y = (alpha * x - alpha * x2 + 2 * math.cos(w) * y1 - (1 - alpha) * y2) / (1 + alpha)
        x2, x1, y2, y1 = x1, x, y1, y
        out.append(env[i] * y)
    return out


# A sine that drops from 2.2 times its pitch in 90 ms, up in 6 ms and gone over about ms.
def thump(freq, ms, gain):
    out = []
    phase = 0.0
    for i in range(at(2 * ms)):
        f = freq * 2.2 * (1 / 2.2) ** min(1.0, i / at(90))
        phase += 2 * math.pi * f / RATE
        level = min(1.0, i / at(6)) * (math.exp(-(i - at(20)) / (ms / 4000 * RATE)) if i > at(20) else 1.0)
        out.append(gain * level * math.sin(phase))
    return out


def bell(freq, ms, gain):
    n = at(2 * ms)
    partials = ((1, 1.0), (2.01, 0.42), (3.0, 0.2), (4.2, 0.09), (5.43, 0.05))
    return mix(*((0, ring(freq * k, gain * a, ms / 1000 / (3 + i * 1.5), n)) for i, (k, a) in enumerate(partials)))


def mix(*parts):
    n = max(at(p[0]) + len(p[1]) for p in parts)
    buf = [0.0] * n
    for offset, samples in parts:
        o = at(offset)
        for i, s in enumerate(samples):
            buf[o + i] += s
    return buf


# Scaled by gain, cut where the rest stays 60 dB under the peak, with a 40 ms fade.
def trim(samples, gain):
    floor = max(abs(s) for s in samples) / 1000
    end = max(i for i, s in enumerate(samples) if abs(s) > floor) + 1
    fade = at(40)
    return [gain * s * min(1.0, (end - i) / fade) for i, s in enumerate(samples[:end])]


def write(name, samples):
    peak = max(1e-06, *(abs(s) for s in samples))
    k = 0.85 / peak if peak > 0.85 else 1.0
    with wave.open(os.path.join(OUT, name + ".wav"), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(b"".join(struct.pack("<h", int(max(-1, min(1, s * k)) * 32767)) for s in samples))


# Only boot.wav is made here: the other sounds came with the Reprise port. Its beats are Boot.qml's: the brake at 900 ms, the opening at 1350 ms;
# 0.6 brings it to the loudness of the chime it replaced.
SOUNDS = {
    "boot": trim(
        mix(
            (0, air(930, 0.06, 820, 40, 200, 1400, sweep=6000, seed=3)),
            (905, thump(46, 800, 0.4)),
            (915, bell(987.77, 2000, 0.045)),
            (925, bell(1479.98, 1700, 0.025)),
            (905, pad(246.94, 1300, 0.04, 15, 900, harm=(1.0, 0.3))),
            (1350, air(520, 0.025, 80, 380, 800, 4200, sweep=1600, seed=5)),
        ),
        0.6,
    ),
}

if __name__ == "__main__":
    for name, samples in SOUNDS.items():
        write(name, samples)
        print(name, f"{len(samples) / RATE * 1000:.0f} ms")
