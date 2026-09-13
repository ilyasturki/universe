#!/usr/bin/env python3
"""Draws qml/assets/controllers/<family>.svg and qml/ui/ControllerHotspots.js from one geometry,
so every hotspot sits on the button it names. Run it after changing a shape; the outputs are
committed."""

import math
import os
import sys

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "universe_ui", "qml")
W, H = 1000, 620
INK = "#f2f3f5"


class Sheet:
    def __init__(self):
        self.parts = []
        self.spots = []

    def spot(self, slot, x, y, w, h, r):
        self.spots.append((slot, x, y, w, h, r))

    def path(self, d, fill=0.05, opacity=0.85, width=2):
        self.parts.append(f'<path d="{d}" fill-opacity="{fill}" stroke-opacity="{opacity}" stroke-width="{width}"/>')

    def rect(self, x, y, w, h, rx, slot=None, fill=0.05, rotate=0, opacity=0.85):
        t = f' transform="rotate({rotate} {x + w / 2} {y + h / 2})"' if rotate else ""
        self.parts.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" fill-opacity="{fill}" stroke-opacity="{opacity}"{t}/>')
        if slot:
            self.spot(slot, x, y, w, h, rx)

    def circle(self, cx, cy, r, slot=None, fill=0.05, opacity=0.85, width=2):
        self.parts.append(f'<circle cx="{cx}" cy="{cy}" r="{r}" fill-opacity="{fill}" stroke-opacity="{opacity}" stroke-width="{width}"/>')
        if slot:
            self.spot(slot, cx - r, cy - r, 2 * r, 2 * r, r)

    def line(self, d, opacity=0.6, width=2.5):
        self.parts.append(f'<path d="{d}" fill="none" stroke-opacity="{opacity}" stroke-width="{width}"/>')

    def svg(self):
        head = (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}">\n'
                f'<g fill="{INK}" stroke="{INK}" stroke-linejoin="round" stroke-linecap="round">\n')
        return head + "\n".join(self.parts) + "\n</g>\n</svg>\n"


# ---- shared pieces -------------------------------------------------------------------------

def stick(s, cx, cy, slot, r=52):
    s.circle(cx, cy, r, slot)
    s.circle(cx, cy, int(r * 0.68), fill=0.08, opacity=0.7)
    s.circle(cx, cy, 6, fill=0.25, opacity=0.4, width=1)


def face(s, cx, cy, d, r, sony):
    spots = {"north": (cx, cy - d), "south": (cx, cy + d), "west": (cx - d, cy), "east": (cx + d, cy)}
    for slot, (x, y) in spots.items():
        s.circle(x, y, r, slot)
    if not sony:
        return
    g = 10
    x, y = spots["north"]
    s.line(f"M {x} {y - g} L {x + g} {y + g * 0.8} L {x - g} {y + g * 0.8} Z")
    x, y = spots["south"]
    s.line(f"M {x - g} {y - g} L {x + g} {y + g} M {x + g} {y - g} L {x - g} {y + g}")
    x, y = spots["west"]
    s.line(f"M {x - g} {y - g} h {2 * g} v {2 * g} h {-2 * g} Z")
    x, y = spots["east"]
    s.line(f"M {x + g} {y} a {g} {g} 0 1 0 {-2 * g} 0 a {g} {g} 0 1 0 {2 * g} 0")


def dpad_sony(s, cx, cy):
    s.circle(cx, cy, 64, fill=0.03, opacity=0.35)
    s.rect(cx - 18, cy - 64, 36, 42, 8, "dpad_up")
    s.rect(cx - 18, cy + 22, 36, 42, 8, "dpad_down")
    s.rect(cx - 64, cy - 18, 42, 36, 8, "dpad_left")
    s.rect(cx + 22, cy - 18, 42, 36, 8, "dpad_right")


def dpad_cross(s, cx, cy, dish):
    if dish:
        s.circle(cx, cy, 62, fill=0.03, opacity=0.4)
    a, l = 12, 48
    s.path(f"M {cx - a} {cy - l} h {2 * a} v {l - a} h {l - a} v {2 * a} h {-(l - a)} v {l - a} h {-2 * a} v {-(l - a)} h {-(l - a)} v {-2 * a} h {l - a} Z")
    s.spot("dpad_up", cx - a, cy - l, 2 * a, l - a, 4)
    s.spot("dpad_down", cx - a, cy + a, 2 * a, l - a, 4)
    s.spot("dpad_left", cx - l, cy - a, l - a, 2 * a, 4)
    s.spot("dpad_right", cx + a, cy - a, l - a, 2 * a, 4)


def shoulders(s, lx, rx, w=130, tw=84, y=86, ty=46):
    s.rect(lx + (w - tw) // 2, ty, tw, 42, 14, "lt", fill=0.04)
    s.rect(rx + (w - tw) // 2, ty, tw, 42, 14, "rt", fill=0.04)
    s.rect(lx, y, w, 32, 12, "lb")
    s.rect(rx, y, w, 32, 12, "rb")


def pill(s, cx, cy, slot, angle):
    s.rect(cx - 7, cy - 20, 14, 40, 7, slot, rotate=angle)


def tab(s, x, y, slot, w=50, h=28):
    s.rect(x, y, w, h, 10, slot, fill=0.1)


# ---- bodies -------------------------------------------------------------------------------

SONY_BODY = ("M 258 148 C 275 126, 305 116, 345 114 L 655 114 C 695 116, 725 126, 742 148 "
             "C 812 165, 848 225, 860 300 C 876 400, 874 522, 840 554 C 806 578, 758 572, 733 545 "
             "C 702 508, 686 462, 648 448 C 598 432, 548 496, 500 498 C 452 496, 402 432, 352 448 "
             "C 314 462, 298 508, 267 545 C 242 572, 194 578, 160 554 C 126 522, 124 400, 140 300 "
             "C 152 225, 188 165, 258 148 Z")

DS4_BODY = ("M 262 150 C 282 128, 312 118, 350 116 L 650 116 C 688 118, 718 128, 738 150 "
            "C 790 176, 826 250, 846 340 C 866 430, 860 520, 828 550 C 798 576, 752 570, 728 542 "
            "C 698 506, 684 462, 646 450 C 598 436, 548 486, 500 488 C 452 486, 402 436, 354 450 "
            "C 316 462, 302 506, 272 542 C 248 570, 202 576, 172 550 C 140 520, 134 430, 154 340 "
            "C 174 250, 210 176, 262 150 Z")

XBOX_BODY = ("M 300 128 C 360 108, 640 108, 700 128 C 762 140, 802 172, 832 232 C 872 322, 892 442, 862 522 "
             "C 842 572, 782 588, 746 556 C 716 526, 700 480, 660 462 C 600 438, 560 456, 500 456 "
             "C 440 456, 400 438, 340 462 C 300 480, 284 526, 254 556 C 218 588, 158 572, 138 522 "
             "C 108 442, 128 322, 168 232 C 198 172, 238 140, 300 128 Z")

SWITCH_BODY = ("M 292 118 C 350 102, 650 102, 708 118 C 770 132, 810 166, 830 222 C 866 312, 880 442, 856 526 "
               "C 838 574, 786 590, 748 558 C 718 528, 704 482, 664 464 C 608 440, 560 452, 500 452 "
               "C 440 452, 392 440, 336 464 C 296 482, 282 528, 252 558 C 214 590, 162 574, 144 526 "
               "C 120 442, 134 312, 170 222 C 190 166, 230 132, 292 118 Z")


def sony(kind):
    s = Sheet()
    shoulders(s, 172, 698)
    s.path(DS4_BODY if kind in ("dualshock4", "8bitdo-pro-3") else SONY_BODY)
    if kind == "dualshock4":
        s.rect(432, 120, 136, 10, 5, fill=0.3, opacity=0.6)
        s.rect(400, 140, 200, 106, 18, fill=0.04)
        pill(s, 352, 178, "select", -16)
        pill(s, 648, 178, "start", 16)
    elif kind == "8bitdo-pro-3":
        s.rect(448, 144, 104, 26, 13, fill=0.06, opacity=0.5)
        pill(s, 352, 178, "select", -16)
        pill(s, 648, 178, "start", 16)
    else:
        s.rect(386, 138, 228, 124, 30, fill=0.04)
        s.line("M 386 214 C 430 236, 570 236, 614 214", opacity=0.25, width=1.5)
        pill(s, 352, 176, "select", -18)
        pill(s, 648, 176, "start", 18)
    dpad_sony(s, 228, 262)
    face(s, 772, 262, 58, 24, sony=kind != "8bitdo-pro-3")
    stick(s, 372, 372, "ls")
    stick(s, 628, 372, "rs")
    if kind == "8bitdo-pro-3":
        s.circle(500, 322, 19, "guide")
        r, pts = 13, []
        for i in range(10):
            rad = r if i % 2 == 0 else r * 0.45
            ang = -math.pi / 2 + i * math.pi / 5
            pts.append(f"{500 + rad * math.cos(ang):.1f} {378 + rad * math.sin(ang):.1f}")
        s.path("M " + " L ".join(pts) + " Z", fill=0.1)
        s.spot("star", 484, 362, 32, 32, 16)
        tab(s, 138, 54, "paddle_l4", 40, 24)
        tab(s, 822, 54, "paddle_r4", 40, 24)
        tab(s, 300, 546, "paddle_pl")
        tab(s, 650, 546, "paddle_pr")
    else:
        s.circle(500, 330, 20, "guide")
        if kind != "dualshock4":
            s.rect(482, 374, 36, 12, 6, "mute" if kind == "dualsense" else None, fill=0.08)
    if kind == "dualsense-edge":
        s.rect(402, 436, 36, 16, 8, "fn_left", fill=0.08)
        s.rect(562, 436, 36, 16, 8, "fn_right", fill=0.08)
        tab(s, 300, 546, "paddle_left")
        tab(s, 650, 546, "paddle_right")
    return s


def xbox(kind):
    s = Sheet()
    shoulders(s, 170, 680, w=150, tw=90)
    s.path(XBOX_BODY)
    stick(s, 256, 240, "ls")
    stick(s, 626, 372, "rs")
    dpad_cross(s, 374, 372, dish=True)
    face(s, 744, 240, 56, 24, sony=False)
    s.circle(500, 160, 26, "guide")
    s.circle(500, 160, 14, fill=0.12, opacity=0.5, width=1.5)
    s.circle(416, 218, 14, "select")
    s.circle(584, 218, 14, "start")
    if kind == "xbox":
        s.rect(484, 246, 32, 22, 6, "share", fill=0.08)
    if kind == "xbox-elite":
        tab(s, 168, 572, "paddle_p3", 50, 26)
        tab(s, 224, 588, "paddle_p4", 50, 26)
        tab(s, 782, 572, "paddle_p1", 50, 26)
        tab(s, 726, 588, "paddle_p2", 50, 26)
    return s


def switch_pro():
    s = Sheet()
    shoulders(s, 168, 680, w=152, tw=92, y=84, ty=44)
    s.path(SWITCH_BODY)
    stick(s, 256, 232, "ls", r=50)
    stick(s, 616, 372, "rs", r=50)
    dpad_cross(s, 384, 372, dish=False)
    face(s, 744, 232, 54, 23, sony=False)
    s.line("M 398 186 h 24", opacity=0.85, width=3)
    s.spot("select", 394, 172, 32, 28, 6)
    s.line("M 578 186 h 24 M 590 174 v 24", opacity=0.85, width=3)
    s.spot("start", 574, 172, 32, 28, 6)
    s.circle(560, 272, 17, "guide")
    s.rect(426, 258, 28, 28, 6, "capture", fill=0.08)
    s.circle(440, 272, 5, fill=0.3, opacity=0.5, width=1)
    return s


SHEETS = {
    "dualsense-edge": lambda: sony("dualsense-edge"),
    "dualsense": lambda: sony("dualsense"),
    "dualshock4": lambda: sony("dualshock4"),
    "8bitdo-pro-3": lambda: sony("8bitdo-pro-3"),
    "xbox": lambda: xbox("xbox"),
    "xbox-elite": lambda: xbox("xbox-elite"),
    "generic": lambda: xbox("generic"),
    "switch-pro": switch_pro,
}


def main():
    out = os.path.normpath(sys.argv[1] if len(sys.argv) > 1 else OUT)
    assets = os.path.join(out, "assets", "controllers")
    os.makedirs(assets, exist_ok=True)
    table = []
    for family, build in SHEETS.items():
        sheet = build()
        with open(os.path.join(assets, f"{family}.svg"), "w") as f:
            f.write(sheet.svg())
        spots = ", ".join(
            f'{{ slot: "{slot}", x: {x / W:.4f}, y: {y / H:.4f}, w: {w / W:.4f}, h: {h / H:.4f}, r: {r / W:.4f} }}'
            for slot, x, y, w, h, r in sheet.spots)
        table.append(f'    "{family}": [\n        {spots}\n    ]')
    js = (".pragma library\n\n"
          "// Generated by tools/controller-art.py with the SVGs: a slot's rect in the art, as fractions of the\n"
          "// image (r is the corner radius as a fraction of the width). Edit the generator, not this file.\n"
          "var TABLE = {\n" + ",\n".join(table) + "\n};\n\n"
          "function has(family) { return TABLE[family] !== undefined; }\n\n"
          "function slots(family) { return TABLE[family] || TABLE[\"generic\"]; }\n")
    with open(os.path.join(out, "ui", "ControllerHotspots.js"), "w") as f:
        f.write(js)
    print(f"{len(SHEETS)} controllers → {assets}")


if __name__ == "__main__":
    main()
