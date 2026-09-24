from PySide6.QtCore import Signal, Slot

from ..qt import Property
from .add import _source_status
from .settings import RowsForm, _add, _row, launch_row

MEMORY_KEY = "onboarded"
PREFERENCE_KEYS = ("hdr",)
FAMILY_DETAIL = "The pad the button hints and the controller art follow until one is plugged in."
READ_ONLY_HOME_MANAGER = "Settings are managed by home-manager on this machine: change them in programs.universe.settings."
READ_ONLY = "config.toml is read-only on this machine: make it writable to change settings here."
NO_GOG_SOURCE = "needs gogdl"
NOT_YET = "not importable yet"

TITLES = {"found": "What's on this machine", "stores": "Your stores", "preferences": "A few choices", "done": "You're set"}
SUBTITLES = {"done": "Everything here can be changed later under Settings."}


def _step(ident):
    return {"id": ident, "title": TITLES[ident], "subtitle": SUBTITLES.get(ident, "")}


def _plural(n, word):
    return f"{n} {word}{'' if n == 1 else 's'}"


def preference_rows(client, controller):
    rows, groups = [], []
    if not controller.families:
        controller.load()
    families = controller.families
    current = next((f["name"] for f in families if f["id"] == controller.family), controller.family)
    row = _row("Controller", "controller.family", "Buttons and glyphs", "enum", current, [f["name"] for f in families], detail=FAMILY_DETAIL)
    row["choiceValues"] = [f["id"] for f in families]
    _add(rows, groups, "Controller", row, caps=True)
    config, gpu = client.config(), client.gpu()
    launch, fits = config.get("launch") or {}, gpu.get("fits") or {}
    for spec in client.launchKeys("global", None):
        if spec["key"] not in PREFERENCE_KEYS or fits.get(spec["key"]) is False:
            continue
        value = launch.get(spec["key"])
        if value in (None, "", {}):
            value = spec["default"]
        row = launch_row("Graphics", spec, value, gpu=gpu)
        row["advanced"] = False
        _add(rows, groups, "Graphics", row, caps=True)
    return rows, groups


def _static(key, label, display, detail=""):
    return {**_row("", key, label, "static", display, detail=detail), "display": display}


def _found_display(launcher):
    state, n = launcher["state"], launcher["count"]
    if state == "importing":
        return "Importing…"
    if state == "imported":
        return f"{_plural(n, 'game')} added" if n else "Nothing new"
    if state == "failed":
        return launcher["error"] or "Failed"
    if not launcher["games"]:
        return "No games"
    return _plural(launcher["games"], "game") + " · " + launcher["detail"]


def _importable(launcher):
    return bool(launcher["importable"] and launcher["games"])


class Onboarding(RowsForm):
    stepChanged = Signal()
    finished = Signal()
    message = Signal(str)

    def __init__(self, client, memory, games, login, controller, parent=None):
        super().__init__(client, parent)
        self._memory = memory
        self._games = games
        self._login = login
        self._controller = controller
        self._steps = []
        self._step = 0
        self._launchers = []
        self._gog_dirs = []
        self._sources = []
        self._writable = True
        self._home_manager = False
        self._summary = []
        self._scan_job = ""
        login.finished.connect(self._on_login)
        client.jobFinished.connect(self._on_job_finished)

    def _needed(self):
        if self._memory.get(MEMORY_KEY):
            return False
        if self._games.count > 0:
            self._memory.set(MEMORY_KEY, True)
            return False
        return True

    @Slot()
    def load(self):
        self._steps, self._step, self._summary, self._scan_job = [_step("found")], 0, [], ""
        self._launchers, self._gog_dirs, self._sources = [], [], []
        self._set_rows([], [])
        self.stepChanged.emit()

        def done(report, error):
            if error:
                self.message.emit(f"Could not look at this machine: {error}")
            report = report or {}
            self._gog_dirs = [str(d) for d in report.get("gog_dirs") or []]
            self._sources = [s for s in self._client.sources() if s.get("available", True) and s.get("enabled", True)]
            gog = any(s["id"] == "gog" for s in self._sources)
            self._launchers = [{**launcher, "state": "", "count": 0, "error": ""} for launcher in report.get("launchers") or []]
            for launcher in self._launchers:
                if launcher.get("via") == "gog" and not gog:
                    launcher["importable"] = False
                launcher["detail"] = "" if launcher["importable"] else NO_GOG_SOURCE if launcher.get("via") == "gog" else NOT_YET
            config = self._client.config() or {}
            self._writable = bool(config.get("config_writable", True))
            self._home_manager = config.get("os") == "nixos"
            steps = ["found"]
            if any(not s.get("logged_in") for s in self._sources):
                steps.append("stores")
            if self._writable:
                steps.append("preferences")
            steps.append("done")
            self._steps = [_step(s) for s in steps]
            self._go(0)

        self._run(self._client.core.discover, done)

    def _step_id(self):
        return self._steps[self._step]["id"] if 0 <= self._step < len(self._steps) else ""

    def _go(self, index):
        self._step = max(0, min(index, len(self._steps) - 1))
        self._refresh()
        self.stepChanged.emit()

    def _refresh(self):
        step = self._step_id()
        rows, groups = [], []
        if step == "found":
            for launcher in filter(lambda launcher: launcher["found"], self._launchers):
                if _importable(launcher) and not launcher["state"]:
                    row = _row("", launcher["id"], launcher["name"], "action", "")
                    row.update(via=launcher["via"], display=_plural(launcher["games"], "game"), action="Adopt" if launcher["via"] == "gog" else "Import")
                else:
                    row = _static(launcher["id"], launcher["name"], _found_display(launcher))
                    row["quiet"] = not _importable(launcher) and not launcher["state"]
                _add(rows, groups, "", row)
            if not rows:
                _add(rows, groups, "", _static("none", "Other launchers", "None found"))
        elif step == "stores":
            for source in self._sources:
                name, signed_in = source.get("name", source["id"]), bool(source.get("logged_in"))
                _add(rows, groups, name, {**_static("logged_in", "Account", _source_status(source)[0]), "module": source["id"]}, caps=True)
                link = _row(name, "link", "Get a sign-in link", "action", "", module=source["id"])
                _add(rows, groups, name, {**link, "action": "Sign in", "display": "", "quiet": signed_in})
                code = _row(name, "code", "Enter the code", "action", "", module=source["id"])
                _add(rows, groups, name, {**code, "action": "Enter", "display": "", "quiet": signed_in})
        elif step == "preferences":
            rows, groups = preference_rows(self._client, self._controller)
        elif step == "done":
            for label, display in self._summary or [("Library", "Nothing added yet: games can join any time from the Library")]:
                _add(rows, groups, "", _static(label, label, display))
            if not self._writable:
                if self._home_manager:
                    _add(rows, groups, "", _static("read_only", "Settings", "Managed by home-manager", READ_ONLY_HOME_MANAGER))
                else:
                    _add(rows, groups, "", _static("read_only", "Settings", "Read-only", READ_ONLY))
        self._set_rows(rows, groups)

    @Slot()
    def next(self):
        if self._step >= len(self._steps) - 1:
            self.finish()
        else:
            self._go(self._step + 1)

    @Slot()
    def back(self):
        self._go(self._step - 1)

    @Slot()
    def finish(self):
        self._memory.set(MEMORY_KEY, True)
        self.finished.emit()

    def _launcher(self, ident):
        return next((launcher for launcher in self._launchers if launcher["id"] == ident), None)

    def _set_state(self, launcher, state, count=0, error=""):
        launcher.update(state=state, count=count, error=error)
        if self._step_id() == "found":
            self._refresh()

    @Slot(int, result=bool)
    def runImport(self, index):
        launcher = self._launcher(self.row(index).get("key", ""))
        if launcher is None or launcher["state"] or self._busy or not _importable(launcher):
            return False
        if launcher["via"] == "lutris":
            self._import_lutris(launcher)
        elif launcher["via"] == "gog":
            self._adopt_gog(launcher)
        elif launcher["via"] == "roms":
            self._import_roms(launcher)
        else:
            return False
        return True

    def _import_lutris(self, launcher):
        def done(report, error):
            if error:
                self._set_state(launcher, "failed", error=error)
                self.message.emit(f"Lutris import failed: {error}")
                return
            imported = list(report.get("imported") or [])
            self._client.libraryChanged.emit([])
            self._set_state(launcher, "imported", len(imported))
            if imported:
                self._summary.append(("Lutris", f"{_plural(len(imported), 'game')} added"))

        self._set_state(launcher, "importing")
        self._run(lambda: self._client.core.import_lutris(True), done)

    def _import_roms(self, launcher):
        def done(report, error):
            if error:
                self._set_state(launcher, "failed", error=error)
                self.message.emit(f"Emulator folder scan failed: {error}")
                return
            imported = list(report.get("imported") or [])
            self._client.libraryChanged.emit([])
            self._set_state(launcher, "imported", len(imported))
            if imported:
                self._summary.append(("Emulator folders", f"{_plural(len(imported), 'game')} added"))

        self._set_state(launcher, "importing")
        self._run(lambda: self._client.core.import_roms(True), done)

    def _adopt_gog(self, launcher):
        current = [d.strip() for d in str(self._client.core.source_settings("gog").get("scan_dirs") or "").split(",") if d.strip()]
        missing = [d for d in self._gog_dirs if d not in current]
        if missing and not self._writable:
            where = "home-manager" if self._home_manager else "config.toml"
            self._set_state(launcher, "failed", error=f"Settings are read-only: add {', '.join(missing)} to sources.gog.scan_dirs in {where}")
            return
        if missing and not self._client.setSourceSetting("gog", "scan_dirs", ",".join(current + missing)):
            self._set_state(launcher, "failed", error="Could not add the folders to the GOG source")
            return
        self._set_state(launcher, "importing")
        self._scan_job = self._client.scan("gog")
        if not self._scan_job:
            self._set_state(launcher, "failed", error="The scan could not start")

    def _on_job_finished(self, job, ok, text):
        if not job or job != self._scan_job:
            return
        self._scan_job = ""
        launcher = self._launcher("heroic-gog")
        if launcher is None:
            return
        if not ok:
            self._set_state(launcher, "failed", error=text)
            self.message.emit(f"GOG scan failed: {text}")
            return
        count = int(text.split(" ")[0]) if text[:1].isdigit() else 0
        self._set_state(launcher, "imported", count)
        if count:
            self._summary.append(("GOG", f"{_plural(count, 'game')} adopted"))

    def _on_login(self, ok, text):
        if not ok:
            return
        self._sources = [s for s in self._client.sources() if s.get("available", True) and s.get("enabled", True)]
        source = next((s for s in self._sources if s["id"] == self._login.source), None)
        if source is not None:
            self._summary.append((source.get("name", source["id"]), "Signed in"))
        if self._step_id() == "stores":
            self._refresh()

    def _write(self, row, payload):
        if row["key"] == "controller.family":
            return self._controller.setFamily(payload)
        return self._client.setConfig(row["key"], payload)

    def _reload(self, row):
        self._refresh()

    needed = Property(bool, _needed, constant=True)
    steps = Property(list, lambda self: [dict(s) for s in self._steps], notify=stepChanged)
    step = Property(int, lambda self: self._step, notify=stepChanged)
    stepId = Property(str, _step_id, notify=stepChanged)
