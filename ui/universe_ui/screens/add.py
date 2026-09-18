import os

from PySide6.QtCore import Property, Signal, Slot

from .runners import _found, suggested_title
from .settings import RowsForm, _group, _row

LINUX_EXTENSIONS = {"", "sh", "x86_64", "x86", "appimage"}


def runner_candidates(runners, path):
    path = str(path or "").rstrip("/")
    ext = "" if os.path.isdir(path) else os.path.splitext(path)[1].lstrip(".").casefold()

    def matches(runner):
        if runner.get("kind") == "linux":
            return ext in LINUX_EXTENSIONS
        return ext in {e.casefold() for e in runner.get("extensions") or []}

    kinds = {"proton": 0, "wine": 1}
    return sorted(runners, key=lambda r: (not matches(r), not _found(r), kinds.get(r.get("kind"), 2), r.get("name", r["id"]).lower()))


def _source_status(source):
    if not source.get("available", True) or not source.get("enabled", True):
        return "Not set up", "Set up"
    if not source.get("logged_in"):
        return "Sign in to install games", "Sign in"
    user = source.get("user") or ""
    return f"Signed in as {user}" if user else "Signed in", "Open"


class AddGameForm(RowsForm):
    message = Signal(str)
    pendingChanged = Signal()
    lutrisChanged = Signal()

    def __init__(self, client, parent=None):
        super().__init__(client, parent)
        self._pending = None
        self._candidates = []
        self._runner = -1
        self._lutris = None
        self._lutris_error = ""

    @Slot()
    def load(self):
        rows = [{**_row("Add a game", "pick_file", "Pick a game file…", "action", ""), "display": "", "action": "Pick a file",
                 "detail": "A program or a ROM: the runner and the title are proposed from the file."}]
        groups = [_group("", [0])]
        stores = []
        for source in self._client.sources():
            status, action = _source_status(source)
            row = _row("Stores", "store", source.get("name", source["id"]), "action", "", module=source["id"])
            row.update(display=status, action=action, source=source["id"], loggedIn=bool(source.get("logged_in")),
                       available=bool(source.get("available", True) and source.get("enabled", True)))
            stores.append(len(rows))
            rows.append(row)
        if stores:
            groups.append(_group("Stores", stores, caps=True))
        rows.append({**_row("Lutris", "lutris", "Import from Lutris", "action", ""), "display": self._lutris_display(), "action": "Import",
                     "detail": "Lutris's games, with their hours and artwork; games already here are kept."})
        groups.append(_group("Lutris", [len(rows) - 1], caps=True))
        self._set_rows(rows, groups)

    def _lutris_display(self):
        if self._lutris_error:
            return "Not found"
        if self._lutris is None:
            return ""
        n = len(self._lutris.get("imported") or [])
        return f"{n} game{'' if n == 1 else 's'} to import" if n else "Nothing new"

    @Slot(str, result=bool)
    def setFile(self, path):
        path = str(path or "")
        if not path:
            return False
        self._candidates = runner_candidates(self._client.runners(), path)
        self._pending = {"file": path}
        self._runner = 0 if self._candidates else -1
        self.pendingChanged.emit()
        return True

    @Slot(int)
    def pickRunner(self, index):
        if 0 <= index < len(self._candidates):
            self._runner = index
            self.pendingChanged.emit()

    @Slot(result=str)
    def pendingTitle(self):
        return suggested_title(self._pending["file"]) if self._pending else ""

    @Slot(str, result=str)
    def addGame(self, title):
        if not self._pending or self._runner < 0:
            return ""
        runner = self._candidates[self._runner]
        pending, self._pending = self._pending, None
        self.pendingChanged.emit()
        ident = self._client.addGame(runner["id"], pending["file"], title.strip())
        if ident:
            self.message.emit(f"Added {title.strip() or ident} through {runner.get('name', runner['id'])}")
        return ident

    @Slot()
    def cancel(self):
        self._pending = None
        self.pendingChanged.emit()

    @Slot()
    def previewLutris(self):
        if self._busy:
            return

        def done(report, error):
            self._lutris, self._lutris_error = (report or None), error
            self.lutrisChanged.emit()
            self.load()

        self._run(lambda: self._client.core.import_lutris(False), done)

    @Slot()
    def importLutris(self):
        if self._busy or not self._lutris:
            return

        def done(report, error):
            self._lutris, self._lutris_error = None, error
            self.lutrisChanged.emit()
            self.load()
            if error:
                self.message.emit(f"Lutris import failed: {error}")
                return
            imported = list(report.get("imported") or [])
            hours = round(sum(float(h) for h in (report.get("hours_imported") or {}).values()))
            self._client.libraryChanged.emit([])
            self.message.emit(f"Imported {len(imported)} game{'' if len(imported) == 1 else 's'} from Lutris"
                              + (f" · {hours} h of play" if hours else ""))

        self._run(lambda: self._client.core.import_lutris(True), done)

    pendingFile = Property(str, lambda self: self._pending["file"] if self._pending else "", notify=pendingChanged)
    runnerChoices = Property("QVariantList", lambda self: [r.get("name", r["id"]) for r in self._candidates], notify=pendingChanged)
    runnerIds = Property("QVariantList", lambda self: [r["id"] for r in self._candidates], notify=pendingChanged)
    runnerIndex = Property(int, lambda self: self._runner, notify=pendingChanged)
    lutris = Property("QVariant", lambda self: dict(self._lutris) if self._lutris else None, notify=lutrisChanged)
    lutrisError = Property(str, lambda self: self._lutris_error, notify=lutrisChanged)
