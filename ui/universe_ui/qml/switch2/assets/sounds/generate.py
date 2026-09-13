import math
import os
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
        s = sum(g * math.sin(phase * k) for k, g in harmonics)
        out.append(gain * env * s)
    return out


def mix(*parts):
    n = max(len(p[1]) + p[0] for p in parts)
    buf = [0.0] * n
    for offset, samples in parts:
        for i, s in enumerate(samples):
            buf[offset + i] += s
    return buf


def write(name, samples):
    peak = max(1e-6, max(abs(s) for s in samples))
    k = 0.85 / peak if peak > 0.85 else 1.0
    with wave.open(os.path.join(OUT, name + ".wav"), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(b"".join(struct.pack("<h", int(max(-1, min(1, s * k)) * 32767)) for s in samples))


def at(ms):
    return int(RATE * ms / 1000)


SOUNDS = {
    "tick": tone(2600, 38, gain=0.32, attack=0.001, curve=3.0, harmonics=((1, 1.0), (2, 0.25))),
    "ok": mix((0, tone(880, 90, gain=0.45, curve=2.6, sweep=0.18, harmonics=((1, 1.0), (2, 0.35), (3, 0.12)))),
              (at(28), tone(1320, 70, gain=0.28, curve=2.8, harmonics=((1, 1.0), (2, 0.2))))),
    "back": mix((0, tone(660, 95, gain=0.42, curve=2.6, sweep=-0.14, harmonics=((1, 1.0), (2, 0.3)))),
                (at(30), tone(495, 70, gain=0.26, curve=2.8))),
    "edge": tone(420, 45, gain=0.22, curve=3.2, harmonics=((1, 1.0), (2, 0.15))),
    "type": tone(1900, 26, gain=0.24, attack=0.001, curve=3.4),
    "home": mix((0, tone(1046, 110, gain=0.36, curve=2.4, harmonics=((1, 1.0), (2, 0.25)))),
                (at(95), tone(1568, 150, gain=0.3, curve=2.2, harmonics=((1, 1.0), (2, 0.2))))),
    "launch": mix((0, tone(784, 180, gain=0.34, curve=2.0, harmonics=((1, 1.0), (2, 0.3)))),
                  (at(110), tone(1175, 220, gain=0.3, curve=1.9, harmonics=((1, 1.0), (2, 0.25)))),
                  (at(220), tone(1568, 320, gain=0.28, curve=1.7, harmonics=((1, 1.0), (2, 0.2))))),
    "select": mix((0, tone(1175, 60, gain=0.34, curve=2.8)), (at(40), tone(1568, 80, gain=0.28, curve=2.8))),
    "open": mix((0, tone(740, 80, gain=0.3, curve=2.6)), (at(50), tone(988, 110, gain=0.28, curve=2.4))),
}

if __name__ == "__main__":
    for name, samples in SOUNDS.items():
        write(name, samples)
        print(name, f"{len(samples) / RATE * 1000:.0f} ms")
