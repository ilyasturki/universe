import json
import math
import os
import re
import statistics
import subprocess
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from datetime import datetime, timedelta

from _common import log, remove

SHOT_RE = re.compile(r"(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})\.(?:png|jpe?g)$", re.I)
# Shots taken just before the recorder started or after it stopped still belong to the sitting.
HEAD_GRACE_S = 90
TAIL_GRACE_S = 120
FRAME_FILL_TARGET = 20
FRAMES_LONG_EDGE = 1568
FRAMES_OVERSAMPLE = 4
FRAMES_WORKERS = 4
TAIL_WINDOW_SEC = 45
TAIL_RESERVE = 3
TAIL_MIN_SEC = 90
GALLERY_FRAME_TARGET = 6
# 8x8 area-averaged gray: below this the frame is black or a flat fade.
FLAT_STDDEV = 4.0
# dHash Hamming distance under which two frames are the same picture.
DUP_DISTANCE = 4


@dataclass
class Image:
    file: str
    t: datetime
    kind: str = "shot"
    tail: bool = False
    off: float = 0.0
    hash: int | None = None
    gi: object = None


class Timeline:
    def __init__(self, start, pauses=()):
        self.start = start
        self.pauses = sorted((a, b) for a, b in pauses if a and b and b > a)

    def offset(self, t):
        off = (t - self.start).total_seconds()
        for a, b in self.pauses:
            if t <= a:
                break
            off -= (min(t, b) - a).total_seconds()
        return off

    def time(self, off):
        t = self.start + timedelta(seconds=off)
        for a, b in self.pauses:
            if t < a:
                break
            t += b - a
        return t

    @classmethod
    def from_env(cls, started_at, pauses_json, session_start, parse):
        start = parse(started_at) or session_start
        try:
            pairs = [(parse(a), parse(b)) for a, b in json.loads(pauses_json or "[]")]
        except (ValueError, TypeError):
            log(f"recording pauses unreadable: {pauses_json!r}")
            pairs = []
        return cls(start, pairs)


def shot_time(name):
    m = SHOT_RE.search(os.path.basename(str(name)))
    if not m:
        return None
    return datetime(*(int(x) for x in m.groups()))


def select_screenshots(attachments_dir, start, end):
    try:
        names = os.listdir(attachments_dir)
    except OSError:
        return []
    lo = start - timedelta(seconds=HEAD_GRACE_S)
    hi = end + timedelta(seconds=TAIL_GRACE_S)
    out = []
    for name in names:
        t = shot_time(name)
        if t and lo <= t <= hi:
            out.append(Image(os.path.join(attachments_dir, name), t, "shot"))
    out.sort(key=lambda im: im.t)
    return out


def even_sample(items, maximum):
    if len(items) <= maximum:
        return list(items)
    if maximum == 1:
        return [items[-1]]
    step = (len(items) - 1) / (maximum - 1)
    return [items[round(i * step)] for i in range(maximum)]


def probe_duration(path):
    try:
        out = subprocess.run(
            ["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "default=nw=1:nk=1", path],
            capture_output=True, text=True, timeout=60,
        ).stdout.strip()
        return float(out) if out else None
    except (OSError, ValueError, subprocess.TimeoutExpired):
        return None


# 64-bit difference hash of 9x8 row-major gray bytes.
def dhash(raw):
    bits = 0
    for r in range(8):
        row = raw[r * 9:(r + 1) * 9]
        for c in range(8):
            bits = (bits << 1) | (1 if row[c] > row[c + 1] else 0)
    return bits


def hamming(a, b):
    if a is None or b is None:
        return 64
    return bin(a ^ b).count("1")


def grab_frame(recording, off, png_path):
    raw_path = png_path + ".raw"
    cmd = [
        "ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-ss", f"{off:.3f}", "-i", recording,
        "-frames:v", "1", "-an", "-update", "1", "-vf", f"scale='min({FRAMES_LONG_EDGE},iw)':-2", png_path,
        "-frames:v", "1", "-an", "-update", "1", "-vf", "scale=9:8:flags=area,format=gray", "-f", "rawvideo", raw_path,
    ]
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
        if r.returncode != 0 or not os.path.exists(png_path) or os.path.getsize(png_path) == 0:
            return None
        with open(raw_path, "rb") as f:
            raw = f.read()
    except (OSError, subprocess.TimeoutExpired):
        return None
    finally:
        remove(raw_path)
    if len(raw) < 72:
        return None
    return dhash(raw[:72]), statistics.pstdev(raw[:72])


def _min_distance(cand, chosen):
    return min((hamming(cand.hash, c.hash) for c in chosen), default=64)


def greedy_farthest(chosen, rest, k):
    pool = list(rest)
    while len(chosen) < k and pool:
        best = max(pool, key=lambda c: _min_distance(c, chosen))
        if chosen and _min_distance(best, chosen) < DUP_DISTANCE:
            break
        chosen.append(best)
        pool.remove(best)
    return chosen


def stratified_pick(cands, k, a, b):
    if k <= 0 or not cands:
        return []
    if len(cands) <= k:
        return dedupe(sorted(cands, key=lambda c: c.off))
    width = (b - a) / k
    buckets = [[] for _ in range(k)]
    for c in cands:
        buckets[min(k - 1, max(0, int((c.off - a) / width)))].append(c)
    chosen, spare = [], []
    for bucket in buckets:
        if not bucket:
            continue
        best = bucket[len(bucket) // 2]
        if chosen:
            best = max(bucket, key=lambda c: _min_distance(c, chosen))
            if _min_distance(best, chosen) < DUP_DISTANCE:
                spare.extend(bucket)
                continue
        chosen.append(best)
        spare.extend(c for c in bucket if c is not best)
    return greedy_farthest(chosen, spare, k)


def dedupe(images):
    kept = []
    for im in images:
        if im.hash is None or _min_distance(im, kept) >= DUP_DISTANCE:
            kept.append(im)
    return kept


def extract_frames(recording, shots, timeline, need, frames_dir, duration_s):
    if need <= 0 or not duration_s or duration_s <= 3 or not os.path.exists(recording):
        return []
    os.makedirs(frames_dir, exist_ok=True)
    dur = float(duration_s)

    def clamp(x):
        return max(1.0, min(dur - 1, x))

    tail_reserve = min(TAIL_RESERVE, need) if dur >= TAIL_MIN_SEC else 0
    tail_start = dur - min(TAIL_WINDOW_SEC, dur / 4) if tail_reserve else dur
    grid_need = need - tail_reserve

    offsets = sorted(timeline.offset(s.t) for s in shots)
    bounds = [0.0] + [o for o in offsets if 0 < o < tail_start] + [tail_start]
    gaps = [(a, b) for a, b in zip(bounds, bounds[1:]) if b - a > 2]
    if not gaps and not tail_reserve:
        return []

    total = sum(b - a for a, b in gaps) or 1.0
    raw = [grid_need * (b - a) / total for a, b in gaps]
    alloc = [math.floor(r) for r in raw]
    used = sum(alloc)
    for i in sorted(range(len(raw)), key=lambda i: raw[i] - math.floor(raw[i]), reverse=True):
        if used >= grid_need:
            break
        alloc[i] += 1
        used += 1

    specs = []
    for gi, (a, b) in enumerate(gaps):
        n = alloc[gi] * FRAMES_OVERSAMPLE
        for i in range(1, n + 1):
            specs.append((gi, clamp(a + (b - a) * i / (n + 1))))
    tail_n = tail_reserve * FRAMES_OVERSAMPLE
    for i in range(1, tail_n + 1):
        specs.append(("tail", clamp(tail_start + (dur - tail_start) * i / tail_n)))

    def work(item):
        idx, (gi, off) = item
        png = os.path.join(frames_dir, f"f_{idx:03d}.png")
        res = grab_frame(recording, off, png)
        if res is None:
            return None
        h, sd = res
        if sd < FLAT_STDDEV:
            os.remove(png)
            return None
        return Image(png, timeline.time(off), "frame", gi == "tail", off, h, gi)

    with ThreadPoolExecutor(max_workers=FRAMES_WORKERS) as pool:
        cands = [c for c in pool.map(work, enumerate(specs)) if c]

    out = []
    for gi, (a, b) in enumerate(gaps):
        if alloc[gi] > 0:
            out.extend(stratified_pick([c for c in cands if c.gi == gi], alloc[gi], a, b))
    if tail_reserve:
        out.extend(stratified_pick([c for c in cands if c.gi == "tail"], tail_reserve, tail_start, dur))
    out = dedupe(sorted(out, key=lambda c: c.off))
    for c in cands:
        if c not in out:
            remove(c.file)
    log(f"extracted {len(out)} frame(s) from {len(specs)} candidate(s) to fill {need} slot(s)")
    return out


def normalize_review(review, count):
    def in_range(n):
        return isinstance(n, int) and not isinstance(n, bool) and 1 <= n <= count
    review = review if isinstance(review, dict) else {}
    unusable = {n for n in (review.get("unusable") or []) if in_range(n)}
    order, ranked = [], set()
    for n in review.get("gallery") or []:
        if not in_range(n) or n in unusable or n in ranked:
            continue
        ranked.add(n)
        order.append(n)
    order.extend(n for n in range(1, count + 1) if n not in ranked and n not in unusable)
    return order, unusable


def pick_gallery(fed, order, unusable, shots, frames):
    rejected = {fed[n - 1].file for n in unusable}
    kept_shots = [s for s in shots if s.file not in rejected]
    if shots and not kept_shots:
        log(f"model marked all {len(shots)} screenshot(s) unusable; keeping them")
        kept_shots = list(shots)
    want = max(0, GALLERY_FRAME_TARGET - len(kept_shots))
    ranked = [fed[n - 1] for n in order if fed[n - 1].kind == "frame"]
    picked = ranked[:want]
    if want > 0 and not any(f.tail for f in picked):
        tail = next((f for f in ranked if f.tail), None)
        if tail:
            picked = picked[:want - 1] + [tail]
    return kept_shots, sorted(picked, key=lambda f: f.t)
