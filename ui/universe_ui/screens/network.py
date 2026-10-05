import contextlib
import os

from PySide6.QtCore import QObject, QTimer, Signal, Slot

from ..qt import Property
from .stream import Streamed

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


# NetworkManager's signal is a percent.
def strength_bars(strength):
    return bars(strength * QUALITY_MAX / 100)


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
        self._set(read_link(self._root, self._wireless))

    def _set(self, link):
        if link != self._link:
            self._link = link
            self.changed.emit()

    # The network watch's link while it runs and NM answers: the sysfs poll stops for it, and comes back with `release`.
    def follow(self, kind, strength):
        self._timer.stop()
        self._set({"kind": kind, "bars": strength_bars(strength) if kind == "wifi" else 0})

    def release(self):
        if not self._timer.isActive():
            self.refresh()
            self._timer.start()

    # "wifi" | "wired" | "" offline
    kind = Property(str, lambda self: self._link["kind"], notify=changed)
    # Wi-Fi signal, 1-3 arcs; 0 on a cable or offline
    bars = Property(int, lambda self: self._link["bars"], notify=changed)


SECURED = ("psk", "sae", "wep", "enterprise")
JOINABLE = ("open", "owe", "psk", "sae")


def network_row(n):
    security = str(n.get("security") or "")
    strength = int(n.get("strength") or 0)
    saved = bool(n.get("saved"))
    return {
        "ssid": str(n.get("ssid") or ""),
        "strength": strength,
        "bars": strength_bars(strength),
        "security": security,
        "secured": security in SECURED,
        # A WEP or enterprise network a desktop saved joins as saved.
        "joinable": security in JOINABLE or saved,
        "saved": saved,
        "active": bool(n.get("active")),
    }


class WifiScreen(Streamed):
    """Wi-Fi through `universe network watch`: the networks in range, joining one, forgetting one. The link it reads drives `status`."""

    changed = Signal()
    # (ssid, connectivity) once a join is up; (ssid, reason, message) when a join or a forget fails, reason as the core gives it.
    joined = Signal(str, str)
    failed = Signal(str, str, str)
    forgot = Signal(str)
    # A connection test's answer: "full", "limited", "portal", "none" or "unknown".
    checked = Signal(str)

    def __init__(self, client, status, parent=None):
        super().__init__(client, "network", parent)
        self._status = status
        self._state = {}
        self._connecting = ""
        self._error = {}
        self._checking = False
        client.networkAsync(self._apply)

    def _apply(self, state):
        state = {k: v for k, v in (state or {}).items() if k != "event"}
        if state != self._state:
            self._state = state
            self.changed.emit()

    def on_line(self, line):
        event, action, ssid = line.get("event"), line.get("action"), str(line.get("ssid") or "")
        reason, message = str(line.get("reason") or "failed"), str(line.get("message") or "")
        if event == "state":
            if line.get("available"):
                self._status.follow(str(line.get("link") or ""), int(line.get("strength") or 0))
            else:
                self._status.release()
            self._apply(line)
        elif event == "connecting":
            self._connecting, self._error = ssid, {}
            self.changed.emit()
        elif event == "done" and action == "connect":
            self._connecting = ""
            self.changed.emit()
            self.joined.emit(ssid, str(line.get("connectivity") or ""))
        elif event == "done" and action == "check":
            self._checking = False
            self._state = {**self._state, "connectivity": str(line.get("connectivity") or "unknown")}
            self.changed.emit()
            self.checked.emit(str(line.get("connectivity") or "unknown"))
        elif event == "failed" and action == "check":
            self._checking = False
            self.changed.emit()
            self.checked.emit("unknown")
        elif event == "done" and action == "forget":
            self.forgot.emit(ssid)
        elif event == "failed" and action in ("connect", "forget"):
            if action == "connect":
                self._connecting = ""
                self._error = {"ssid": ssid, "reason": reason, "message": message}
                self.changed.emit()
            self.failed.emit(ssid, reason, message)
        elif event == "off":
            self._status.release()
            if self._connecting:
                self._connecting = ""
                self.changed.emit()

    # Whether joining `ssid` takes a password typed first: a secured network not saved yet.
    @Slot(str, result=bool)
    def needsPassword(self, ssid):
        n = next((network_row(n) for n in self._get("networks", []) if n.get("ssid") == ssid), None)
        return n is not None and n["secured"] and not n["saved"]

    @Slot(str, str, result=bool)
    def join(self, ssid, password=""):
        command = {"cmd": "connect", "ssid": ssid}
        if password:
            command["password"] = password
        if not self.send(command):
            return False
        self._connecting, self._error = ssid, {}
        self.changed.emit()
        return True

    @Slot(str, result=bool)
    def forget(self, ssid):
        return self.send({"cmd": "forget", "ssid": ssid})

    # NetworkManager's connection test, run now: `checked` answers.
    @Slot(result=bool)
    def check(self):
        if self._checking or not self.send({"cmd": "check"}):
            return False
        self._checking = True
        self.changed.emit()
        return True

    @Slot(bool, result=bool)
    def setEnabled(self, on):
        return self.send({"cmd": "wifi", "on": bool(on)})

    @Slot()
    def clearError(self):
        if self._error:
            self._error = {}
            self.changed.emit()

    def _get(self, key, default):
        return self._state.get(key, default)

    # NetworkManager answers and lists a Wi-Fi card: the Network pages show.
    available = Property(bool, lambda self: bool(self._get("available", False)) and bool(self._get("device", "")), notify=changed)
    enabled = Property(bool, lambda self: bool(self._get("enabled", False)), notify=changed)
    # "wifi" | "wired" | "" offline
    link = Property(str, lambda self: str(self._get("link", "")), notify=changed)
    ssid = Property(str, lambda self: str(self._get("ssid", "")), notify=changed)
    # "full" | "limited" | "portal" | "none" | "unknown", as NetworkManager last checked
    connectivity = Property(str, lambda self: str(self._get("connectivity", "")), notify=changed)
    # {ssid, strength, bars, security, secured, joinable, saved, active}: the joined one first, then by signal.
    networks = Property("QVariantList", lambda self: [network_row(n) for n in self._get("networks", [])], notify=changed)
    # The network a join is under way to, "" when none.
    connecting = Property(str, lambda self: self._connecting, notify=changed)
    checking = Property(bool, lambda self: self._checking, notify=changed)
    # The last join's failure, {ssid, reason, message}, empty once another starts.
    error = Property("QVariantMap", lambda self: dict(self._error), notify=changed)
