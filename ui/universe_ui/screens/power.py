import os
import re

from PySide6.QtCore import QObject, QTimer, Signal

from ..qt import Property

SYSFS = "/sys/class/power_supply"
FAKE = os.path.join(os.path.dirname(os.path.dirname(__file__)), "fixtures", "power_supply")
POLL_MS = 10000
LED_SETTLE_MS = 1000
LEVELS = {"full": 100, "high": 80, "normal": 50, "low": 20, "critical": 5}


def _read(path):
    try:
        with open(path) as f:
            return f.read().strip()
    except OSError:
        return ""


# The evdev nodes of each HID device: /sys/class/input/eventN/device/device is the device the battery hangs off.
def read_inputs(root):
    out = {}
    try:
        names = os.listdir(root)
    except OSError:
        return out
    for name in names:
        if name.startswith("event"):
            out.setdefault(os.path.realpath(os.path.join(root, name, "device", "device")), []).append(name)
    return out


# The number the kernel lit on a pad: hid-playstation and hid-nintendo light a pattern across the HID device's player-N LEDs.
DUALSENSE_LEDS = {(3,): 1, (2, 4): 2, (1, 3, 5): 3, (1, 2, 4, 5): 4, (1, 2, 3, 4, 5): 5}
NINTENDO_LEDS = {(1,): 1, (1, 2): 2, (1, 2, 3): 3, (1, 2, 3, 4): 4, (1, 4): 5, (1, 3): 6, (1, 3, 4): 7, (2, 3): 8}
LED_PATTERNS = {5: DUALSENSE_LEDS, 4: NINTENDO_LEDS}


def read_player(root, event):
    device = os.path.realpath(os.path.join(root, "input", event, "device", "device"))
    leds = os.path.join(root, "leds")
    try:
        names = os.listdir(leds)
    except OSError:
        return 0
    player_leds, lit, xpads = 0, [], []
    for name in names:
        owner = os.path.realpath(os.path.join(leds, name, "device"))
        index = name.rpartition(":player-")[2]
        if owner == device and ":player-" in name and index.isdigit():
            player_leds += 1
            if _read(os.path.join(leds, name, "brightness")) not in ("", "0"):
                lit.append(int(index))
        # xpad hangs its LED off the USB device, one level above the interface the event node belongs to.
        elif owner == os.path.dirname(device) and re.fullmatch(r"xpad\d+", name):
            xpads.append(name)
    if player_leds:
        return LED_PATTERNS.get(player_leds, {}).get(tuple(sorted(lit)), 0)
    if len(xpads) == 1:
        # The Xbox 360 LED command: 2-5 flash then light quadrant 1-4, 6-9 light it straight away.
        command = _read(os.path.join(leds, xpads[0], "brightness"))
        if command.isdigit() and 2 <= int(command) <= 9:
            return (int(command) - 2) % 4 + 1
    return 0


# Lights win: a pad the kernel numbered keeps it, the rest take the lowest numbers left, in connection order.
def number_pads(events, lit):
    players = {e: lit[e] for e in events if lit.get(e)}
    taken = set(players.values())
    for event in events:
        if event not in players:
            n = 1
            while n in taken:
                n += 1
            players[event] = n
            taken.add(n)
    return players


# kind: "system" (the laptop's battery) or "pad" (a controller's: the kernel scopes those to Device).
def read_sources(root):
    out = []
    try:
        names = sorted(os.listdir(root))
    except OSError:
        return out
    for name in names:
        d = os.path.join(root, name)
        if _read(os.path.join(d, "type")) != "Battery" or _read(os.path.join(d, "present")) == "0":
            continue
        capacity = _read(os.path.join(d, "capacity"))
        if capacity.isdigit():
            percent = min(100, int(capacity))
        else:
            percent = LEVELS.get(_read(os.path.join(d, "capacity_level")).lower())
            if percent is None:
                continue
        kind = "pad" if _read(os.path.join(d, "scope")) == "Device" else "system"
        out.append(
            {
                "name": name,
                "kind": kind,
                "percent": percent,
                "charging": _read(os.path.join(d, "status")) in ("Charging", "Full"),
                "inputs": _read(os.path.join(d, "inputs")).split()
                or (read_inputs(os.path.join(os.path.dirname(root), "input")).get(os.path.realpath(os.path.join(d, "..", "..")), []) if kind == "pad" else []),
            }
        )
    out.sort(key=lambda s: (s["kind"] != "system", s["name"]))
    return out


class Power(QObject):
    sourcesChanged = Signal()

    def __init__(self, root=SYSFS, parent=None):
        super().__init__(parent)
        self._root = root
        self._sources = []
        self._read = []
        self._pads = {}
        self._timer = QTimer(self)
        self._timer.setInterval(POLL_MS)
        self._timer.timeout.connect(self.refresh)
        self.refresh()
        self._timer.start()

    def refresh(self):
        self._read = read_sources(self._root)
        self._merge()

    # A pad the controller watcher sees, keyed by its event node, in connection order; its charge when the kernel keeps no supply for it.
    def pad(self, event, name, family, battery=None):
        if event not in self._pads:
            # A driver registers its player LEDs after the event node the watcher announces.
            QTimer.singleShot(LED_SETTLE_MS, self, self.refresh)
        self._pads[event] = {"name": name, "family": family, "battery": self._charge(battery)}
        self._merge()

    def report(self, event, battery):
        if event in self._pads:
            self._pads[event]["battery"] = self._charge(battery)
            self._merge()

    def forget(self, event):
        if self._pads.pop(event, None) is not None:
            self._merge()

    @staticmethod
    def _charge(battery):
        return None if battery is None else {"percent": int(battery["percent"]), "charging": bool(battery["charging"])}

    def _merge(self):
        listed = {event for s in self._read for event in s["inputs"]}
        sources = [dict(s) for s in self._read] + [
            {"name": p["name"], "kind": "pad", **p["battery"], "inputs": [event]}
            for event, p in self._pads.items()
            if p["battery"] is not None and event not in listed
        ]
        shown = sum(1 for s in sources if s["kind"] == "pad")
        root = os.path.dirname(self._root)
        players = number_pads(list(self._pads), {e: read_player(root, e) for e in self._pads}) if shown > 1 else {}
        for source in sources:
            owner = next((e for e in source["inputs"] if e in self._pads), "")
            source["family"] = self._pads[owner]["family"] if owner else ""
            source["player"] = players.get(owner, 0)
        if sources != self._sources:
            self._sources = sources
            self.sourcesChanged.emit()

    def forInput(self, event):
        return next((s for s in self._sources if event in s["inputs"]), None)

    sources = Property(list, lambda self: [dict(s) for s in self._sources], notify=sourcesChanged)
    count = Property(int, lambda self: len(self._sources), notify=sourcesChanged)
