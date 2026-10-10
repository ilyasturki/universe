"""The recorded pads under tests/pads, and the evdev codes their entries name."""

import json
from pathlib import Path

CORPUS = Path(__file__).resolve().parents[2] / "tests" / "pads"

EV_SYN, EV_KEY, EV_ABS, EV_FF = 0x00, 0x01, 0x03, 0x15
FF_RUMBLE = 0x50

KEYS = {
    "KEY_F12": 88,
    "KEY_RECORD": 167,
    **{f"BTN_{n}": 0x100 + n for n in range(10)},
    "BTN_TRIGGER": 0x120,
    "BTN_THUMB": 0x121,
    "BTN_THUMB2": 0x122,
    "BTN_TOP": 0x123,
    "BTN_TOP2": 0x124,
    "BTN_PINKIE": 0x125,
    "BTN_BASE": 0x126,
    "BTN_BASE2": 0x127,
    "BTN_SOUTH": 0x130,
    "BTN_EAST": 0x131,
    "BTN_C": 0x132,
    "BTN_NORTH": 0x133,
    "BTN_WEST": 0x134,
    "BTN_Z": 0x135,
    "BTN_TL": 0x136,
    "BTN_TR": 0x137,
    "BTN_TL2": 0x138,
    "BTN_TR2": 0x139,
    "BTN_SELECT": 0x13A,
    "BTN_START": 0x13B,
    "BTN_MODE": 0x13C,
    "BTN_THUMBL": 0x13D,
    "BTN_THUMBR": 0x13E,
    "BTN_DPAD_UP": 0x220,
    "BTN_DPAD_DOWN": 0x221,
    "BTN_DPAD_LEFT": 0x222,
    "BTN_DPAD_RIGHT": 0x223,
    "BTN_GRIPL": 0x224,
    "BTN_GRIPR": 0x225,
    "BTN_GRIPL2": 0x226,
    "BTN_GRIPR2": 0x227,
    **{f"BTN_TRIGGER_HAPPY{n}": 0x2C0 + n - 1 for n in range(1, 41)},
}
AXES = {
    "ABS_X": 0x00,
    "ABS_Y": 0x01,
    "ABS_Z": 0x02,
    "ABS_RX": 0x03,
    "ABS_RY": 0x04,
    "ABS_RZ": 0x05,
    "ABS_THROTTLE": 0x06,
    "ABS_RUDDER": 0x07,
    "ABS_WHEEL": 0x08,
    "ABS_GAS": 0x09,
    "ABS_BRAKE": 0x0A,
    **{f"ABS_HAT{n}{xy}": 0x10 + 2 * n + i for n in range(4) for i, xy in enumerate("XY")},
}
STICKS = ("lx", "ly", "rx", "ry")
TRIGGERS = ("lt", "rt")


def key(name):
    return KEYS[name] if name in KEYS else int(name)


def source(text):
    """`BTN_SOUTH` → ("key", code); `ABS_HAT0Y-` → ("abs", code, False); an axis without a sign is positive."""
    if text.startswith("ABS_"):
        name, sign = (text[:-1], text[-1]) if text[-1] in "+-" else (text, "+")
        return ("abs", AXES[name], sign == "+")
    return ("key", key(text))


class Pad:
    """One file of tests/pads: a pad on one bus, under one driver."""

    def __init__(self, name, entry):
        self.name = name
        self.entry = entry
        self.evdev = entry["evdev"]
        self.buttons = entry.get("buttons", {})
        self.axes = entry.get("axes", {})
        self.family = entry["family"]
        self.bus = entry["bus"]
        self.driver = entry.get("driver", "")
        self.vendor = int(self.evdev["vendor"], 16)
        self.product = int(self.evdev["product"], 16)
        self.version = int(self.evdev.get("version", "0"), 16)
        self.hid = entry.get("hid")
        self.usb = entry.get("usb")
        self.sdl = entry.get("sdl")

    def key_codes(self):
        return [key(k) for k in self.evdev["keys"]]

    def ranges(self):
        """evdev axis code → [min, max, fuzz, flat]."""
        return {AXES[name]: r for name, r in self.evdev["abs"].items()}

    def axis(self, role):
        """(code, runs backwards) of a stick's or a trigger's axis, None when the pad has no axis for it."""
        name = self.axes.get(role)
        if not name:
            return None
        return AXES[name.rstrip("-")], name.endswith("-")

    def __repr__(self):
        return f"Pad({self.name})"


def load(name):
    return Pad(name, json.loads((CORPUS / f"{name}.json").read_text()))


def names():
    return sorted(p.stem for p in CORPUS.glob("*.json"))
