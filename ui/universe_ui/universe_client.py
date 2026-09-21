import json
import logging
import os
import tempfile
import threading
from pathlib import Path

from PySide6.QtCore import QFileSystemWatcher, QObject, QTimer, Signal, Slot
from PySide6.QtGui import QImage

from .errors import UniverseError
from .qt import QULONGLONG, QVARIANT, Property


class _NoCoreError(Exception):
    pass


def _core_error() -> type[Exception]:
    try:
        from universe_core import UniverseError
    except ImportError:  # --fake without the extension built
        return _NoCoreError
    return UniverseError


CoreError = _core_error()


log = logging.getLogger("universe.client")


# `universe splash`'s format: `<w> <h>\n` then RGB32 rows; the helper deletes the file once shown.
def write_poster(image, ident):
    try:
        image = image.convertToFormat(QImage.Format.Format_RGB32)
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
                f.writelines(bits[y * stride : y * stride + width * 4] for y in range(height))
        return str(path)
    except OSError as e:
        log.warning("launch poster: %s", e)
        return ""


class CoreClient(QObject):
    sessionStarted = Signal(str, str)
    # (session, game, duration_s, end): `end` as the session row's (quit, stopped, crashed, killed)
    sessionEnded = Signal(str, str, int, str)
    libraryChanged = Signal(list)
    recordingFiled = Signal(str, str, str)
    entryWritten = Signal(str, str)
    progress = Signal(str, QULONGLONG, QULONGLONG, str)
    jobFinished = Signal(str, bool, str)
    mediaChanged = Signal(str)
    modulesChanged = Signal()
    sourcesChanged = Signal()
    currentSessionChanged = Signal()
    launched = Signal(str, str)
    notice = Signal(str)
    launchFailed = Signal(str, str)
    sessionShown = Signal(str, bool)
    error = Signal(str, str)

    _deliver = Signal(object)

    def __init__(self, core, parent=None):
        super().__init__(parent)
        self._core = core
        self._current = None
        self._data = Path(core.data_home())
        self._state = Path(core.state_home())
        overrides = (self.config().get("paths") or {}).get("overrides")
        self._overrides = Path(os.path.expanduser(overrides)) if overrides else None
        self._job_seq = 0
        self._jobs = {}
        self._closed = False
        self._skipped_told = False
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

    def shutdown(self):
        self._closed = True
        close = getattr(self._core, "shutdown", None)
        if close is not None:
            close()
        self._poll.stop()
        self._debounce.stop()
        if self._watcher.directories():
            self._watcher.removePaths(self._watcher.directories())

    def _call(self, fn, *args):
        try:
            return fn(*args)
        except UniverseError:
            raise
        except CoreError as e:
            kind, message = ([*list(e.args), "", ""])[:2]
            raise UniverseError(kind or "Io", message or kind) from None

    def _guarded(self, default, fn, *args):
        try:
            value = self._call(fn, *args)
        except UniverseError as e:
            self.error.emit(e.kind, e.message)
            return default
        return default if value is None else value

    def attempt(self, work):
        try:
            return self._call(work), ""
        except UniverseError as e:
            return None, e.message or e.kind

    def _done(self, fn, *args):
        try:
            self._call(fn, *args)
        except UniverseError as e:
            self.error.emit(e.kind, e.message)
            return False
        return True

    def _call_async(self, work, on_reply=None, on_error=None):
        on_error = on_error or (lambda e: self.error.emit(e.kind, e.message))

        def run():
            try:
                value = self._call(work)
            except UniverseError as e:
                err = e
                self._deliver.emit(lambda: on_error(err))
                return
            if on_reply is not None:
                reply = on_reply
                self._deliver.emit(lambda: reply(value))

        threading.Thread(target=run, daemon=True, name="core-call").start()

    def runAsync(self, work, on_done):
        def run():
            result = work()
            self._deliver.emit(lambda: on_done(result))

        threading.Thread(target=run, daemon=True, name="runAsync").start()

    def refreshCurrent(self):
        current = self._guarded(None, self._core.current)
        if current != self._current:
            self._current = current
            self.currentSessionChanged.emit()

    currentSession = Property(QVARIANT, lambda self: self._current, notify=currentSessionChanged)

    @Slot(result="QVariant")
    def list(self):
        return self._guarded([], self._core.list)

    @Slot(str, result="QVariant")
    def game(self, ident):
        return self._guarded({}, self._core.get, ident)

    @Slot(str, result="QVariant")
    def resolve(self, query):
        return self._guarded([], self._core.resolve, query)

    @Slot(str, str, str, result=bool)
    def set(self, ident, key, value):
        return self._done(self._core.set, ident, key, str(value))

    @Slot(str, bool, result=bool)
    def remove(self, ident, purge):
        return self._done(self._core.remove, ident, purge)

    @Slot(str, result=bool)
    def uninstall(self, ident):
        return self._done(self._core.uninstall, ident)

    @Slot()
    def rescan(self):
        self._guarded(None, self._core.reload)

    # config.toml and the modules, not the library.
    def reloadSettings(self):
        self._guarded(None, self._core.reload_settings)

    @Slot(bool, result="QVariant")
    def importLutris(self, apply):
        return self._guarded({}, self._core.import_lutris, apply)

    @Slot(result="QVariant")
    def discover(self):
        return self._guarded({}, self._core.discover)

    @Slot(str, str, str, result=str)
    def addGame(self, runner, path, title):
        ident = self._guarded("", self._core.add_game, {"runner": runner, "exe": path, "title": title})
        if ident:
            self.libraryChanged.emit([ident])
        return ident

    @Slot(result="QVariant")
    def runners(self):
        return self._guarded([], self._core.runners)

    @Slot(str, str, str, result=bool)
    def setRunnerSetting(self, runner, key, value):
        return self._done(self._core.set_runner_setting, runner, key, str(value))

    def launch(self, ident, screen, poster=None):
        def on_reply(session_id):
            session_id = str(session_id or "")
            self._track(session_id, ident)
            self.launched.emit(session_id, ident)
            self._wait_window(session_id)

        def start(splash):
            self._call_async(lambda: self._core.launch(ident, screen, splash), on_reply, lambda e: self.launchFailed.emit(ident, e.message))

        if poster is None or poster.isNull():
            start("")
        else:
            self.runAsync(lambda: write_poster(poster, ident), start)

    @Slot(str)
    def stop(self, session_id):
        self._call_async(lambda: self._core.stop(session_id))

    def stopNow(self, session_id):
        self._guarded(None, self._core.stop, session_id)

    @Slot()
    def focusSession(self):
        self._call_async(self._core.focus_session)

    @Slot()
    def focusLauncher(self):
        self._call_async(lambda: self._core.focus_pid(os.getpid()), on_error=lambda e: log.info("focus launcher: %s", e.message))

    def freeze(self, on, on_reply=None):
        self._call_async(lambda: self._core.freeze(on), on_reply)

    nested = property(lambda self: bool(self._core.nested()))

    def gameShown(self):
        return self._guarded(False, self._core.nest_game_shown)

    def gameShownAsync(self, on_reply):
        self._call_async(self._core.nest_game_shown, lambda shown: on_reply(bool(shown)), on_error=lambda e: on_reply(False))

    def overlay(self, window, input, opacity):
        return self._done(self._core.nest_overlay, int(window), bool(input), int(opacity))

    def frame(self, on_done):
        self._call_async(self._core.nest_frame, lambda path: on_done(str(path or "")), lambda e: on_done(""))

    def hostGamescope(self, screen):
        return self._guarded(None, self._core.host_gamescope, screen or "")

    def setFpsLimit(self, on_reply):
        self._call_async(self._core.set_fps_limit, lambda combo: on_reply(str(combo or "")))

    def setMangohud(self, on, on_reply=None):
        self._call_async(lambda: self._core.set_mangohud(on), on_reply)

    def nestFilter(self, filter, sharpness):
        return self._done(self._core.nest_filter, filter, sharpness)

    def volumeAsync(self, change, value, on_reply):
        self._call_async(lambda: self._core.volume(change, int(value)), on_reply)

    def adoptScope(self):
        try:
            return str(self._call(self._core.adopt_scope) or "")
        except UniverseError as e:
            log.warning("adopt_scope: %s", e.message)
            return ""

    @Slot(result=str)
    def screenshot(self):
        return self._guarded("", self._core.screenshot)

    @Slot(str, result="QVariantList")
    def screenshots(self, ident):
        return self._guarded([], self._core.screenshots, ident)

    @Slot(str, str, result=bool)
    def removeScreenshot(self, ident, name):
        if not self._done(self._core.remove_screenshot, ident, name):
            return False
        self.libraryChanged.emit([ident])
        return True

    @Slot(str, result="QVariant")
    def sessions(self, ident):
        return self._guarded([], self._core.sessions, ident)

    # The unit's journal on the worker: the read shells out to journalctl.
    def sessionLogAsync(self, ident, session, tail, on_reply, on_error=None):
        self._call_async(lambda: self._core.session_log(ident, session, tail), on_reply, on_error)

    @Slot(str, result="QVariant")
    def media(self, ident):
        return self._guarded([], self._core.media, ident)

    # `build(rows)` runs on the worker too: a long list costs the UI thread only the reply.
    def mediaAsync(self, ident, build, on_reply):
        self._call_async(lambda: build(self._core.media(ident)), on_reply)

    def _wait_window(self, session_id):
        def missed(e):
            log.info("session window: %s", e.message)
            self.sessionShown.emit(session_id, False)

        # None is "no window yet" as much as "session gone": the poster holds while the session lives.
        def wait():
            while True:
                window = self._core.wait_session_window(session_id, 60000)
                current = self._core.current()
                if window or not current or current.get("session_id") != session_id:
                    return window

        self._call_async(wait, lambda window: self.sessionShown.emit(session_id, bool(window)), missed)

    # Once per run, after the session toast: the core skips the hooks of an enabled module whose binaries are missing and says so only in its log.
    def _notice_skipped_modules(self):
        if self._skipped_told:
            return
        self._skipped_told = True
        for m in self.modules():
            if m.get("enabled") and not m.get("available", True):
                self.notice.emit(f"{m.get('name', m['id'])} was on but ran nothing: missing {', '.join(m.get('missing') or [])}")

    def _track(self, session_id, ident):
        self.refreshCurrent()
        self.sessionStarted.emit(session_id, ident)
        self._poll.start()

    def _poll_session(self):
        if not self._current:
            self._poll.stop()
        elif not self._guarded(None, self._core.current):
            self._ended(self._current)

    # `currentSessionChanged` fires at once; the reload and the closed line run on a worker, the toast, the stats and a pending launch follow the reply.
    def _ended(self, marker):
        session_id, ident = marker["session_id"], marker["id"]
        self._poll.stop()
        self.focusLauncher()
        self.refreshCurrent()

        def closed():
            self._core.reload_game(ident)
            return next((s for s in self._core.sessions(ident) if s.get("session") == session_id), {})

        def landed(line):
            line = line or {}
            self.sessionEnded.emit(session_id, ident, int(line.get("duration_s") or 0), str(line.get("end") or ""))
            QTimer.singleShot(4500, self._notice_skipped_modules)
            self.libraryChanged.emit([ident])
            if line.get("recording"):
                self.recordingFiled.emit(session_id, ident, str(line["recording"].get("path") or ""))

        def missed(e):
            self.error.emit(e.kind, e.message)
            landed({})

        self._call_async(closed, landed, missed)

    @Slot(result="QVariant")
    def sources(self):
        return self._guarded([], self._core.sources)

    @Slot(str, result=str)
    def loginUrl(self, source):
        return self._guarded("", self._core.login_url, source)

    @Slot(str, str, result=str)
    def login(self, source, code):
        return self._job("login", source, lambda progress: self._core.login(source, code))

    @Slot(str, bool, result="QVariant")
    def sourceLibrary(self, source, refresh):
        """Raises when the store cannot be reached, so the caller keeps the listing it has instead of an empty one."""
        return self._call(self._core.library, source, refresh) or []

    @Slot(str, str, result="QVariant")
    def search(self, source, query):
        return self._guarded([], self._core.search, source, query)

    @Slot(str, str, result="QVariant")
    def info(self, source, game_id):
        return self._guarded({}, self._core.info, source, game_id)

    @Slot(str, str, result=str)
    def install(self, source, game_id):
        return self._job("install", game_id, lambda progress: self._core.install(source, game_id, progress), source=source)

    @Slot(str, str, result=str)
    def update(self, source, game_id):
        return self._job("update", game_id, lambda progress: f"{self._core.update(source, game_id, progress)} updated", source=source)

    @Slot(str, result=bool)
    def cancel(self, job):
        """Stops a job: SIGTERMs the source behind an install or update (it finishes as failed), lets a media
        fetch end after the game in hand (it finishes as ok); either way `cancelled` is set on it."""
        j = self._jobs.get(job)
        if not j or j["finished"]:
            return False
        if j["kind"] == "media":
            j["cancelled"] = True
            self._guarded(None, self._core.media_cancel)
            return True
        if j["kind"] not in ("install", "update") or not j["target"]:
            return False
        j["cancelled"] = True
        return bool(self._guarded(False, self._core.cancel, j["source"], j["target"]))

    @Slot(result="QVariant")
    def updates(self):
        return self._guarded([], self._core.updates)

    @Slot(str, result=str)
    def scan(self, source):
        return self._job("scan", source, lambda progress: f"{self._core.scan(source, progress)} game(s)")

    @Slot(result="QVariant")
    def jobs(self):
        return [dict(j) for j in self._jobs.values()]

    @Slot(str, bool, result=str)
    def mediaRefresh(self, ident, force):
        return self._job("media", ident, lambda progress: "{}/{} updated".format(*self._core.media_refresh(ident, force, progress)))

    @Slot(str, result="QVariant")
    def mediaStatus(self, ident):
        return self._guarded([], self._core.media_status, ident)

    @Slot(str, str, str, result=str)
    def mediaSetSlot(self, ident, slot, path):
        placed = self._guarded("", self._core.media_set_slot, ident, slot, path)
        if placed:
            self.mediaChanged.emit(ident)
        return placed

    @Slot(str, str, str, result=str)
    def mediaSetUrl(self, ident, slot, url):
        placed = self._guarded("", self._core.media_set_url, ident, slot, url)
        if placed:
            self.mediaChanged.emit(ident)
        return placed

    @Slot(str, str, result=bool)
    def mediaUnset(self, ident, slot):
        gone = self._guarded(False, self._core.media_unset, ident, slot)
        if gone:
            self.mediaChanged.emit(ident)
        return gone

    @Slot(str, str, int, result="QVariant")
    def mediaCandidates(self, ident, slot, page=0):
        return self._guarded({}, self._core.media_candidates, ident, slot, page)

    @Slot(str, str, result="QVariant")
    def mediaSearch(self, ident, query):
        return self._guarded([], self._core.media_search, ident, query)

    @Slot(str, str, str, result=bool)
    def mediaPin(self, ident, provider, provider_id):
        return self._done(self._core.media_pin, ident, provider, provider_id)

    @Slot(str, result="QVariant")
    def recordings(self, ident):
        return [row for row in self.sessions(ident) if row.get("recording")]

    @Slot(str, str, result=str)
    def fileRecording(self, session_id, path):
        return self._guarded("", self._core.file_recording, session_id, path)

    @Slot(str, str, result=bool)
    def removeRecording(self, ident, session_id):
        if not self._done(self._core.remove_recording, ident, session_id):
            return False
        self.recordingFiled.emit("", ident, "")
        return True

    @Slot(str, result="QVariant")
    def journal(self, ident):
        return self._guarded([], self._core.journal, ident)

    @Slot(str, str, result=bool)
    def removeJournalEntry(self, ident, session_id):
        if not self._done(self._core.remove_journal_entry, ident, session_id):
            return False
        self.entryWritten.emit("", ident)
        return True

    @Slot(str, result=str)
    def renderJournal(self, ident):
        return self._guarded("", self._core.render_journal, ident)

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

    @Slot(result="QVariant")
    def modules(self):
        return self._guarded([], self._core.modules)

    @Slot(str, bool)
    def enableModule(self, ident, enabled):
        if self._done(self._core.enable_module, ident, enabled):
            self.modulesChanged.emit()

    @Slot(str, str, result="QVariant")
    def getSettings(self, module, game_id):
        return self._guarded({}, self._core.module_settings, module, game_id)

    @Slot(str, str, result="QVariant")
    def settingChoices(self, module, key):
        return self._guarded([], self._core.module_setting_choices, module, key)

    @Slot(str, str, str, str, result=bool)
    def setSetting(self, module, game_id, key, value):
        return self._done(self._core.set_module_setting, module, game_id, key, str(value))

    @Slot(str, bool)
    def enableSource(self, ident, enabled):
        if self._done(self._core.enable_source, ident, enabled):
            self.sourcesChanged.emit()

    @Slot(str, result="QVariant")
    def getSourceSettings(self, source):
        return self._guarded({}, self._core.source_settings, source)

    @Slot(str, str, result="QVariant")
    def sourceSettingChoices(self, source, key):
        return self._guarded([], self._core.source_setting_choices, source, key)

    @Slot(str, str, str, result=bool)
    def setSourceSetting(self, source, key, value):
        return self._done(self._core.set_source_setting, source, key, str(value))

    @Slot(str, result="QVariant")
    def settings(self, ident):
        out = {}
        for module in self.modules():
            if module.get("enabled") and any(s.get("scope") == "game" for s in module.get("settings") or []):
                out[module["id"]] = self.getSettings(module["id"], ident)
        return out

    @Slot(result="QVariant")
    def doctor(self):
        return self._guarded([], self._core.doctor)

    @Slot(result="QVariant")
    def config(self):
        return self._guarded({}, self._core.settings)

    @Slot(str, str, result=bool)
    def setConfig(self, key, value):
        return self._done(self._core.set_setting, key, str(value))

    @Slot(str, result="QVariant")
    def screenMode(self, screen):
        return self._guarded({}, self._core.screen_mode, screen or "")

    @Slot(str, "QVariant", result="QVariant")
    def launchKeys(self, scope, screen):
        return self._guarded([], self._core.launch_keys, scope, dict(screen) if screen else None)

    @Slot(result="QVariant")
    def gpu(self):
        return self._guarded(None, self._core.gpu) or {}

    @Slot(result=str)
    def version(self):
        return self._guarded("", self._core.version)

    @Slot(result="QVariant")
    def controllerState(self):
        return self._guarded({}, self._core.controller_state)

    def controllerPads(self):
        return self._guarded([], self._core.controller_pads)

    @Slot(str, result=bool)
    def controllerBind(self, payload):
        return self._done(self._core.set_controller_macro, json.loads(payload) if isinstance(payload, str) else dict(payload or {}))

    @Slot(str, str, str, result=bool)
    def controllerUnbind(self, family, button, trigger):
        return self._done(self._core.remove_controller_macro, family, button, trigger)

    @Slot(str, str, str, result=bool)
    def controllerSetButton(self, family, slot, codes):
        return self._done(self._core.set_controller_button, family, slot, json.loads(codes) if isinstance(codes, str) else codes)

    def _rewatch(self):
        games = self._data / "games"
        for d in (games, self._state):
            d.mkdir(parents=True, exist_ok=True)
        wanted = {str(games), str(self._state)}
        for d in games.iterdir():
            if d.is_dir():
                wanted.update(str(p) for p in (d, d / "journal", d / "journal" / "attachments", d / "media", d / "screenshots") if p.is_dir())
        if self._overrides and self._overrides.is_dir():
            wanted.add(str(self._overrides))
            wanted.update(str(p) for d in self._overrides.iterdir() if d.is_dir() for p in (d, d / "screenshots") if p.is_dir())
        new = sorted(wanted - set(self._watcher.directories()))
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
                    if root and path.is_relative_to(root):
                        ids.add(path.relative_to(root).parts[0])
                        break
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
            was = self._current
            self.refreshCurrent()
            if self._current and not was:
                self._track(self._current["session_id"], self._current["id"])
            elif was and not self._current:
                self._ended(was)

    def _job(self, kind, target, work, source=""):
        self._job_seq += 1
        job = f"job-{self._job_seq}"
        self._jobs[job] = {
            "id": job,
            "kind": kind,
            "target": target,
            "source": source,
            "done": 0,
            "total": 0,
            "message": "",
            "finished": False,
            "ok": False,
            "cancelled": False,
        }

        def progress(done, total, message):
            self._deliver.emit(lambda: self._job_progress(job, done, total, message))

        def run():
            try:
                message, ok = str(self._call(work, progress)), True
            except UniverseError as e:
                message, ok = e.message, False
            except Exception as e:  # noqa: BLE001
                message, ok = str(e), False
            self._deliver.emit(lambda: self._job_finished(job, ok, message))

        threading.Thread(target=run, daemon=True, name=job).start()
        return job

    def _job_progress(self, job, done, total, message):
        self._jobs[job].update(done=int(done), total=int(total), message=message)
        self.progress.emit(job, int(done), int(total), message)

    def _job_finished(self, job, ok, message):
        self._jobs[job].update(finished=True, ok=ok, message=message)
        self.jobFinished.emit(job, ok, message)
        self.libraryChanged.emit([])
