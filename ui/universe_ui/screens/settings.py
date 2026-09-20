# A row's `type` is bool, enum, string, path, int, map, info or action; a group's `rows` and `control` index the flat row list.
# An `advanced` row sits in an `advanced` group, shown behind the page's Advanced row.

import json
import os

from PySide6.QtCore import Property, QObject, Signal, Slot

ASSETS = os.path.join(os.path.dirname(os.path.dirname(__file__)), "qml", "assets", "runners")
LOGOS = {os.path.splitext(f)[0]: f"assets/runners/{f}" for f in sorted(os.listdir(ASSETS))}

ADVANCED_KEY = "advanced"
ADVANCED_DETAIL = "Settings for power users: sync modes, scaling, upscaler upgrades, programs and folders."


def _display(kind, value):
    if kind == "bool":
        return "On" if value else "Off"
    if isinstance(value, dict):
        return ", ".join(f"{k}={v}" for k, v in value.items()) or "—"
    if value is None or value == "" or value == [] or (kind in ("int", "string") and value == 0):
        return "—"
    if isinstance(value, list):
        return ", ".join(str(v) for v in value)
    return str(value)


def _row(section, key, label, kind, value, choices=None, module="", detail="", inherited=False, advanced=False):
    row = {
        "section": section, "key": key, "label": label, "type": kind, "value": value,
        "display": _display(kind, value), "choices": list(choices or []), "module": module,
        "detail": detail, "inherited": inherited, "advanced": advanced,
    }
    if kind == "map":
        row["entries"] = [{"name": k, "value": str(v)} for k, v in (value or {}).items()]
    return row


def _group(title, rows, meta="", warning="", caps=False, control=-1, off=False, advanced=False):
    return {"title": title, "meta": meta, "warning": warning, "caps": caps, "control": control,
            "off": off, "advanced": advanced, "rows": list(rows)}


def _add(rows, groups, section, row, **group):
    advanced = bool(row.get("advanced"))
    target = next((g for g in groups if g["title"] == section and g["advanced"] == advanced), None)
    if target is None:
        target = _group(section, [], advanced=advanced, **group)
        groups.append(target)
    target["rows"].append(len(rows))
    rows.append(row)


def advanced_row(shown):
    row = _row("", ADVANCED_KEY, "Advanced", "action", shown, detail=ADVANCED_DETAIL)
    row.update(display="", action="Hide" if shown else "Show", icon="sliders")
    return row


def _meta(entry):
    version = entry.get("version")
    return f"v{version}" if version else ""


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


ROW_TYPES = {"resolution": "string", "refresh": "int", "fps": "string", "proton": "enum", "list": "string"}


def screen_label(mode):
    w, h, hz = int(mode.get("width") or 0), int(mode.get("height") or 0), int(mode.get("refresh") or 0)
    if not w or not h:
        return ""
    return f"{w}×{h}" + (f" @ {hz} Hz" if hz else "")


def _card_meta(section, mode, gpu):
    if section == "Display":
        return " ".join(p for p in (str(mode.get("screen") or ""), screen_label(mode)) if p)
    if section == UPSCALING:
        return str(gpu.get("label") or "")
    return ""


def auto_rate(mode, gamescope, gamescope_refresh):
    if gamescope and str(gamescope_refresh or "").isdigit():
        return int(gamescope_refresh)
    return int(mode.get("refresh") or 0)


def proton_choices(config):
    choices = sorted((config.get("proton") or {}).keys())
    default = str(_dig(config, "launch.proton", "") or "")
    if default and default not in choices:
        choices.insert(0, default)
    return choices


UPSCALING = "Upscaling"


def gpu_note(spec, gpu):
    fit = (gpu.get("fits") or {}).get(spec["key"])
    if fit is None:
        return spec["description"]
    return spec["description"] + (" Works on your GPU." if fit else " Not for your GPU.")


def global_launch_rows(rows, groups, client, config, mode, takes, gpu=None):
    launch = config.get("launch") or {}
    protons = proton_choices(config)
    hz = auto_rate(mode, launch.get("gamescope", True), launch.get("gamescope_refresh"))
    for spec in client.launchKeys("global", mode):
        if not takes(spec):
            continue
        value = launch.get(spec["key"])
        if value in (None, "", {}):
            value = spec["default"]
        section = spec["section"]
        _add(rows, groups, section, launch_row(section, spec, value, protons=protons, auto_hz=hz, gpu=gpu), caps=True, meta=_card_meta(section, mode, gpu or {}))


def launch_row(section, spec, value, inherited=False, protons=(), auto_hz=0, gpu=None):
    kind = ROW_TYPES.get(spec["type"], spec["type"])
    choices, values = [str(c) for c in spec["choices"]], None
    if spec["type"] in ("enum", "int") and choices:
        choices, values = ["default"] + choices, [""] + choices
    elif spec["type"] == "proton":
        choices = list(protons)
    if kind == "bool":
        value = bool(value)
    elif kind == "map":
        value = dict(value) if isinstance(value, dict) else {}
    elif values:
        value = "default" if value in (None, "") else str(value)
    elif choices and value is not None:
        value = str(value)
    row = _row(section, "launch." + spec["key"], spec["label"], kind, value, choices, detail=gpu_note(spec, gpu or {}), inherited=inherited,
               advanced=bool(spec.get("advanced")))
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

        def finish(result):
            self._busy -= 1
            done(*result)
            self.busyChanged.emit()

        self._client.runAsync(lambda: self._client.attempt(work), finish)

    busy = Property(bool, lambda self: self._busy > 0, notify=busyChanged)


class AdvancedRows:
    # The row list with its Advanced row: the gate at `_gate`, the advanced groups shown behind it while `_show_advanced`.

    def _init_rows(self):
        self._rows = []
        self._groups = []
        self._gate = -1
        self._show_advanced = False

    def _set_rows(self, rows, groups):
        rows, groups = list(rows), list(groups)
        self._gate = -1
        if any(g.get("advanced") for g in groups):
            self._gate = len(rows)
            rows.append(advanced_row(self._show_advanced))
        self._rows = rows
        self._groups = groups
        self.rowsChanged.emit()

    def _shown_groups(self):
        if self._gate < 0:
            return list(self._groups)
        basic = [g for g in self._groups if not g["advanced"]]
        more = [g for g in self._groups if g["advanced"]] if self._show_advanced else []
        # `wide`: the gate spans every column, the advanced cards flow under it.
        return basic + [{**_group("", [self._gate]), "wide": True}] + more

    def _set_show_advanced(self, shown):
        shown = bool(shown)
        if shown == self._show_advanced:
            return
        self._show_advanced = shown
        if self._gate >= 0:
            self._rows[self._gate] = advanced_row(shown)
        self.rowsChanged.emit()
        self.advancedChanged.emit()

    def _row_at(self, index):
        return self._rows[index] if 0 <= index < len(self._rows) else {}

    def _index_of_key(self, key, module=""):
        return next((i for i, r in enumerate(self._rows) if r.get("key") == key and (not module or r.get("module") == module)), -1)

    def _reveal(self, key, module=""):
        index = self._index_of_key(key, module)
        if index >= 0 and self._rows[index].get("advanced"):
            self._set_show_advanced(True)
        return index


class RowsForm(AdvancedRows, AsyncScreen):
    rowsChanged = Signal()
    advancedChanged = Signal()

    def __init__(self, client, parent=None):
        super().__init__(client, parent)
        self._init_rows()

    @Slot(int, result="QVariant")
    def row(self, index):
        return self._row_at(index)

    @Slot(str, result=int)
    def indexOf(self, ident):
        return next((i for i, r in enumerate(self._rows) if r.get("module") == ident), -1)

    @Slot(str, str, result=int)
    def reveal(self, key, module=""):
        return self._reveal(key, module)

    @Slot(int, "QVariant", result=bool)
    def setValue(self, index, value):
        row = self.row(index)
        if not row:
            return False
        ok = self._write(row, _to_bus(row, value))
        if ok:
            self._reload(row)
        return bool(ok)

    # One entry of a map row: `launch.env.FOO`; an empty value removes it.
    @Slot(int, str, str, result=bool)
    def setMapEntry(self, index, name, value):
        row = self.row(index)
        name = str(name or "").strip()
        if not row or row.get("type") != "map" or not name:
            return False
        ok = self._write({**row, "key": row["key"] + "." + name, "type": "string"}, str(value or ""))
        if ok:
            self._reload(row)
        return bool(ok)

    @Slot(int)
    def toggle(self, index):
        row = self.row(index)
        if row.get("type") == "bool":
            self.setValue(index, not row.get("value"))

    rows = Property("QVariantList", lambda self: list(self._rows), notify=rowsChanged)
    groups = Property("QVariantList", AdvancedRows._shown_groups, notify=rowsChanged)
    basicGroups = Property("QVariantList", lambda self: [g for g in self._groups if not g["advanced"]], notify=rowsChanged)
    advancedGroups = Property("QVariantList", lambda self: [g for g in self._groups if g["advanced"]], notify=rowsChanged)
    hasAdvanced = Property(bool, lambda self: self._gate >= 0, notify=rowsChanged)
    showAdvanced = Property(bool, lambda self: self._show_advanced, AdvancedRows._set_show_advanced, notify=advancedChanged)
    count = Property(int, lambda self: len(self._rows), notify=rowsChanged)


HIDE_CURSOR = "Hide the desktop cursor while the game runs."

CORE_ROWS = [
    ("Desktop and library", "desktop.hide_cursor", "Hide the cursor while playing", "bool", False),
    ("Desktop and library", "favorite", "Favourite", "bool", False),
    ("Desktop and library", "hidden", "Hidden", "bool", False),
    ("Desktop and library", "tags", "Tags", "string", False),
    ("Artwork", "metadata.sgdb_id", "SteamGridDB id", "int", True),
    ("Artwork", "metadata.rawg_id", "RAWG id", "int", True),
]


def _runner_spec(runners, runner_id):
    return next((r for r in runners if r["id"] == runner_id), None) or {"id": runner_id, "name": runner_id, "kind": "", "platforms": [], "options": []}


def game_launch_rows(game, effective, runners):
    runner_id = str(effective.get("runner") or "proton")
    spec = _runner_spec(runners, runner_id)
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
    return rows, spec["name"], kind


def build_game(client, game_id, screen_mode):
    """A game's settings rows: the launch cards, the runner's, the program, the library flags, then each module's game settings."""
    game = client.game(game_id)
    config = client.config()
    title = str(game.get("title") or game_id)
    effective = game.get("effective") or {}
    launch, runner_name, runner_kind = game_launch_rows(game, effective, client.runners())
    rows, groups = [], []
    mode = screen_mode()
    protons = proton_choices(config)
    hz = auto_rate(mode, effective.get("gamescope", True), effective.get("gamescope_refresh"))
    gpu = client.gpu()
    for spec in client.launchKeys("game", mode):
        if spec["runners"] and runner_kind not in spec["runners"]:
            continue
        own = _dig(game, "launch." + spec["key"])
        value, inherited = own, False
        if own in (None, "", {}) and spec["scope"] == "both":
            value, inherited = effective.get(spec["key"]), True
        section = runner_name if spec["section"] == "Proton" else spec["section"]
        if section == "Launch":
            launch.append(launch_row(section, spec, value, inherited, protons, hz))
            continue
        _add(rows, groups, section, launch_row(section, spec, value, inherited, protons, hz, gpu), caps=True, meta=_card_meta(section, mode, gpu))
    for row in launch:
        _add(rows, groups, "Launch", row, caps=True)
    for section, key, label, kind, advanced in CORE_ROWS:
        value = _dig(game, key)
        inherited = False
        if key == "desktop.hide_cursor" and value is None:
            value, inherited = effective.get("hide_cursor"), True
        if kind == "bool":
            value = bool(value)
        _add(rows, groups, section, _row(section, key, label, kind, value, inherited=inherited, detail=HIDE_CURSOR if key == "desktop.hide_cursor" else "", advanced=advanced), caps=True)
    modules = {m["id"]: m for m in client.modules()}
    for module_id, values in client.settings(game_id).items():
        module = modules.get(module_id) or {}
        name = module.get("name", module_id)
        for setting in module.get("settings") or []:
            if setting.get("scope") != "game":
                continue
            key = setting["key"]
            value = values.get(key, setting.get("default"))
            _add(rows, groups, name, _row(name, key, setting.get("label", key), setting.get("type", "string"), value, setting.get("choices"), module_id,
                                          advanced=bool(setting.get("advanced"))), meta=_meta(module))
    return rows, groups, title


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
        if game_id != self._game_id:
            self._set_show_advanced(False)
        self._game_id = game_id
        self.gameIdChanged.emit()
        rows, groups, self._title = build_game(self._client, game_id, self._screen_mode)
        self.titleChanged.emit()
        self._set_rows(rows, groups)

    def _write(self, row, payload):
        if row["module"]:
            return self._client.setSetting(row["module"], self._game_id, row["key"], payload)
        return self._client.set(self._game_id, row["key"], payload)

    def _reload(self, row):
        self.load(self._game_id)

    gameId = Property(str, lambda self: self._game_id, notify=gameIdChanged)
    title = Property(str, lambda self: self._title, notify=titleChanged)


def _state(entry):
    if not entry.get("available", True):
        missing = ", ".join(entry.get("missing") or [])
        return "unavailable" + (f": missing {missing}" if missing else "")
    return ""


class ModuleApi:
    source = False
    kind = "Modules"

    def _entries(self):
        return self._client.modules()

    def _enable(self, ident, enabled):
        self._client.enableModule(ident, enabled)

    def _settings(self, ident):
        return self._client.getSettings(ident, "")

    def _choices(self, ident, key):
        return self._client.settingChoices(ident, key)

    def _set(self, ident, key, value):
        return self._client.setSetting(ident, "", key, value)


class SourceApi:
    source = True
    kind = "Sources"

    def _entries(self):
        return self._client.sources()

    def _enable(self, ident, enabled):
        self._client.enableSource(ident, enabled)

    def _settings(self, ident):
        return self._client.getSourceSettings(ident)

    def _choices(self, ident, key):
        return self._client.sourceSettingChoices(ident, key)

    def _set(self, ident, key, value):
        return self._client.setSourceSetting(ident, key, value)


class ListForm(RowsForm):
    section = "Modules"

    def _show(self, entries):
        rows, on, off = [], [], []
        for entry in entries:
            ident = entry["id"]
            name = entry.get("name", ident)
            enabled = bool(entry.get("enabled"))
            warning = _state(entry)
            row = _row(self.section, "module", name, "action", enabled, module=ident)
            row.update(display="Unavailable" if warning else "On" if enabled else "Off", action="Open", runner="", switch=True,
                       meta=_meta(entry), warning=warning, source=self.source,
                       detail=warning.replace("unavailable", "On, but its hooks are skipped" if enabled else "Cannot be enabled", 1) if warning else _meta(entry))
            (on if enabled else off).append(len(rows))
            rows.append(row)
        groups = [_group("", on)] if on else []
        if off:
            groups.append(_group("Off", off, caps=True, off=True))
        self._set_rows(rows, groups)

    @Slot()
    def load(self):
        self._show(self._entries())

    @Slot(int)
    def toggle(self, index):
        row = self.row(index)
        if not row:
            return
        self._enable(row["module"], not row["value"])
        self.load()


class ModulesForm(ModuleApi, ListForm):
    doctorChanged = Signal()

    def __init__(self, client, parent=None):
        super().__init__(client, parent)
        self._doctor = []
        self._doctor_groups = []
        client.modulesChanged.connect(self.load)

    @Slot()
    def loadDoctor(self):
        def work():
            names = {m["id"]: m.get("name", m["id"]) for m in self._client.modules() + self._client.sources()}
            return names, self._client.doctor()

        self._client.runAsync(work, lambda result: self._show_doctor(*result))

    def _show_doctor(self, names, checks):
        rows, groups = [], []
        for check in checks:
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


class SourcesForm(SourceApi, ListForm):
    section = "Sources"

    def __init__(self, client, parent=None):
        super().__init__(client, parent)
        client.sourcesChanged.connect(self.load)

    @Slot()
    def load(self):
        self._client.runAsync(self._entries, self._show)


def page_info(api, entry, ident):
    name = entry.get("name", ident)
    return {"id": ident, "name": name, "meta": _meta(entry), "description": str(entry.get("description") or ""), "warning": _state(entry),
            "enabled": bool(entry.get("enabled")), "source": api.source, "logged_in": bool(entry.get("logged_in")), "user": str(entry.get("user") or "")}


def signin_rows(entry, name, rows, groups):
    logged_in = bool(entry.get("logged_in"))
    user = str(entry.get("user") or "")
    signin = _group("Sign-in", [], caps=True)
    for row in (
        _row(name, "logged_in", "Signed in", "info", logged_in, module=entry["id"], detail=user or ("yes" if logged_in else "no")),
        {**_row(name, "link", "Get a sign-in link", "action", "", module=entry["id"]), "action": "Sign in", "display": ""},
        {**_row(name, "code", "Enter the code", "action", "", module=entry["id"]), "action": "Enter", "display": ""},
    ):
        signin["rows"].append(len(rows))
        rows.append(row)
    groups.append(signin)


def build_page(api, ident, entries, choices_of=lambda ident, key, values: None):
    """A module's or a source's page: the switch, a source's sign-in, then its global settings, the advanced and config-only ones behind the gate.
    `choices_of` answers a dynamic setting's choices, or None while they are not known."""
    entry = next((m for m in entries if m["id"] == ident), None)
    if entry is None:
        return {}, [], []
    info = page_info(api, entry, ident)
    name, enabled = info["name"], info["enabled"]
    control = _row(name, "enabled", "Enabled", "bool", enabled, module=ident)
    control["disabled"] = bool(info["warning"]) and not enabled
    rows = [control]
    groups = [_group("", [0])]
    if not enabled:
        return info, rows, groups
    if api.source:
        signin_rows(entry, name, rows, groups)
    values = api._settings(ident)
    for setting in entry.get("settings") or []:
        if setting.get("scope") not in ("global", "config"):
            continue
        key = setting["key"]
        choices = [str(c) for c in setting.get("choices") or []]
        if setting.get("dynamic"):
            choices = choices_of(ident, key, values) or choices
        row = _row(name, key, setting.get("label", key), setting.get("type", "string"), values.get(key, setting.get("default")), choices, ident,
                   advanced=bool(setting.get("advanced")) or setting.get("scope") == "config")
        _add(rows, groups, "Settings", row, caps=True)
    return info, rows, groups


class PageForm(RowsForm):
    # A `dynamic` setting's choices are fetched once per state of the entry's settings.
    moduleChanged = Signal()

    def __init__(self, client, parent=None):
        super().__init__(client, parent)
        self._module = {}
        self._ident = ""
        self._dynamic = {}
        self._pending = set()

    def _fetch_dynamic(self, cache_key, ident, key):
        if cache_key in self._pending:
            return
        self._pending.add(cache_key)

        def done(choices):
            self._pending.discard(cache_key)
            self._dynamic[cache_key] = [str(c) for c in choices]
            self.reload()

        self._client.runAsync(lambda: self._choices(ident, key), done)

    def _dynamic_choices(self, ident, key, values):
        cache_key = (ident, key, json.dumps(values, sort_keys=True, default=str))
        if cache_key in self._dynamic:
            return self._dynamic[cache_key]
        self._fetch_dynamic(cache_key, ident, key)
        return None

    @Slot()
    def reload(self):
        if self._ident:
            self.load(self._ident)

    @Slot(str)
    def load(self, ident):
        self._open(ident)
        self._set_rows(*self._build(ident, self._entries()))
        self.moduleChanged.emit()

    def _open(self, ident):
        if ident != self._ident:
            self._set_show_advanced(False)
        self._ident = ident

    def _build(self, ident, entries):
        self._module, rows, groups = build_page(self, ident, entries, self._dynamic_choices)
        return rows, groups

    info = Property("QVariant", lambda self: dict(self._module), notify=moduleChanged)

    def _write(self, row, payload):
        if row["key"] == "enabled":
            self._enable(row["module"], payload == "true")
            return True
        return self._set(row["module"], row["key"], payload)

    def _reload(self, row):
        self.load(row["module"])


class ModuleForm(ModuleApi, PageForm):
    def __init__(self, client, parent=None):
        super().__init__(client, parent)
        client.modulesChanged.connect(self.reload)


class SourceForm(SourceApi, PageForm):
    def __init__(self, client, parent=None):
        super().__init__(client, parent)
        client.sourcesChanged.connect(self.reload)

    @Slot(str)
    def load(self, ident):
        self._open(ident)

        def done(entries):
            if self._ident == ident:
                self._set_rows(*self._build(ident, entries))
                self.moduleChanged.emit()

        self._client.runAsync(self._entries, done)
