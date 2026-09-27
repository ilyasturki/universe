import contextlib
import os

from PySide6.QtCore import QObject, QTimer, Signal

from ..qt import Property

SYSFS = "/sys/class/net"
WIRELESS = "/proc/net/wireless"
FAKE = os.path.join(os.path.dirname(os.path.dirname(__file__)), "fixtures", "net")
POLL_MS = 10000
# /proc/net/wireless link quality is out of 70 on every driver that fills it.
QUALITY_MAX = 70


def _read(path):
    try:
        with open(path) as f:
            return f.read().strip()
    except OSError:
        return ""


def read_quality(path):
    out = {}
    for line in _read(path).splitlines()[2:]:
        name, _, rest = line.partition(":")
        fields = rest.split()
        if len(fields) > 1:
            with contextlib.suppress(ValueError):
                out[name.strip()] = float(fields[1].rstrip("."))
    return out


def bars(quality):
    share = quality / QUALITY_MAX
    return 3 if share >= 0.6 else 2 if share >= 0.35 else 1


# The link the machine is online through: a cable wins over Wi-Fi; virtual links (lo, VPNs, bridges) have no device.
def read_link(root, wireless):
    wired = wifi = None
    try:
        names = sorted(os.listdir(root))
    except OSError:
        return {"kind": "", "bars": 0}
    for name in names:
        d = os.path.join(root, name)
        if not os.path.exists(os.path.join(d, "device")) or _read(os.path.join(d, "operstate")) != "up":
            continue
        if "DEVTYPE=wlan" in _read(os.path.join(d, "uevent")).split():
            wifi = wifi or name
        else:
            wired = wired or name
    if wired:
        return {"kind": "wired", "bars": 0}
    if wifi:
        return {"kind": "wifi", "bars": bars(read_quality(wireless).get(wifi, QUALITY_MAX))}
    return {"kind": "", "bars": 0}


class Network(QObject):
    changed = Signal()

    def __init__(self, root=SYSFS, wireless=WIRELESS, parent=None):
        super().__init__(parent)
        self._root = root
        self._wireless = wireless
        self._link = {"kind": "", "bars": 0}
        self._timer = QTimer(self)
        self._timer.setInterval(POLL_MS)
        self._timer.timeout.connect(self.refresh)
        self.refresh()
        self._timer.start()

    def refresh(self):
        link = read_link(self._root, self._wireless)
        if link != self._link:
            self._link = link
            self.changed.emit()

    # "wifi" | "wired" | "" offline
    kind = Property(str, lambda self: self._link["kind"], notify=changed)
    # Wi-Fi signal, 1-3 arcs; 0 on a cable or offline
    bars = Property(int, lambda self: self._link["bars"], notify=changed)
