"""`api.universe`: the core's methods as slots over one core object (`universe_core.Core`, or `FakeCore`),
and the signals derived from its files and marker — the core pushes nothing."""

import json
import logging
import os
import tempfile
import threading
from pathlib import Path

from PySide6.QtCore import (
    Property,
    QFileSystemWatcher,
    QObject,
    QTimer,
    Signal,
    Slot,
)

from .errors import UniverseError

try:
    from universe_core import UniverseError as CoreError
except ImportError:  # --fake without the extension built
    class CoreError(Exception):
        pass

log = logging.getLogger("universe.client")


def write_poster(image, ident):
    """The poster in `universe splash`'s format — `<w> <h>\n` then RGB32 rows — under the runtime dir;
    the helper deletes it once shown. `""` when it cannot be written: the launch goes on without it."""
    try:
        from PySide6.QtGui import QImage

        image = image.convertToFormat(QImage.Format_RGB32)
        width, height, stride = image.width(), image.height(), image.bytesPerLine()
        base = Path(os.environ.get("XDG_RUNTIME_DIR") or tempfile.gettempdir()) / "universe"
        base.mkdir(parents=True, exist_ok=True)
        path = base / f"splash-{ident}.bgrx"
        bits = image.constBits()
        with open(path, "wb") as f:
            f.write(f"{width} {height}\n".encode())
            if stride == width * 4:
                f.write(bits)
            else:
                for y in range(height):
                    f.write(bits[y * stride:y * stride + width * 4])
        return str(path)
    except (OSError, AttributeError, ImportError) as e:
        log.warning("launch poster: %s", e)
        return ""


class CoreClient(QObject):
    """The surface QML sees as `api.universe`: the core's calls as slots, and the signals derived from
    the files and the marker (docs/frontends.md § Changes)."""

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
    # The game's window is on screen and has the focus (`ok`), or nobody can tell: no shell
    # extension, or no window within the wait.
    sessionShown = Signal(str, bool)
    error = Signal(str, str)

    _deliver = Signal(object)

    def __init__(self, core, parent=None):
        super().__init__(parent)
        self._core = core
        self._current = None
        self._data = Path(core.data_home())
        self._state = Path(core.state_home())
        self._overrides = self._overrides_dir()
        self._job_seq = 0
        self._jobs = {}
        self._tracked = None
        self._closed = False
        self._deliver.connect(lambda fn: None if self._closed else fn())
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

    @property
    def core(self):
        return self._core

    # Nothing reaches the screens past this point: threads still out deliver into the void.
    def shutdown(self):
        self._closed = True
        close = getattr(self._core, "shutdown", None)
        if close is not None:
            close()
        self._poll.stop()
        self._debounce.stop()
        if self._watcher.directories():
            self._watcher.removePaths(self._watcher.directories())

    # -- the core's errors, once ------------------------------------------------------------

    def _call(self, fn, *args):
        try:
            return fn(*args)
        except UniverseError:
            raise
        except CoreError as e:
            kind, message = (list(e.args) + ["", ""])[:2]
            raise UniverseError(kind or "Io", message or kind) from None

    def _guarded(self, default, fn, *args):
        try:
            return self._call(fn, *args)
        except UniverseError as e:
            self.error.emit(e.kind, e.message)
            return default

    def _done(self, fn, *args):
        try:
            self._call(fn, *args)
        except UniverseError as e:
            self.error.emit(e.kind, e.message)
            return False
        return True

    # Launch blocks on pre-launch hooks (up to 20 s): keep the event loop, hence the animation, alive.
    def _call_async(self, work, on_reply, on_error):
        def run():
            try:
                value = self._call(work)
            except UniverseError as e:
                err = e
                self._deliver.emit(lambda: on_error(err))
                return
            self._deliver.emit(lambda: on_reply(value))

        threading.Thread(target=run, daemon=True, name="core-call").start()

    def runAsync(self, work, on_done):
        """`work()` off the UI thread, `on_done(result)` back on it."""
        def run():
            result = work()
            self._deliver.emit(lambda: on_done(result))

        threading.Thread(target=run, daemon=True, name="runAsync").start()

    # -- session state ---------------------------------------------------------------------

    def refreshCurrent(self):
        current = self._guarded(None, self._core.current) or None
        if current != self._current:
            self._current = current
            self.currentSessionChanged.emit()

    currentSession = Property("QVariant", lambda self: self._current, notify=currentSessionChanged)

    # -- library ---------------------------------------------------------------------------

    @Slot(result="QVariant")
    def list(self):
        return self._guarded([], self._core.list) or []

    @Slot(str, result="QVariant")
    def game(self, ident):
        return self._guarded({}, self._core.get, ident) or {}

    @Slot(str, result="QVariant")
    def resolve(self, query):
        return list(self._guarded([], self._core.resolve, query) or [])

    @Slot(str, str, str, result=bool)
    def set(self, ident, key, value):
        return self._done(self._core.set, ident, key, str(value))

    @Slot(str, bool, result=bool)
    def remove(self, ident, purge):
        return self._done(self._core.remove, ident, bool(purge))

    @Slot(str, result=bool)
    def uninstall(self, ident):
        return self._done(self._core.uninstall, ident)

    @Slot()
    def rescan(self):
        self._guarded(None, self._core.reload)

    @Slot(bool, result="QVariant")
    def importLutris(self, apply):
        return self._guarded({}, self._core.import_lutris, bool(apply)) or {}

    @Slot(str, str, str, result=str)
    def addGame(self, runner, path, title):
        ident = str(self._guarded("", self._core.add_game, {"runner": runner, "exe": path, "title": title}) or "")
        if ident:
            self.libraryChanged.emit([ident])
        return ident

    # -- runners ---------------------------------------------------------------------------

    @Slot(result="QVariant")
    def runners(self):
        return self._guarded([], self._core.runners) or []

    @Slot(str, str, str, result=bool)
    def setRunnerSetting(self, runner, key, value):
        return self._done(self._core.set_runner_setting, runner, key, str(value))

    # -- the running session ---------------------------------------------------------------

    # `poster` is the launch poster as a QImage: written for gamescope's keep-alive window (the game's
    # splash from the first frame of gamescope to the game's own window), off the UI thread first.
    def launch(self, ident, screen, poster=None):
        def on_reply(session_id):
            session_id = str(session_id or "")
            self._track(session_id, ident)
            self.launched.emit(session_id, ident)
            self._wait_window(session_id)

        def on_error(e):
            self.launchFailed.emit(ident, e.message)

        def start(splash):
            self._call_async(lambda: self._core.launch(ident, screen, splash), on_reply, on_error)

        if poster is None or poster.isNull():
            start("")
        else:
            self.runAsync(lambda: write_poster(poster, ident), start)

    # Off the UI thread: a stop waits for the unit, up to a second SIGTERM some seconds later.
    @Slot(str)
    def stop(self, session_id):
        self._call_async(lambda: self._core.stop(session_id), lambda value: None, lambda e: self.error.emit(e.kind, e.message))

    # On this thread, for a host on its way out: the unit is down when it returns.
    def stopNow(self, session_id):
        self._guarded(None, self._core.stop, session_id)

    @Slot()
    def focusSession(self):
        self._call_async(self._core.focus_session, lambda value: None, lambda e: self.error.emit(e.kind, e.message))

    # Quiet: off GNOME the compositor decides, and it usually gets it right.
    @Slot()
    def focusLauncher(self):
        self._call_async(lambda: self._core.focus_pid(os.getpid()), lambda value: None, lambda e: log.info("focus launcher: %s", e.message))

    # Quiet on purpose: without a user systemd there is no scope, and a toast for that would be noise.
    def adoptScope(self):
        try:
            return str(self._call(self._core.adopt_scope) or "")
        except UniverseError as e:
            log.warning("adopt_scope: %s", e.message)
            return ""

    @Slot(result=str)
    def screenshot(self):
        return str(self._guarded("", self._core.screenshot) or "")

    # Newest first; "" spans every visible game.
    @Slot(str, result="QVariant")
    def sessions(self, ident):
        return self._guarded([], self._core.sessions, ident) or []

    # Blocks in the core until the game's window maps and gets the focus, or the session ends first.
    def _wait_window(self, session_id):
        def run():
            try:
                shown = bool(self._call(self._core.wait_session_window, session_id, 60000))
            except UniverseError as e:
                log.info("session window: %s", e.message)
                shown = False
            self._deliver.emit(lambda: self.sessionShown.emit(session_id, shown))

        threading.Thread(target=run, daemon=True, name="session-window").start()

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
        if self._guarded(None, self._core.current):
            return
        session_id, ident = self._tracked
        self._tracked = None
        self._poll.stop()
        # The game's window is gone: home takes the screen, whatever Mutter's stack says.
        self.focusLauncher()
        self._guarded(None, self._core.reload_game, ident)
        line = next((s for s in self.sessions(ident) if s.get("session") == session_id), {})
        self.refreshCurrent()
        self.sessionEnded.emit(session_id, ident, int(line.get("duration_s") or 0))
        self.libraryChanged.emit([ident])
        if line.get("recording"):
            self.recordingFiled.emit(session_id, ident, str(line["recording"].get("path") or ""))

    # -- sources ---------------------------------------------------------------------------

    @Slot(result="QVariant")
    def sources(self):
        return self._guarded([], self._core.sources) or []

    @Slot(str, result=str)
    def loginUrl(self, source):
        return str(self._guarded("", self._core.login_url, source) or "")

    @Slot(str, str, result=str)
    def login(self, source, code):
        return self._job("login", source, lambda progress: self._core.login(source, code))

    @Slot(str, result="QVariant")
    def sourceLibrary(self, source):
        return self._guarded([], self._core.library, source, False) or []

    @Slot(str, str, result="QVariant")
    def search(self, source, query):
        return self._guarded([], self._core.search, source, query) or []

    @Slot(str, str, result="QVariant")
    def info(self, source, game_id):
        return self._guarded({}, self._core.info, source, game_id) or {}

    @Slot(str, str, result=str)
    def install(self, source, game_id):
        return self._job("install", game_id, lambda progress: self._core.install(source, game_id, progress))

    @Slot(str, str, result=str)
    def update(self, source, game_id):
        return self._job("update", game_id, lambda progress: "%d updated" % self._core.update(source, game_id, progress))

    @Slot(result="QVariant")
    def updates(self):
        return self._guarded([], self._core.updates) or []

    @Slot(str, result=str)
    def scan(self, source):
        return self._job("scan", source, lambda progress: "%d game(s)" % self._core.scan(source, progress))

    @Slot(result="QVariant")
    def jobs(self):
        return [dict(j) for j in self._jobs.values()]

    # -- media -----------------------------------------------------------------------------

    @Slot(str, bool, result=str)
    def mediaRefresh(self, ident, force):
        return self._job("media", ident, lambda progress: "%d/%d updated" % tuple(self._core.media_refresh(ident, bool(force), progress)))

    @Slot(str, result="QVariant")
    def mediaStatus(self, ident):
        return self._guarded([], self._core.media_status, ident) or []

    # A pick or its removal reaches the library through mediaChanged, as a refresh does.
    @Slot(str, str, str, result=str)
    def mediaSetSlot(self, ident, slot, path):
        placed = str(self._guarded("", self._core.media_set_slot, ident, slot, path) or "")
        if placed:
            self.mediaChanged.emit(ident)
        return placed

    @Slot(str, str, str, result=str)
    def mediaSetUrl(self, ident, slot, url):
        placed = str(self._guarded("", self._core.media_set_url, ident, slot, url) or "")
        if placed:
            self.mediaChanged.emit(ident)
        return placed

    @Slot(str, str, result=bool)
    def mediaUnset(self, ident, slot):
        gone = bool(self._guarded(False, self._core.media_unset, ident, slot))
        if gone:
            self.mediaChanged.emit(ident)
        return gone

    @Slot(str, str, int, result="QVariant")
    def mediaCandidates(self, ident, slot, page=0):
        return self._guarded({}, self._core.media_candidates, ident, slot, int(page)) or {}

    @Slot(str, str, result="QVariant")
    def mediaSearch(self, ident, query):
        return self._guarded([], self._core.media_search, ident, query) or []

    @Slot(str, str, str, result=bool)
    def mediaPin(self, ident, provider, provider_id):
        return self._done(self._core.media_pin, ident, provider, provider_id)

    # -- recordings and journal ------------------------------------------------------------

    @Slot(str, result="QVariant")
    def recordings(self, ident):
        return [row for row in self.sessions(ident) if row.get("recording")]

    @Slot(str, str, result=str)
    def fileRecording(self, session_id, path):
        return str(self._guarded("", self._core.file_recording, session_id, path) or "")

    @Slot(str, str, result=bool)
    def removeRecording(self, ident, session_id):
        if not self._done(self._core.remove_recording, ident, session_id):
            return False
        self.recordingFiled.emit("", ident, "")
        return True

    @Slot(str, result="QVariant")
    def journal(self, ident):
        return self._guarded([], self._core.journal, ident) or []

    @Slot(str, str, result=bool)
    def removeJournalEntry(self, ident, session_id):
        if not self._done(self._core.remove_journal_entry, ident, session_id):
            return False
        self.entryWritten.emit("", ident)
        return True

    @Slot(str, result=str)
    def renderJournal(self, ident):
        return str(self._guarded("", self._core.render_journal, ident) or "")

    @Slot(str, "QVariant")
    def addEntry(self, session_id, entry):
        entry = json.loads(entry) if isinstance(entry, str) else dict(entry or {})
        self._guarded(None, self._core.add_entry, session_id, entry)

    @Slot(result="QVariant")
    def pendingJournals(self):
        try:
            return list(self._call(self._core.pending_journals) or [])
        except UniverseError as e:
            log.warning("pending_journals: %s", e.message)
            return []

    # -- modules and settings --------------------------------------------------------------

    @Slot(result="QVariant")
    def modules(self):
        return self._guarded([], self._core.modules) or []

    @Slot(str, bool)
    def enableModule(self, ident, enabled):
        if self._done(self._core.enable_module, ident, bool(enabled)):
            self.modulesChanged.emit()

    @Slot(str, str, result="QVariant")
    def getSettings(self, module, game_id):
        return self._guarded({}, self._core.module_settings, module, game_id) or {}

    @Slot(str, str, result="QVariant")
    def settingChoices(self, module, key):
        return list(self._guarded([], self._core.module_setting_choices, module, key) or [])

    @Slot(str, str, str, str, result=bool)
    def setSetting(self, module, game_id, key, value):
        return self._done(self._core.set_module_setting, module, game_id, key, str(value))

    @Slot(str, result="QVariant")
    def settings(self, ident):
        """Per-game module settings: {module id: merged settings} for enabled modules."""
        out = {}
        for module in self.modules():
            if not module.get("enabled", False):
                continue
            if not any(s.get("scope") == "game" for s in module.get("settings") or []):
                continue
            out[module["id"]] = self.getSettings(module["id"], ident)
        return out

    @Slot(result="QVariant")
    def doctor(self):
        return self._guarded([], self._core.doctor) or []

    @Slot(result="QVariant")
    def config(self):
        return self._guarded({}, self._core.settings) or {}

    @Slot(str, str, result=bool)
    def setConfig(self, key, value):
        return self._done(self._core.set_setting, key, str(value))

    @Slot(str, result="QVariant")
    def screenMode(self, screen):
        """`{screen, width, height, refresh}`: the mode gamescope is told, of `screen` or the default."""
        return self._guarded({}, self._core.screen_mode, screen or "") or {}

    @Slot(str, "QVariant", result="QVariant")
    def launchKeys(self, scope, screen):
        """The launch keys of `scope` (game, global, both) as rows; `screen` (a `screenMode`) sizes the choices."""
        return list(self._guarded([], self._core.launch_keys, scope, dict(screen) if screen else None) or [])

    @Slot(result=str)
    def version(self):
        try:
            return str(self._call(self._core.version) or "")
        except UniverseError:
            return ""

    # -- controller ------------------------------------------------------------------------

    @Slot(result="QVariant")
    def controllerState(self):
        return self._guarded({}, self._core.controller_state) or {}

    def controllerPads(self):
        return self._guarded([], self._core.controller_pads) or []

    @Slot(str, result=bool)
    def controllerBind(self, payload):
        return self._done(self._core.set_controller_macro, json.loads(payload) if isinstance(payload, str) else dict(payload or {}))

    @Slot(str, str, str, result=bool)
    def controllerUnbind(self, family, button, trigger):
        return self._done(self._core.remove_controller_macro, family, button, trigger)

    @Slot(str, str, str, result=bool)
    def controllerSetButton(self, family, slot, codes):
        return self._done(self._core.set_controller_button, family, slot, json.loads(codes) if isinstance(codes, str) else codes)

    # -- file watches ----------------------------------------------------------------------

    def _overrides_dir(self):
        paths = self.config().get("paths") or {}
        return Path(os.path.expanduser(str(paths.get("overrides") or ""))) if paths.get("overrides") else None

    # The picks the CLI makes land in the overrides directory: watched like games/<id>/media.
    def _rewatch(self):
        games = self._data / "games"
        for d in (games, self._state):
            d.mkdir(parents=True, exist_ok=True)
        wanted = {str(games), str(self._state)}
        for d in games.iterdir():
            if d.is_dir():
                wanted.update(str(p) for p in (d, d / "journal", d / "media") if p.is_dir())
        if self._overrides and self._overrides.is_dir():
            wanted.add(str(self._overrides))
            wanted.update(str(p) for d in self._overrides.iterdir() if d.is_dir() for p in (d, d / "screenshots") if p.is_dir())
        have = set(self._watcher.directories())
        new = sorted(wanted - have)
        if new:
            self._watcher.addPaths(new)

    def _mark(self, path):
        if self._closed:
            return
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
            elif self._overrides and path == self._overrides:
                # A game's first pick creates its directory, already filled before it can be watched.
                watched = set(self._watcher.directories())
                ids.update(d.name for d in self._overrides.iterdir() if d.is_dir() and str(d) not in watched)
            else:
                for root in (games, self._overrides):
                    try:
                        ids.add(path.relative_to(root).parts[0])
                        break
                    except (ValueError, TypeError):
                        continue
        self._rewatch()
        if whole:
            self._guarded(None, self._core.reload)
            self.libraryChanged.emit([])
        for ident in sorted(ids):
            try:
                self._call(self._core.reload_game, ident)
            except UniverseError:
                continue
            self.libraryChanged.emit([ident])
            self.recordingFiled.emit("", ident, "")
            self.entryWritten.emit("", ident)
        if state:
            self.refreshCurrent()
            if self._current and not self._tracked:
                self._track(self._current["session_id"], self._current["id"])
            elif self._tracked:
                # The marker went: session-end has filed the session, no need to wait for the poll.
                self._poll_session()

    # -- jobs: the work runs in this process, on a thread; closing the UI aborts it -----------

    def _job(self, kind, target, work):
        self._job_seq += 1
        job = f"job-{self._job_seq}"
        self._jobs[job] = {"id": job, "kind": kind, "target": target, "done": 0, "total": 0, "message": "", "finished": False, "ok": False}

        def progress(done, total, message):
            self._deliver.emit(lambda: self._job_progress(job, done, total, message))

        def run():
            try:
                message, ok = str(self._call(work, progress)), True
            except UniverseError as e:
                message, ok = e.message, False
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
