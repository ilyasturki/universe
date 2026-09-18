# A row's `type` is bool, enum, string, path, int, info or action; a group's `rows` and `control` index the flat row list.

import json
import os

from PySide6.QtCore import Property, QObject, Signal, Slot

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
    target = next((g for g in groups if g["title"] == section), None)
    if target is None:
        target = _group(section, [], **group)
        groups.append(target)
    target["rows"].append(len(rows))
    rows.append(row)


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
        if value in (None, ""):
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
    elif values:
        value = "default" if value in (None, "") else str(value)
    elif choices and value is not None:
        value = str(value)
    row = _row(section, "launch." + spec["key"], spec["label"], kind, value, choices, detail=gpu_note(spec, gpu or {}), inherited=inherited)
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


class RowsForm(AsyncScreen):
    rowsChanged = Signal()

    def __init__(self, client, parent=None):
        super().__init__(client, parent)
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


HIDE_CURSOR = "Hide the desktop cursor while the game runs."

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
        game = self._client.game(game_id)
        config = self._client.config()
        self._title = str(game.get("title") or game_id)
        self.titleChanged.emit()
        effective = game.get("effective") or {}
        launch, runner_name, runner_kind = self._launch_rows(game, effective)
        rows, groups = [], []
        mode = self._screen_mode()
        protons = proton_choices(config)
        hz = auto_rate(mode, effective.get("gamescope", True), effective.get("gamescope_refresh"))
        gpu = self._client.gpu()
        for spec in self._client.launchKeys("game", mode):
            if spec["runners"] and runner_kind not in spec["runners"]:
                continue
            own = _dig(game, "launch." + spec["key"])
            value, inherited = own, False
            if own in (None, "") and spec["scope"] == "both":
                value, inherited = effective.get(spec["key"]), True
            section = runner_name if spec["section"] == "Proton" else spec["section"]
            if section == "Launch":
                launch.append(launch_row(section, spec, value, inherited, protons, hz))
                continue
            _add(rows, groups, section, launch_row(section, spec, value, inherited, protons, hz, gpu), caps=True, meta=_card_meta(section, mode, gpu))
        for row in launch:
            _add(rows, groups, "Launch", row, caps=True)
        for section, key, label, kind in CORE_ROWS:
            value = _dig(game, key)
            inherited = False
            if key == "desktop.hide_cursor" and value is None:
                value, inherited = effective.get("hide_cursor"), True
            if kind == "bool":
                value = bool(value)
            _add(rows, groups, section, _row(section, key, label, kind, value, inherited=inherited, detail=HIDE_CURSOR if key == "desktop.hide_cursor" else ""), caps=True)
        modules = {m["id"]: m for m in self._client.modules()}
        for module_id, values in self._client.settings(game_id).items():
            module = modules.get(module_id) or {}
            name = module.get("name", module_id)
            group = _group(name, [], meta=_meta(module))
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
        runners = self._client.runners()
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
        return rows, spec["name"], kind

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
            row.update(display="On" if enabled else "Unavailable" if warning else "Off", action="Open", runner="", switch=True,
                       meta=_meta(entry), warning=warning, source=self.source,
                       detail=warning.replace("unavailable", "Cannot be enabled", 1) if warning and not enabled else _meta(entry))
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


class PageForm(RowsForm):
    # A `dynamic` setting's choices are fetched once per state of the entry's settings.
    moduleChanged = Signal()

    def __init__(self, client, parent=None):
        super().__init__(client, parent)
        self._module = {}
        self._ident = ""
        self._dynamic = {}
        self._pending = set()

    def _extra_rows(self, entry, name, rows, groups):
        pass

    def _fetch_dynamic(self, cache_key, ident, key):
        if cache_key in self._pending:
            return
        self._pending.add(cache_key)

        def done(choices):
            self._pending.discard(cache_key)
            self._dynamic[cache_key] = [str(c) for c in choices]
            self.reload()

        self._client.runAsync(lambda: self._choices(ident, key), done)

    @Slot()
    def reload(self):
        if self._ident:
            self.load(self._ident)

    @Slot(str)
    def load(self, ident):
        self._ident = ident
        self._set_rows(*self._build(ident, self._entries()))
        self.moduleChanged.emit()

    def _build(self, ident, entries):
        entry = next((m for m in entries if m["id"] == ident), None)
        if entry is None:
            self._module = {}
            return [], []
        name = entry.get("name", ident)
        enabled = bool(entry.get("enabled"))
        warning = _state(entry)
        self._module = {"id": ident, "name": name, "meta": _meta(entry), "warning": warning, "enabled": enabled,
                        "source": self.source, "logged_in": bool(entry.get("logged_in")), "user": str(entry.get("user") or "")}
        control = _row(name, "enabled", "Enabled", "bool", enabled, module=ident)
        control["disabled"] = bool(warning) and not enabled
        rows = [control]
        groups = [_group("", [0])]
        if not enabled:
            return rows, groups
        self._extra_rows(entry, name, rows, groups)
        values = self._settings(ident)
        settings = _group("Settings", [], caps=True)
        for setting in entry.get("settings") or []:
            if setting.get("scope") != "global":
                continue
            key = setting["key"]
            choices = [str(c) for c in setting.get("choices") or []]
            if setting.get("dynamic"):
                cache_key = (ident, key, json.dumps(values, sort_keys=True, default=str))
                if cache_key in self._dynamic:
                    choices = self._dynamic[cache_key]
                else:
                    self._fetch_dynamic(cache_key, ident, key)
            settings["rows"].append(len(rows))
            rows.append(_row(name, key, setting.get("label", key), setting.get("type", "string"), values.get(key, setting.get("default")), choices, ident))
        if settings["rows"]:
            groups.append(settings)
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

    def _extra_rows(self, entry, name, rows, groups):
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

    @Slot(str)
    def load(self, ident):
        self._ident = ident

        def done(entries):
            if self._ident == ident:
                self._set_rows(*self._build(ident, entries))
                self.moduleChanged.emit()

        self._client.runAsync(self._entries, done)
