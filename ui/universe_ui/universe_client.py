"""QtDBus client for io.github.ilyasturki.Universe (docs/api.md), and a fixture-backed twin.

Payloads are JSON strings on the bus; every method here hands QML plain dicts and lists.
Signals are relayed as Qt signals. The daemon's absence surfaces as `error`, never as an
exception in QML.
"""

import json
import os
import shutil
import subprocess
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path

import shiboken6
from PySide6.QtCore import (
    SLOT,
    Property,
    QObject,
    QProcess,
    QTimer,
    Signal,
    Slot,
)

SERVICE = "io.github.ilyasturki.Universe"
PATH = "/io/github/ilyasturki/Universe"
IFACES = {
    "Library1": SERVICE + ".Library1",
    "Session1": SERVICE + ".Session1",
    "Sources1": SERVICE + ".Sources1",
    "Media1": SERVICE + ".Media1",
    "Recording1": SERVICE + ".Recording1",
    "Journal1": SERVICE + ".Journal1",
    "Modules1": SERVICE + ".Modules1",
    "Settings1": SERVICE + ".Settings1",
}
PROPERTIES = "org.freedesktop.DBus.Properties"
ERROR_PREFIX = SERVICE + ".Error."


class UniverseError(Exception):
    def __init__(self, kind, message):
        super().__init__(message)
        self.kind = kind
        self.message = message


def _json(text, default):
    if text is None or text == "":
        return default
    if not isinstance(text, str):
        return text
    try:
        return json.loads(text)
    except ValueError:
        return default


def _bus_bool(value):
    return value if isinstance(value, bool) else str(value).lower() in ("1", "true", "yes", "on")


class UniverseClientBase(QObject):
    """The surface QML sees as `api.universe`. Subclasses implement `_call` and `_property`."""

    sessionStarted = Signal(str, str)
    sessionEnded = Signal(str, str, int)
    libraryChanged = Signal("QVariantList")
    recordingFiled = Signal(str, str, str)
    entryWritten = Signal(str, str)
    progress = Signal(str, "qulonglong", "qulonglong", str)
    jobFinished = Signal(str, bool, str)
    mediaChanged = Signal(str)
    modulesChanged = Signal()
    currentSessionChanged = Signal()
    launched = Signal(str, str)
    launchFailed = Signal(str, str)
    error = Signal(str, str)

    def __init__(self, parent=None):
        super().__init__(parent)
        self._current = None

    # -- transport -------------------------------------------------------------------------

    def _call(self, iface, method, *args):
        raise NotImplementedError

    def _call_async(self, iface, method, args, on_reply, on_error):
        try:
            on_reply(self._call(iface, method, *args))
        except UniverseError as e:
            on_error(e)

    def _property(self, iface, name):
        raise NotImplementedError

    def _guarded(self, default, iface, method, *args, decode=True):
        try:
            value = self._call(iface, method, *args)
        except UniverseError as e:
            self.error.emit(e.kind, e.message)
            return default
        return _json(value, default) if decode else value

    # -- session state ---------------------------------------------------------------------

    def refreshCurrent(self):
        try:
            raw = self._property("Session1", "Current")
        except UniverseError as e:
            self.error.emit(e.kind, e.message)
            raw = ""
        current = _json(raw, None) or None
        if current != self._current:
            self._current = current
            self.currentSessionChanged.emit()

    def _get_current(self):
        return self._current

    currentSession = Property("QVariant", _get_current, notify=currentSessionChanged)

    # -- Library1 --------------------------------------------------------------------------

    @Slot(result="QVariant")
    def list(self):
        return self._guarded([], "Library1", "List")

    @Slot(str, result="QVariant")
    def game(self, ident):
        return self._guarded({}, "Library1", "Get", ident)

    @Slot(str, result="QVariant")
    def resolve(self, query):
        return list(self._guarded([], "Library1", "Resolve", query, decode=False) or [])

    @Slot(str, str, str, result=bool)
    def set(self, ident, key, value):
        try:
            self._call("Library1", "Set", ident, key, str(value))
        except UniverseError as e:
            self.error.emit(e.kind, e.message)
            return False
        return True

    @Slot(str, bool, result=bool)
    def remove(self, ident, purge):
        return self._guarded(None, "Library1", "Remove", ident, bool(purge), decode=False) is not False

    @Slot()
    def rescan(self):
        self._guarded(None, "Library1", "Rescan", decode=False)

    @Slot(bool, result="QVariant")
    def importLutris(self, apply):
        return self._guarded({}, "Library1", "ImportLutris", bool(apply))

    # -- Session1 --------------------------------------------------------------------------

    @Slot(str, str)
    def launch(self, ident, screen):
        def on_reply(value):
            self.launched.emit(str(value or ""), ident)

        def on_error(e):
            self.launchFailed.emit(ident, e.message)

        self._call_async("Session1", "Launch", (ident, screen), on_reply, on_error)

    @Slot(str)
    def stop(self, session_id):
        self._guarded(None, "Session1", "Stop", session_id, decode=False)

    @Slot(result=str)
    def screenshot(self):
        return str(self._guarded("", "Session1", "Screenshot", decode=False) or "")

    @Slot(str, result="QVariant")
    def sessions(self, ident):
        return self._guarded([], "Session1", "Sessions", ident)

    # -- Sources1 --------------------------------------------------------------------------

    @Slot(result="QVariant")
    def sources(self):
        return self._guarded([], "Sources1", "List")

    @Slot(str, result=str)
    def loginUrl(self, source):
        return str(self._guarded("", "Sources1", "LoginUrl", source, decode=False) or "")

    @Slot(str, str, result=str)
    def login(self, source, code):
        return str(self._guarded("", "Sources1", "Login", source, code, decode=False) or "")

    @Slot(str, result="QVariant")
    def sourceLibrary(self, source):
        return self._guarded([], "Sources1", "Library", source)

    @Slot(str, str, result="QVariant")
    def search(self, source, query):
        return self._guarded([], "Sources1", "Search", source, query)

    @Slot(str, str, result="QVariant")
    def info(self, source, game_id):
        return self._guarded({}, "Sources1", "Info", source, game_id)

    @Slot(str, str, result=str)
    def install(self, source, game_id):
        return str(self._guarded("", "Sources1", "Install", source, game_id, decode=False) or "")

    @Slot(str, str, result=str)
    def update(self, source, game_id):
        return str(self._guarded("", "Sources1", "Update", source, game_id, decode=False) or "")

    @Slot(result="QVariant")
    def updates(self):
        return self._guarded([], "Sources1", "Updates")

    @Slot(str, result=str)
    def scan(self, source):
        return str(self._guarded("", "Sources1", "Scan", source, decode=False) or "")

    @Slot(result="QVariant")
    def jobs(self):
        return self._guarded([], "Sources1", "Jobs")

    # -- Media1 ----------------------------------------------------------------------------

    @Slot(str, bool, result=str)
    def mediaRefresh(self, ident, force):
        return str(self._guarded("", "Media1", "Refresh", ident, bool(force), decode=False) or "")

    @Slot(str, str, str)
    def mediaSetSlot(self, ident, slot, path):
        self._guarded(None, "Media1", "SetSlot", ident, slot, path, decode=False)

    @Slot(str, str)
    def mediaUnset(self, ident, slot):
        self._guarded(None, "Media1", "Unset", ident, slot, decode=False)

    @Slot(str, str, result="QVariant")
    def mediaCandidates(self, ident, slot):
        return self._guarded([], "Media1", "Candidates", ident, slot)

    @Slot(str, str, str)
    def mediaPin(self, ident, provider, provider_id):
        self._guarded(None, "Media1", "Pin", ident, provider, provider_id, decode=False)

    # -- Recording1 / Journal1 -------------------------------------------------------------

    @Slot(str, result="QVariant")
    def recordings(self, ident):
        return self._guarded([], "Recording1", "List", ident)

    @Slot(str, str, result=str)
    def fileRecording(self, session_id, path):
        return str(self._guarded("", "Recording1", "File", session_id, path, decode=False) or "")

    @Slot(str, result="QVariant")
    def journal(self, ident):
        return self._guarded([], "Journal1", "List", ident)

    @Slot(str, result=str)
    def renderJournal(self, ident):
        return str(self._guarded("", "Journal1", "Render", ident, decode=False) or "")

    @Slot(str, "QVariant")
    def addEntry(self, session_id, entry):
        payload = entry if isinstance(entry, str) else json.dumps(entry)
        self._guarded(None, "Journal1", "AddEntry", session_id, payload, decode=False)

    # -- Modules1 / Settings1 --------------------------------------------------------------

    @Slot(result="QVariant")
    def modules(self):
        return self._guarded([], "Modules1", "List")

    @Slot(str, bool)
    def enableModule(self, ident, enabled):
        self._guarded(None, "Modules1", "Enable", ident, bool(enabled), decode=False)

    @Slot(str, str, result="QVariant")
    def getSettings(self, module, game_id):
        return self._guarded({}, "Modules1", "GetSettings", module, game_id)

    @Slot(str, str, str, str, result=bool)
    def setSetting(self, module, game_id, key, value):
        try:
            self._call("Modules1", "SetSetting", module, game_id, key, str(value))
        except UniverseError as e:
            self.error.emit(e.kind, e.message)
            return False
        return True

    @Slot(str, result="QVariant")
    def settings(self, ident):
        """Per-game module settings: {module id: merged settings} for enabled modules."""
        out = {}
        for module in self.modules() or []:
            if not module.get("enabled", False):
                continue
            if not any(s.get("scope") == "game" for s in module.get("settings") or []):
                continue
            out[module["id"]] = self.getSettings(module["id"], ident)
        return out

    @Slot(result="QVariant")
    def doctor(self):
        return self._guarded([], "Modules1", "Doctor")

    @Slot(result="QVariant")
    def config(self):
        return self._guarded({}, "Settings1", "Get")

    @Slot(str, str, result=bool)
    def setConfig(self, key, value):
        try:
            self._call("Settings1", "Set", key, str(value))
        except UniverseError as e:
            self.error.emit(e.kind, e.message)
            return False
        return True

    @Slot(result=str)
    def version(self):
        try:
            return str(self._property("Settings1", "Version") or "")
        except UniverseError:
            return ""


class UniverseClient(UniverseClientBase):
    """The real thing: method calls over the session bus, signals via typed slots (T4)."""

    def __init__(self, bus=None, parent=None):
        super().__init__(parent)
        from PySide6.QtDBus import QDBusConnection

        self._bus = bus or QDBusConnection.sessionBus()
        self.connected = {}
        self._watchers = []
        self._subscribe()
        self.refreshCurrent()

    # A wrong SLOT signature loses the signal silently; `connected` records what bound.
    def _subscribe(self):
        bus = self._bus
        subs = [
            ("Session1", "SessionStarted", "_onSessionStarted(QString,QString)"),
            ("Session1", "SessionEnded", "_onSessionEnded(QString,QString,uint)"),
            ("Library1", "LibraryChanged", "_onLibraryChanged(QStringList)"),
            ("Recording1", "RecordingFiled", "_onRecordingFiled(QString,QString,QString)"),
            ("Journal1", "EntryWritten", "_onEntryWritten(QString,QString)"),
            ("Sources1", "Progress", "_onProgress(QString,qulonglong,qulonglong,QString)"),
            ("Sources1", "JobFinished", "_onJobFinished(QString,bool,QString)"),
            ("Media1", "MediaChanged", "_onMediaChanged(QString)"),
            ("Modules1", "ModulesChanged", "_onModulesChanged()"),
        ]
        for iface, name, slot in subs:
            self.connected[name] = bus.connect(SERVICE, PATH, IFACES[iface], name, self, SLOT(slot))
        self.connected["PropertiesChanged"] = bus.connect(
            SERVICE, PATH, PROPERTIES, "PropertiesChanged", self, SLOT("_onPropertiesChanged(QDBusMessage)")
        )

    @Slot(str, str)
    def _onSessionStarted(self, session_id, ident):
        self.refreshCurrent()
        self.sessionStarted.emit(session_id, ident)

    @Slot(str, str, "uint")
    def _onSessionEnded(self, session_id, ident, duration):
        self.refreshCurrent()
        self.sessionEnded.emit(session_id, ident, int(duration))

    @Slot("QStringList")
    def _onLibraryChanged(self, ids):
        self.libraryChanged.emit(list(ids))

    @Slot(str, str, str)
    def _onRecordingFiled(self, session_id, ident, path):
        self.recordingFiled.emit(session_id, ident, path)

    @Slot(str, str)
    def _onEntryWritten(self, session_id, ident):
        self.entryWritten.emit(session_id, ident)

    @Slot(str, "qulonglong", "qulonglong", str)
    def _onProgress(self, job_id, done, total, message):
        self.progress.emit(job_id, int(done), int(total), message)

    @Slot(str, bool, str)
    def _onJobFinished(self, job_id, ok, message):
        self.jobFinished.emit(job_id, bool(ok), message)

    @Slot(str)
    def _onMediaChanged(self, ident):
        self.mediaChanged.emit(ident)

    @Slot()
    def _onModulesChanged(self):
        self.modulesChanged.emit()

    # The a{sv} payload is not readable from PySide6 (T4); re-read the property instead.
    @Slot("QDBusMessage")
    def _onPropertiesChanged(self, message):
        args = message.arguments()
        if args and args[0] == IFACES["Session1"]:
            self.refreshCurrent()

    def _message(self, iface, method, args):
        from PySide6.QtDBus import QDBusMessage

        msg = QDBusMessage.createMethodCall(SERVICE, PATH, IFACES[iface], method)
        msg.setArguments(list(args))
        return msg

    @staticmethod
    def _unpack(reply):
        from PySide6.QtDBus import QDBusMessage

        if reply.type() == QDBusMessage.MessageType.ErrorMessage:
            name = reply.errorName() or ""
            message = reply.errorMessage() or name
            kind = name[len(ERROR_PREFIX):] if name.startswith(ERROR_PREFIX) else name
            # universed 0.1 folds the kind into a generic Failed error's message.
            if message.startswith(ERROR_PREFIX):
                head, _, rest = message.partition(": ")
                kind, message = head[len(ERROR_PREFIX):], rest or message
            raise UniverseError(kind, message)
        args = reply.arguments()
        return args[0] if args else None

    def _call(self, iface, method, *args):
        return self._unpack(self._bus.call(self._message(iface, method, args)))

    # Launch blocks on pre-launch hooks (up to 20 s): keep the event loop, hence the animation, alive.
    def _call_async(self, iface, method, args, on_reply, on_error):
        from PySide6.QtDBus import QDBusPendingCallWatcher

        watcher = QDBusPendingCallWatcher(self._bus.asyncCall(self._message(iface, method, args)), self)
        self._watchers.append(watcher)

        def finished(w):
            self._watchers.remove(w)
            w.deleteLater()
            try:
                on_reply(self._unpack(w.reply()))
            except UniverseError as e:
                on_error(e)

        watcher.finished.connect(finished)

    def _property(self, iface, name):
        from PySide6.QtDBus import QDBusMessage

        msg = QDBusMessage.createMethodCall(SERVICE, PATH, PROPERTIES, "Get")
        msg.setArguments([IFACES[iface], name])
        value = self._unpack(self._bus.call(msg))
        return value.variant() if hasattr(value, "variant") else value


# ---------------------------------------------------------------------------------------------
# Fixture-backed twin


FIXTURE = Path(__file__).parent / "fixtures" / "library.json"


def _now():
    return datetime.now(timezone.utc).astimezone().replace(microsecond=0).isoformat()


class FakeClient(UniverseClientBase):
    """The same surface, served from fixtures/library.json. Launch runs a 2 s fake session."""

    def __init__(self, fixture=FIXTURE, fake_launch=False, art_dir=None, parent=None):
        super().__init__(parent)
        with open(fixture) as f:
            self._data = json.load(f)
        self._fake_launch = fake_launch
        self._art_dir = art_dir or tempfile.mkdtemp(prefix="universe-ui-fake-")
        self._jobs = {}
        self._job_hooks = {}
        self._job_seq = 0
        self._job_timer = QTimer(self)
        self._job_timer.setInterval(150)
        self._job_timer.timeout.connect(self._tick_jobs)
        self._process = None
        self._session_started = None
        self._config = dict(self._data.get("config") or {})
        self._paint_art()

    def _paint_art(self):
        from .fixtures.art import paint_library

        paint_library(self._data["games"], self._art_dir)
        for game in self._data["games"]:
            media = game.setdefault("media", {})
            media["screenshots"] = list(media.get("screenshots") or [])

    @property
    def art_dir(self):
        return self._art_dir

    def _game(self, ident):
        for game in self._data["games"]:
            if game["id"] == ident:
                return game
        raise UniverseError("NotFound", f"no game '{ident}'")

    def _resolved(self, game):
        out = json.loads(json.dumps(game))
        out.setdefault("stats", {"hours": 0, "play_count": 0, "last_played": None})
        out.setdefault("removed", False)
        launch = out.setdefault("launch", {})
        for key, default in (self._config.get("launch") or {}).items():
            launch.setdefault(key, default)
        modules = out.setdefault("modules", {})
        for module in self._data.get("modules", []):
            merged = modules.setdefault(module["id"], {})
            for setting in module.get("settings", []):
                if setting.get("scope") == "game":
                    merged.setdefault(setting["key"], setting.get("default"))
        return out

    # -- transport ---------------------------------------------------------------------------

    def _call(self, iface, method, *args):
        handler = getattr(self, f"_{iface}_{method}", None)
        if handler is None:
            raise UniverseError("Unavailable", f"{iface}.{method} is not in the fixture")
        return handler(*args)

    def _property(self, iface, name):
        if iface == "Session1" and name == "Current":
            return json.dumps(self._current) if self._current else ""
        if iface == "Settings1" and name == "Version":
            return "0.0.0-fake"
        raise UniverseError("Invalid", f"no property {iface}.{name}")

    # -- Library1 ----------------------------------------------------------------------------

    def _Library1_List(self):
        games = [self._resolved(g) for g in self._data["games"]]
        games.sort(key=lambda g: (g.get("hidden", False), -(_epoch(g["stats"].get("last_played")))))
        return json.dumps(games)

    def _Library1_Get(self, ident):
        return json.dumps(self._resolved(self._game(ident)))

    def _Library1_Resolve(self, query):
        q = query.casefold()
        exact = [g["id"] for g in self._data["games"] if g["id"] == q or g["title"].casefold() == q]
        if exact:
            return exact
        return [g["id"] for g in self._data["games"] if q in g["title"].casefold() or q in g["id"]]

    def _Library1_Set(self, ident, key, value):
        game = self._game(ident)
        node = game
        parts = key.split(".")
        if parts[0] == "capture":
            parts = ["modules", "capture"] + parts[1:]
        for part in parts[:-1]:
            node = node.setdefault(part, {})
        leaf = parts[-1]
        if value == "":
            node.pop(leaf, None)
        elif value in ("true", "false"):
            node[leaf] = value == "true"
        elif leaf in ("tags",):
            node[leaf] = [v.strip() for v in value.split(",") if v.strip()]
        else:
            node[leaf] = value
        self.libraryChanged.emit([ident])

    def _Library1_Remove(self, ident, purge):
        self._game(ident)["removed"] = True
        self.libraryChanged.emit([ident])

    def _Library1_Rescan(self):
        self.libraryChanged.emit([])

    def _Library1_ImportLutris(self, apply):
        return json.dumps({"imported": [], "env_diff": [], "hours": 0, "applied": _bus_bool(apply)})

    # -- Session1 ----------------------------------------------------------------------------

    def _Session1_Launch(self, ident, screen):
        if self._current:
            raise UniverseError("Busy", f"{self._current['title']} is running")
        game = self._game(ident)
        session_id = time.strftime("%Y%m%d-%H%M%S")
        self._current = {
            "session_id": session_id,
            "id": ident,
            "title": game["title"],
            "unit": f"universe-game-{ident}-{session_id}.scope",
            "screen": screen,
            "started_at": _now(),
        }
        self._session_started = time.monotonic()
        self.currentSessionChanged.emit()
        QTimer.singleShot(0, lambda: self.sessionStarted.emit(session_id, ident))
        if self._fake_launch and shutil.which("sleep"):
            self._process = QProcess(self)
            self._process.finished.connect(lambda code, status: self._end_session(code))
            self._process.start("sleep", ["2"])
            self._process.waitForStarted(1000)
        else:
            QTimer.singleShot(2000, lambda: self._end_session(0))
        return session_id

    def shutdown(self):
        self._job_timer.stop()
        if self._process is not None:
            self._process.finished.disconnect()
            self._process.kill()
            self._process.waitForFinished(1000)
            self._process = None

    def _end_session(self, exit_code):
        current = self._current
        if not current or not shiboken6.isValid(self):
            return
        duration = max(1, int(round(time.monotonic() - self._session_started)))
        game = self._game(current["id"])
        stats = game.setdefault("stats", {"hours": 0, "play_count": 0, "last_played": None})
        stats["hours"] = float(stats.get("hours") or 0) + duration / 3600
        stats["play_count"] = int(stats.get("play_count") or 0) + 1
        stats["last_played"] = _now()
        self._data.setdefault("sessions", {}).setdefault(current["id"], []).insert(0, {
            "session": current["session_id"], "game": current["id"],
            "started_at": current["started_at"], "ended_at": _now(), "duration_s": duration,
            "source": "daemon", "unit": current["unit"], "screen": current["screen"],
            "exit": exit_code, "recording": None,
        })
        self._current = None
        self._process = None
        self.currentSessionChanged.emit()
        self.libraryChanged.emit([current["id"]])
        self.sessionEnded.emit(current["session_id"], current["id"], duration)

    def _Session1_Stop(self, session_id):
        if self._process is not None:
            self._process.kill()
        elif self._current:
            self._end_session(-15)

    def _Session1_Screenshot(self):
        return os.path.join(self._art_dir, "screenshot.png")

    def _Session1_Sessions(self, ident):
        return json.dumps(self._data.get("sessions", {}).get(ident, []))

    # -- Sources1 ----------------------------------------------------------------------------

    def _Sources1_List(self):
        return json.dumps(self._data.get("sources", []))

    def _Sources1_LoginUrl(self, source):
        return self._data.get("login_url", "https://example.invalid/login")

    def _Sources1_Login(self, source, code):
        def done():
            for s in self._data.get("sources", []):
                if s["id"] == source:
                    s["logged_in"] = True
        return self._start_job(f"Logging in to {source}", 3, on_done=done)

    def _Sources1_Library(self, source):
        return json.dumps(self._data.get("source_library", {}).get(source, []))

    def _Sources1_Search(self, source, query):
        q = query.casefold()
        catalog = self._data.get("source_library", {}).get(source, []) + self._data.get("catalog", [])
        return json.dumps([g for g in catalog if q in g["title"].casefold()])

    def _Sources1_Info(self, source, game_id):
        for g in self._data.get("source_library", {}).get(source, []):
            if g["id"] == game_id:
                return json.dumps(g)
        return "{}"

    def _Sources1_Install(self, source, game_id):
        def done():
            for g in self._data.get("source_library", {}).get(source, []):
                if g["id"] == game_id:
                    g["installed"] = True
                    g["dir"] = f"/mnt/games/PC/{g['title']}"
            self.libraryChanged.emit([])
        return self._start_job(f"Installing {game_id}", 20, on_done=done)

    def _Sources1_Update(self, source, game_id):
        def done():
            self._data["updates"] = [u for u in self._data.get("updates", []) if game_id and u["id"] != game_id]
            self.libraryChanged.emit([])
        return self._start_job("Updating" + (f" {game_id}" if game_id else " everything"), 12, on_done=done)

    def _Sources1_Updates(self):
        return json.dumps(self._data.get("updates", []))

    def _Sources1_Scan(self, source):
        return self._start_job("Scanning", 4)

    def _Sources1_Jobs(self):
        return json.dumps(list(self._jobs.values()))

    # A per-job QTimer captured by its own slot outlives the client and crashes the next event loop.
    def _start_job(self, message, steps, on_done=None):
        self._job_seq += 1
        job_id = f"job-{self._job_seq}"
        job = {"id": job_id, "message": message, "done": 0, "total": steps, "finished": False}
        self._jobs[job_id] = job
        self._job_hooks[job_id] = on_done
        if not self._job_timer.isActive():
            self._job_timer.start()
        return job_id

    def _tick_jobs(self):
        for job in [j for j in self._jobs.values() if not j["finished"]]:
            job["done"] += 1
            self.progress.emit(job["id"], job["done"], job["total"], f"{job['message']} ({job['done']}/{job['total']})")
            if job["done"] >= job["total"]:
                job["finished"] = True
                on_done = self._job_hooks.pop(job["id"], None)
                if on_done:
                    on_done()
                self.jobFinished.emit(job["id"], True, f"{job['message']}: done")
        if all(j["finished"] for j in self._jobs.values()):
            self._job_timer.stop()

    # -- Media1 ------------------------------------------------------------------------------

    def _Media1_Refresh(self, ident, force):
        return self._start_job("Refreshing media", 5, on_done=lambda: self.mediaChanged.emit(ident))

    def _Media1_SetSlot(self, ident, slot, path):
        self._game(ident).setdefault("media", {})[slot] = path
        self.mediaChanged.emit(ident)

    def _Media1_Unset(self, ident, slot):
        self._game(ident).setdefault("media", {}).pop(slot, None)
        self.mediaChanged.emit(ident)

    def _Media1_Candidates(self, ident, slot):
        return "[]"

    def _Media1_Pin(self, ident, provider, provider_id):
        self._game(ident).setdefault("metadata", {})[f"{provider}_id"] = provider_id

    # -- Recording1 / Journal1 ---------------------------------------------------------------

    def _Recording1_List(self, ident):
        recordings = self._data.get("recordings", {}).get(ident, [])
        for rec in recordings:
            if not rec.get("path"):
                rec["path"] = self._fake_clip(ident, rec["session"])
        return json.dumps(recordings)

    def _Recording1_File(self, session_id, path):
        return path

    # A real clip when ffmpeg is around, so the preview has something to play; a name otherwise.
    def _fake_clip(self, ident, session):
        out = os.path.join(self._art_dir, f"{ident}-{session}.mkv")
        if os.path.exists(out):
            return out
        ffmpeg = shutil.which("ffmpeg")
        if ffmpeg:
            subprocess.run(
                [ffmpeg, "-loglevel", "error", "-y", "-f", "lavfi", "-i", "testsrc=size=640x360:rate=30:duration=3",
                 "-f", "lavfi", "-i", "sine=frequency=440:duration=3", "-c:v", "libx264", "-preset", "ultrafast",
                 "-pix_fmt", "yuv420p", "-c:a", "aac", "-shortest", out],
                capture_output=True, timeout=30,
            )
        return out

    def _Journal1_List(self, ident):
        return json.dumps(self._data.get("journal", {}).get(ident, []))

    def _Journal1_Render(self, ident):
        return os.path.join(self._art_dir, f"{ident}.md")

    def _Journal1_AddEntry(self, session_id, payload):
        entry = _json(payload, {})
        self._data.setdefault("journal", {}).setdefault(entry.get("game", ""), []).insert(0, entry)
        self.entryWritten.emit(session_id, entry.get("game", ""))

    # -- Modules1 / Settings1 ----------------------------------------------------------------

    def _Modules1_List(self):
        return json.dumps(self._data.get("modules", []))

    def _Modules1_Enable(self, ident, enabled):
        for module in self._data.get("modules", []):
            if module["id"] == ident:
                module["enabled"] = _bus_bool(enabled)
        self.modulesChanged.emit()

    def _module(self, ident):
        for module in self._data.get("modules", []):
            if module["id"] == ident:
                return module
        raise UniverseError("NotFound", f"no module '{ident}'")

    def _Modules1_GetSettings(self, module_id, game_id):
        module = self._module(module_id)
        merged = {s["key"]: s.get("default") for s in module.get("settings", [])}
        merged.update(self._config.get("modules", {}).get(module_id, {}))
        if game_id:
            merged.update(self._game(game_id).get("modules", {}).get(module_id, {}))
        return json.dumps(merged)

    def _Modules1_SetSetting(self, module_id, game_id, key, value):
        module = self._module(module_id)
        schema = {s["key"]: s for s in module.get("settings", [])}
        if key not in schema:
            raise UniverseError("Invalid", f"{module_id} has no setting '{key}'")
        kind = schema[key].get("type")
        if kind == "bool":
            value = _bus_bool(value)
        elif kind == "int":
            value = int(value)
        elif kind == "enum" and value not in (schema[key].get("choices") or []):
            raise UniverseError("Invalid", f"'{value}' is not a choice of {key}")
        if game_id:
            self._game(game_id).setdefault("modules", {}).setdefault(module_id, {})[key] = value
            self.libraryChanged.emit([game_id])
        else:
            self._config.setdefault("modules", {}).setdefault(module_id, {})[key] = value
            self.modulesChanged.emit()

    def _Modules1_Doctor(self):
        return json.dumps(self._data.get("doctor", []))

    def _Settings1_Get(self):
        return json.dumps(self._config)

    def _Settings1_Set(self, key, value):
        node = self._config
        parts = key.split(".")
        for part in parts[:-1]:
            node = node.setdefault(part, {})
        node[parts[-1]] = value


def _epoch(value):
    if not value:
        return 0
    try:
        return datetime.fromisoformat(str(value)).timestamp()
    except ValueError:
        return 0
