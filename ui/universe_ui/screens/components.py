from PySide6.QtCore import QTimer, Signal, Slot

from ..qt import QVARIANT, Property
from .media import _size
from .onboarding import _plural
from .settings import RowsForm, _group, _row, runner_logo
from .sources import _age, _free_space

KINDS = (("proton", "Proton"), ("wine", "Wine"), ("emulator", "Emulators"), ("tool", "Tools"), ("system", "System"))
ORIGINS = {
    "universe": "Universe",
    "nix": "Nix",
    "system": "system",
    "lutris": "Lutris",
    "steam": "Steam",
    "heroic": "Heroic",
    "umu": "umu",
    "local": "your proton folder",
    "config": "config.toml",
}
NO_BUILD = "No download"
NO_BUILD_META = "Install these from your distribution: upstream ships no build Universe can fetch"
FIRST_CHECK_MS = 90_000
CHECK_EVERY_MS = 24 * 3600 * 1000


def _origin(build):
    return ORIGINS.get(build.get("origin", ""), build.get("origin", ""))


def build_text(build):
    return f"{build.get('version') or 'unknown version'} · {_origin(build)}"


def _icon(component):
    if component["kind"] in ("emulator", "wine"):
        return runner_logo(component["id"]) or "play"
    if component["kind"] == "proton":
        return runner_logo("proton") or "play"
    return "terminal"


def _ask(message, detail, yes, no="Not now"):
    return {"message": message, "detail": detail, "yes": yes, "no": no}


def _latest(component):
    return component.get("latest") or {}


def _managed(component):
    return [b for b in component.get("builds") or [] if b.get("managed")]


def _found(component):
    return [b for b in component.get("builds") or [] if not b.get("managed")]


def _tag(component, busy):
    if busy:
        return "Installing…"
    if component.get("update"):
        return f"Update {component['update']}"
    if component.get("proposal") == "newer":
        return f"{_latest(component).get('version')} available"
    if component.get("proposal") == "install":
        return "Needed"
    if component.get("recent"):
        return "Updated"
    return ""


def _detail(component):
    parts = []
    used = int(component.get("used_by") or 0)
    if used:
        parts.append(f"Runs {_plural(used, 'game')}.")
    in_use, latest = component.get("in_use"), _latest(component)
    if component["kind"] == "system" and not in_use:
        packages = " and ".join(component.get("packages") or [])
        parts.append(
            f"Not installed: Universe can install {packages} from your distribution, asking your password."
            if component.get("installable")
            else component.get("fix", "")
        )
    elif in_use and in_use.get("managed"):
        parts.append("Installed by Universe, which keeps it up to date and the previous build to go back to.")
    elif in_use:
        parts.append(f"From {_origin(in_use)}.")
        if component.get("proposal") == "newer":
            parts.append(f"Universe can install {latest.get('version')} beside it and use that one.")
    elif latest:
        parts.append(f"Not installed: Universe can install {latest.get('version')}.")
    else:
        parts.append("Not installed, and there is no build Universe can download: install it from your distribution.")
    return " ".join(parts)


def component_row(component, job):
    busy = bool(job) and job.get("ok") is None and job.get("component") == component["id"]
    in_use, latest = component.get("in_use"), _latest(component)
    display = build_text(in_use) if in_use else ("Not installed" if latest or component["kind"] == "system" else NO_BUILD)
    tag = _tag(component, busy)
    wants = bool(component.get("update") or component.get("proposal")) or not in_use
    row = _row(component["kind"], "component", component["name"], "action", display, module=component["id"], detail=_detail(component))
    row.update(
        display=display,
        icon=_icon(component),
        iconSlot=True,
        tag=tag,
        accent=bool(tag),
        size=_size(latest["size"]) if latest.get("size") and wants and latest else "",
        action="Cancel" if busy else "Options",
        component=component["id"],
        progress=(job["done"] / job["total"]) if busy and job.get("total") else 0,
    )
    return row


def build_components(listing, job):
    components = listing.get("components") or []
    catalogue = listing.get("catalogue") or {}
    rows, groups = [], []
    recent = [c for c in components if c.get("recent")]
    if recent:
        at = len(rows)
        for c in recent:
            row = _row("Recently updated", "recent", c["name"], "action", c["recent"]["version"], module=c["id"])
            row.update(display=f"{c['recent']['version']} · {_age(c['recent']['at'])}", icon=_icon(c), iconSlot=True, action="Options", component=c["id"])
            rows.append(row)
        groups.append(_group("Recently updated", range(at, len(rows)), meta="Roll one back from its options"))
    idle = []
    for kind, title in KINDS:
        members = [c for c in components if c["kind"] == kind]
        live = [c for c in members if kind == "system" or c.get("in_use") or c.get("latest") or c.get("builds")]
        idle.extend(c for c in members if c not in live)
        if not live:
            continue

        def rank(c):
            urgency = 0 if c.get("proposal") == "install" else 1 if c.get("proposal") or c.get("update") else 2
            return (urgency, not c.get("in_use"), -int(c.get("used_by") or 0), c["name"].lower())

        at = len(rows)
        rows.extend(component_row(c, job) for c in sorted(live, key=rank))
        installed = sum(1 for c in live if c.get("in_use"))
        groups.append(_group(title, range(at, len(rows)), meta=f"{installed} of {len(live)} installed"))
    if catalogue.get("error"):
        age = _age(catalogue.get("fetched_at") or "")
        warning = "The catalogue could not be reached" + (f" · listing from {age}" if age else "")
        if groups:
            groups[0]["warning"] = warning
    if idle:
        at = len(rows)
        rows.extend(component_row(c, job) for c in sorted(idle, key=lambda c: c["name"].lower()))
        groups.append(_group(NO_BUILD, range(at, len(rows)), meta=NO_BUILD_META, off=True))
    return rows, groups


def actions(component, busy):
    if busy:
        return [{"icon": "stop", "label": "Cancel", "action": "cancel", "danger": True}]
    if component["kind"] == "system":
        installable = component.get("installable") and not component.get("in_use")
        return [{"icon": "download", "label": "Install from the distribution", "action": "install"}] if installable else []
    out = []
    managed, latest = _managed(component), _latest(component)
    have = {b["version"] for b in managed}
    runner = component["kind"] in ("emulator", "wine")
    switchable = component["kind"] != "tool"
    if component.get("update"):
        out.append({"icon": "refresh", "label": f"Update to {component['update']}", "action": "update"})
    elif latest and latest.get("version") not in have:
        use = switchable and component.get("in_use") is not None
        out.append({"icon": "download", "label": f"Install {latest['version']}" + (" and use it" if use else ""), "action": "install"})
    if switchable:
        for b in component.get("builds") or []:
            if b.get("in_use"):
                continue
            target = b["version"] if b.get("managed") else ("system" if runner else b.get("name") or b["version"])
            out.append({"icon": "play", "label": f"Use {build_text(b)}", "action": "use:" + target})
    if component["kind"] == "proton" and component.get("family") and component.get("setting") not in ("", component["family"]) and managed:
        out.append({"icon": "play", "label": f"Follow the newest {component['name']}", "action": "use:latest"})
    others = [a for a in component.get("available") or [] if not a.get("installed") and a.get("version") != latest.get("version")]
    if others:
        out.append({"icon": "plus", "label": "Install another version…", "action": "versions"})
    newest = managed[0] if managed else None
    if newest and (len(managed) > 1 or _found(component)) and not newest.get("pinned"):
        out.append({"icon": "refresh", "label": f"Roll back {newest['version']}", "action": "rollback", "danger": True})
    for b in managed:
        if not b.get("pinned") and not b.get("in_use"):
            size = f" · {_size(b['disk'])}" if b.get("disk") else ""
            out.append({"icon": "trash", "label": f"Remove {b['version']}{size}", "action": "remove:" + b["version"], "danger": True})
    if runner:
        out.append({"icon": "sliders", "label": "Runner settings", "action": "runner"})
    return out


def version_actions(component):
    out = []
    for a in component.get("available") or []:
        if a.get("installed") or a.get("version") == _latest(component).get("version"):
            continue
        parts = [a["version"], _size(a["size"]) if a.get("size") else "", (a.get("date") or "")[:10], "rolled back" if a.get("skipped") else ""]
        out.append({"icon": "download", "label": " · ".join(p for p in parts if p), "action": "install:" + a["version"]})
    return out


class ComponentsForm(RowsForm):
    message = Signal(str)
    jobChanged = Signal()
    listingChanged = Signal()
    runnerRequested = Signal(str)
    # gameId, component id, name, version
    installProposed = Signal(str, str, str, str)
    readyToLaunch = Signal(str)

    def __init__(self, client, parent=None):
        super().__init__(client, parent)
        self._listing = {}
        self._job = None
        self._after = None
        self._launch = ""
        client.progress.connect(self._on_progress)
        client.jobFinished.connect(self._on_job_finished)
        client.launchFailed.connect(self._on_launch_failed)
        self._timer = QTimer(self)
        self._timer.setSingleShot(True)
        self._timer.timeout.connect(self._auto_update)
        self._timer.start(FIRST_CHECK_MS)

    def _show(self):
        rows, groups = build_components(self._listing, self._job)
        self._set_rows(rows, groups)
        self.listingChanged.emit()

    def _fetch(self, refresh):
        def done(listing, error):
            if error:
                self.message.emit(f"Components: {error}")
            self._listing = listing or self._listing
            self._show()

        self._run(lambda: self._client.components(refresh), done)

    @Slot()
    def load(self):
        self._fetch(False)

    @Slot()
    def refresh(self):
        self._fetch(True)

    def _component(self, index):
        return self._by_id(self.row(index).get("component", ""))

    def _by_id(self, ident):
        return next((c for c in self._listing.get("components") or [] if c["id"] == ident), None)

    def busyOn(self, ident):
        return bool(self._job) and self._job["ok"] is None and self._job["component"] == ident

    @Slot(int, result="QVariant")
    def actions(self, index):
        c = self._component(index)
        return actions(c, self.busyOn(c["id"])) if c else []

    @Slot(int, result="QVariant")
    def versionActions(self, index):
        c = self._component(index)
        return version_actions(c) if c else []

    @Slot(int, str, result="QVariant")
    def confirm(self, index, action):
        c = self._component(index)
        if not c:
            return None
        if action == "install" and c["kind"] == "system":
            packages = ", ".join(c.get("packages") or [])
            return _ask(f"Install {c['name']}?", f"{packages} from your distribution's packages. It asks for your password.", "Install")
        if action == "install" or action.startswith("install:"):
            version = action.partition(":")[2] or _latest(c).get("version", "")
            build = next((a for a in c.get("available") or [] if a["version"] == version), {})
            return self._install_question(c, version, build.get("size", 0))
        if action.startswith("remove:"):
            return _ask(f"Remove {c['name']} {action.partition(':')[2]}?", "Universe can install it again later.", "Remove", "Keep it")
        if action == "rollback":
            message = f"Roll back {c['name']} {_managed(c)[0]['version']}?"
            return _ask(message, "Universe removes it and skips that version; the next one updates as usual.", "Roll back", "Keep it")
        return None

    def _install_question(self, component, version, size):
        free = _free_space(self._client.core.data_home())
        parts = [f"{_size(size)} to download" if size else "", f"{_size(free)} free" if free else ""]
        return _ask(f"Install {component['name']} {version}?", " · ".join(p for p in parts if p), "Install")

    @Slot(int, str, result=bool)
    def act(self, index, action):
        c = self._component(index)
        return bool(c) and self._act(c, action)

    def _act(self, c, action):
        ident, verb, arg = c["id"], *action.partition(":")[::2]
        if verb == "cancel":
            if not self._job or not self._client.cancel(self._job["id"]):
                return False
            self._job.update({"cancelled": True, "message": f"Stopping {self._job['title']}…"})
            self.jobChanged.emit()
            return True
        if verb == "runner":
            self.runnerRequested.emit(ident)
            return True
        if verb in ("install", "update"):
            follow = verb == "install" and c["kind"] != "tool" and c.get("in_use") is not None and not arg
            label = f"Updating {c['name']}" if verb == "update" else f"Installing {c['name']} {arg or _latest(c).get('version', '')}".strip()
            job = self._client.componentUpdate(ident) if verb == "update" else self._client.componentInstall(ident, arg)
            return self._begin(job, c, label, "latest" if follow else "")
        if verb == "use":
            ok = self._client.componentUse(ident, arg)
            if ok:
                self.message.emit(f"{c['name']}: " + ("the newest build" if arg == "latest" else "the system's" if arg == "system" else arg))
                self.load()
            return ok
        if verb == "remove":
            return self._after_call(lambda: self._client.componentRemove(ident, arg), f"Removed {c['name']} {arg}")
        if verb == "rollback":
            return self._after_call(lambda: self._client.componentRollback(ident), f"Rolled back {c['name']}")
        return False

    def _after_call(self, call, text):
        def done(ok, error):
            self.message.emit(error or text)
            self.load()

        self._run(call, done)
        return True

    def _begin(self, job_id, component, label, follow=""):
        if not job_id:
            return False
        if self._job and self._job["ok"] is None:
            self.message.emit(f"{self._job['label']} first: cancel it or wait")
            return False
        self._job = {
            "id": job_id,
            "component": component["id"],
            "title": component["name"],
            "label": label,
            "message": label,
            "done": 0,
            "total": 0,
            "ok": None,
        }
        self._after = follow
        self.jobChanged.emit()
        self._show()
        return True

    def needed(self):
        return [c for c in self._listing.get("components") or [] if c.get("proposal") == "install" and c["kind"] in ("proton", "wine", "emulator")]

    @Slot(str, result=bool)
    def installById(self, ident):
        c = self._by_id(ident)
        if c is None:
            self.load()
            return False
        return self._act(c, "install")

    @Slot(str, str, result=bool)
    def installFor(self, game_id, ident):
        c = self._by_id(ident)
        if c is None or not self._act(c, "install"):
            return False
        self._launch = game_id
        return True

    @Slot(str, result="QVariant")
    def question(self, ident):
        c = self._by_id(ident)
        return self._install_question(c, _latest(c).get("version", ""), _latest(c).get("size", 0)) if c else None

    def _on_progress(self, job_id, done, total, message):
        if not self._job or self._job["id"] != job_id:
            return
        self._job.update({"done": int(done), "total": int(total), "message": message or self._job["label"]})
        self.jobChanged.emit()

    def _finished_text(self, ok, text):
        job = self._job or {}
        if job.get("quiet"):
            return f"Updated {text}" if ok and text else ""
        if not ok:
            return "Stopped " + job["label"][0].lower() + job["label"][1:] if job.get("cancelled") else text
        if job["label"].startswith("Updating"):
            return f"Updated {text}" if text else f"{job['title']} is up to date"
        return "Installed" + job["label"].removeprefix("Installing")

    def _on_job_finished(self, job_id, ok, text):
        if not self._job or self._job["id"] != job_id:
            return
        finished = self._finished_text(ok, text)
        self._job.update({"ok": bool(ok), "message": finished or self._job["message"]})
        self.jobChanged.emit()
        if ok and self._after:
            self._client.componentUse(self._job["component"], self._after)
        self._after = None
        if finished:
            self.message.emit(finished)
        if ok and self._launch:
            self.readyToLaunch.emit(self._launch)
        self._launch = ""
        self.load()

    def _on_launch_failed(self, game_id, message):
        if "not found" not in message:
            return
        game = self._client.game(game_id) or {}
        runner = str((game.get("effective") or {}).get("runner") or "")
        c = self._by_id(runner)
        if c is None or not _latest(c) or c.get("in_use"):
            return
        self.installProposed.emit(game_id, c["id"], c["name"], _latest(c).get("version", ""))

    def _auto_update(self):
        self._timer.start(CHECK_EVERY_MS)
        config = self._client.config() or {}
        running = self._client.currentSession or (self._job and self._job["ok"] is None)
        if not (config.get("components") or {}).get("auto_update", True) or running:
            return
        job = self._client.componentUpdate("")
        if job:
            self._job = {"id": job, "component": "", "title": "", "label": "Updating", "message": "", "done": 0, "total": 0, "ok": None, "quiet": True}

    def shutdown(self):
        self._timer.stop()

    job = Property(QVARIANT, lambda self: dict(self._job) if self._job and not self._job.get("quiet") else None, notify=jobChanged)
    pending = Property(int, lambda self: sum(1 for c in self._listing.get("components") or [] if c.get("update") or c.get("proposal")), notify=listingChanged)
