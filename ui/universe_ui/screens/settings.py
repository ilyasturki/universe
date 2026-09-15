# A row's `type` is bool, enum, string, path, int, info or action; a group's `rows` and `control` index the flat row list.

import json
import os

from PySide6.QtCore import Property, QObject, Signal, Slot

from ..errors import UniverseError

ASSETS = os.path.join(os.path.dirname(os.path.dirname(__file__)), "qml", "assets", "runners")
LOGOS = {os.path.splitext(f)[0]: f"assets/runners/{f}" for f in sorted(os.listdir(ASSETS))}


def _display(kind, value):
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
        "display": _display(kind, value), "choices": list(choices or []), "module": module,
        "detail": detail, "inherited": inherited,
    }


def _group(title, rows, meta="", warning="", caps=False, control=-1, off=False):
    return {"title": title, "meta": meta, "warning": warning, "caps": caps, "control": control,
            "off": off, "rows": list(rows)}


def _add(rows, groups, section, row, **group):
    if not groups or groups[-1]["title"] != section:
        groups.append(_group(section, [], **group))
    groups[-1]["rows"].append(len(rows))
    rows.append(row)


def _module_meta(module):
    version = module.get("version")
    return " · ".join(p for p in (f"v{version}" if version else "", *(module.get("kind") or [])) if p)


def _dig(data, dotted, default=None):
    node = data
    for part in dotted.split("."):
        if not isinstance(node, dict) or part not in node:
            return default
        node = node[part]
    return node


def runner_logo(runner_id):
    return LOGOS.get(runner_id, "")


def _to_bus(row, value):
    if row["type"] == "bool":
        return "true" if value else "false"
    if isinstance(value, list):
        return ",".join(str(v) for v in value)
    payload = "" if value is None else str(value)
    if row.get("choiceValues") and payload in row["choices"]:
        payload = row["choiceValues"][row["choices"].index(payload)]
    return payload


# The catalogue's kinds as the rows' types; a `list` shows and edits as its comma-joined string.
ROW_TYPES = {"resolution": "string", "refresh": "int", "fps": "string", "proton": "enum", "list": "string"}


def screen_label(mode):
    """`3840×2160 @ 60 Hz`, or empty when the mode is unknown."""
    w, h, hz = int(mode.get("width") or 0), int(mode.get("height") or 0), int(mode.get("refresh") or 0)
    if not w or not h:
        return ""
    return f"{w}×{h}" + (f" @ {hz} Hz" if hz else "")


def auto_rate(mode, gamescope, gamescope_refresh):
    """The rate `fps_limit = auto` stands for: the gamescope one when set, else the screen's."""
    if gamescope and str(gamescope_refresh or "").isdigit():
        return int(gamescope_refresh)
    return int(mode.get("refresh") or 0)


def proton_choices(config):
    """The config's `[proton]` names, the default first when it is not one of them (a path)."""
    choices = sorted((config.get("proton") or {}).keys())
    default = str(_dig(config, "launch.proton", "") or "")
    if default and default not in choices:
        choices.insert(0, default)
    return choices


def launch_row(section, spec, value, inherited=False, protons=(), auto_hz=0):
    """A row over a `launchKeys` entry and the value shown; an enum or int with choices lists a `default`
    choice standing for the key left empty (`choiceValues` maps the choices to what is written)."""
    kind = ROW_TYPES.get(spec["type"], spec["type"])
    choices, values = [str(c) for c in spec["choices"]], None
    if spec["type"] in ("enum", "int") and choices:
        choices, values = ["default"] + choices, [""] + choices
    elif spec["type"] == "proton":
        choices = list(protons)
    if kind == "bool":
        value = bool(value)
    elif values:
        value = choices[0] if value in (None, "") else choices[values.index(str(value))] if str(value) in values else str(value)
    elif choices and value is not None:
        value = str(value)
    row = _row(section, "launch." + spec["key"], spec["label"], kind, value, choices, detail=spec["description"], inherited=inherited)
    if values:
        row["choiceValues"] = values
    if spec["type"] == "fps" and value == "auto" and auto_hz:
        row["display"] = f"auto · {auto_hz}"
    return row


class AsyncScreen(QObject):
    busyChanged = Signal()

    def __init__(self, client, parent=None):
        super().__init__(parent)
        self._client = client
        self._busy = 0

    def _run(self, work, done):
        self._busy += 1
        self.busyChanged.emit()

        def guarded():
            try:
                return work(), ""
            except UniverseError as e:
                return None, e.message or e.kind

        def finish(result):
            self._busy -= 1
            done(*result)
            self.busyChanged.emit()

        self._client.runAsync(guarded, finish)

    busy = Property(bool, lambda self: self._busy > 0, notify=busyChanged)


class RowsForm(QObject):
    rowsChanged = Signal()

    def __init__(self, client, parent=None):
        super().__init__(parent)
        self._client = client
        self._rows = []
        self._groups = []

    def _set_rows(self, rows, groups):
        self._rows = rows
        self._groups = groups
        self.rowsChanged.emit()

    @Slot(int, result="QVariant")
    def row(self, index):
        return self._rows[index] if 0 <= index < len(self._rows) else {}

    @Slot(str, result=int)
    def indexOf(self, ident):
        return next((i for i, r in enumerate(self._rows) if r.get("module") == ident), -1)

    @Slot(int, "QVariant", result=bool)
    def setValue(self, index, value):
        row = self.row(index)
        if not row:
            return False
        ok = self._write(row, _to_bus(row, value))
        if ok:
            self._reload(row)
        return bool(ok)

    @Slot(int)
    def toggle(self, index):
        row = self.row(index)
        if row.get("type") == "bool":
            self.setValue(index, not row.get("value"))

    rows = Property("QVariantList", lambda self: list(self._rows), notify=rowsChanged)
    groups = Property("QVariantList", lambda self: list(self._groups), notify=rowsChanged)
    count = Property(int, lambda self: len(self._rows), notify=rowsChanged)


CORE_ROWS = [
    ("Desktop and library", "desktop.hide_cursor", "Hide the cursor while playing", "bool"),
    ("Desktop and library", "favorite", "Favourite", "bool"),
    ("Desktop and library", "hidden", "Hidden", "bool"),
    ("Desktop and library", "tags", "Tags", "string"),
    ("Artwork", "metadata.sgdb_id", "SteamGridDB id", "int"),
    ("Artwork", "metadata.rawg_id", "RAWG id", "int"),
]


class GameSettingsForm(RowsForm):
    gameIdChanged = Signal()
    titleChanged = Signal()

    def __init__(self, client, screen_mode=lambda: {}, parent=None):
        super().__init__(client, parent)
        self._screen_mode = screen_mode
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
        effective = game.get("effective") or {}
        rows, runner_kind = self._launch_rows(game, effective)
        groups = [_group("Launch", range(len(rows)), caps=True)]
        mode = self._screen_mode() or {}
        protons = proton_choices(config)
        hz = auto_rate(mode, effective.get("gamescope", True), effective.get("gamescope_refresh"))
        for spec in self._client.launchKeys("game", mode):
            if spec["runners"] and runner_kind not in spec["runners"]:
                continue
            # A key the game leaves empty takes the global value, `effective` says which.
            own = _dig(game, "launch." + spec["key"])
            value, inherited = own, False
            if own in (None, "") and spec["scope"] == "both":
                value, inherited = effective.get(spec["key"]), True
            section = "Gamescope" if spec["section"] == "Gamescope" else "Launch"
            _add(rows, groups, section, launch_row(section, spec, value, inherited, protons, hz), caps=True)
        for section, key, label, kind in CORE_ROWS:
            value = _dig(game, key)
            inherited = False
            if key == "desktop.hide_cursor" and value is None:
                value, inherited = effective.get("hide_cursor"), True
            if kind == "bool":
                value = bool(value)
            _add(rows, groups, section, _row(section, key, label, kind, value, inherited=inherited), caps=True)
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

    def _launch_rows(self, game, effective):
        runners = list(self._client.runners() or [])
        runner_id = str(effective.get("runner") or "proton")
        spec = next((r for r in runners if r["id"] == runner_id), None) or {"id": runner_id, "name": runner_id, "kind": "", "platforms": [], "options": []}
        kind = spec.get("kind") or ""
        rows = []
        names = [r["name"] for r in runners] or [spec["name"]]
        picker = _row("Launch", "launch.runner", "Runner", "enum", spec["name"], names)
        picker["choiceValues"] = [r["id"] for r in runners] or [runner_id]
        picker["icon"] = runner_logo(runner_id)
        rows.append(picker)
        rows.append(_row("Launch", "launch.exe", "File" if kind == "emulator" else "Program", "path", _dig(game, "launch.exe") or ""))
        if kind == "emulator":
            platforms = list(spec.get("platforms") or [])
            if len(platforms) > 1:
                rows.append(_row("Launch", "platform", "Platform", "enum", game.get("platform") or platforms[0], platforms))
            rows.append(_row("Launch", "launch.runner_exe", spec["name"] + " program", "path", _dig(game, "launch.runner_exe") or effective.get("runner_path") or "",
                             inherited=not _dig(game, "launch.runner_exe")))
            options = effective.get("options") or {}
            own = _dig(game, "launch.options") or {}
            for option in spec.get("options") or []:
                key = option["key"]
                value = options.get(key, option.get("default"))
                if option.get("type") == "bool":
                    value = bool(value)
                rows.append(_row("Launch", f"launch.options.{key}", option.get("label", key), option.get("type", "string"),
                                 value, option.get("choices"), inherited=key not in own))
        return rows, kind

    def _write(self, row, payload):
        if row["module"]:
            return self._client.setSetting(row["module"], self._game_id, row["key"], payload)
        return self._client.set(self._game_id, row["key"], payload)

    def _reload(self, row):
        self.load(self._game_id)

    gameId = Property(str, lambda self: self._game_id, notify=gameIdChanged)
    title = Property(str, lambda self: self._title, notify=titleChanged)


def _module_state(module):
    if not module.get("available", True):
        missing = ", ".join(module.get("missing") or [])
        return "unavailable" + (f": missing {missing}" if missing else "")
    return ""


class ModulesForm(RowsForm):
    doctorChanged = Signal()

    def __init__(self, client, parent=None):
        super().__init__(client, parent)
        self._doctor = []
        self._doctor_groups = []
        client.modulesChanged.connect(self.load)

    @Slot()
    def load(self):
        rows, on, off = [], [], []
        for module in self._client.modules() or []:
            ident = module["id"]
            name = module.get("name", ident)
            enabled = bool(module.get("enabled"))
            warning = _module_state(module)
            row = _row("Modules", "module", name, "action", enabled, module=ident)
            row.update(display="On" if enabled else "Unavailable" if warning else "Off", action="Open", runner="",
                       meta=_module_meta(module), warning=warning, kind=list(module.get("kind") or []),
                       detail=warning.replace("unavailable", "Cannot be enabled", 1) if warning and not enabled else _module_meta(module))
            (on if enabled else off).append(len(rows))
            rows.append(row)
        groups = [_group("", on)]
        if off:
            groups.append(_group("Off", off, caps=True, off=True))
        self._set_rows(rows, groups)

    @Slot(int)
    def toggle(self, index):
        row = self.row(index)
        if not row:
            return
        self._client.enableModule(row["module"], not row["value"])
        self.load()

    @Slot()
    def loadDoctor(self):
        names = {m["id"]: m.get("name", m["id"]) for m in self._client.modules() or []}
        rows, groups = [], []
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

    doctor = Property("QVariantList", lambda self: list(self._doctor), notify=doctorChanged)
    doctorGroups = Property("QVariantList", lambda self: list(self._doctor_groups), notify=doctorChanged)


class ModuleForm(RowsForm):
    # A `dynamic` setting's choices come from the module, fetched once per state of its settings.
    moduleChanged = Signal()

    def __init__(self, client, parent=None):
        super().__init__(client, parent)
        self._module = {}
        self._ident = ""
        self._dynamic = {}
        self._pending = set()
        client.modulesChanged.connect(self.reload)

    def _choices(self, ident, key, setting, values):
        choices = [str(c) for c in setting.get("choices") or []]
        if setting.get("dynamic"):
            choices = self._dynamic.get(self._dynamic_key(ident, key, values), choices)
        return choices

    @staticmethod
    def _dynamic_key(ident, key, values):
        return (ident, key, json.dumps(values, sort_keys=True, default=str))

    def _fetch_dynamic(self, ident, key, values):
        cache_key = self._dynamic_key(ident, key, values)
        if cache_key in self._dynamic or cache_key in self._pending:
            return
        self._pending.add(cache_key)

        def done(choices):
            self._pending.discard(cache_key)
            self._dynamic[cache_key] = [str(c) for c in choices or []]
            self.reload()

        self._client.runAsync(lambda: self._client.settingChoices(ident, key), done)

    def reload(self):
        if self._ident:
            self.load(self._ident)

    @Slot(str)
    def load(self, ident):
        self._ident = ident
        self._set_rows(*self._build(ident))
        self.moduleChanged.emit()

    def _build(self, ident):
        module = next((m for m in self._client.modules() or [] if m["id"] == ident), None)
        if module is None:
            self._module = {}
            return [], []
        name = module.get("name", ident)
        enabled = bool(module.get("enabled"))
        warning = _module_state(module)
        self._module = {"id": ident, "name": name, "meta": _module_meta(module), "warning": warning,
                        "kind": list(module.get("kind") or []), "enabled": enabled}
        control = _row(name, "enabled", "Enabled", "bool", enabled, module=ident)
        control["disabled"] = bool(warning) and not enabled
        rows = [control]
        groups = [_group("", [0])]
        if not enabled:
            return rows, groups
        values = self._client.getSettings(ident, "") or {}
        settings = _group("Settings", [], caps=True)
        for setting in module.get("settings") or []:
            if setting.get("scope") != "global":
                continue
            key = setting["key"]
            if setting.get("dynamic"):
                self._fetch_dynamic(ident, key, values)
            settings["rows"].append(len(rows))
            rows.append(_row(name, key, setting.get("label", key), setting.get("type", "string"),
                             values.get(key, setting.get("default")), self._choices(ident, key, setting, values), ident))
        if settings["rows"]:
            groups.append(settings)
        return rows, groups

    info = Property("QVariant", lambda self: dict(self._module), notify=moduleChanged)

    def _write(self, row, payload):
        if row["key"] == "enabled":
            self._client.enableModule(row["module"], payload == "true")
            return True
        return self._client.setSetting(row["module"], "", row["key"], payload)

    def _reload(self, row):
        self.load(row["module"])
