import math
import os
import random
import struct
import wave

RATE = 48000
OUT = os.path.dirname(os.path.abspath(__file__))


def at(ms):
    return int(RATE * ms / 1000)


def envelope(n, attack, fall):
    a = max(1, at(attack))
    k = fall / 20 * math.log(10) / RATE
    tail = max(1, at(3))
    return [min(1.0, (i + 1) / a) * math.exp(k * max(0, i - a)) * min(1.0, (n - i) / tail) for i in range(n)]


# fall: dB per second after the attack; glide: ms from freq to `to`.
def tone(freq, ms, gain, attack=1.0, fall=-600, to=None, glide=0, harm=((1, 1.0),)):
    n = at(ms)
    env = envelope(n, attack, fall)
    g = max(1, at(glide))
    out = []
    phase = 0.0
    for i in range(n):
        f = freq if to is None else freq + (to - freq) * min(1.0, i / g)
        phase += 2 * math.pi * f / RATE
        out.append(gain * env[i] * sum(h * math.sin(k * phase) for k, h in harm))
    return out


def bandpass(samples, lo, hi):
    fc = math.sqrt(lo * hi)
    w = 2 * math.pi * fc / RATE
    alpha = math.sin(w) / (2 * fc / (hi - lo))
    b0, b2, a0, a1, a2 = alpha, -alpha, 1 + alpha, -2 * math.cos(w), 1 - alpha
    for _ in range(2):
        x1 = x2 = y1 = y2 = 0.0
        out = []
        for x in samples:
            y = (b0 * x + b2 * x2 - a1 * y1 - a2 * y2) / a0
            x2, x1, y2, y1 = x1, x, y1, y
            out.append(y)
        samples = out
    return samples


def noise(ms, gain, lo, hi, attack=0.3, fall=-2500, seed=1):
    rnd = random.Random(seed)
    n = at(ms)
    env = envelope(n, attack, fall)
    raw = bandpass([rnd.uniform(-1, 1) for _ in range(n)], lo, hi)
    peak = max(1e-9, *(abs(s) for s in raw))
    return [gain * e * s / peak for e, s in zip(env, raw, strict=True)]


def mix(*parts):
    n = max(at(p[0]) + len(p[1]) for p in parts)
    buf = [0.0] * n
    for offset, samples in parts:
        o = at(offset)
        for i, s in enumerate(samples):
            buf[o + i] += s
    return buf


def echo(samples, ms, gain):
    return mix((0, samples), (ms, [s * gain for s in samples]))


def write(name, samples, level):
    win = at(10)
    loud = max(math.sqrt(sum(s * s for s in samples[i : i + win]) / win) for i in range(0, max(1, len(samples) - win), win // 4))
    k = 10 ** (level / 20) / max(1e-9, loud)
    peak = max(abs(s) for s in samples) * k
    k *= min(1.0, 0.95 / peak)
    with wave.open(os.path.join(OUT, name + ".wav"), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(b"".join(struct.pack("<h", int(max(-1, min(1, s * k)) * 32767)) for s in samples))


def square(*gains):
    return tuple((2 * i + 1, g) for i, g in enumerate(gains))


# parts: (frequency ratio, gain, fall) per partial, each decaying on its own.
def bell(freq, ms, gain, attack, parts, to=None, glide=0):
    return mix(*[(0, tone(freq * r, ms, gain * g, attack=attack, fall=f, to=None if to is None else to * r, glide=glide)) for r, g, f in parts])


def hit(ms, gain, lo=300, hi=16000, seed=1):
    return noise(ms, gain, lo, hi, attack=0.3, fall=-3000, seed=seed)


def tick_click(freq, air, lo=4000, hi=11000, seed=1):
    return mix(
        (0, tone(freq, 26, 1.0, attack=0.4, fall=-2300)), (0, noise(16, air, lo, hi, fall=-2800, seed=seed)), (0, hit(8, air * 0.25, 800, 16000, seed + 50))
    )


# Name: (samples, loudness in dBFS over the loudest 10 ms).
SOUNDS = {
    "tick": (tick_click(1570, 1.6, 4000, 9500, seed=1), -18),
    "tick-side": (tick_click(3140, 1.0, 4500, 12000, seed=2), -18),
    "tick-tile": (
        mix(
            (0, bell(1593, 26, 1.0, 0.4, ((1, 1.0, -2400), (2.633, 0.5, -2800), (1.75, 0.3, -3000), (5.02, 0.25, -3000), (0.794, 0.25, -2600)))),
            (0, noise(18, 0.9, 1200, 14000, fall=-2400, seed=3)),
        ),
        -18,
    ),
    "edge": (
        mix(
            (0, bell(1560, 20, 0.8, 0.4, ((1, 1.0, -2600), (1.93, 0.3, -3000), (3.9, 0.35, -3000), (5.8, 0.25, -3000)))),
            (0, noise(14, 0.35, 2500, 10000, fall=-3000, seed=4)),
            (8, tone(421, 70, 1.0, attack=14, fall=-1100)),
            (8, noise(50, 0.5, 250, 650, attack=12, fall=-1000, seed=5)),
        ),
        -23,
    ),
    "ok": (
        mix(
            (0, bell(2400, 26, 1.0, 4, ((1, 1.0, -2200), (0.91, 0.5, -2600), (1.065, 0.6, -2600)))),
            (0, noise(24, 0.9, 3500, 15000, attack=4, fall=-2200, seed=6)),
            (0, noise(24, 0.5, 700, 3500, attack=4, fall=-2600, seed=7)),
        ),
        -14,
    ),
    "back": (
        mix(
            (0, tone(773, 22, 0.6, attack=1, fall=-2200)),
            (0, tone(1335, 18, 0.5, attack=0.6, fall=-2600)),
            (0, noise(22, 1.1, 500, 2000, attack=3, fall=-2200, seed=8)),
            (0, noise(20, 1.0, 2000, 7000, attack=2, fall=-2400, seed=9)),
            (0, noise(16, 0.5, 7000, 16000, attack=1, fall=-3000, seed=10)),
        ),
        -12,
    ),
    "open": (echo(tone(1570, 110, 1.0, attack=15, fall=-1000), 190, 0.04), -12),
    "type": (
        mix(
            (0, bell(2695, 18, 1.0, 0.4, ((1, 1.0, -3000), (0.826, 0.5, -3000)))),
            (0, noise(12, 0.7, 8500, 12500, fall=-3500, seed=11)),
            (0, hit(6, 0.2, 800, 16000, seed=12)),
            (30, bell(2695, 18, 0.6, 0.4, ((1, 1.0, -3000), (0.826, 0.5, -3000)))),
            (30, noise(12, 0.45, 8500, 12500, fall=-3500, seed=13)),
        ),
        -18,
    ),
    "tab": (
        mix(
            (0, bell(2139, 26, 1.0, 0.5, ((1, 1.0, -2200), (1.267, 0.18, -2600), (1.435, 0.13, -2600), (1.619, 0.09, -2600)))),
            (0, noise(14, 0.25, 7000, 10000, fall=-2800, seed=14)),
            (62, noise(55, 0.3, 1200, 9000, attack=12, fall=-700, seed=15)),
        ),
        -15,
    ),
    "select": (
        mix(
            (0, tone(987, 130, 1.0, attack=18, fall=-320, harm=square(1.0, 0.19, 0.15, 0.21, 0.15))),
            (64, tone(1172, 120, 1.0, attack=20, fall=-430, harm=((1, 1.0), (2, 0.13), (3, 0.53), (5, 0.12), (7, 0.25), (9, 0.07)))),
        ),
        -12,
    ),
    "deselect": (
        mix(
            (0, tone(1328, 20, 0.9, attack=3, fall=-1800, harm=((1, 1.0), (2.6, 0.12), (7, 0.05)))),
            (6, tone(1195, 30, 1.0, attack=1, fall=-1500, harm=((1, 1.0), (1.92, 0.32)))),
            (12, tone(1031, 30, 0.2, attack=1, fall=-1400, harm=((1, 1.0), (1.64, 0.25)))),
            (0, hit(8, 0.15, 3000, 12000, seed=16)),
        ),
        -12,
    ),
    "home": (
        mix(
            (0, hit(6, 0.25, 300, 12000, seed=17)),
            (0, tone(211, 240, 1.0, attack=18, fall=-430, to=178, glide=70)),
            (0, tone(891, 70, 0.25, attack=8, fall=-540, to=782, glide=60)),
            (0, tone(585, 70, 0.14, attack=6, fall=-430)),
            (4, tone(1875, 50, 0.08, attack=4, fall=-500)),
            (18, tone(984, 20, 0.05, attack=2, fall=-900)),
            (8, tone(352, 30, 0.14, attack=2, fall=-1000, to=372, glide=16)),
            (72, hit(6, 0.18, 300, 12000, seed=18)),
            (76, tone(305, 330, 0.68, attack=20, fall=-180, to=294, glide=40)),
            (76, tone(1219, 90, 0.095, attack=14, fall=-330, to=1171, glide=30)),
            (76, tone(586, 30, 0.05, attack=8, fall=-900, to=554, glide=20)),
            (80, tone(1969, 25, 0.024, attack=6, fall=-460)),
            (214, hit(6, 0.2, 300, 12000, seed=19)),
            (218, tone(445, 380, 0.57, attack=22, fall=-195, to=440, glide=30)),
            (218, tone(4500, 45, 0.17, attack=12, fall=-900)),
            (220, tone(1781, 70, 0.1, attack=16, fall=-350, to=1748, glide=30)),
            (296, tone(727, 150, 0.17, attack=24, fall=-280, to=665, glide=40)),
            (294, tone(2640, 50, 0.055, attack=12, fall=-430)),
        ),
        -5,
    ),
    "launch": (
        mix(
            (0, hit(14, 0.6, 200, 16000, seed=20)),
            (0, tone(1611, 25, 0.09, attack=1, fall=-700, harm=((1, 1.0), (0.83, 0.45)))),
            (0, tone(4679, 50, 0.06, attack=1, fall=-190)),
            (0, tone(79, 45, 0.045, attack=6, fall=-220)),
            (180, hit(8, 0.35, 800, 16000, seed=21)),
            (180, tone(2043, 40, 0.48, attack=8, fall=-820)),
            (186, tone(3196, 20, 0.03, attack=8, fall=-420)),
            (204, hit(8, 0.3, 800, 16000, seed=22)),
            (210, tone(2906, 15, 0.056, attack=1, fall=-1400)),
            (218, tone(2137, 20, 0.19, attack=4, fall=-670)),
            (230, tone(2625, 15, 0.09, attack=1, fall=-2000)),
            (232, bell(3258, 260, 1.0, 32, ((1, 1.0, -196), (1.82, 0.05, -218), (2.06, 0.08, -136), (3.06, 0.023, -160)), to=3330, glide=40)),
            (260, tone(3117, 20, 0.057, attack=1, fall=-290)),
        ),
        -6,
    ),
    "icon-grid": (
        mix(
            (0, noise(45, 0.9, 5500, 12000, attack=10, fall=-400, seed=23)),
            (72, bell(1781, 30, 1.0, 4, ((1, 1.0, -1800), (1.84, 0.48, -1600), (3.43, 0.33, -1400)), to=1868, glide=16)),
            (72, hit(12, 0.8, 150, 16000, seed=24)),
            (284, bell(1843, 26, 0.35, 2, ((1, 1.0, -2000), (1.84, 0.4, -1800)))),
            (284, hit(12, 0.6, 150, 16000, seed=25)),
        ),
        -9,
    ),
    "icon-news": (
        mix(
            (0, noise(90, 0.4, 1000, 16000, attack=6, fall=-300, seed=26)),
            (0, tone(393, 130, 0.58, attack=50, fall=-360, harm=((1, 1.0), (2, 0.28), (3, 0.2)))),
            (0, tone(496, 130, 0.64, attack=50, fall=-200, harm=((1, 1.0), (2, 0.1), (3, 0.2), (4, 0.17), (5, 0.07), (6, 0.08)))),
            (286, noise(40, 0.35, 2000, 16000, attack=4, fall=-500, seed=27)),
            (298, tone(786, 120, 0.7, attack=36, fall=-430, harm=((1, 1.0), (2, 0.23), (3, 0.23)))),
        ),
        -9,
    ),
    "icon-shop": (
        mix(
            (0, tone(1582, 900, 0.5, attack=20, fall=-42)),
            (0, tone(8366, 60, 0.012, attack=6, fall=-120)),
            (0, hit(20, 0.12, 200, 9000, seed=28)),
            (200, noise(40, 0.12, 200, 16000, attack=2, fall=-900, seed=29)),
            (202, tone(2662, 700, 0.16, attack=34, fall=-46)),
            (250, noise(40, 0.12, 200, 16000, attack=2, fall=-900, seed=30)),
            (254, tone(2368, 650, 0.12, attack=58, fall=-47)),
            (298, noise(40, 0.12, 200, 16000, attack=2, fall=-900, seed=31)),
            (302, tone(3991, 600, 0.18, attack=50, fall=-51, harm=((1, 1.0), (2.48, 0.12), (1.6, 0.1)))),
        ),
        -9,
    ),
    "icon-album": (
        mix(
            (0, noise(40, 1.0, 3600, 16000, attack=6, fall=-450, seed=32)),
            (52, bell(1953, 90, 0.7, 26, ((1, 1.0, -410), (0.5, 0.8, -380), (1.5, 0.17, -300)))),
            (200, noise(36, 0.7, 3600, 16000, attack=6, fall=-500, seed=33)),
            (210, tone(1329, 60, 0.65, attack=16, fall=-500, harm=((1, 1.0), (2, 0.15), (1.32, 0.15)))),
        ),
        -9,
    ),
    "icon-controllers": (
        mix(
            (0, noise(30, 1.0, 3500, 16000, attack=10, fall=-1100, seed=34)),
            (0, tone(2539, 20, 0.14, attack=4, fall=-800, harm=((1, 1.0), (1.12, 0.9)))),
            (238, tone(2054, 45, 0.6, attack=12, fall=-750, to=2090, glide=20)),
            (238, bell(664, 30, 0.2, 10, ((1, 1.0, -1100), (1.73, 0.9, -1100), (5.07, 0.9, -1200), (6.38, 0.75, -1200)))),
            (238, noise(26, 1.0, 250, 16000, attack=8, fall=-1000, seed=35)),
        ),
        -9,
    ),
    "icon-settings": (
        mix(
            *[
                p
                for i, (t, g) in enumerate(((0, 0.4), (24, 0.44), (47, 0.67), (71, 1.0), (91, 0.22), (112, 0.06), (147, 0.76), (158, 0.31), (191, 0.74)))
                for p in (
                    (t, noise(20, g, 150, 16000, attack=1, fall=-1600, seed=40 + i)),
                    (t, tone(4900 + (i % 2) * 400, 18, g * 0.2, attack=1, fall=-1800, harm=((1, 1.0), (0.55, 0.8), (1.8, 0.5)))),
                )
            ]
        ),
        -9,
    ),
    "icon-power": (
        mix(
            (0, noise(28, 0.6, 300, 16000, attack=8, fall=-1100, seed=50)),
            (0, tone(1265, 30, 0.3, attack=10, fall=-1200, harm=((1, 1.0), (0.78, 0.63), (1.63, 0.5), (1.37, 0.4), (2.43, 0.28)))),
            (42, bell(656, 70, 1.0, 18, ((1, 1.0, -1100), (1.93, 0.14, -800), (3.88, 0.12, -700), (4.91, 0.05, -600)), to=590, glide=50)),
        ),
        -9,
    ),
    "boot": (
        echo(
            mix(
                (0, bell(1047, 700, 1.0, 6, ((1, 1.0, -60), (2.0, 0.2, -90), (3.0, 0.08, -120)))),
                (140, bell(1319, 700, 0.85, 6, ((1, 1.0, -60), (2.0, 0.2, -90)))),
                (280, bell(1568, 900, 0.8, 6, ((1, 1.0, -50), (2.0, 0.18, -80)))),
                (0, noise(520, 0.12, 2000, 9000, attack=60, fall=-30, seed=30)),
            ),
            220,
            0.12,
        ),
        -15,
    ),
}

if __name__ == "__main__":
    for name, (samples, level) in SOUNDS.items():
        write(name, samples, level)
        print(name, f"{len(samples) / RATE * 1000:.0f} ms")
