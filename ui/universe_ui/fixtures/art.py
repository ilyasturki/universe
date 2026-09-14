"""Placeholder artwork for the fixture library, painted once per run with QPainter."""

import hashlib
import os

from PySide6.QtCore import QRectF, Qt
from PySide6.QtGui import QColor, QFont, QFontMetricsF, QImage, QLinearGradient, QPainter, QPen

SLOTS = {
    "box_front": (600, 900),
    "square": (600, 600),
    "banner": (920, 430),
    "background": (1920, 1080),
    "logo": (960, 300),
    "screenshot": (1280, 720),
}


def _palette(ident):
    h = hashlib.sha1(ident.encode()).digest()
    hue = h[0] * 360 // 255
    return QColor.fromHsl(hue, 120, 60), QColor.fromHsl((hue + 40) % 360, 140, 28)


def _paint(path, size, ident, title, kind):
    if os.path.exists(path):
        return
    w, h = size
    image = QImage(w, h, QImage.Format.Format_ARGB32_Premultiplied)
    image.fill(Qt.GlobalColor.transparent if kind == "logo" else Qt.GlobalColor.black)
    light, dark = _palette(ident)
    p = QPainter(image)
    p.setRenderHint(QPainter.RenderHint.Antialiasing)
    p.setRenderHint(QPainter.RenderHint.TextAntialiasing)
    if kind != "logo":
        grad = QLinearGradient(0, 0, w, h)
        grad.setColorAt(0.0, light)
        grad.setColorAt(1.0, dark)
        p.fillRect(0, 0, w, h, grad)
        p.setPen(QPen(QColor(255, 255, 255, 40), max(2, w // 120)))
        step = max(40, w // 12)
        for x in range(-h, w, step):
            p.drawLine(x, h, x + h, 0)
    if kind in ("box_front", "square", "banner", "logo"):
        font = QFont("Archivo", max(18, w // (14 if kind in ("logo", "banner") else 9)))
        font.setBold(True)
        margin = w // 12
        rect = QRectF(margin, margin, w - margin * 2, h - margin * 2)
        longest = max(title.split(), key=len, default="")
        while font.pointSize() > 12 and QFontMetricsF(font).horizontalAdvance(longest) > rect.width():
            font.setPointSize(font.pointSize() - 1)
        p.setFont(font)
        p.setPen(QColor(255, 255, 255, 235))
        align = Qt.AlignmentFlag.AlignLeft | Qt.AlignmentFlag.AlignBottom if kind == "logo" else Qt.AlignmentFlag.AlignCenter
        p.drawText(rect, int(align | Qt.TextFlag.TextWordWrap), title)
    if kind == "screenshot":
        p.setBrush(QColor(0, 0, 0, 90))
        p.setPen(Qt.PenStyle.NoPen)
        p.drawRoundedRect(QRectF(w * 0.05, h * 0.78, w * 0.9, h * 0.16), 12, 12)
        font = QFont("Archivo", h // 20)
        p.setFont(font)
        p.setPen(QColor(255, 255, 255, 220))
        p.drawText(QRectF(w * 0.07, h * 0.78, w * 0.86, h * 0.16), int(Qt.AlignmentFlag.AlignVCenter), title)
    p.end()
    image.save(path)


CANDIDATES = 6


def paint_candidates(art_dir, ident, slot, title):
    """Six painted takes on a slot, as SteamGridDB would offer: `{provider, id, url, thumb, score, slot}`."""
    size = SLOTS.get(slot) or SLOTS["box_front"]
    items = []
    for n in range(CANDIDATES):
        path = os.path.join(art_dir, f"{ident}-{slot}-candidate{n}.png")
        _paint(path, size, f"{ident}/{slot}/{n}", f"{title}\n№ {n + 1}", slot)
        items.append({"provider": "sgdb", "id": 100 + n, "url": path, "thumb": path, "score": (CANDIDATES - n) * 1000, "slot": slot})
    return items


def paint_candidate(art_dir, ident, slot, url):
    size = SLOTS.get(slot) or SLOTS["box_front"]
    path = os.path.join(art_dir, f"{ident}-{slot}-{hashlib.sha1(url.encode()).hexdigest()[:8]}.png")
    _paint(path, size, url, url.rsplit("/", 1)[-1], slot)
    return path


def paint_library(games, art_dir):
    """Fill each game's `media` with painted files; `hidden` games get none, like an unscraped title."""
    os.makedirs(art_dir, exist_ok=True)
    for game in games:
        ident, title = game["id"], game.get("title", game["id"])
        media = game.setdefault("media", {})
        if game.get("hidden"):
            continue
        for slot, size in SLOTS.items():
            if slot == "screenshot":
                shots = []
                for n in (1, 2):
                    path = os.path.join(art_dir, f"{ident}-screenshot{n}.png")
                    _paint(path, size, f"{ident}{n}", f"{title} — {n}", "screenshot")
                    shots.append(path)
                media["screenshots"] = shots
                continue
            path = os.path.join(art_dir, f"{ident}-{slot}.png")
            _paint(path, size, ident, title, slot)
            media[slot] = path
