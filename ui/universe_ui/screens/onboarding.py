from PySide6.QtCore import Signal, Slot

from ..qt import Property
from .add import _source_status
from .settings import RowsForm, _add, _plural, _row, launch_row, login_rows

MEMORY_KEY = "onboarded"
PREFERENCE_KEYS = ("hdr",)
FAMILY_DETAIL = "The pad the button hints and the controller art follow until one is plugged in."
READ_ONLY_HOME_MANAGER = "Settings are managed by home-manager on this machine: change them in programs.universe.settings."
READ_ONLY = "config.toml is read-only on this machine: make it writable to change settings here."
NEEDED = "Runners your games need"
NOT_YET = "not importable yet"
IMPORTERS = ("lutris", "roms")

TITLES = {"found": "What's on this machine", "stores": "Your stores", "preferences": "A few choices", "done": "You're set"}
SUBTITLES = {"done": "Everything here can be changed later under Settings."}


def _step(ident):
    return {"id": ident, "title": TITLES[ident], "subtitle": SUBTITLES.get(ident, "")}


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
    if state in ("failed", "waiting"):
        return launcher["error"] or "Failed"
    if not launcher["games"]:
        return "No games"
    return _plural(launcher["games"], "game") + " · " + launcher["detail"]


def _importable(launcher):
    return bool(launcher["importable"] and launcher["games"])


def _unusable(source, via):
    """Why a launcher's games cannot go to the source that adopts them; empty when they can."""
    if source is None:
        return f"no {via} source"
    return "" if source.get("available", True) else "needs " + ", ".join(source.get("missing") or ["its programs"])


class Onboarding(RowsForm):
    stepChanged = Signal()
    finished = Signal()
    message = Signal(str)

    def __init__(self, client, memory, games, login, controller, components, parent=None):
        super().__init__(client, parent)
        self._memory = memory
        self._games = games
        self._login = login
        self._controller = controller
        self._components = components
        self._steps = []
        self._step = 0
        self._launchers = []
        self._gog_dirs = []
        self._sources = []
        self._writable = True
        self._home_manager = False
        self._summary = []
        self._scans = {}
        login.finished.connect(self._on_login)
        client.jobFinished.connect(self._on_job_finished)
        components.listingChanged.connect(lambda: self._refresh() if self._step_id() == "found" else None)

    def _needed(self):
        if self._client.onboarded():
            return False
        # ui-memory.json held the flag before the core kept one for every frontend: carried over once.
        if self._memory.get(MEMORY_KEY) or self._games.count > 0:
            self._client.markOnboarded()
            return False
        return True

    @Slot()
    def load(self):
        self._steps, self._step, self._summary, self._scans = [_step("found")], 0, [], {}
        self._launchers, self._gog_dirs, self._sources = [], [], []
        self._set_rows([], [])
        self.stepChanged.emit()

        def done(report, error):
            if error:
                self.message.emit(f"Could not look at this machine: {error}")
            report = report or {}
            self._gog_dirs = [str(d) for d in report.get("gog_dirs") or []]
            self._launchers = [{**launcher, "state": "", "count": 0, "error": ""} for launcher in report.get("launchers") or []]
            found = {launcher.get("via") for launcher in self._launchers if launcher.get("found")}
            everything = self._client.sources()
            self._sources = [s for s in everything if s.get("available", True) and (s.get("enabled", True) or s["id"] in found)]
            known = {s["id"]: s for s in everything}
            for launcher in self._launchers:
                via = launcher.get("via") or ""
                why = _unusable(known.get(via), via) if via and via not in IMPORTERS else ""
                if why:
                    launcher["importable"] = False
                launcher["detail"] = why or ("" if launcher["importable"] else NOT_YET)
            config = self._client.config() or {}
            self._writable = bool(config.get("config_writable", True))
            self._home_manager = config.get("config_owner") == "home-manager"
            steps = ["found"]
            if any(not s.get("logged_in") for s in self._sources):
                steps.append("stores")
            if self._writable:
                steps.append("preferences")
            steps.append("done")
            self._steps = [_step(s) for s in steps]
            self._go(0)

        self._run(self._client.core.discover, done)
        self._components.load()

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
                    row.update(via=launcher["via"], display=_plural(launcher["games"], "game"), action="Import" if launcher["via"] in IMPORTERS else "Adopt")
                else:
                    row = _static(launcher["id"], launcher["name"], _found_display(launcher))
                    row["quiet"] = not _importable(launcher) and not launcher["state"]
                _add(rows, groups, "", row)
            if not rows:
                _add(rows, groups, "", _static("none", "Other launchers", "None found"))
            for component in self._components.needed():
                row = _row(NEEDED, "component", component["name"], "action", "")
                busy = self._components.busyOn(component["id"])
                used = int(component.get("used_by") or 0)
                row.update(
                    via="component",
                    component=component["id"],
                    display="Installing…" if busy else _plural(used, "game"),
                    action="" if busy else "Install",
                    detail=component.get("notice") or "",
                )
                _add(rows, groups, NEEDED, row, caps=True)
        elif step == "stores":
            for source in self._sources:
                name, signed_in = source.get("name", source["id"]), bool(source.get("logged_in"))
                if not source.get("enabled", True):
                    _add(rows, groups, name, _row(name, "enabled", f"Use {name}", "bool", False, module=source["id"]), caps=True)
                    continue
                _add(rows, groups, name, {**_static("logged_in", "Account", _source_status(source)[0]), "module": source["id"]}, caps=True)
                for row in login_rows(source, name):
                    _add(rows, groups, name, {**row, "quiet": signed_in})
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
        self._client.markOnboarded()
        self.finished.emit()

    def _launcher(self, ident):
        return next((launcher for launcher in self._launchers if launcher["id"] == ident), None)

    def _set_state(self, launcher, state, count=0, error=""):
        launcher.update(state=state, count=count, error=error)
        if self._step_id() == "found":
            self._refresh()

    @Slot(int, result=bool)
    def runImport(self, index):
        row = self.row(index)
        if row.get("via") == "component":
            ident = row["component"]
            return not self._components.busyOn(ident) and (self._components.propose(ident) or self._components.installById(ident))
        launcher = self._launcher(row.get("key", ""))
        if launcher is None or launcher["state"] or self._busy or not _importable(launcher):
            return False
        if launcher["via"] == "lutris":
            self._import_lutris(launcher)
        elif launcher["via"] == "roms":
            self._import_roms(launcher)
        elif launcher["via"] == "gog":
            self._adopt_gog(launcher)
        else:
            self._adopt(launcher)
        return True

    def _import_lutris(self, launcher):
        def done(report, error):
            if error:
                self._set_state(launcher, "failed", error=error)
                self.message.emit(f"Lutris import failed: {error}")
                return
            imported = list(report.get("imported") or [])
            self._client.libraryChanged.emit([])
            self._components.load()
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
            self._components.load()
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
        self._scan(launcher)

    def _source(self, ident):
        return next((s for s in self._sources if s["id"] == ident), None)

    def _reread(self):
        by_id = {s["id"]: s for s in self._client.sources()}
        self._sources = [by_id[s["id"]] for s in self._sources if s["id"] in by_id]

    def _adopt(self, launcher):
        source = self._source(launcher["via"])
        if source is None:
            self._set_state(launcher, "failed", error=_unusable(None, launcher["via"]))
            return
        if not source.get("enabled", True):
            if not self._writable:
                self._set_state(launcher, "failed", error=f"Settings are read-only: add {launcher['via']} to sources.enabled")
                return
            self._client.enableSource(launcher["via"], True)
            self._reread()
        self._scan(launcher)

    def _scan(self, launcher):
        self._set_state(launcher, "importing")
        job = self._client.scan(launcher["via"])
        if job:
            self._scans[job] = launcher["id"]
        else:
            self._set_state(launcher, "failed", error="The scan could not start")

    def _on_job_finished(self, job, ok, text):
        launcher = self._launcher(self._scans.pop(job, "")) if job else None
        if launcher is None:
            return
        source = self._source(launcher["via"]) or {}
        name = source.get("name") or launcher["via"]
        if not ok:
            self._set_state(launcher, "failed", error=text)
            self.message.emit(f"{name} scan failed: {text}")
            return
        count = int(self._client.jobResult(job) or 0)
        if not count and not source.get("logged_in", True):
            self._set_state(launcher, "waiting", error=f"Sign in to {name} to adopt them")
            if all(s["id"] != "stores" for s in self._steps):
                self._steps.insert(1, _step("stores"))
                self.stepChanged.emit()
            return
        if count:
            self._components.load()
        self._set_state(launcher, "imported", count)
        if count:
            self._summary.append((name, f"{_plural(count, 'game')} adopted"))

    def _on_login(self, ok, text):
        if not ok:
            return
        self._reread()
        source = self._source(self._login.source)
        if source is not None:
            self._summary.append((source.get("name", source["id"]), "Signed in"))
        for launcher in self._launchers:
            if launcher["state"] == "waiting" and launcher["via"] == self._login.source:
                self._scan(launcher)
        if self._step_id() == "stores":
            self._refresh()

    def _write(self, row, payload):
        if row["key"] == "controller.family":
            return self._controller.setFamily(payload)
        if row["key"] == "enabled" and row.get("module"):
            self._client.enableSource(row["module"], payload == "true")
            self._reread()
            return True
        return self._client.setConfig(row["key"], payload)

    def _reload(self, row):
        self._refresh()

    needed = Property(bool, _needed, constant=True)
    steps = Property(list, lambda self: [dict(s) for s in self._steps], notify=stepChanged)
    step = Property(int, lambda self: self._step, notify=stepChanged)
    stepId = Property(str, _step_id, notify=stepChanged)
