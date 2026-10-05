import os
from datetime import datetime

from PySide6.QtCore import QObject, Signal, Slot

from ..qt import Property
from .media import _size, _when

OWNERS = {
    "universe": "Universe's prefix",
    "steam": "Steam's prefix",
    "lutris": "Lutris's prefix",
    "wine": "Wine's default prefix",
    "elsewhere": "A prefix of its own",
}
OWNER_DETAILS = {
    "steam": "Steam's compatdata: it stays where Steam keeps it, and its saves are backed up from there.",
    "wine": "Plain wine and other launchers share it: it stays where it is, and its saves are backed up from there.",
    "lutris": "Imported from Lutris. Moving it into Universe's prefixes keeps the game launchable.",
    "elsewhere": "Outside Universe's prefixes. Moving it there keeps the game launchable.",
}
ROOTS = {
    "games": "Games",
    "prefixes": "Wine prefixes",
    "saves": "Save backups",
    "recordings": "Recordings",
    "library": "Library data",
    "components": "Runners and tools",
    "logs": "Logs",
}
LEFTOVERS = {"prefix": "Prefix no game uses", "recordings": "Archived recordings", "game": "Removed game", "logs": "Removed game's logs"}
QUESTIONS = {
    "reset": ("Reset the prefix?", "Its saves are backed up first. The next launch makes a fresh prefix.", "Reset"),
    "restore": ("Restore this backup?", "The saves on disk now are replaced.", "Restore"),
    "move": ("Move the prefix into Universe's prefixes?", "Every game using it follows. Across drives this copies it, which can take a while.", "Move"),
    "trash": ("Move this to the trash?", "It can be brought back from the trash until it is emptied.", "Move to Trash"),
    "keep_local": ("Keep the saves on this device?", "The cloud's saves are replaced by these.", "Keep These"),
    "keep_cloud": ("Keep the cloud's saves?", "The saves on this device are backed up, then replaced.", "Keep the Cloud's"),
}
CLOUD_STATES = {
    "": "Not synced yet",
    "synced": "Synced",
    "conflict": "Changed on both sides",
    "offline": "Store out of reach",
    "error": "Not synced",
    "unsupported": "Not kept",
}
CLOUD_KEEPS = {"keep_local": "keep-local", "keep_cloud": "keep-cloud"}
KEEPS = {"lutris": "Lutris", "elsewhere": "Any other launcher using it"}
TOOLS = ("winecfg", "winetricks", "kill")


def _home(path):
    path = str(path or "")
    home = os.path.expanduser("~")
    return "~" + path[len(home) :] if path == home or path.startswith(home + "/") else path


def _head(label, display=""):
    return {"heading": True, "key": "", "label": label, "display": display, "type": "heading"}


def _static(key, label, display, detail=""):
    return {"key": key, "label": label, "type": "static", "display": display, "detail": detail}


def _action(key, label, display="", detail="", disabled=False, danger=False):
    return {"key": key, "label": label, "type": "action", "display": display, "detail": detail, "disabled": disabled, "danger": danger}


def _files(n):
    return "1 file" if n == 1 else f"{n} files"


def _day(value):
    try:
        when = datetime.fromisoformat(str(value)).astimezone()
    except ValueError:
        return str(value or "")
    return _when(when.isoformat())


def game_rows(data):
    rows = []
    saves = data.get("saves") or {}
    backups = saves.get("backups") or []
    files = saves.get("files") or []
    rows.append(_head("Saves", _size(saves.get("bytes")) if files else ""))
    if not saves.get("engine"):
        rows.append(_static("saves_folder", "Save folder", "Every game's", _home(saves.get("folder"))))
    else:
        if saves.get("error"):
            rows.append(_static("saves_status", "Saves", "Not found", saves["error"]))
        else:
            where = os.path.commonpath([f["path"] for f in files]) if files else ""
            rows.append(_static("saves_status", saves.get("name") or "Saves", f"{_files(len(files))} · {_size(saves.get('bytes'))}", _home(where)))
        keep = int(saves.get("keep") or 5)
        last = f"Last {_day(backups[0]['when'])}" if backups else "None yet"
        detail = f"Kept in {_home(saves.get('dir'))}, the last {keep}" + (", one after each session." if saves.get("auto") else ".")
        rows.append(_action("backup", "Back Up Now", last, detail))
        rows.append(_action("restore", "Restore the Latest Backup", "", "The saves on disk now are replaced.", disabled=not backups))
        rows.append(_action("export", "Export the Backups", "", "One zip in your home folder.", disabled=not backups))
    rows.extend(_cloud_rows(saves.get("cloud")))
    if saves.get("engine") and backups:
        rows.append(_head("Backups", _size(saves.get("backups_bytes"))))
        rows.extend(_action(f"restore:{b['id']}", _day(b["when"]), _size(b["bytes"]), "") for b in backups)
    prefix = data.get("prefix")
    if prefix:
        rows.append(_head("Wine Prefix", _size(prefix.get("bytes"))))
        shared = ", ".join(prefix.get("shared_with") or [])
        detail = _home(prefix.get("path")) + (f" · shared with {shared}" if shared else "")
        rows.append(_static("prefix", OWNERS.get(prefix.get("owner"), "Prefix"), _size(prefix.get("bytes")), detail))
        if prefix.get("owner") in OWNER_DETAILS:
            rows[-1]["secondary"] = OWNER_DETAILS[prefix["owner"]]
        if prefix.get("movable"):
            rows.append(_action("move", "Move into Universe's Prefixes", "", _home(prefix.get("target"))))
        rows.append(_action("winecfg", "Wine Configuration", "", "winecfg in this prefix."))
        rows.append(_action("winetricks", "Winetricks", "", "Install runtimes and fonts into this prefix."))
        rows.append(_action("run", "Run a Program in the Prefix…", "", "An installer or a tool, in this game's prefix."))
        rows.append(_action("kill", "Stop the Prefix's Programs", "", "Ends wineserver and everything running in this prefix."))
        if prefix.get("owner") == "universe" and not shared:
            rows.append(_action("reset", "Reset the Prefix…", "", "Its saves are backed up first.", danger=True))
    rows.append(_head("Storage", _size(data.get("total"))))
    install = data.get("install")
    if install:
        rows.append(_static("install", "Install folder", _size(install.get("bytes")), _home(install.get("path"))))
    universe = data.get("universe") or {}
    parts = universe.get("parts") or {}
    detail = " · ".join(f"{label} {_size(parts.get(key))}" for key, label in (("media", "Artwork"), ("screenshots", "Screenshots"), ("journal", "Journal")))
    rows.append(_static("universe", "Universe's files", _size(universe.get("bytes")), detail))
    recordings = data.get("recordings") or {}
    rows.append(_static("recordings", "Recordings", _size(recordings.get("bytes")), _home(recordings.get("path"))))
    logs = data.get("logs") or {}
    if logs.get("bytes"):
        rows.append(_static("logs", "Logs", _size(logs.get("bytes")), _home(logs.get("path"))))
    return rows


def _cloud_rows(cloud):
    if not isinstance(cloud, dict):
        return []
    state = str(cloud.get("state") or "")
    display = CLOUD_STATES.get(state, state)
    if state == "synced" and cloud.get("at"):
        display = f"Synced {_day(cloud['at'])}"
    places = [_home(p.get("path")) for p in cloud.get("locations") or [] if isinstance(p, dict)]
    detail = cloud.get("message") or " · ".join(places)
    if not cloud.get("enabled") and state != "conflict":
        display = "Off"
        detail = "Sessions sync once Cloud saves is on in the store's settings, for every game or this one."
    rows = [_static("cloud_status", "Cloud Saves", display, detail)]
    if state == "conflict":
        rows.append(_action("keep_local", "Keep the Saves on This Device", "", "The cloud's saves are replaced."))
        rows.append(_action("keep_cloud", "Keep the Cloud's Saves", "", "The saves here are backed up, then replaced."))
    return rows


def storage_rows(storage):
    rows = [_head("Folders")]
    for root in storage.get("roots") or []:
        free = f" · {_size(root['free'])} free" if root.get("free") else ""
        rows.append(_static(f"root:{root['id']}", ROOTS.get(root["id"], root["id"]), _size(root.get("bytes")) + free, _home(root.get("path"))))
    games = storage.get("games") or []
    rows.append(_head("Games", _size(sum(g.get("bytes", 0) for g in games))))
    for g in games:
        parts = [(label, g.get(key, 0)) for key, label in (("install", "Install"), ("prefix", "Prefix"), ("saves", "Saves"), ("recordings", "Recordings"))]
        detail = " · ".join(f"{label} {_size(n)}" for label, n in parts if n)
        row = _action(f"game:{g['id']}", g.get("title") or g["id"], _size(g.get("bytes")), detail)
        row["gameId"] = g["id"]
        rows.append(row)
    leftovers = storage.get("leftovers") or []
    rows.append(_head("Leftovers", _size(storage.get("leftover_bytes")) if leftovers else "None"))
    for item in leftovers:
        label = LEFTOVERS.get(item["kind"], item["kind"])
        if item.get("title"):
            label = f"{label}: {item['title']}"
        row = _action(f"trash:{item['path']}", label, _size(item.get("bytes")), _home(item["path"]))
        row["path"] = ""
        row["target"] = item["path"]
        rows.append(row)
    return rows


class GameData(QObject):
    changed = Signal()
    busyChanged = Signal()
    # (action key, ok, message): every look's toast
    finished = Signal(str, bool, str)

    def __init__(self, client, parent=None):
        super().__init__(parent)
        self._client = client
        self._game_id = ""
        self._data = {}
        self._rows = []
        self._loading = False
        self._error = ""
        self._busy = ""
        self._generation = 0
        client.libraryChanged.connect(lambda ids: self._game_id in ids and self.reload())

    @Slot(str)
    def load(self, game_id):
        if game_id != self._game_id:
            self._data, self._rows = {}, []
        self._game_id = game_id
        self.reload()

    @Slot()
    def reload(self):
        if not self._game_id:
            return
        self._generation += 1
        generation = self._generation
        self._loading, self._error = True, ""
        self.changed.emit()

        def landed(data):
            if generation != self._generation:
                return
            self._data, self._rows, self._loading = data or {}, game_rows(data or {}), False
            self.changed.emit()

        def missed(e):
            if generation != self._generation:
                return
            self._loading, self._error = False, e.message
            self.changed.emit()

        self._client.gameDataAsync(self._game_id, landed, missed)

    @Slot()
    def unload(self):
        self._generation += 1
        self._game_id, self._data, self._rows, self._loading, self._error = "", {}, [], False, ""
        self.changed.emit()

    @Slot(str, result="QVariant")
    def question(self, key):
        kind = "restore" if key.startswith("restore") else key
        if kind not in QUESTIONS:
            return None
        title, detail, confirm = QUESTIONS[kind]
        asked = {"title": title, "detail": detail, "confirm": confirm, "danger": kind in ("reset", "restore", *CLOUD_KEEPS)}
        prefix = self._data.get("prefix") or {}
        who = KEEPS.get(str(prefix.get("owner") or ""))
        if kind == "move" and who:
            asked["detail"] = f"{detail} {who} keeps pointing at the old path, {_home(prefix.get('path'))}."
            asked["stale"] = str(prefix.get("path") or "")
        return asked

    def _run(self, key, start, said):
        if self._busy:
            return False
        self._busy = key
        self.busyChanged.emit()
        self.changed.emit()
        ident = self._game_id

        def done(reply):
            self._busy = ""
            self.busyChanged.emit()
            self.changed.emit()
            self.finished.emit(key, True, said(reply))
            if ident == self._game_id:
                self.reload()

        def failed(e):
            self._busy = ""
            self.busyChanged.emit()
            self.changed.emit()
            self.finished.emit(key, False, e.message)

        start(ident, done, failed)
        return True

    @Slot(str, result=bool)
    def act(self, key):
        c = self._client
        if key == "backup":
            return self._run(key, c.savesBackupAsync, _backed_up)
        if key == "restore" or key.startswith("restore:"):
            backup = key.partition(":")[2]
            return self._run(key, lambda i, d, f: c.savesRestoreAsync(i, backup, d, f), lambda r: f"Restored {_files(len(r.get('files') or []))}")
        if key == "export":
            return self._run(key, lambda i, d, f: c.savesExportAsync(i, "~", d, f), lambda path: f"Exported to {_home(path)}")
        if key == "move":
            return self._run(key, c.movePrefixAsync, lambda r: f"Moved to {_home(r.get('to'))}")
        if key == "reset":
            return self._run(key, c.resetPrefixAsync, lambda r: "Prefix reset, saves backed up" if r.get("backup") else "Prefix reset")
        if key in TOOLS:
            return self._run(key, lambda i, d, f: c.prefixToolAsync(i, key, [], d, f), _tool_done)
        if key in CLOUD_KEEPS:
            kept = "Kept the saves on this device" if key == "keep_local" else "Kept the cloud's saves"
            return self._run(key, lambda i, d, f: c.savesCloudAsync(i, CLOUD_KEEPS[key], d, f), lambda _: kept)
        return False

    @Slot(str, result=bool)
    def runProgram(self, path):
        c = self._client
        name = os.path.basename(path)
        return self._run("run", lambda i, d, f: c.prefixToolAsync(i, "run", [path], d, f), lambda _: f"{name} started")

    gameId = Property(str, lambda self: self._game_id, notify=changed)
    title = Property(str, lambda self: str(self._data.get("title") or ""), notify=changed)
    total = Property(str, lambda self: _size(self._data.get("total")) if self._data else "", notify=changed)

    def _shown(self):
        return [{**r, "display": "Working…", "disabled": True} if r["key"] and r["key"] == self._busy else dict(r) for r in self._rows]

    rows = Property(list, _shown, notify=changed)
    count = Property(int, lambda self: len(self._rows), notify=changed)
    loading = Property(bool, lambda self: self._loading, notify=changed)
    error = Property(str, lambda self: self._error, notify=changed)
    busy = Property(str, lambda self: self._busy, notify=busyChanged)
    hasPrefix = Property(bool, lambda self: bool(self._data.get("prefix")), notify=changed)


def _tool_done(reply):
    if "stopped" in reply:
        return "The prefix's programs stopped" if reply["stopped"] else "Nothing was running in the prefix"
    return {"winecfg": "Wine Configuration", "winetricks": "Winetricks"}[reply["tool"]] + " started"


def _backed_up(reply):
    change = reply.get("change")
    if change == "same":
        return "Saves unchanged since the last backup"
    if change == "none":
        return "No saves found"
    return f"Backed up {_files(len(reply.get('files') or []))}"


class Storage(QObject):
    changed = Signal()
    finished = Signal(str, bool, str)

    def __init__(self, client, parent=None):
        super().__init__(parent)
        self._client = client
        self._data = {}
        self._rows = []
        self._loading = False
        self._error = ""
        self._generation = 0
        self._roots = []

    @Slot()
    def loadFree(self):
        def landed(roots):
            self._roots = roots or []
            self.changed.emit()

        self._client.diskFreeAsync(landed)

    @Slot()
    def load(self):
        self._generation += 1
        generation = self._generation
        self._loading, self._error = True, ""
        self.changed.emit()

        def landed(data):
            if generation != self._generation:
                return
            self._data, self._rows, self._loading = data or {}, storage_rows(data or {}), False
            self.changed.emit()

        def missed(e):
            if generation != self._generation:
                return
            self._loading, self._error = False, e.message
            self.changed.emit()

        self._client.storageAsync(landed, missed)

    @Slot()
    def unload(self):
        self._generation += 1
        self._data, self._rows, self._loading = {}, [], False
        self.changed.emit()

    @Slot(str, result="QVariant")
    def question(self, key):
        if not key.startswith("trash:"):
            return None
        title, detail, confirm = QUESTIONS["trash"]
        return {"title": title, "detail": detail, "confirm": confirm, "danger": True}

    @Slot(str, result=bool)
    def act(self, key):
        if not key.startswith("trash:"):
            return False
        path = key.partition(":")[2]

        def done(_):
            self.finished.emit(key, True, f"Moved {_home(path)} to the trash")
            self.load()

        self._client.trashLeftoverAsync(path, done, lambda e: self.finished.emit(key, False, e.message))
        return True

    def _free(self, root):
        r = next((r for r in self._data.get("roots") or self._roots if r["id"] == root), None)
        return _size(r["free"]) if r and r.get("free") else ""

    rows = Property(list, lambda self: [dict(r) for r in self._rows], notify=changed)
    count = Property(int, lambda self: len(self._rows), notify=changed)
    loading = Property(bool, lambda self: self._loading, notify=changed)
    error = Property(str, lambda self: self._error, notify=changed)
    used = Property(str, lambda self: _size(sum(r.get("bytes", 0) for r in self._data.get("roots") or [])) if self._data else "", notify=changed)
    free = Property(str, lambda self: self._free("games"), notify=changed)
    leftovers = Property(str, lambda self: _size(self._data.get("leftover_bytes")) if self._data.get("leftovers") else "", notify=changed)
