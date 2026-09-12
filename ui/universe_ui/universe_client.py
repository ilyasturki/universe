"""The core as QML sees it (`api.universe`): in-process through `universe_core` (PyO3), and a
fixture-backed twin.

Payloads are JSON strings from the core; every method here hands QML plain dicts and lists.
Failures surface as `error`, never as an exception in QML.
"""

import json
import os
import shutil
import subprocess
import tempfile
import threading
import time
from datetime import datetime, timezone
from pathlib import Path

import shiboken6
from PySide6.QtCore import (
    Property,
    QFileSystemWatcher,
    QObject,
    QProcess,
    QTimer,
    Signal,
    Slot,
)


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


class CoreClient(UniverseClientBase):
    """The core in this process (`universe_core`). Calls block on the library; what other processes
    write — the CLI, systemd's `session-end`, the hooks — surfaces through watches on games/ and state/."""

    _deliver = Signal(object)

    def __init__(self, parent=None):
        super().__init__(parent)
        import universe_core

        self._mod = universe_core
        self._core = universe_core.Core()
        self._data = Path(universe_core.data_home())
        self._state = Path(universe_core.state_home())
        self._job_seq = 0
        self._jobs = {}
        self._tracked = None
        self._deliver.connect(lambda fn: fn())
        self._dirty = set()
        self._debounce = QTimer(self)
        self._debounce.setSingleShot(True)
        self._debounce.setInterval(300)
        self._debounce.timeout.connect(self._flush)
        self._poll = QTimer(self)
        self._poll.setInterval(2000)
        self._poll.timeout.connect(self._poll_session)
        self._watcher = QFileSystemWatcher(self)
        self._watcher.directoryChanged.connect(self._mark)
        self._rewatch()
        self.refreshCurrent()
        if self._current:
            self._track(self._current["session_id"], self._current["id"])

    # -- transport -------------------------------------------------------------------------

    def _call(self, iface, method, *args):
        fn = _CORE_CALLS.get((iface, method))
        if fn is None:
            raise UniverseError("Unavailable", f"{iface}.{method} is not provided by the core")
        try:
            return fn(self, *args)
        except self._mod.UniverseError as e:
            kind, message = (list(e.args) + ["", ""])[:2]
            raise UniverseError(kind or "Io", message or kind) from None

    # Launch blocks on pre-launch hooks (up to 20 s): keep the event loop, hence the animation, alive.
    def _call_async(self, iface, method, args, on_reply, on_error):
        def run():
            try:
                value = self._call(iface, method, *args)
            except UniverseError as e:
                self._deliver.emit(lambda: on_error(e))
                return
            self._deliver.emit(lambda: on_reply(value))

        threading.Thread(target=run, daemon=True, name=f"{iface}.{method}").start()

    def _property(self, iface, name):
        if (iface, name) == ("Session1", "Current"):
            return self._core.current_json()
        if (iface, name) == ("Settings1", "Version"):
            return self._mod.version()
        raise UniverseError("Unavailable", f"{iface}.{name}")

    # -- the running session ---------------------------------------------------------------

    def _launched(self, session_id, ident):
        self._deliver.emit(lambda: self._track(session_id, ident))
        return session_id

    def _track(self, session_id, ident):
        self._tracked = (session_id, ident)
        self.refreshCurrent()
        self.sessionStarted.emit(session_id, ident)
        self._poll.start()

    # systemd owns the game; the session is over once `session-end` has run and the unit is gone.
    def _poll_session(self):
        if not self._tracked:
            self._poll.stop()
            return
        if self._core.current_json():
            return
        session_id, ident = self._tracked
        self._tracked = None
        self._poll.stop()
        try:
            self._core.reload_game(ident)
        except self._mod.UniverseError:
            pass
        sessions = _json(self._guarded("[]", "Session1", "Sessions", ident, decode=False), [])
        line = next((s for s in sessions if s.get("session") == session_id), {})
        self.refreshCurrent()
        self.sessionEnded.emit(session_id, ident, int(line.get("duration_s") or 0))
        self.libraryChanged.emit([ident])
        if line.get("recording"):
            self.recordingFiled.emit(session_id, ident, line["recording"])

    # -- file watches ----------------------------------------------------------------------

    def _rewatch(self):
        games = self._data / "games"
        for d in (games, self._state):
            d.mkdir(parents=True, exist_ok=True)
        wanted = {str(games), str(self._state)}
        for d in games.iterdir():
            if d.is_dir():
                wanted.update(str(p) for p in (d, d / "journal", d / "media") if p.is_dir())
        have = set(self._watcher.directories())
        new = sorted(wanted - have)
        if new:
            self._watcher.addPaths(new)

    def _mark(self, path):
        self._dirty.add(path)
        self._debounce.start()

    def _flush(self):
        dirty, self._dirty = self._dirty, set()
        games = self._data / "games"
        ids, whole, state = set(), False, False
        for p in dirty:
            path = Path(p)
            if path == self._state:
                state = True
            elif path == games:
                whole = True
            else:
                try:
                    ids.add(path.relative_to(games).parts[0])
                except ValueError:
                    pass
        self._rewatch()
        if whole:
            self._guarded(None, "Library1", "Rescan", decode=False)
            self.libraryChanged.emit([])
        for ident in sorted(ids):
            try:
                self._core.reload_game(ident)
            except self._mod.UniverseError:
                continue
            self.libraryChanged.emit([ident])
            self.recordingFiled.emit("", ident, "")
            self.entryWritten.emit("", ident)
        if state:
            self.refreshCurrent()
            if self._current and not self._tracked:
                self._track(self._current["session_id"], self._current["id"])

    # -- jobs: the work runs in this process, on a thread; closing the UI aborts it -----------

    def _job(self, kind, target, work):
        self._job_seq += 1
        job = f"job-{self._job_seq}"
        self._jobs[job] = {"id": job, "kind": kind, "target": target, "done": 0, "total": 0, "message": "", "finished": False, "ok": False}

        def progress(done, total, message):
            self._deliver.emit(lambda: self._job_progress(job, done, total, message))

        def run():
            try:
                message, ok = str(work(progress)), True
            except self._mod.UniverseError as e:
                message, ok = (e.args[1] if len(e.args) > 1 else str(e)), False
            except Exception as e:  # noqa: BLE001 — a job always reports its end
                message, ok = str(e), False
            self._deliver.emit(lambda: self._job_finished(job, ok, message))

        threading.Thread(target=run, daemon=True, name=job).start()
        return job

    def _job_progress(self, job, done, total, message):
        entry = self._jobs.get(job)
        if entry:
            entry.update(done=int(done), total=int(total), message=message)
        self.progress.emit(job, int(done), int(total), message)

    def _job_finished(self, job, ok, message):
        entry = self._jobs.get(job)
        if entry:
            entry.update(finished=True, ok=bool(ok), message=message)
        self.jobFinished.emit(job, bool(ok), message)
        self.libraryChanged.emit([])


_CORE_CALLS = {
    ("Library1", "List"): lambda s: s._core.list_json(),
    ("Library1", "Get"): lambda s, ident: s._core.get_json(ident),
    ("Library1", "Resolve"): lambda s, query: s._core.resolve(query),
    ("Library1", "Set"): lambda s, ident, key, value: s._core.set(ident, key, value),
    ("Library1", "Remove"): lambda s, ident, purge: s._core.remove(ident, _bus_bool(purge)),
    ("Library1", "Rescan"): lambda s: s._core.reload(),
    ("Library1", "ImportLutris"): lambda s, apply: s._core.import_lutris(_bus_bool(apply)),
    ("Session1", "Launch"): lambda s, ident, screen: s._launched(s._core.launch(ident, screen), ident),
    ("Session1", "Stop"): lambda s, session_id: s._core.stop(session_id),
    ("Session1", "Screenshot"): lambda s: s._core.screenshot(),
    ("Session1", "Sessions"): lambda s, ident: s._core.sessions_json(ident),
    ("Sources1", "List"): lambda s: s._core.sources_json(),
    ("Sources1", "LoginUrl"): lambda s, source: s._core.login_url(source),
    ("Sources1", "Login"): lambda s, source, code: s._job("login", source, lambda p: s._core.login(source, code)),
    ("Sources1", "Library"): lambda s, source: s._core.library_json(source, False),
    ("Sources1", "RefreshLibrary"): lambda s, source: s._core.library_json(source, True),
    ("Sources1", "Search"): lambda s, source, query: s._core.search_json(source, query),
    ("Sources1", "Info"): lambda s, source, game_id: s._core.info_json(source, game_id),
    ("Sources1", "Install"): lambda s, source, game_id: s._job("install", game_id, lambda p: s._core.install(source, game_id, p)),
    ("Sources1", "Update"): lambda s, source, game_id: s._job("update", game_id, lambda p: "%d updated" % s._core.update(source, game_id, p)),
    ("Sources1", "Updates"): lambda s: s._core.updates_json(),
    ("Sources1", "Scan"): lambda s, source: s._job("scan", source, lambda p: "%d game(s)" % s._core.scan(source, p)),
    ("Sources1", "Jobs"): lambda s: json.dumps(list(s._jobs.values())),
    ("Media1", "Refresh"): lambda s, ident, force: s._job("media", ident, lambda p: "%d/%d updated" % s._core.media_refresh(ident, _bus_bool(force), p)),
    ("Media1", "SetSlot"): lambda s, ident, slot, path: s._core.media_set_slot(ident, slot, path),
    ("Media1", "Unset"): lambda s, ident, slot: s._core.media_unset(ident, slot),
    ("Media1", "Candidates"): lambda s, ident, slot: s._core.media_candidates_json(ident, slot),
    ("Media1", "Pin"): lambda s, ident, provider, provider_id: s._core.media_pin(ident, provider, provider_id),
    ("Recording1", "List"): lambda s, ident: s._core.recordings_json(ident),
    ("Recording1", "File"): lambda s, session_id, path: s._core.file_recording(session_id, path),
    ("Journal1", "List"): lambda s, ident: s._core.journal_json(ident),
    ("Journal1", "Render"): lambda s, ident: s._core.render_journal(ident),
    ("Journal1", "AddEntry"): lambda s, session_id, payload: s._core.add_entry(session_id, payload),
    ("Modules1", "List"): lambda s: s._core.modules_json(),
    ("Modules1", "Enable"): lambda s, ident, enabled: (s._core.enable_module(ident, _bus_bool(enabled)), s.modulesChanged.emit())[0],
    ("Modules1", "GetSettings"): lambda s, module, game_id: s._core.module_settings_json(module, game_id),
    ("Modules1", "SetSetting"): lambda s, module, game_id, key, value: s._core.set_module_setting(module, game_id, key, value),
    ("Modules1", "Doctor"): lambda s: s._core.doctor_json(),
    ("Settings1", "Get"): lambda s: s._core.settings_json(),
    ("Settings1", "Set"): lambda s, key, value: s._core.set_setting(key, value),
    ("Settings1", "Reload"): lambda s: s._core.reload(),
}


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
        # As the core does: the game keeps only what it sets, `effective` fills the rest.
        launch = out.setdefault("launch", {})
        desktop = out.setdefault("desktop", {})
        defaults = {**(self._config.get("launch") or {}), **(self._config.get("desktop") or {})}
        out["effective"] = {
            key: (launch if key != "hide_cursor" else desktop).get(key, default)
            for key, default in defaults.items()
            if key in ("proton", "esync", "fsync", "mangohud", "hide_cursor")
        }
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
