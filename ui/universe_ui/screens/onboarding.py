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
RUNNING = ("queued", "importing")

TITLES = {"found": "What's on this machine", "stores": "Your stores", "preferences": "Controller and screen", "done": "You're set"}
SUBTITLES = {
    "found": "Games other launchers installed here. Adding them moves nothing.",
    "stores": "Sign in to see and install the games you own.",
    "preferences": "Both can be changed later in Settings.",
    "done": "Everything here can be changed later in Settings.",
}
DONE_RUNNING = ("Still adding games", "They keep arriving on Home after you finish.")
DONE_EMPTY = ("Nothing added yet", "Add games any time from Home.")
DONE_HOME_MANAGER = "Settings come from home-manager on this machine."
DONE_READ_ONLY = "config.toml is read-only here, so settings stay as they are."
HDR_DETAIL = "Games that support HDR send it to an HDR screen."


def _step(ident):
    return {"id": ident, "title": TITLES[ident], "subtitle": SUBTITLES.get(ident, "")}


def preference_rows(controller, config, gpu, keys):
    rows, groups = [], []
    if not controller.families:
        controller.load()
    families = controller.families
    current = next((f["name"] for f in families if f["id"] == controller.family), controller.family)
    row = _row("Controller", "controller.family", "Buttons and glyphs", "enum", current, [f["name"] for f in families], detail=FAMILY_DETAIL)
    row["choiceValues"] = [f["id"] for f in families]
    _add(rows, groups, "Controller", row, caps=True)
    launch, fits = config.get("launch") or {}, gpu.get("fits") or {}
    for spec in keys:
        if spec["key"] not in PREFERENCE_KEYS or fits.get(spec["key"]) is False:
            continue
        value = launch.get(spec["key"])
        if value in (None, "", {}):
            value = spec["default"]
        row = launch_row("Graphics", spec, value, gpu=gpu)
        row.update(advanced=False, detail=HDR_DETAIL if spec["key"] == "hdr" else row["detail"])
        _add(rows, groups, "Graphics", row, caps=True)
    return rows, groups


def _static(key, label, display, detail=""):
    return {**_row("", key, label, "static", display, detail=detail), "display": display}


def _found_display(launcher):
    """The row's short word; the reason, when there is one, is its detail line."""
    state, n = launcher["state"], launcher["count"]
    if state == "queued":
        return "Waiting…"
    if state == "importing":
        return "Adding…"
    if state == "imported":
        return f"{_plural(n, 'game')} added" if n else "Nothing new"
    if state == "failed":
        return "Not added"
    if state == "waiting":
        return "Waiting for a sign-in"
    if not launcher["games"]:
        return "No games"
    if launcher["blocked"]:
        return _plural(launcher["games"], "game")
    return _plural(launcher["games"], "game") + " · " + launcher["detail"]


def _where(launcher, sources):
    """Where a launcher's games go once added."""
    if launcher["via"] == "lutris":
        return "They join the library with their play time."
    if launcher["via"] == "roms":
        return "The games your emulators list join the library."
    name = (sources.get(launcher["via"]) or {}).get("name") or launcher["via"]
    return f"They play and update through Universe's {name} source."


def _importable(launcher):
    return bool(launcher["importable"] and launcher["games"])


def _unusable(source, via):
    """Why a launcher's games cannot go to the source that adopts them; empty when they can."""
    if source is None:
        return f"no {via} source"
    return "" if source.get("available", True) else "needs " + ", ".join(source.get("missing") or ["its programs"])


def _shown(entries):
    """`(section, row, caps)` with the quiet rows left out (nothing to bring over, a signed-in store's sign-in), unless that is all of them."""
    kept = [e for e in entries if not e[1].get("quiet")]
    rows, groups = [], []
    for section, row, caps in kept or entries:
        _add(rows, groups, section, row, caps=caps)
    return rows, groups


class Onboarding(RowsForm):
    stepChanged = Signal()
    headChanged = Signal()
    loadingChanged = Signal()
    idleChanged = Signal()
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
        self._loading = False
        self._launchers = []
        self._queue = []
        self._gog_dirs = []
        self._sources = []
        self._signed_in = []
        self._prefs = {"config": {}, "gpu": {}, "keys": []}
        self._writable = True
        self._home_manager = False
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

    def _set_loading(self, loading):
        self._loading = loading
        self.loadingChanged.emit()

    @Slot()
    def load(self):
        self._steps, self._step, self._scans, self._queue, self._signed_in = [_step("found")], 0, {}, [], []
        self._launchers, self._gog_dirs, self._sources = [], [], []
        self._set_loading(True)
        self._set_rows([], [])
        self.stepChanged.emit()
        self.headChanged.emit()
        client = self._client

        # The first sources() of a process asks every store whether its sign-in still holds: off the UI thread, with the rest.
        def look():
            return client.core.discover(), client.sources(), client.config(), client.gpu(), client.launchKeys("global", None), client.getSourceSettings("gog")

        def done(found, error):
            if error:
                self.message.emit(f"Could not look at this machine: {error}")
            report, everything, config, gpu, keys, gog = found or ({}, [], {}, {}, [], {})
            self._prefs = {"config": config or {}, "gpu": gpu or {}, "keys": keys or []}
            self._writable = bool(self._prefs["config"].get("config_writable", True))
            self._home_manager = self._prefs["config"].get("config_owner") == "home-manager"
            self._gog_dirs = [str(d) for d in report.get("gog_dirs") or []]
            self._launchers = [{**launcher, "state": "", "count": 0, "error": "", "blocked": ""} for launcher in report.get("launchers") or []]
            found_via = {launcher.get("via") for launcher in self._launchers if launcher.get("found")}
            # A source that is off is offered only where the config takes the write that turns it on.
            self._sources = [s for s in everything if s.get("available", True) and (s.get("enabled", True) or (self._writable and s["id"] in found_via))]
            known = {s["id"]: s for s in everything}
            for launcher in self._launchers:
                via = launcher.get("via") or ""
                why = _unusable(known.get(via), via) if via and via not in IMPORTERS else ""
                if why:
                    launcher["importable"] = False
                launcher["detail"] = why or ("" if launcher["importable"] else NOT_YET)
                launcher["where"] = _where(launcher, known)
                launcher["blocked"] = "" if self._writable or why else self._blocked(launcher, known.get(via), gog or {})
                if launcher["blocked"]:
                    launcher["importable"] = False
            steps = ["found"]
            if any(not s.get("logged_in") for s in self._sources):
                steps.append("stores")
            if self._writable:
                steps.append("preferences")
            steps.append("done")
            self._steps = [_step(s) for s in steps]
            self._set_loading(False)
            self._go(0)

        self._run(look, done)
        self._components.load()

    def _owner(self):
        return "home-manager" if self._home_manager else "config.toml"

    def _blocked(self, launcher, source, gog):
        """Under a read-only config, what the user has to write for a launcher's games to be adopted; empty when nothing."""
        via = launcher["via"]
        if via in IMPORTERS or source is None:
            return ""
        if not source.get("enabled", True):
            return f"{source.get('name', via)} is off: add {via} to sources.enabled in {self._owner()}."
        current = [d.strip() for d in str(gog.get("scan_dirs") or "").split(",") if d.strip()]
        missing = [d for d in self._gog_dirs if d not in current] if via == "gog" else []
        return f"Add {', '.join(missing)} to sources.gog.scan_dirs in {self._owner()}." if missing else ""

    def _step_id(self):
        return self._steps[self._step]["id"] if 0 <= self._step < len(self._steps) else ""

    def _go(self, index):
        self._step = max(0, min(index, len(self._steps) - 1))
        self._refresh()
        self.stepChanged.emit()
        self.headChanged.emit()

    def _running(self):
        return any(launcher["state"] in RUNNING for launcher in self._launchers)

    def _summary(self):
        """What the steps brought in, as the launchers stand now: one still importing reads so, and joins when it ends."""
        lines = []
        for launcher in self._launchers:
            state, n = launcher["state"], launcher["count"]
            if state in RUNNING:
                lines.append((launcher["id"], launcher["name"], "Adding…", state))
            elif state == "imported" and n:
                lines.append((launcher["id"], launcher["name"], f"{_plural(n, 'game')} added", state))
        for ident in self._signed_in:
            source = self._source(ident) or {"name": ident}
            lines.append((ident, source.get("name", ident), "Signed in", "signed_in"))
        return lines

    def _refresh(self):
        step = self._step_id()
        entries = []
        if step == "found":
            for launcher in filter(lambda launcher: launcher["found"], self._launchers):
                if _importable(launcher) and not launcher["state"]:
                    row = _row("", launcher["id"], launcher["name"], "action", "", detail=launcher["where"])
                    # `verb`: the row says what A does, the looks draw no arrow to a page it does not open.
                    row.update(via=launcher["via"], display="Add " + _plural(launcher["games"], "game"), action="Add", verb=True)
                else:
                    row = _static(launcher["id"], launcher["name"], _found_display(launcher), detail=launcher["error"] or launcher["blocked"])
                    # A reason is read whole: the folder to add is at its end.
                    row.update(quiet=not _importable(launcher) and not launcher["state"] and not launcher["blocked"], wraps=bool(row["detail"]))
                entries.append(("", {**row, "state": launcher["state"], "count": launcher["count"]}, False))
            if not entries:
                entries.append(("", _static("none", "Other launchers", "None found"), False))
            for component in self._components.needed():
                row = _row(NEEDED, "component", component["name"], "action", "")
                busy = self._components.busyOn(component["id"])
                used = int(component.get("used_by") or 0)
                row.update(
                    via="component",
                    component=component["id"],
                    display="Installing…" if busy else "Install for " + _plural(used, "game"),
                    action="" if busy else "Install",
                    detail=component.get("notice") or "",
                    verb=True,
                )
                entries.append((NEEDED, row, True))
        elif step == "stores":
            for source in self._sources:
                name, signed_in = source.get("name", source["id"]), bool(source.get("logged_in"))
                if not source.get("enabled", True):
                    entries.append((name, _row(name, "enabled", f"Use {name}", "bool", False, module=source["id"]), True))
                    continue
                entries.append((name, {**_static("logged_in", "Account", _source_status(source)[0]), "module": source["id"]}, True))
                entries.extend((name, {**row, "quiet": signed_in}, True) for row in login_rows(source, name))
        elif step == "preferences":
            self._set_rows(*preference_rows(self._controller, **self._prefs))
            return
        elif step == "done":
            for key, label, display, state in self._summary():
                entries.append(("", {**_static(key, label, display), "state": state}, False))
            if not self._writable:
                why = ("Managed by home-manager", READ_ONLY_HOME_MANAGER) if self._home_manager else ("Read-only", READ_ONLY)
                entries.append(("", {**_static("read_only", "Settings", *why), "wraps": True}, False))
        self._set_rows(*_shown(entries))

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
        if self._step_id() in ("found", "done"):
            self._refresh()
        self.headChanged.emit()

    @Slot(int, result=bool)
    def runImport(self, index):
        row = self.row(index)
        if row.get("via") == "component":
            ident = row["component"]
            return not self._components.busyOn(ident) and (self._components.propose(ident) or self._components.installById(ident))
        launcher = self._launcher(row.get("key", ""))
        if launcher is None or launcher["state"] or not _importable(launcher):
            return False
        if launcher["via"] in IMPORTERS:
            self._queue.append(launcher)
            self._set_state(launcher, "queued")
            self._next_import()
        elif launcher["via"] == "gog":
            self._adopt_gog(launcher)
        else:
            self._adopt(launcher)
        return True

    def _next_import(self):
        """One importer at a time, both writing the library; the rest of the setup stays usable meanwhile."""
        if not self._queue or any(launcher["state"] == "importing" and launcher["via"] in IMPORTERS for launcher in self._launchers):
            return
        launcher = self._queue.pop(0)
        if launcher["via"] == "lutris":
            self._import(launcher, lambda: self._client.core.import_lutris(True), "Lutris import failed")
        else:
            self._import(launcher, lambda: self._client.core.import_roms(True), "Emulator folder scan failed")

    def _import(self, launcher, work, failed):
        def done(report, error):
            if error:
                self._set_state(launcher, "failed", error=error)
                self.message.emit(f"{failed}: {error}")
            else:
                imported = list(report.get("imported") or [])
                self._client.libraryChanged.emit([])
                self._components.load()
                self._set_state(launcher, "imported", len(imported))
                art = [f["id"] for f in imported if isinstance(f, dict)]
                if art:
                    self._client.mediaRefreshMany(art)
            self._next_import()

        self._set_state(launcher, "importing")
        self._run(work, done)

    def _adopt_gog(self, launcher):
        current = [d.strip() for d in str(self._client.core.source_settings("gog").get("scan_dirs") or "").split(",") if d.strip()]
        missing = [d for d in self._gog_dirs if d not in current]
        if missing and not self._writable:
            self._set_state(launcher, "failed", error=f"Settings are read-only: add {', '.join(missing)} to sources.gog.scan_dirs in {self._owner()}.")
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
                self._set_state(launcher, "failed", error=f"Settings are read-only: add {launcher['via']} to sources.enabled in {self._owner()}.")
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
            self._set_state(launcher, "waiting", error=f"Sign in to {name} on the stores step to add them.")
            if all(s["id"] != "stores" for s in self._steps):
                self._steps.insert(1, _step("stores"))
                self.stepChanged.emit()
            return
        if count:
            self._components.load()
        self._set_state(launcher, "imported", count)

    def _on_login(self, ok, text):
        if not ok:
            return
        self._reread()
        source = self._source(self._login.source)
        if source is not None and source["id"] not in self._signed_in:
            self._signed_in.append(source["id"])
        for launcher in self._launchers:
            if launcher["state"] == "waiting" and launcher["via"] == self._login.source:
                self._scan(launcher)
        if self._step_id() in ("stores", "done"):
            self._refresh()
        self.headChanged.emit()

    def _write(self, row, payload):
        if row["key"] == "controller.family":
            return self._controller.setFamily(payload)
        if row["key"] == "enabled" and row.get("module"):
            self._client.enableSource(row["module"], payload == "true")
            self._reread()
            return True
        ok = self._client.setConfig(row["key"], payload)
        if ok:
            self._prefs["config"] = self._client.config()
        return ok

    def _reload(self, row):
        self._refresh()

    def _head(self):
        step = self._step_id()
        if step != "done":
            return TITLES.get(step, ""), SUBTITLES.get(step, "")
        if self._running():
            return DONE_RUNNING
        if not self._summary():
            return DONE_EMPTY
        if not self._writable:
            return TITLES["done"], DONE_HOME_MANAGER if self._home_manager else DONE_READ_ONLY
        return TITLES["done"], SUBTITLES["done"]

    def _set_rows(self, rows, groups):
        super()._set_rows(rows, groups)
        self.idleChanged.emit()

    def _added(self):
        """Games came in, or are on their way: leaving the setup then skips nothing."""
        return any(launcher["state"] in RUNNING or (launcher["state"] == "imported" and launcher["count"]) for launcher in self._launchers)

    def _idle(self):
        """No row on the step does anything: the looks put the focus on Continue, or Finish."""
        return all(row["type"] in ("static", "info") for row in self._rows)

    needed = Property(bool, _needed, constant=True)
    steps = Property(list, lambda self: [dict(s) for s in self._steps], notify=stepChanged)
    step = Property(int, lambda self: self._step, notify=stepChanged)
    stepId = Property(str, _step_id, notify=stepChanged)
    title = Property(str, lambda self: self._head()[0], notify=headChanged)
    subtitle = Property(str, lambda self: self._head()[1], notify=headChanged)
    loading = Property(bool, lambda self: self._loading, notify=loadingChanged)
    idle = Property(bool, _idle, notify=idleChanged)
    added = Property(bool, _added, notify=headChanged)
