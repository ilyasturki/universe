from PySide6.QtCore import Signal, Slot

from ..qt import QVARIANT, Property
from .stream import Streamed

# What each kind of question asks the user, `{name}` and `{code}` filled in.
ASKS = {
    "confirm": ("Pair {name}?", "Check that {name} shows {code}."),
    "authorize": ("Pair {name}?", "It asks to connect to this machine."),
    "passkey": ("Pair {name}", "Type the code {name} shows."),
    "pin": ("Pair {name}", "Type its PIN: often 0000 or 1234."),
    "display": ("Pair {name}", "Type {code} on {name}, then press Enter."),
}
CABLE = "It is plugged in by cable: after this it connects without one."


def device_row(d):
    return {
        "address": str(d.get("address") or ""),
        "name": str(d.get("name") or d.get("address") or ""),
        "kind": str(d.get("kind") or "other"),
        "icon": str(d.get("icon") or ""),
        "paired": bool(d.get("paired")),
        "connected": bool(d.get("connected")),
        "battery": d.get("battery"),
    }


class BluetoothScreen(Streamed):
    """Bluetooth through `universe bluetooth watch`, which is also the pairing agent: the devices, pairing, forgetting, and the
    agent's questions as `request`. `busy()` says a game holds the screen: a question then gets a no and a notice instead."""

    changed = Signal()
    requestChanged = Signal()
    # (action, address) once an action took; (action, address, reason, message) when one failed.
    done = Signal(str, str)
    failed = Signal(str, str, str, str)

    def __init__(self, client, busy=lambda: False, parent=None):
        super().__init__(client, "bluetooth", parent)
        self._busy = busy
        self._state = {}
        self._pairing = ""
        self._request = None
        client.bluetoothAsync(self._apply)

    def _apply(self, state):
        state = {k: v for k, v in (state or {}).items() if k != "event"}
        if state != self._state:
            self._state = state
            self.changed.emit()

    def _set_request(self, request):
        if request != self._request:
            self._request = request
            self.requestChanged.emit()

    def _ask(self, line):
        kind, name = str(line.get("kind") or ""), str(line.get("name") or line.get("address") or "")
        if self._busy():
            if kind == "display":
                self.send({"cmd": "cancel", "address": line.get("address")})
            else:
                self.send({"cmd": "answer", "id": line.get("id"), "yes": False})
            self._client.notice.emit(f"Plug {name} in again from HOME" if kind == "authorize" else f"Pair {name} again from HOME")
            return
        title, detail = ASKS.get(kind, ASKS["authorize"])
        cable = kind == "authorize" and line.get("device_kind") == "pad" and str(line.get("address") or "") != self._pairing
        self._set_request(
            {
                "id": line.get("id"),
                "kind": kind,
                "address": str(line.get("address") or ""),
                "name": name,
                "deviceKind": str(line.get("device_kind") or ""),
                "code": str(line.get("code") or ""),
                "entered": int(line.get("entered") or 0),
                "title": title.format(name=name),
                "detail": CABLE if cable else detail.format(name=name, code=line.get("code") or ""),
            }
        )

    def on_line(self, line):
        event, action, address = line.get("event"), str(line.get("action") or ""), str(line.get("address") or "")
        if event == "state":
            self._apply(line)
        elif event == "request":
            self._ask(line)
        elif event == "cancel":
            self._set_request(None)
        elif event in ("done", "failed"):
            if action == "pair":
                self._pairing = ""
                self._set_request(None)
                self.changed.emit()
            if event == "done":
                self.done.emit(action, address)
            # The watch searches again once the adapter can; a cancel that fails found no pairing left to stop.
            elif action not in ("scan", "cancel"):
                self.failed.emit(action, address, str(line.get("reason") or "failed"), str(line.get("message") or ""))
        elif event == "off":
            self._set_request(None)
            if self._pairing:
                self._pairing = ""
                self.changed.emit()

    @Slot(str, result=bool)
    def pair(self, address):
        if not self.send({"cmd": "pair", "address": address}):
            return False
        self._pairing = address
        self.changed.emit()
        return True

    @Slot(str, result=bool)
    def connectDevice(self, address):
        return self.send({"cmd": "connect", "address": address})

    @Slot(str, result=bool)
    def disconnectDevice(self, address):
        return self.send({"cmd": "disconnect", "address": address})

    @Slot(str, result=bool)
    def forget(self, address):
        return self.send({"cmd": "remove", "address": address})

    @Slot(bool, result=bool)
    def setPowered(self, on):
        return self.send({"cmd": "power", "on": bool(on)})

    # The answer to `request`: yes or no, or the code or PIN typed.
    @Slot(bool)
    def answer(self, yes):
        request = self._request
        if request is None:
            return
        self._set_request(None)
        if request["kind"] == "display":
            if not yes:
                self.send({"cmd": "cancel", "address": request["address"]})
            return
        self.send({"cmd": "answer", "id": request["id"], "yes": bool(yes)})

    @Slot(str)
    def answerText(self, value):
        request = self._request
        if request is None:
            return
        self._set_request(None)
        self.send({"cmd": "answer", "id": request["id"], "value": str(value)} if value else {"cmd": "answer", "id": request["id"], "yes": False})

    def _devices(self, paired):
        return [device_row(d) for d in self._state.get("devices") or [] if bool(d.get("paired")) == paired]

    available = Property(bool, lambda self: bool(self._state.get("available")), notify=changed)
    powered = Property(bool, lambda self: bool(self._state.get("powered")), notify=changed)
    discovering = Property(bool, lambda self: bool(self._state.get("discovering")), notify=changed)
    # {address, name, kind (pad, audio, keyboard, mouse, phone, other), icon, paired, connected, battery}, by name.
    paired = Property("QVariantList", lambda self: self._devices(True), notify=changed)
    found = Property("QVariantList", lambda self: self._devices(False), notify=changed)
    # The address a pairing is under way to, "" when none.
    pairing = Property(str, lambda self: self._pairing, notify=changed)
    # The agent's question waiting on the user, else null: {id, kind (confirm, authorize, passkey, pin, display), address, name,
    # deviceKind, code, entered, title, detail}.
    request = Property(QVARIANT, lambda self: self._request, notify=requestChanged)
