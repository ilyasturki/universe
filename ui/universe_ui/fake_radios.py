import copy
import os

# Every fixture network takes this password.
PASSWORD = "hunter22"
NETWORKS = [
    {"ssid": "Home", "strength": 78, "security": "psk", "saved": True, "active": True},
    {"ssid": "Atelier", "strength": 64, "security": "sae", "saved": False, "active": False},
    {"ssid": "Café Lumière", "strength": 52, "security": "open", "saved": False, "active": False},
    {"ssid": "Campus", "strength": 41, "security": "enterprise", "saved": False, "active": False},
    {"ssid": "Neighbours", "strength": 23, "security": "psk", "saved": False, "active": False},
]
PAIRED = [
    {
        "address": "E8:47:3A:D5:D8:A7",
        "name": "DualSense Wireless Controller",
        "kind": "pad",
        "icon": "input-gaming",
        "paired": True,
        "connected": True,
        "trusted": True,
        "battery": 80,
    },
    {
        "address": "AC:80:0A:6B:21:BC",
        "name": "WH-1000XM4",
        "kind": "audio",
        "icon": "audio-headset",
        "paired": True,
        "connected": False,
        "trusted": True,
        "battery": None,
    },
]
# What a scan finds; the keyboard asks to confirm KEYBOARD_PASSKEY.
FOUND = [
    {"address": "98:B6:E9:12:34:56", "name": "Pro Controller", "kind": "pad", "icon": "input-gaming"},
    {"address": "F0:3E:90:AB:CD:EF", "name": "K380 Keyboard", "kind": "keyboard", "icon": "input-keyboard"},
    {"address": "70:99:1C:11:22:33", "name": "JBL Flip 6", "kind": "audio", "icon": "audio-card"},
]
KEYBOARD_PASSKEY = "284719"


def _found(d):
    return {**d, "paired": False, "connected": False, "trusted": False, "battery": None}


class _Watch:
    """One `universe <kind> watch`: commands in through `send`, lines out through `emit`, delays on the fake core's clock."""

    def __init__(self, radio, emit):
        self._radio = radio
        self._emit = emit
        self.open = True

    def emit(self, line):
        if self.open:
            self._emit(dict(line))

    def send(self, command):
        if self.open:
            self._radio.command(self, dict(command))

    def stop(self):
        self.open = False
        self._radio.watches.discard(self)
        self._radio.stopped(self)


class FakeWifi:
    """NetworkManager as the fixture has it. `UNIVERSE_FAKE_NETWORK=offline` starts off every network, `none` with no NetworkManager."""

    def __init__(self, later, step):
        self._later = later
        self._step = step
        self.watches = set()
        mode = os.environ.get("UNIVERSE_FAKE_NETWORK", "")
        self.available = mode != "none"
        self.enabled = True
        self.networks = [] if not self.available else [{**n, "active": n["active"] and mode != "offline"} for n in NETWORKS]
        self.connectivity = "full" if self.joined() else "none"
        self.commands = []

    def joined(self):
        return next((n for n in self.networks if n["active"]), None)

    def state(self):
        joined = self.joined()
        if not self.available:
            return {"available": False, "enabled": False, "device": "", "link": "", "ssid": "", "strength": 0, "connectivity": "", "networks": []}
        return {
            "available": True,
            "enabled": self.enabled,
            "device": "wlan0",
            "link": "wifi" if joined else "",
            "ssid": joined["ssid"] if joined else "",
            "strength": joined["strength"] if joined else 0,
            "connectivity": self.connectivity,
            "networks": copy.deepcopy(self.networks) if self.enabled else [],
        }

    def watch(self, emit):
        w = _Watch(self, emit)
        self.watches.add(w)
        w.emit({"event": "ready"})
        w.emit({"event": "state", **self.state()})
        return w

    def stopped(self, w):
        pass

    def _changed(self):
        for w in list(self.watches):
            w.emit({"event": "state", **self.state()})

    def _network(self, ssid):
        return next((n for n in self.networks if n["ssid"] == ssid), None)

    def command(self, w, cmd):
        self.commands.append(cmd)
        what, ssid = cmd.get("cmd"), cmd.get("ssid", "")
        if what == "wifi":
            self.enabled = bool(cmd.get("on", True))
            if not self.enabled:
                for n in self.networks:
                    n["active"] = False
                self.connectivity = "none"
            w.emit({"event": "done", "action": "wifi"})
            self._changed()
        elif what == "scan":
            if cmd.get("on", True):
                w.emit({"event": "done", "action": "scan"})
        elif what == "connect":
            w.emit({"event": "connecting", "ssid": ssid})
            self._later(self._step() * 4, lambda: self._join(w, ssid, cmd.get("password")))
        elif what == "forget":
            n = self._network(ssid)
            if n is None or not n["saved"]:
                w.emit({"event": "failed", "action": "forget", "ssid": ssid, "reason": "notfound", "message": f"{ssid} is not saved"})
                return
            n["saved"] = n["active"] = False
            self.connectivity = "full" if self.joined() else "none"
            w.emit({"event": "done", "action": "forget", "ssid": ssid})
            self._changed()
        elif what == "check":
            w.emit({"event": "done", "action": "check", "connectivity": self.connectivity})
        elif what == "quit":
            w.stop()

    def _join(self, w, ssid, password):
        n = self._network(ssid)
        if n is None:
            w.emit({"event": "failed", "action": "connect", "ssid": ssid, "reason": "notfound", "message": f"{ssid} is not in range"})
            return
        if n["security"] in ("wep", "enterprise"):
            failure = ("failed", f"{ssid} uses {n['security']} security, which Universe does not set up")
        elif n["security"] in ("psk", "sae") and not n["saved"] and password != PASSWORD:
            failure = ("password", "the password was not accepted" if password else f"{ssid} needs a password")
        else:
            failure = None
        if failure:
            w.emit({"event": "failed", "action": "connect", "ssid": ssid, "reason": failure[0], "message": failure[1]})
            return
        for other in self.networks:
            other["active"] = other is n
        n["saved"] = True
        self.connectivity = "full"
        self._changed()
        w.emit({"event": "done", "action": "connect", "ssid": ssid, "connectivity": "full"})


class FakeBluetooth:
    """BlueZ as the fixture has it: two paired devices, three a scan finds. `UNIVERSE_FAKE_BLUETOOTH=none` has no adapter."""

    def __init__(self, later, step, default_agent):
        self._later = later
        self._step = step
        self._default = default_agent
        self.watches = set()
        self.available = os.environ.get("UNIVERSE_FAKE_BLUETOOTH", "") != "none"
        self.powered = True
        self.discovering = False
        # A watch asked for a search: it goes on again when the adapter does, as the core's watch has it.
        self.searching = False
        self.devices = copy.deepcopy(PAIRED) if self.available else []
        self.commands = []
        self._asked = {}
        self._next = 0
        self._pending = None

    def state(self):
        if not self.available:
            return {"available": False, "powered": False, "discovering": False, "devices": []}
        devices = sorted(self.devices, key=lambda d: (not d["paired"], d["name"].lower()))
        return {"available": True, "powered": self.powered, "discovering": self.discovering, "devices": copy.deepcopy(devices)}

    def watch(self, emit):
        w = _Watch(self, emit)
        self.watches.add(w)
        w.emit({"event": "ready", "agent": True, "default": self._default()})
        w.emit({"event": "state", **self.state()})
        return w

    def stopped(self, w):
        if not self.watches:
            self.searching = False
            if self.discovering:
                self._scan(False)

    def _changed(self):
        for w in list(self.watches):
            w.emit({"event": "state", **self.state()})

    def _device(self, address):
        return next((d for d in self.devices if d["address"] == address), None)

    def _scan(self, on):
        self.discovering = on and self.powered
        if self.discovering:
            known = {d["address"] for d in self.devices}
            self.devices += [_found(d) for d in FOUND if d["address"] not in known]
        else:
            self.devices = [d for d in self.devices if d["paired"]]

    def ask(self, address, kind, code=""):
        """A question from BlueZ to the agent: the keyboard's passkey, or a pad plugged in by cable (`authorize`)."""
        d = self._device(address) or next((_found(f) for f in FOUND if f["address"] == address), None)
        if d is None or not self.watches:
            return 0
        self._next += 1
        self._asked[self._next] = address
        for w in list(self.watches):
            w.emit(
                {
                    "event": "request",
                    "id": self._next,
                    "kind": kind,
                    "address": address,
                    "name": d["name"],
                    "device_kind": d["kind"],
                    "code": code,
                    "entered": 0,
                }
            )
        return self._next

    def cancel_ask(self):
        self._asked.clear()
        for w in list(self.watches):
            w.emit({"event": "cancel"})

    def command(self, w, cmd):
        self.commands.append(cmd)
        what, address = cmd.get("cmd"), cmd.get("address", "")
        if what == "scan":
            self.searching = bool(cmd.get("on", True))
            self._scan(self.searching)
            self._changed()
        elif what == "power":
            self.powered = bool(cmd.get("on", True))
            if not self.powered:
                for dev in self.devices:
                    dev["connected"] = False
            self._scan(self.searching)
            self._changed()
        elif what in ("pair", "connect", "disconnect", "remove"):
            self._act(w, what, address)
        elif what == "answer":
            asked = self._asked.pop(cmd.get("id"), None)
            if asked is None:
                return
            yes = cmd.get("yes") is True or bool(cmd.get("value"))
            pending = self._pending
            if pending and pending[1] == asked:
                self._pending = None
                self._later(self._step() * 2, lambda: self._paired(pending[0], asked, yes))
            elif yes and (dev := self._device(asked) or self._adopt(asked)):
                dev.update(paired=True, trusted=True)
                self._changed()
        elif what == "cancel":
            if self._pending and self._pending[1] == address:
                self._pending = None
                self.cancel_ask()
                w.emit({"event": "failed", "action": "pair", "address": address, "reason": "canceled", "message": "Authentication Canceled"})
        elif what == "quit":
            w.stop()

    def _act(self, w, what, address):
        d = self._device(address)
        if d is None:
            w.emit({"event": "failed", "action": what, "address": address, "reason": "failed", "message": f"not found: no Bluetooth device {address}"})
            return
        if what == "pair" and d["kind"] == "keyboard":
            self._pending = (w, address)
            self.ask(address, "confirm", KEYBOARD_PASSKEY)
            return
        if what == "pair":
            self._later(self._step() * 4, lambda: self._paired(w, address, True))
            return
        if what == "remove":
            self.devices.remove(d)
        else:
            d["connected"] = what == "connect"
        w.emit({"event": "done", "action": what, "address": address})
        self._changed()

    def _adopt(self, address):
        found = next((_found(f) for f in FOUND if f["address"] == address), None)
        if found is not None:
            self.devices.append(found)
        return found

    def _paired(self, w, address, ok):
        d = self._device(address)
        if d is None:
            return
        if not ok:
            w.emit({"event": "failed", "action": "pair", "address": address, "reason": "rejected", "message": "Authentication Rejected"})
            return
        d.update(paired=True, trusted=True, connected=True)
        self._changed()
        w.emit({"event": "done", "action": "pair", "address": address, "connected": True})
