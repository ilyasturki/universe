import json
import logging
import os
import shutil

from PySide6.QtCore import Property, QObject, QProcess, QTimer, Signal, Slot

from .settings import AdvancedRows, _add, _dig, _group, _row

log = logging.getLogger("universe.controller")

TRIGGERS = ("press", "hold")
HOME_SLOT = "guide"
HOME_TEXT = "HOME · press for the menu, hold to go home"
AXES = ("lx", "ly", "rx", "ry", "lt", "rt")
BUS_NAMES = {"bluetooth": "Bluetooth", "usb": "USB"}
PASSIVE_TEXT = "Macros are running in the game session"
PASSIVE_DETAIL = "Live presses and learning resume when it ends"
RESTART_MS = 2000
RESTART_MAX_MS = 30000

# [controller] keys behind the page's Advanced row: (key, label, choices, detail).
TIMING_ROWS = [
    ("controller.hold_ms", "Hold length (ms)", ("400", "600", "800", "1000"), "A press this long is a hold; a macro on hold fires then."),
    ("controller.volume_step", "Volume step (%)", ("1", "2", "5", "10"), "How much a volume macro moves the default sink per press, 1 to 100."),
]
TIMING_DEFAULTS = {"controller.hold_ms": 600, "controller.volume_step": 2}


class Watcher(QObject):
    event = Signal(object)

    def __init__(self, parent=None):
        super().__init__(parent)
        self._process = None
        self._buffer = b""

    def start(self, families=None):
        program = os.environ.get("UNIVERSE_BIN") or shutil.which("universe")
        if not program:
            log.warning("no universe binary: controller macros are off")
            return False
        self._process = QProcess(self)
        self._process.setProcessChannelMode(QProcess.ProcessChannelMode.ForwardedErrorChannel)
        self._process.readyReadStandardOutput.connect(self._read)
        self._process.finished.connect(self._finished)
        self._process.start(program, ["controller", "watch", "--json", "--wait"])
        return True

    def _finished(self, code, status):
        log.info("controller watcher ended (%s)", code)
        if self._process is not None:
            self._process.deleteLater()
            self._process = None
        self.event.emit({"event": "off", "code": code})

    def _read(self):
        self._buffer += bytes(self._process.readAllStandardOutput().data())
        while b"\n" in self._buffer:
            line, self._buffer = self._buffer.split(b"\n", 1)
            try:
                payload = json.loads(line)
            except ValueError:
                continue
            if isinstance(payload, dict):
                self.event.emit(payload)

    def send(self, command):
        if self._process is None or self._process.state() == QProcess.ProcessState.NotRunning:
            return False
        self._process.write((json.dumps(command) + "\n").encode())
        return True

    def stop(self):
        if self._process is None:
            return
        process, self._process = self._process, None
        process.finished.disconnect(self._finished)
        process.closeWriteChannel()
        process.terminate()
        if not process.waitForFinished(2000):
            process.kill()
            process.waitForFinished(1000)


class FakeWatcher(QObject):
    event = Signal(object)

    def __init__(self, family="dualsense-edge", unbound=(), parent=None):
        super().__init__(parent)
        self._family = family
        self._unbound = set(unbound)
        self._families = {}
        self.ident = "event30"
        self.commands = []
        self.started = False

    def start(self, families=None):
        self._families = dict(families or {})
        self.started = True
        self.emit({"event": "ready"})
        if self._family != "none":
            self.emit(self.device())
        return True

    def device(self, ident="event30", family=None, bus="bluetooth"):
        family = family or self._family
        spec = self._families.get(family) or {"name": family, "slots": []}
        slots = {}
        for slot in spec.get("slots") or []:
            codes = [] if slot["id"] in self._unbound else (slot.get("codes") or [])
            slots[slot["id"]] = {"code": codes[0] if codes else None, "bound": bool(codes)}
        return {"event": "device", "id": ident, "name": spec.get("name") or family, "family": family, "bus": bus, "slots": slots}

    def send(self, command):
        self.commands.append(dict(command))
        return True

    def emit(self, line):
        self.event.emit(dict(line))

    def press(self, slot, down=True):
        self.emit({"event": "button", "id": self.ident, "slot": slot, "code": "", "pressed": bool(down)})

    def axis(self, name, value):
        self.emit({"event": "axis", "id": self.ident, "axis": name, "value": float(value)})

    def exit(self, code=1):
        self.started = False
        self.emit({"event": "off", "code": code})

    def stop(self):
        self.started = False


class ControllerScreen(AdvancedRows, QObject):
    devicesChanged = Signal()
    currentChanged = Signal()
    stateChanged = Signal()
    rowsChanged = Signal()
    advancedChanged = Signal()
    statusChanged = Signal()
    testingChanged = Signal()
    buttonPressed = Signal(str, str, bool)
    axisMoved = Signal(str, str, float)
    unknownPressed = Signal(str, str)
    learned = Signal(str, str, str)
    macroNotice = Signal(str)
    screenshotTaken = Signal(str)
    message = Signal(str)

    def __init__(self, client, memory, power, parent=None):
        super().__init__(parent)
        self._client = client
        self._memory = memory
        self._power = power
        self._watcher = None
        self._devices = []
        self._current = ""
        self._state = {}
        self._init_rows()
        self._status = "off"
        self._learning = ""
        self._testing = False
        self._suspended = False
        self._docked = False
        self._passive = False
        self._wanted = ""
        self.restart_ms = RESTART_MS
        self._restart_delay = RESTART_MS
        self._last_family = str(memory.get("controllerFamily") or "dualsense")
        power.sourcesChanged.connect(self._rebuild)
        self._rebuild()

    def start(self, watcher):
        self.load()
        self._watcher = watcher
        watcher.event.connect(self._on_event)
        return self._launch()

    def _launch(self):
        if self._watcher is None:
            return False
        if not self._watcher.start(self._families()):
            self._watcher = None
            return False
        if self._suspended:
            self._watcher.send({"cmd": "suspend", "dock": self._docked})
        return True

    def _restart(self):
        if self._watcher is None or self._status != "off":
            return
        if not self._launch():
            self._rebuild()

    def shutdown(self):
        if self._watcher is not None:
            self._watcher.stop()
            self._watcher = None

    @Slot()
    def load(self):
        self._state = dict(self._client.controllerState())
        if self._passive:
            self._enumerate()
        self._rebuild()
        self.stateChanged.emit()

    def _enumerate(self):
        self._devices = [self._entry(p) for p in self._client.controllerPads() if p.get("id")]
        if self._device() is None:
            self._current = self._devices[0]["id"] if self._devices else ""
            if self._devices:
                self._remember(self._devices[0]["family"])
            self.currentChanged.emit()
        self.devicesChanged.emit()

    @staticmethod
    def _entry(line):
        ident = str(line.get("id") or "")
        return {"id": ident, "name": str(line.get("name") or ident), "family": str(line.get("family") or "generic"),
                "bus": str(line.get("bus") or ""), "slots": dict(line.get("slots") or {})}

    def _families(self):
        return {f["id"]: f for f in self._state.get("families") or [] if f.get("id")}

    def _presets(self):
        return {p["id"]: p for p in self._state.get("presets") or [] if p.get("id")}

    def _macro(self, family, slot, trigger):
        for m in self._state.get("macros") or []:
            if m.get("button") == slot and m.get("trigger") == trigger and m.get("family") in (family, "*"):
                return dict(m, label=self._label(m))
        return None

    def _device(self, ident=None):
        ident = self._current if ident is None else ident
        return next((d for d in self._devices if d["id"] == ident), None)

    def _remember(self, family):
        if not family:
            return
        self._last_family = family
        self._memory.set("controllerFamily", family)

    def _on_event(self, line):
        kind = line.get("event")
        ident = str(line.get("id") or "")
        if kind == "device":
            self._upsert(line)
        elif kind == "gone":
            self._remove(ident)
        elif kind == "button":
            self.buttonPressed.emit(ident, str(line.get("slot") or ""), bool(line.get("pressed")))
        elif kind == "axis":
            axis = str(line.get("axis") or "")
            if axis in AXES:
                self.axisMoved.emit(ident, axis, float(line.get("value") or 0.0))
        elif kind == "unknown":
            self.unknownPressed.emit(ident, str(line.get("code") or ""))
        elif kind == "hud":
            self.macroNotice.emit(self._hud_notice(line))
        elif kind == "screenshot":
            self.screenshotTaken.emit(str(line.get("path") or ""))
        elif kind == "learned":
            self._learned(line)
        elif kind == "learn_timeout":
            if self._stop_learning():
                self.message.emit("No button pressed: learning stopped")
        elif kind == "error":
            self._stop_learning()
            self.message.emit(str(line.get("message") or "Controller error"))
        elif kind in ("waiting", "ready", "off"):
            self._status = kind
            if kind == "waiting":
                self._passive = True
                self._stop_testing()
                self._enumerate()
            elif kind == "ready":
                self._restart_delay = self.restart_ms
                if self._passive:
                    self._passive = False
                    self._clear_devices()
            else:
                self._passive = False
                self._stop_learning()
                self._stop_testing()
                self._clear_devices()
            self._rebuild()
            self.statusChanged.emit()
            if kind == "off" and self._watcher is not None:
                QTimer.singleShot(self._restart_delay, self._restart)
                self._restart_delay = min(max(self._restart_delay, 1) * 2, RESTART_MAX_MS)

    @staticmethod
    def _hud_notice(line):
        shown = line.get("shown")
        if shown is None:
            return "MangoHud: no game running"
        title = str(line.get("title") or "")
        return f"MangoHud {'shown' if shown else 'hidden'}" + (f" · {title}" if title else "")

    def _stop_learning(self):
        if not self._learning:
            return False
        self._learning = ""
        self.statusChanged.emit()
        return True

    def _stop_testing(self):
        if not self._testing:
            return False
        self._testing = False
        if self._watcher is not None:
            self._watcher.send({"cmd": "axes", "on": False})
        self.testingChanged.emit()
        return True

    def _clear_devices(self):
        self._devices = []
        self._current = ""
        self.currentChanged.emit()
        self.devicesChanged.emit()

    def _upsert(self, line):
        ident = str(line.get("id") or "")
        entry = self._entry(line)
        for i, d in enumerate(self._devices):
            if d["id"] == ident:
                self._devices[i] = entry
                break
        else:
            self._devices.append(entry)
        if self._device() is None or (self._wanted and entry["name"] == self._wanted):
            self._current = ident
            self._wanted = ""
            self.currentChanged.emit()
        if self._current == ident:
            self._remember(entry["family"])
        self._rebuild()
        self.devicesChanged.emit()

    def _remove(self, ident):
        gone = self._device(ident)
        if gone is None:
            return
        self._devices = [d for d in self._devices if d["id"] != ident]
        if self._current == ident:
            self._wanted = gone["name"] if self._devices else ""
            self._current = self._devices[0]["id"] if self._devices else ""
            if self._devices:
                self._remember(self._devices[0]["family"])
            else:
                self._stop_testing()
            self.currentChanged.emit()
        if not self._devices:
            self._stop_learning()
        self._rebuild()
        self.devicesChanged.emit()

    def _learned(self, line):
        family, slot, code = str(line.get("family") or ""), str(line.get("slot") or ""), str(line.get("code") or "")
        previous = line.get("from")
        for device in self._devices:
            if device["family"] != family:
                continue
            device["slots"][slot] = {"code": code, "bound": True}
            if previous and previous != slot:
                device["slots"][previous] = {"code": None, "bound": False}
        self._stop_learning()
        # The watcher wrote config.toml from its own process; this one's core rereads it first.
        self._client.rescan()
        self.load()
        self.learned.emit(family, slot, code)

    def _label(self, macro):
        action = str(macro.get("action") or "")
        if action == "keys":
            return str(macro.get("keys") or "Key combo")
        if action == "command":
            return "Command"
        return str(self._presets().get(action, {}).get("label") or action)

    def _display(self, bound, press, hold):
        if not bound:
            return "Unbound"
        parts = []
        if press:
            parts.append("Press · " + self._label(press))
        if hold:
            parts.append("Hold · " + self._label(hold))
        return " / ".join(parts) or "—"

    def _rebuild(self):
        rows, groups = [], []
        device = self._device()
        if device is None and self._status == "off" and self._watcher is not None:
            rows.append(_row("Controller", "", "Controller macros stopped", "info", False, detail="Restarting"))
            groups.append(_group("Controller", [0]))
        elif device is None:
            rows.append(_row("Controller", "", "No controller connected", "info", False))
            groups.append(_group("Controller", [0]))
        else:
            family = self._families().get(device["family"])
            if family is not None:
                slots = list(family.get("slots") or [])
                slots = [s for s in slots if s.get("extra")] + [s for s in slots if not s.get("extra")]
                name = str(family.get("name") or device["name"])
            else:
                slots = [{"id": s, "label": s.replace("_", " ").capitalize(), "codes": [], "extra": True} for s in device["slots"]]
                name = device["name"]
            if self._passive:
                rows.append(_row(name, "", PASSIVE_TEXT, "info", False))
            if len(self._devices) > 1:
                names = [d["name"] for d in self._devices]
                rows.append(_row(name, "device", "Controller", "enum", device["name"], choices=names))
            if self._status == "ready" and not self._passive:
                row = _row(name, "test", "Test the buttons", "action", "")
                row.update(display="", family=device["family"], icon="gamepad", action="Start")
                rows.append(row)
            for slot in slots:
                binding = device["slots"].get(slot["id"]) or {}
                bound = bool(binding.get("bound"))
                home = slot["id"] == HOME_SLOT
                press = None if home else self._macro(device["family"], slot["id"], "press")
                hold = None if home else self._macro(device["family"], slot["id"], "hold")
                display = HOME_TEXT if home and bound else self._display(bound, press, hold)
                row = _row(name, slot["id"], str(slot.get("label") or slot["id"]), "action", display)
                row.update(slot=slot["id"], family=device["family"], bound=bound, code=str(binding.get("code") or ""),
                           extra=bool(slot.get("extra")), press=press, hold=hold, home=home, action="Configure")
                rows.append(row)
            extras = sum(1 for s in slots if s.get("extra"))
            meta = [BUS_NAMES.get(device["bus"], device["bus"])]
            if device["name"] != name:
                meta.insert(0, device["name"])
            battery = self._power.forInput(device["id"])
            if battery is not None:
                meta.append(f"{battery['percent']}%" + (", charging" if battery["charging"] else ""))
            meta.append(f"{extras} extra button" + ("" if extras == 1 else "s"))
            groups.append(_group(name, list(range(len(rows))), meta=" · ".join(m for m in meta if m)))
        config = self._client.config()
        for key, label, choices, detail in TIMING_ROWS:
            value = _dig(config, key)
            row = _row("Timing", key, label, "int", str(TIMING_DEFAULTS[key] if value in (None, "") else value), choices, detail=detail, advanced=True)
            _add(rows, groups, "Timing", row, caps=True)
        self._set_rows(rows, groups)

    @Slot(int, result="QVariant")
    def row(self, index):
        return self._row_at(index)

    @Slot(str, str, result=int)
    def reveal(self, key, module=""):
        return self._reveal(key, module)

    @Slot(int, "QVariant", result=bool)
    def setValue(self, index, value):
        row = self.row(index)
        if str(row.get("key") or "").startswith("controller."):
            if not self._client.setConfig(row["key"], str(value)):
                return False
            self._rebuild()
            return True
        if row.get("key") != "device":
            return False
        device = next((d for d in self._devices if d["name"] == str(value)), None)
        if device is None:
            return False
        self.setCurrent(device["id"])
        return True

    @Slot(str, str, str, str, str, result=bool)
    def bind(self, slot, trigger, action, keys, command):
        device = self._device()
        if device is None:
            self.message.emit("No controller connected")
            return False
        if trigger not in TRIGGERS or not action:
            return False
        if slot == HOME_SLOT:
            self.message.emit("Guide is HOME: a press opens the menu, a hold goes home")
            return False
        if self._presets().get(action, {}).get("hold_only") and trigger != "hold":
            self.message.emit(self._presets()[action].get("label", action) + " only fires on a hold")
            return False
        payload = {"family": device["family"], "button": slot, "trigger": trigger, "action": action,
                   "keys": keys or "", "command": command or ""}
        if not self._client.controllerBind(json.dumps(payload)):
            return False
        self.load()
        self.reload()
        return True

    @Slot(str, str, result=bool)
    def unbind(self, slot, trigger):
        device = self._device()
        if device is None:
            return False
        if not self._client.controllerUnbind(device["family"], slot, trigger or ""):
            return False
        self.load()
        self.reload()
        return True

    @Slot(str, result=bool)
    def learn(self, slot):
        device = self._device()
        if device is None or self._watcher is None:
            self.message.emit("No controller connected")
            return False
        if self._passive or self._status != "ready":
            self.message.emit(PASSIVE_DETAIL if self._passive else "Controller macros are not running")
            return False
        self._learning = slot
        self.statusChanged.emit()
        return bool(self._watcher.send({"cmd": "learn", "id": device["id"], "slot": slot}))

    @Slot()
    def cancelLearn(self):
        if self._stop_learning() and self._watcher is not None:
            self._watcher.send({"cmd": "cancel"})

    @Slot(bool, result=bool)
    def setTesting(self, on):
        if not on:
            return self._stop_testing()
        if self._testing:
            return True
        if self._device() is None or self._watcher is None or self._passive or self._status != "ready":
            self.message.emit(PASSIVE_DETAIL if self._passive else "No controller connected")
            return False
        self._testing = True
        self._watcher.send({"cmd": "axes", "on": True})
        self.testingChanged.emit()
        return True

    # `dock`: the presets marked docked (volume, mute, MangoHud) still fire.
    @Slot()
    def suspend(self, dock=False):
        self._suspended = True
        self._docked = bool(dock)
        if self._watcher is not None:
            self._watcher.send({"cmd": "suspend", "dock": self._docked})

    @Slot()
    def resume(self):
        self._suspended = False
        self.cancelLearn()
        self._stop_testing()
        if self._watcher is not None:
            self._watcher.send({"cmd": "resume"})

    @Slot(str, result=bool)
    def run(self, action, keys=""):
        if self._watcher is None or self._status != "ready":
            return False
        return bool(self._watcher.send({"cmd": "run", "action": action, "keys": keys}))

    def reload(self):
        if self._watcher is not None:
            self._watcher.send({"cmd": "reload"})

    def setCurrent(self, ident):
        device = self._device(ident)
        if device is None or ident == self._current:
            return
        self._current = ident
        self._wanted = ""
        self._remember(device["family"])
        self.cancelLearn()
        self._stop_testing()
        self._rebuild()
        self.currentChanged.emit()
        self.devicesChanged.emit()

    def _family(self):
        device = self._device()
        return device["family"] if device is not None else self._last_family

    def _unbound(self):
        return [r["slot"] for r in self._rows if "slot" in r and not r["bound"]]

    devices = Property("QVariantList", lambda self: [dict(d) for d in self._devices], notify=devicesChanged)
    current = Property(str, lambda self: self._current, setCurrent, notify=currentChanged)
    state = Property("QVariant", lambda self: dict(self._state), notify=stateChanged)
    presets = Property("QVariantList", lambda self: list(self._state.get("presets") or []), notify=stateChanged)
    rows = Property("QVariantList", lambda self: list(self._rows), notify=rowsChanged)
    groups = Property("QVariantList", AdvancedRows._shown_groups, notify=rowsChanged)
    showAdvanced = Property(bool, lambda self: self._show_advanced, AdvancedRows._set_show_advanced, notify=advancedChanged)
    count = Property(int, lambda self: len(self._rows), notify=rowsChanged)
    unboundSlots = Property("QVariantList", _unbound, notify=rowsChanged)
    family = Property(str, _family, notify=devicesChanged)
    connected = Property(bool, lambda self: self._device() is not None, notify=devicesChanged)
    status = Property(str, lambda self: self._status, notify=statusChanged)
    passive = Property(bool, lambda self: self._passive, notify=statusChanged)
    learning = Property(str, lambda self: self._learning, notify=statusChanged)
    testing = Property(bool, lambda self: self._testing, notify=testingChanged)
