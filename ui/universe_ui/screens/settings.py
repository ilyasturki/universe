"""Rows for the per-game settings page and the Modules page, rendered generically by QML.

A row is {section, key, label, type, value, display, choices, module, detail, inherited};
`type` is one of bool, enum, string, path, int, info, action. Groups arrange the rows into
cards: {title, meta, warning, caps, control, off, rows}, `rows` and `control` indexing the
flat row list. QML picks the control by type and calls setValue(index, value) with the result.
"""

import json
import os

from PySide6.QtCore import Property, QObject, Signal, Slot

ASSETS = os.path.join(os.path.dirname(os.path.dirname(__file__)), "qml", "assets", "runners")
LOGOS = {os.path.splitext(f)[0]: f"assets/runners/{f}" for f in sorted(os.listdir(ASSETS))}


def _display(kind, value, choices=None):
    if kind == "bool":
        return "On" if value else "Off"
    if value is None or value == "" or value == [] or (kind in ("int", "string") and value == 0):
        return "—"
    if isinstance(value, list):
        return ", ".join(str(v) for v in value)
    return str(value)


def _row(section, key, label, kind, value, choices=None, module="", detail="", inherited=False, dynamic=False):
    return {
        "section": section, "key": key, "label": label, "type": kind, "value": value,
        "display": _display(kind, value, choices), "choices": list(choices or []), "module": module,
        "detail": detail, "inherited": inherited, "dynamic": dynamic,
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


def runner_logo(runner_id):
    return LOGOS.get(runner_id, "")


def _to_bus(kind, value):
    if kind == "bool":
        return "true" if value else "false"
    if isinstance(value, list):
        return ",".join(str(v) for v in value)
    return "" if value is None else str(value)


GAMESCOPE_SCALERS = ["auto", "integer", "fit", "fill", "stretch"]
GAMESCOPE_FILTERS = ["linear", "nearest", "fsr", "nis", "pixel"]
GAMESCOPE_SHARPNESS = ["0", "2", "5", "10", "15", "20"]
REFRESH_RATES = [240, 165, 144, 120, 100, 90, 75, 60, 50, 48, 40, 30]
RESOLUTION_HEIGHTS = [2160, 1800, 1440, 1080, 720]


def screen_label(mode):
    """`3840×2160 @ 60 Hz`, or empty when the mode is unknown."""
    w, h, hz = int(mode.get("width") or 0), int(mode.get("height") or 0), int(mode.get("refresh") or 0)
    if not w or not h:
        return ""
    return f"{w}×{h}" + (f" @ {hz} Hz" if hz else "")


def resolution_choices(mode):
    """`auto`, the screen, then the standard heights below it at the screen's aspect ratio."""
    w, h = int(mode.get("width") or 0), int(mode.get("height") or 0)
    if not w or not h:
        return ["auto", "1920x1080", "1280x720"]
    out = ["auto"]
    for hh in [h] + RESOLUTION_HEIGHTS:
        if hh > h:
            continue
        ww = round(w * hh / h / 2) * 2
        if f"{ww}x{hh}" not in out:
            out.append(f"{ww}x{hh}")
    return out


def refresh_choices(mode):
    """`auto`, the screen's rate, then the common rates below it: a game sees no more than the screen shows."""
    hz = int(mode.get("refresh") or 0)
    if not hz:
        return ["auto"] + [str(r) for r in REFRESH_RATES]
    return ["auto"] + [str(r) for r in sorted({hz, *[r for r in REFRESH_RATES if r < hz]}, reverse=True)]


def gamescope_rows(mode):
    """(key, label, kind, choices, choiceValues) of the gamescope fields; a `choiceValues` list
    maps the choices to what is written, its first entry standing for the key left empty."""
    return [
        ("launch.gamescope_resolution", "Resolution", "string", resolution_choices(mode), None),
        ("launch.gamescope_refresh", "Refresh rate", "int", refresh_choices(mode), None),
        ("launch.gamescope_scaler", "Scaler", "enum", ["default"] + GAMESCOPE_SCALERS, [""] + GAMESCOPE_SCALERS),
        ("launch.gamescope_filter", "Filter", "enum", ["default"] + GAMESCOPE_FILTERS, [""] + GAMESCOPE_FILTERS),
        ("launch.gamescope_sharpness", "Sharpness", "int", ["default"] + GAMESCOPE_SHARPNESS, [""] + GAMESCOPE_SHARPNESS),
        ("launch.gamescope_adaptive_sync", "Adaptive sync", "bool", None, None),
    ]


def fps_limit_choices(mode):
    """`auto` (the refresh the game sees), `none`, then the rates the screen can show."""
    return ["auto", "none"] + refresh_choices(mode)[1:]


def fps_limit_row(section, value, mode, inherited=False, gamescope=True, gamescope_refresh="auto"):
    """The MangoHud limiter's row; `auto` shows the rate it stands for: the gamescope one when set, else the screen's."""
    row = choice_row(section, "launch.fps_limit", "Frame rate limit", "string", value or "auto", fps_limit_choices(mode), None, inherited=inherited)
    hz = int(mode.get("refresh") or 0)
    if gamescope and str(gamescope_refresh or "").isdigit():
        hz = int(gamescope_refresh)
    if row["value"] == "auto" and hz:
        row["display"] = f"auto · {hz}"
    return row


def choice_row(section, key, label, kind, value, choices, values, inherited=False):
    """A row whose listed choices may stand for other written values (`values`, see gamescope_rows)."""
    if values:
        empty = value in (None, "")
        value = choices[0] if empty else choices[values.index(str(value))] if str(value) in values else str(value)
    elif kind != "bool" and value is not None:
        value = str(value)
    row = _row(section, key, label, kind, value, choices, inherited=inherited)
    if values:
        row["choiceValues"] = list(values)
    return row


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
    ("Desktop and library", "desktop.hide_cursor", "Hide the cursor while playing", "bool"),
    ("Desktop and library", "favorite", "Favourite", "bool"),
    ("Desktop and library", "hidden", "Hidden", "bool"),
    ("Desktop and library", "tags", "Tags", "string"),
    ("Artwork", "metadata.sgdb_id", "SteamGridDB id", "int"),
    ("Artwork", "metadata.rawg_id", "RAWG id", "int"),
]
LAUNCH_ROWS = {
    "proton": [("launch.proton", "Proton", "enum"), ("launch.esync", "Esync", "bool"), ("launch.fsync", "Fsync", "bool"), ("launch.ntsync", "NTSync", "bool"), ("launch.wayland", "Wayland", "bool"), ("launch.hdr", "HDR", "bool"), ("launch.dlss_upgrade", "DLSS upgrade", "bool"), ("launch.fsr4_upgrade", "FSR 4 upgrade", "bool"), ("launch.xess_upgrade", "XeSS upgrade", "bool"), ("launch.optiscaler", "OptiScaler", "bool"), ("launch.prefix", "Wine prefix", "path")],
    "wine": [("launch.esync", "Esync", "bool"), ("launch.fsync", "Fsync", "bool"), ("launch.prefix", "Wine prefix", "path")],
}
COMMON_LAUNCH_ROWS = [("launch.mangohud", "MangoHud", "bool"), ("launch.fps_limit", "Frame rate limit", "string"), ("launch.wrapper", "Wrapper command", "string"), ("launch.args", "Arguments", "string"), ("launch.working_dir", "Working directory", "path")]


class GameSettingsForm(RowsForm):
    gameIdChanged = Signal()
    titleChanged = Signal()

    def __init__(self, client, screen_name=lambda: "", parent=None):
        super().__init__(client, parent)
        self._screen_name = screen_name
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
        launch = [("Launch", key, label, kind) for key, label, kind in LAUNCH_ROWS.get(runner_kind, []) + COMMON_LAUNCH_ROWS]
        mode = self._client.screenMode(self._screen_name()) or {}
        gamescope = [("Gamescope", "launch.gamescope", "Gamescope", "bool")]
        gamescope += [("Gamescope", key, label, kind) for key, label, kind, _, _ in gamescope_rows(mode)]
        gamescope.append(("Gamescope", "launch.gamescope_args", "Arguments", "string"))
        listed = {key: (kind, choices, values) for key, _, kind, choices, values in gamescope_rows(mode)}
        for section, key, label, kind in launch + gamescope + CORE_ROWS:
            value = _dig(game, key)
            if key == "launch.fps_limit":
                if not groups or groups[-1]["title"] != section:
                    groups.append(_group(section, [], caps=True))
                groups[-1]["rows"].append(len(rows))
                rows.append(fps_limit_row(section, value or effective.get("fps_limit"), mode, inherited=value in (None, ""),
                                          gamescope=bool(effective.get("gamescope", True)), gamescope_refresh=effective.get("gamescope_refresh")))
                continue
            if key in listed:
                # A field left empty takes the global one, `effective` says which; the choices carry the screen.
                own = value
                if own in (None, ""):
                    value = effective.get(key.split(".", 1)[1])
                _, choices, values = listed[key]
                if not groups or groups[-1]["title"] != section:
                    groups.append(_group(section, [], caps=True))
                groups[-1]["rows"].append(len(rows))
                rows.append(choice_row(section, key, label, kind, value, choices, values, inherited=own in (None, "")))
                continue
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

    @Slot(int, "QVariant", result=bool)
    def setValue(self, index, value):
        if not (0 <= index < len(self._rows)):
            return False
        row = self._rows[index]
        payload = _to_bus(row["type"], value)
        if row.get("choiceValues") and payload in row["choices"]:
            payload = row["choiceValues"][row["choices"].index(payload)]
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


def _module_state(module):
    if not module.get("available", True):
        missing = ", ".join(module.get("missing") or [])
        return "unavailable" + (f": missing {missing}" if missing else "")
    return ""


class ModulesForm(RowsForm):
    """Settings › Modules: one row per module, its name and whether it runs; the row opens the
    module's page (ModuleForm), and the list toggles it in place. Then Doctor's checks, one card
    per module."""

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

    @Slot(str, result=int)
    def indexOf(self, ident):
        return next((i for i, r in enumerate(self._rows) if r["module"] == ident), -1)

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

    doctor = Property("QVariantList", lambda self: list(self._doctor), notify=doctorChanged)
    doctorGroups = Property("QVariantList", lambda self: list(self._doctor_groups), notify=doctorChanged)


class ModuleForm(RowsForm):
    """One module's page: its enable switch, then its global settings. A setting the module
    lists live (`dynamic`) gets its choices off the UI thread, once per state of the module's
    settings."""

    moduleChanged = Signal()

    def __init__(self, client, screen_hz=lambda: 0, parent=None):
        super().__init__(client, parent)
        self._screen_hz = screen_hz
        self._module = {}
        self._ident = ""
        self._dynamic = {}
        self._pending = set()
        self._loading = False
        client.modulesChanged.connect(self.reload)

    def _choices(self, ident, key, setting, values):
        choices = [str(c) for c in setting.get("choices") or []]
        if setting.get("dynamic"):
            choices = self._dynamic.get(self._dynamic_key(ident, key, values), choices)
        # Recording above the screen's rate captures nothing more; the rates it can't reach go.
        if ident == "capture" and key == "fps":
            hz = self._screen_hz() or 0
            if hz > 0:
                choices = [c for c in choices if not c.isdigit() or int(c) <= max(hz, 30)]
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
            if not self._loading:
                self.reload()

        self._client.runAsync(lambda: self._client.settingChoices(ident, key), done)

    @Slot()
    def reload(self):
        if self._ident:
            self.load(self._ident)

    @Slot(str)
    def load(self, ident):
        self._ident = ident
        self._loading = True
        try:
            self._set_rows(*self._build(ident))
        finally:
            self._loading = False
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
            dynamic = bool(setting.get("dynamic"))
            if dynamic:
                self._fetch_dynamic(ident, key, values)
            settings["rows"].append(len(rows))
            rows.append(_row(name, key, setting.get("label", key), setting.get("type", "string"),
                             values.get(key, setting.get("default")), self._choices(ident, key, setting, values),
                             ident, dynamic=dynamic))
        if settings["rows"]:
            groups.append(settings)
        return rows, groups

    info = Property("QVariant", lambda self: dict(self._module), notify=moduleChanged)

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
            self.load(row["module"])
        return bool(ok)

    @Slot(int)
    def toggle(self, index):
        row = self.row(index)
        if row.get("type") == "bool":
            self.setValue(index, not row.get("value"))
