"""Rows for the per-game settings page and the Modules page, rendered generically by QML.

A row is {section, key, label, type, value, display, choices, module, detail, inherited};
`type` is one of bool, enum, string, path, int, info, action. Groups arrange the rows into
cards: {title, meta, warning, caps, control, off, rows}, `rows` and `control` indexing the
flat row list. QML picks the control by type and calls setValue(index, value) with the result.
"""

from PySide6.QtCore import Property, QObject, Signal, Slot


def _display(kind, value, choices=None):
    if kind == "bool":
        return "On" if value else "Off"
    if value is None or value == "" or value == [] or (kind in ("int", "string") and value == 0):
        return "—"
    if isinstance(value, list):
        return ", ".join(str(v) for v in value)
    return str(value)


def _row(section, key, label, kind, value, choices=None, module="", detail="", inherited=False):
    return {
        "section": section, "key": key, "label": label, "type": kind, "value": value,
        "display": _display(kind, value, choices), "choices": list(choices or []), "module": module,
        "detail": detail, "inherited": inherited,
    }


def _group(title, rows, meta="", warning="", caps=False, control=-1, off=False):
    return {"title": title, "meta": meta, "warning": warning, "caps": caps, "control": control,
            "off": off, "rows": list(rows)}


def _module_meta(module):
    parts = []
    if module.get("version"):
        parts.append(f"v{module['version']}")
    parts.append(" · ".join(module.get("kind") or []))
    return " · ".join(p for p in parts if p)


def _dig(data, dotted, default=None):
    node = data
    for part in dotted.split("."):
        if not isinstance(node, dict) or part not in node:
            return default
        node = node[part]
    return node


def _to_bus(kind, value):
    if kind == "bool":
        return "true" if value else "false"
    if isinstance(value, list):
        return ",".join(str(v) for v in value)
    return "" if value is None else str(value)


class RowsForm(QObject):
    rowsChanged = Signal()
    busyChanged = Signal()

    def __init__(self, client, parent=None):
        super().__init__(parent)
        self._client = client
        self._rows = []
        self._groups = []
        self._busy = False

    def _set_rows(self, rows, groups):
        self._rows = rows
        self._groups = groups
        self.rowsChanged.emit()

    @Slot(int, result="QVariant")
    def row(self, index):
        return self._rows[index] if 0 <= index < len(self._rows) else {}

    rows = Property("QVariantList", lambda self: list(self._rows), notify=rowsChanged)
    groups = Property("QVariantList", lambda self: list(self._groups), notify=rowsChanged)
    count = Property(int, lambda self: len(self._rows), notify=rowsChanged)
    busy = Property(bool, lambda self: self._busy, notify=busyChanged)


# Core keys Library1.Set accepts, grouped as the page shows them. Proton choices come from config.
CORE_ROWS = [
    ("Launch", "launch.proton", "Proton", "enum"),
    ("Launch", "launch.esync", "Esync", "bool"),
    ("Launch", "launch.fsync", "Fsync", "bool"),
    ("Launch", "launch.mangohud", "MangoHud", "bool"),
    ("Launch", "launch.args", "Arguments", "string"),
    ("Launch", "launch.working_dir", "Working directory", "path"),
    ("Desktop and library", "desktop.hide_cursor", "Hide the cursor while playing", "bool"),
    ("Desktop and library", "favorite", "Favourite", "bool"),
    ("Desktop and library", "hidden", "Hidden", "bool"),
    ("Desktop and library", "sort_title", "Sort title", "string"),
    ("Desktop and library", "tags", "Tags", "string"),
    ("Artwork", "metadata.sgdb_id", "SteamGridDB id", "string"),
    ("Artwork", "metadata.rawg_id", "RAWG id", "string"),
]


class GameSettingsForm(RowsForm):
    gameIdChanged = Signal()
    titleChanged = Signal()

    def __init__(self, client, parent=None):
        super().__init__(client, parent)
        self._game_id = ""
        self._title = ""

    @Slot(str)
    def load(self, game_id):
        self._game_id = game_id
        self.gameIdChanged.emit()
        game = self._client.game(game_id) or {}
        config = self._client.config() or {}
        self._title = str(game.get("title") or game_id)
        self.titleChanged.emit()
        rows = []
        groups = []
        effective = game.get("effective") or {}
        for section, key, label, kind in CORE_ROWS:
            value = _dig(game, key)
            # A launch or desktop key the game leaves empty takes the global value.
            inherited = False
            if value in (None, "") and key.startswith(("launch.", "desktop.")):
                value = effective.get(key.split(".", 1)[1])
                inherited = value not in (None, "")
            choices = []
            if key == "launch.proton":
                choices = sorted((config.get("proton") or {}).keys())
                default = _dig(config, "launch.proton", "")
                if default and default not in choices:
                    choices.insert(0, default)
                if not value and default:
                    value, inherited = default, True
            if kind == "bool":
                if value is None and key.startswith(("launch.", "desktop.")):
                    value, inherited = bool(_dig(config, key, False)), True
                value = bool(value)
            if not groups or groups[-1]["title"] != section:
                groups.append(_group(section, [], caps=True))
            groups[-1]["rows"].append(len(rows))
            rows.append(_row(section, key, label, kind, value, choices, inherited=inherited))
        modules = {m["id"]: m for m in self._client.modules() or []}
        for module_id, values in (self._client.settings(game_id) or {}).items():
            module = modules.get(module_id) or {}
            name = module.get("name", module_id)
            group = _group(name, [], meta=_module_meta(module))
            for setting in module.get("settings") or []:
                if setting.get("scope") != "game":
                    continue
                key = setting["key"]
                value = values.get(key, setting.get("default"))
                group["rows"].append(len(rows))
                rows.append(_row(name, key, setting.get("label", key),
                                 setting.get("type", "string"), value, setting.get("choices"), module_id))
            if group["rows"]:
                groups.append(group)
        self._set_rows(rows, groups)

    @Slot(int, "QVariant", result=bool)
    def setValue(self, index, value):
        if not (0 <= index < len(self._rows)):
            return False
        row = self._rows[index]
        payload = _to_bus(row["type"], value)
        if row["module"]:
            ok = self._client.setSetting(row["module"], self._game_id, row["key"], payload)
        else:
            ok = self._client.set(self._game_id, row["key"], payload)
        if ok:
            self.load(self._game_id)
        return bool(ok)

    @Slot(int)
    def toggle(self, index):
        row = self.row(index)
        if row.get("type") == "bool":
            self.setValue(index, not row.get("value"))

    gameId = Property(str, lambda self: self._game_id, notify=gameIdChanged)
    title = Property(str, lambda self: self._title, notify=titleChanged)


class ModulesForm(RowsForm):
    """Every module as a card: its enable toggle in the header, its global settings below;
    then Doctor's checks, one card per module."""

    doctorChanged = Signal()

    def __init__(self, client, parent=None):
        super().__init__(client, parent)
        self._doctor = []
        self._doctor_groups = []
        client.modulesChanged.connect(self.load)

    @Slot()
    def load(self):
        rows = []
        groups = []
        for module in self._client.modules() or []:
            ident = module["id"]
            name = module.get("name", ident)
            enabled = bool(module.get("enabled"))
            warning = ""
            if not module.get("available", True):
                missing = ", ".join(module.get("missing") or [])
                warning = "unavailable" + (f": missing {missing}" if missing else "")
            group = _group(name, [], meta=_module_meta(module), warning=warning, control=len(rows), off=not enabled)
            rows.append(_row(name, "enabled", "Enabled", "bool", enabled, module=ident))
            if enabled:
                values = self._client.getSettings(ident, "") or {}
                for setting in module.get("settings") or []:
                    if setting.get("scope") != "global":
                        continue
                    key = setting["key"]
                    group["rows"].append(len(rows))
                    rows.append(_row(name, key, setting.get("label", key), setting.get("type", "string"),
                                     values.get(key, setting.get("default")), setting.get("choices"), ident))
            groups.append(group)
        self._set_rows(rows, groups)

    @Slot()
    def loadDoctor(self):
        names = {m["id"]: m.get("name", m["id"]) for m in self._client.modules() or []}
        rows = []
        groups = []
        for check in self._client.doctor() or []:
            ident = check.get("module") or ""
            name = names.get(ident, ident) or "Core"
            group = next((g for g in groups if g["title"] == name), None)
            if group is None:
                group = _group(name, [])
                groups.append(group)
            group["rows"].append(len(rows))
            rows.append(_row(name, "", check.get("check", ""), "info", bool(check.get("ok")),
                             detail=str(check.get("detail") or ""), module=ident))
        groups.sort(key=lambda g: g["title"] != "Core")
        for group in groups:
            passed = sum(1 for i in group["rows"] if rows[i]["value"])
            group["meta"] = f"{passed} of {len(group['rows'])} checks pass"
        self._doctor = rows
        self._doctor_groups = groups
        self.doctorChanged.emit()

    @Slot(int, "QVariant", result=bool)
    def setValue(self, index, value):
        if not (0 <= index < len(self._rows)):
            return False
        row = self._rows[index]
        if row["key"] == "enabled":
            self._client.enableModule(row["module"], bool(value))
            ok = True
        else:
            ok = self._client.setSetting(row["module"], "", row["key"], _to_bus(row["type"], value))
        if ok:
            self.load()
        return bool(ok)

    @Slot(int)
    def toggle(self, index):
        row = self.row(index)
        if row.get("type") == "bool":
            self.setValue(index, not row.get("value"))

    doctor = Property("QVariantList", lambda self: list(self._doctor), notify=doctorChanged)
    doctorGroups = Property("QVariantList", lambda self: list(self._doctor_groups), notify=doctorChanged)
