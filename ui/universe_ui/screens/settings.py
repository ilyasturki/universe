"""Rows for the per-game settings page and the Modules page, rendered generically by QML.

A row is {section, key, label, type, value, display, choices, module}; `type` is one of
bool, enum, string, path, int, header, info, action. QML picks the control by type and calls
setValue(index, value) with the result.
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


def _row(section, key, label, kind, value, choices=None, module="", detail=""):
    return {
        "section": section, "key": key, "label": label, "type": kind, "value": value,
        "display": _display(kind, value, choices), "choices": list(choices or []), "module": module,
        "detail": detail,
    }


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
        self._busy = False

    def _set_rows(self, rows):
        self._rows = rows
        self.rowsChanged.emit()

    @Slot(int, result="QVariant")
    def row(self, index):
        return self._rows[index] if 0 <= index < len(self._rows) else {}

    rows = Property("QVariantList", lambda self: list(self._rows), notify=rowsChanged)
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
        effective = game.get("effective") or {}
        for section, key, label, kind in CORE_ROWS:
            value = _dig(game, key)
            if value in (None, "") and key.startswith(("launch.", "desktop.")):
                value = effective.get(key.split(".", 1)[1])
            choices = []
            if key == "launch.proton":
                choices = sorted((config.get("proton") or {}).keys())
                default = _dig(config, "launch.proton", "")
                if default and default not in choices:
                    choices.insert(0, default)
                value = value or default
            if kind == "bool":
                value = bool(value) if value is not None else bool(_dig(config, key, False))
            rows.append(_row(section, key, label, kind, value, choices))
        modules = {m["id"]: m for m in self._client.modules() or []}
        for module_id, values in (self._client.settings(game_id) or {}).items():
            module = modules.get(module_id) or {}
            for setting in module.get("settings") or []:
                if setting.get("scope") != "game":
                    continue
                key = setting["key"]
                value = values.get(key, setting.get("default"))
                rows.append(_row(module.get("name", module_id), key, setting.get("label", key),
                                 setting.get("type", "string"), value, setting.get("choices"), module_id))
        self._set_rows(rows)

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
    """Every module: an enable toggle, its global settings, then Doctor's checks."""

    doctorChanged = Signal()

    def __init__(self, client, parent=None):
        super().__init__(client, parent)
        self._doctor = []
        client.modulesChanged.connect(self.load)

    @Slot()
    def load(self):
        rows = []
        for module in self._client.modules() or []:
            ident = module["id"]
            name = module.get("name", ident)
            state = []
            if module.get("version"):
                state.append(f"v{module['version']}")
            state.append(" · ".join(module.get("kind") or []))
            if not module.get("available", True):
                missing = ", ".join(module.get("missing") or [])
                state.append("unavailable" + (f": missing {missing}" if missing else ""))
            rows.append(_row(name, "", name, "header", None, detail=" · ".join(s for s in state if s), module=ident))
            rows.append(_row(name, "enabled", "Enabled", "bool", bool(module.get("enabled")), module=ident))
            if not module.get("enabled"):
                continue
            values = self._client.getSettings(ident, "") or {}
            for setting in module.get("settings") or []:
                if setting.get("scope") != "global":
                    continue
                key = setting["key"]
                rows.append(_row(name, key, setting.get("label", key), setting.get("type", "string"),
                                 values.get(key, setting.get("default")), setting.get("choices"), ident))
        self._set_rows(rows)

    @Slot()
    def loadDoctor(self):
        self._doctor = [
            _row(c.get("module") or "Core", "", c.get("check", ""), "info", bool(c.get("ok")),
                 detail=str(c.get("detail") or ""), module=c.get("module") or "")
            for c in self._client.doctor() or []
        ]
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
