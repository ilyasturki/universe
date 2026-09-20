import os

from PySide6.QtCore import QObject, QTimer, Signal

from ..qt import Property

SYSFS = "/sys/class/power_supply"
FAKE = os.path.join(os.path.dirname(os.path.dirname(__file__)), "fixtures", "power_supply")
POLL_MS = 10000
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
        self._timer = QTimer(self)
        self._timer.setInterval(POLL_MS)
        self._timer.timeout.connect(self.refresh)
        self.refresh()
        self._timer.start()

    def refresh(self):
        sources = read_sources(self._root)
        if sources != self._sources:
            self._sources = sources
            self.sourcesChanged.emit()

    def forInput(self, event):
        return next((s for s in self._sources if event in s["inputs"]), None)

    sources = Property(list, lambda self: [dict(s) for s in self._sources], notify=sourcesChanged)
    count = Property(int, lambda self: len(self._sources), notify=sourcesChanged)
