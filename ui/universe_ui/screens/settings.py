# A row's `type` is bool, enum, string, path, int, info or action; a group's `rows` and `control` index the flat row list.
# An `advanced` row sits in an `advanced` group, shown while the form's `showAdvanced` is set (`load` clears it): folded into
# the basic group titled like it or like its `home` — its rows after the group's `divider`, each folded group ruled off by
# one of `dividers` — or, with no such group, as a group of its own after the basic ones. A gated form (the controller's)
# appends an Advanced action row that opens them; the other pages flip `showAdvanced` from a button.
# `origin` is where a value comes from when the row can inherit: "game" (set on the game), "runner" (set on the runner),
# "global" (config.toml sets it), "default" (neither does); empty when the row has no such story. `inherited` is true for
# the last two. A map key (launch.env) is one row per entry, `entry` naming the map, then an `action` row with `map` set
# that adds one; an entry's empty value removes it.
import json
import os
import re
from collections.abc import Callable
from typing import TYPE_CHECKING, Any

from PySide6.QtCore import QObject, Signal, Slot

from ..qt import QVARIANT, Property

if TYPE_CHECKING:
    from ..universe_client import CoreClient

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


def _row(section, key, label, kind, value, choices=None, module="", detail="", inherited=False, advanced=False, origin=""):
    return {
        "section": section,
        "key": key,
        "label": label,
        "type": kind,
        "value": value,
        "display": _display(kind, value),
        "choices": list(choices or []),
        "module": module,
        "detail": detail,
        "inherited": inherited or origin in ("global", "default"),
        "advanced": advanced,
        "origin": origin,
    }


def _group(title, rows, meta="", warning="", caps=False, control=-1, off=False, advanced=False):
    return {
        "title": title,
        "meta": meta,
        "warning": warning,
        "caps": caps,
        "control": control,
        "off": off,
        "advanced": advanced,
        "home": "",
        "rows": list(rows),
        "divider": -1,
        "dividers": [],
    }


def _is_set(value):
    return value is not None and value != "" and value != {}


def origin_of(own, global_value):
    return "game" if _is_set(own) else "global" if _is_set(global_value) else "default"


def _add(rows, groups, section, row, home="", **group):
    advanced = bool(row.get("advanced"))
    target = next((g for g in groups if g["title"] == section and g["advanced"] == advanced), None)
    if target is None:
        target = _group(section, [], advanced=advanced, **group)
        target["home"] = home if advanced else ""
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


ROW_TYPES = {"resolution": "string", "refresh": "int", "fps": "string", "proton": "enum", "list": "string", "toggle": "enum"}


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


def toggle_auto(key, gpu, mode):
    """What a toggle key on `auto` comes to on this machine: the GPU's say for an upgrade, the screen's VRR for adaptive sync; None when unknown."""
    if key == "gamescope_adaptive_sync":
        return None if "vrr" not in mode else bool(mode.get("vrr"))
    return ((gpu or {}).get("auto") or {}).get(key)


# The basic card an advanced card folds into when the page has it; the runner's card stands in for Proton.
HOMES = {"Scaling": "Display", "Environment": "Launch", "Sync": "Proton", "Upscaling": "Proton", "Logs": "Proton", "Artwork": "Desktop and library"}

MAP_NOUNS = {"env": "a variable", "dll_overrides": "an override"}
MAP_FIELDS = {"env": ["Variable", "Value"], "dll_overrides": ["DLL", "Override"]}


def map_rows(section, spec, entries, own=None):
    """A map key's rows: one per entry of `entries`, then the row that adds one. On a game's page `own` is what the game sets;
    the other entries are the global's."""
    key = "launch." + spec["key"]
    advanced = bool(spec.get("advanced"))
    rows = []
    for name, value in (entries or {}).items():
        origin = "" if own is None else "game" if name in own else "global"
        row = _row(section, f"{key}.{name}", name, "string", str(value), detail=spec["description"], advanced=advanced, origin=origin)
        row["entry"] = key
        rows.append(row)
    add = _row(section, key, "Add " + MAP_NOUNS.get(spec["key"], "an entry") + "…", "action", "", detail=spec["description"], advanced=advanced)
    add.update(display="", action="Add", icon="plus", map=True, fields=MAP_FIELDS.get(spec["key"], ["Name", "Value"]))
    rows.append(add)
    return rows


def global_launch_rows(rows, groups, client, config, mode, takes, gpu=None, homes=HOMES):
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
        card = {"caps": True, "meta": _card_meta(section, mode, gpu or {}), "home": homes.get(section, "")}
        if spec["type"] == "map":
            for row in map_rows(section, spec, launch.get(spec["key"])):
                _add(rows, groups, section, row, **card)
            continue
        _add(rows, groups, section, launch_row(section, spec, value, protons=protons, auto_hz=hz, gpu=gpu, mode=mode), **card)


# What gamescope does when a scaling key is left unset.
GAMESCOPE_DEFAULTS = {"gamescope_scaler": "auto", "gamescope_filter": "linear", "gamescope_sharpness": "2"}


def auto_display(spec, value, auto_hz, mode, gpu, origin=""):
    """The value with what it comes to on this machine after a dot: `auto · 144`; None when nothing is known. A scaling key
    left unset reads `default · linear` on the global page; on a game's, whose rows tag their origin, the bare built-in."""
    key = spec["key"]
    if value == "auto":
        if spec["type"] == "fps" and auto_hz:
            return f"auto · {auto_hz}"
        if key == "gamescope_refresh" and int(mode.get("refresh") or 0):
            return f"auto · {int(mode['refresh'])}"
        if key == "gamescope_resolution" and int(mode.get("width") or 0):
            return f"auto · {int(mode['width'])}×{int(mode['height'])}"
        if spec["type"] == "toggle":
            auto = toggle_auto(key, gpu, mode)
            return None if auto is None else "auto · " + ("On" if auto else "Off")
    if value == "default" and key in GAMESCOPE_DEFAULTS:
        return GAMESCOPE_DEFAULTS[key] if origin else f"default · {GAMESCOPE_DEFAULTS[key]}"
    return None


def launch_row(section, spec, value, protons=(), auto_hz=0, gpu=None, mode=None, origin="", global_label=""):
    """One launch key's row. On a game's page `origin` says where `value` came from and `global_label` what the global
    comes to: the choice that clears the game's own value reads `Global · <that>`, on the global page `Default · <built-in>`."""
    kind = ROW_TYPES.get(spec["type"], spec["type"])
    mode = mode or {}
    choices, values = [str(c) for c in spec["choices"]], None
    if spec["type"] in ("enum", "int", "toggle") and choices:
        builtin = spec["default"] or GAMESCOPE_DEFAULTS.get(spec["key"], "")
        clear = ("Global" + (f" · {global_label}" if global_label else "")) if origin else ("Default" + (f" · {builtin}" if builtin else ""))
        choices, values = [clear, *choices], ["", *choices]
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
    row = _row(
        section,
        "launch." + spec["key"],
        spec["label"],
        kind,
        value,
        choices,
        detail=gpu_note(spec, gpu or {}),
        advanced=bool(spec.get("advanced")),
        origin=origin,
    )
    if values:
        row["choiceValues"] = values
    if spec["type"] == "toggle":
        if isinstance(value, bool):
            value = "on" if value else "off"
        row["value"] = row["display"] = str(value)
    shown = auto_display(spec, value, auto_hz, mode, gpu, origin)
    if shown is not None:
        row["display"] = shown
    # A value this page does not set itself: the picker opens on the clearing choice, the row still shows what applies.
    if values and (origin in ("global", "default") or value == "default"):
        row["value"] = choices[0]
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


# A QObject to the type checker only: the host's signals resolve through it, the runtime class stays a plain mixin.
class AdvancedRows(QObject if TYPE_CHECKING else object):
    # The row list and its advanced groups, shown while `_show_advanced`; a gated form opens them from an Advanced row at `_gate`.
    rowsChanged: Signal
    advancedChanged: Signal
    gated = False

    def _init_rows(self):
        self._rows = []
        self._groups = []
        self._gate = -1
        self._has_advanced = False
        self._show_advanced = False

    def _set_rows(self, rows, groups):
        rows, groups = list(rows), list(groups)
        self._gate = -1
        self._has_advanced = any(g.get("advanced") for g in groups)
        if self._has_advanced and self.gated:
            self._gate = len(rows)
            rows.append(advanced_row(self._show_advanced))
        self._rows = rows
        self._groups = groups
        self.rowsChanged.emit()

    def _shown_groups(self):
        if not self._has_advanced:
            return list(self._groups)
        basic = [dict(g) for g in self._groups if not g["advanced"]]
        advanced = [g for g in self._groups if g["advanced"]] if self._show_advanced else []
        homes = [next((b for b in basic if b["title"] and b["title"] == (g["home"] or g["title"])), None) for g in advanced]
        more = [g for g, home in zip(advanced, homes, strict=True) if home is None]
        folded = [(g, home) for g, home in zip(advanced, homes, strict=True) if home is not None]
        # A card's own advanced rows come first, the cards homed in it after them, each under a rule of its own.
        for group, home in sorted(folded, key=lambda pair: bool(pair[0]["home"])):
            at, label = len(home["rows"]), group["title"] if group["home"] else ""
            if home["divider"] < 0:
                home["divider"] = at
                home["dividers"] = [{"at": at, "label": "Advanced" + (f" · {label}" if label else "")}]
            else:
                home["dividers"] = [*home["dividers"], {"at": at, "label": label}]
            home["rows"] = [*home["rows"], *group["rows"]]
        if self._gate < 0:
            return [*basic, *more]
        # `wide`: the gate spans every column, the advanced-only cards flow under it.
        return [*basic, {**_group("", [self._gate]), "wide": True}, *more]

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
    _write: Callable[[dict, Any], Any]
    _reload: Callable[[dict], Any]

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

    # One entry of a map, from its add row or one of its entries: `launch.env.FOO`; an empty value removes it.
    # A name is one key of the map: letters, digits, underscores and dashes (a dot would nest a table).
    @Slot(int, str, str, result=bool)
    def setMapEntry(self, index, name, value):
        row = self.row(index)
        name = re.sub(r"[^A-Za-z0-9_\-]", "", str(name or ""))
        key = row.get("entry") or (row["key"] if row.get("map") else "")
        if not key or not name:
            return False
        ok = self._write({**row, "key": key + "." + name, "type": "string"}, str(value or ""))
        if ok:
            self._reload(row)
        return bool(ok)

    # The row's own value goes: a game's back to the global's or the default, a runner's back to what was found, a map's
    # entry out of the map.
    @Slot(int, result=bool)
    def reset(self, index):
        row = self.row(index)
        if not self.resettable(row):
            return False
        ok = self._write(row, "")
        if ok:
            self._reload(row)
        return bool(ok)

    # A look that rebuilds its rows as JS objects hands one over as a QJSValue.
    @Slot("QVariant", result=bool)
    def resettable(self, row):
        row = (row.toVariant() if hasattr(row, "toVariant") else row) or {}
        return row.get("origin") in ("game", "runner") or (bool(row.get("entry")) and row.get("origin") != "global")

    @Slot(int)
    def toggle(self, index):
        row = self.row(index)
        if row.get("type") == "bool":
            self.setValue(index, not row.get("value"))

    rows = Property(list, lambda self: list(self._rows), notify=rowsChanged)
    groups = Property(list, AdvancedRows._shown_groups, notify=rowsChanged)
    basicGroups = Property(list, lambda self: [g for g in self._groups if not g["advanced"]], notify=rowsChanged)
    advancedGroups = Property(list, lambda self: [g for g in self._groups if g["advanced"]], notify=rowsChanged)
    hasAdvanced = Property(bool, lambda self: self._has_advanced, notify=rowsChanged)
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


def game_launch_rows(game, effective, runners, config_set):
    runner_id = str(effective.get("runner") or "proton")
    spec = _runner_spec(runners, runner_id)
    kind = spec.get("kind") or ""
    rows = []
    names = [r["name"] for r in runners] or [spec["name"]]
    picker = _row("Launch", "launch.runner", "Runner", "enum", spec["name"], names)
    picker["choiceValues"] = [r["id"] for r in runners] or [runner_id]
    picker["valueIcon"] = runner_logo(runner_id)
    picker["icons"] = [runner_logo(v) for v in picker["choiceValues"]]
    rows.append(picker)
    rows.append(_row("Launch", "launch.exe", "File" if kind == "emulator" else "Program", "path", _dig(game, "launch.exe") or ""))
    if kind == "emulator":
        platforms = list(spec.get("platforms") or [])
        if len(platforms) > 1:
            rows.append(_row("Launch", "platform", "Platform", "enum", game.get("platform") or platforms[0], platforms))
        runner_set = (config_set.get("runners") or {}).get(runner_id) or {}
        rows.append(
            _row(
                "Launch",
                "launch.runner_exe",
                spec["name"] + " program",
                "path",
                _dig(game, "launch.runner_exe") or effective.get("runner_path") or "",
                origin=origin_of(_dig(game, "launch.runner_exe"), runner_set.get("exe")) if effective.get("runner_path") else "",
            )
        )
        options = effective.get("options") or {}
        own = _dig(game, "launch.options") or {}
        for option in spec.get("options") or []:
            key = option["key"]
            value = options.get(key, option.get("default"))
            if option.get("type") == "bool":
                value = bool(value)
            rows.append(
                _row(
                    "Launch",
                    f"launch.options.{key}",
                    option.get("label", key),
                    option.get("type", "string"),
                    value,
                    option.get("choices"),
                    origin="game" if key in own else "global" if key in runner_set else "default",
                )
            )
    return rows, spec["name"], kind


# A game-scope key whose empty value the launch fills in itself: the row shows what `effective` says as its default.
COMPUTED = ("working_dir", "prefix")


def build_game(client, game_id, screen_mode):
    """A game's settings rows: the launch cards, the runner's, the program, the library flags, then each module's game settings."""
    game = client.game(game_id)
    config = client.config()
    config_set = config.get("set") or {}
    title = str(game.get("title") or game_id)
    effective = game.get("effective") or {}
    launch, runner_name, runner_kind = game_launch_rows(game, effective, client.runners(), config_set)
    rows, groups = [], []
    mode = screen_mode()
    protons = proton_choices(config)
    hz = auto_rate(mode, effective.get("gamescope", True), effective.get("gamescope_refresh"))
    gpu = client.gpu()
    for spec in client.launchKeys("game", mode):
        if spec["runners"] and runner_kind not in spec["runners"]:
            continue
        key = spec["key"]
        own = _dig(game, "launch." + key)
        section = runner_name if spec["section"] == "Proton" else spec["section"]
        card = {"caps": True, "meta": _card_meta(section, mode, gpu), "home": HOMES.get(section, "")}
        if card["home"] == "Proton":
            card["home"] = runner_name
        if spec["type"] == "map":
            own = dict(own) if isinstance(own, dict) else {}
            merged = {**(_dig(config, "launch." + key) or {}), **own} if spec["scope"] == "both" else own
            for row in map_rows(section, spec, merged, own):
                _add(rows, groups, section, row, **card)
            continue
        value, origin, global_label = own, "", ""
        if spec["scope"] == "both":
            own_global = _dig(config_set, "launch." + key)
            origin = origin_of(own, own_global)
            # The global's own value, else the built-in: what clearing the game's comes to.
            global_value = own_global if _is_set(own_global) else spec["default"]
            if spec["type"] == "toggle":
                global_label = "on" if global_value is True else "off" if global_value is False else str(global_value)
            elif spec["type"] in ("enum", "int"):
                global_label = str(global_value) if _is_set(global_value) else GAMESCOPE_DEFAULTS.get(key, "default")
            if origin != "game":
                # A toggle's effective value is what auto came to: the row inherits the global switch itself.
                # The global's gamescope arguments are not merged into `effective`: they come from the config.
                value = global_value if spec["type"] == "toggle" else _dig(config, "launch." + key) if key == "gamescope_args" else effective.get(key)
        elif key in COMPUTED:
            origin = origin_of(own, None)
            if origin != "game":
                value = effective.get(key) or ""
        row = launch_row(section, spec, value, protons=protons, auto_hz=hz, gpu=gpu, mode=mode, origin=origin, global_label=global_label)
        if section == "Launch":
            launch.append(row)
            continue
        _add(rows, groups, section, row, **card)
    for row in launch:
        _add(rows, groups, "Launch", row, caps=True)
    for section, key, label, kind, advanced in CORE_ROWS:
        value = _dig(game, key)
        origin = ""
        if key == "desktop.hide_cursor":
            origin = origin_of(value, _dig(config_set, key))
            if origin != "game":
                value = effective.get("hide_cursor")
        if kind == "bool":
            value = bool(value)
        _add(
            rows,
            groups,
            section,
            _row(section, key, label, kind, value, origin=origin, detail=HIDE_CURSOR if key == "desktop.hide_cursor" else "", advanced=advanced),
            caps=True,
            home=HOMES.get(section, ""),
        )
    modules = {m["id"]: m for m in client.modules()}
    own_modules = game.get("modules") or {}
    set_modules = config_set.get("modules") or {}
    for module_id, values in client.settings(game_id).items():
        module = modules.get(module_id) or {}
        name = module.get("name", module_id)
        own, global_set = own_modules.get(module_id) or {}, set_modules.get(module_id) or {}
        for setting in module.get("settings") or []:
            if setting.get("scope") != "game":
                continue
            key = setting["key"]
            value = values.get(key, setting.get("default"))
            _add(
                rows,
                groups,
                name,
                _row(
                    name,
                    key,
                    setting.get("label", key),
                    setting.get("type", "string"),
                    value,
                    setting.get("choices"),
                    module_id,
                    advanced=bool(setting.get("advanced")),
                    origin="game" if key in own else "global" if key in global_set else "default",
                ),
                meta=_meta(module),
            )
    return rows, groups, title


class GameSettingsForm(RowsForm):
    gameIdChanged = Signal()
    titleChanged = Signal()

    def __init__(self, client, screen_mode: Callable[[], dict] = dict, parent=None):
        super().__init__(client, parent)
        self._screen_mode = screen_mode
        self._game_id = ""
        self._title = ""

    @Slot(str)
    def load(self, game_id):
        self._set_show_advanced(False)
        self._game_id = game_id
        self.gameIdChanged.emit()
        self._refresh()

    def _refresh(self):
        rows, groups, self._title = build_game(self._client, self._game_id, self._screen_mode)
        self.titleChanged.emit()
        self._set_rows(rows, groups)

    def _write(self, row, payload):
        if row["module"]:
            return self._client.setSetting(row["module"], self._game_id, row["key"], payload)
        return self._client.set(self._game_id, row["key"], payload)

    def _reload(self, row):
        self._refresh()

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
    _client: "CoreClient"

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
    _client: "CoreClient"

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
    source: bool
    _entries: Callable[[], list]
    _enable: Callable[[str, bool], Any]

    def _show(self, entries):
        rows, on, off = [], [], []
        for entry in entries:
            ident = entry["id"]
            name = entry.get("name", ident)
            enabled = bool(entry.get("enabled"))
            warning = _state(entry)
            row = _row(self.section, "module", name, "action", enabled, module=ident)
            row.update(
                display="Unavailable" if warning else "On" if enabled else "Off",
                action="Open",
                runner="",
                switch=True,
                meta=_meta(entry),
                warning=warning,
                source=self.source,
                detail=warning.replace("unavailable", "On, but its hooks are skipped" if enabled else "Cannot be enabled", 1) if warning else _meta(entry),
            )
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
            rows.append(_row(name, "", check.get("check", ""), "info", bool(check.get("ok")), detail=str(check.get("detail") or ""), module=ident))
        groups.sort(key=lambda g: g["title"] != "Core")
        for group in groups:
            passed = sum(1 for i in group["rows"] if rows[i]["value"])
            group["meta"] = f"{passed} of {len(group['rows'])} checks pass"
        self._doctor = rows
        self._doctor_groups = groups
        self.doctorChanged.emit()

    doctor = Property(list, lambda self: list(self._doctor), notify=doctorChanged)
    doctorGroups = Property(list, lambda self: list(self._doctor_groups), notify=doctorChanged)


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
    return {
        "id": ident,
        "name": name,
        "meta": _meta(entry),
        "description": str(entry.get("description") or ""),
        "warning": _state(entry),
        "enabled": bool(entry.get("enabled")),
        "source": api.source,
        "logged_in": bool(entry.get("logged_in")),
        "user": str(entry.get("user") or ""),
    }


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
    groups = [_group("Settings", [0], caps=True)]
    if not enabled:
        return info, rows, groups
    values = api._settings(ident)
    for setting in entry.get("settings") or []:
        if setting.get("scope") not in ("global", "config"):
            continue
        key = setting["key"]
        choices = [str(c) for c in setting.get("choices") or []]
        if setting.get("dynamic"):
            choices = choices_of(ident, key, values) or choices
        row = _row(
            name,
            key,
            setting.get("label", key),
            setting.get("type", "string"),
            values.get(key, setting.get("default")),
            choices,
            ident,
            advanced=bool(setting.get("advanced")) or setting.get("scope") == "config",
        )
        _add(rows, groups, "Settings", row, caps=True)
    if api.source:
        signin_rows(entry, name, rows, groups)
    return info, rows, groups


class PageForm(RowsForm):
    # A `dynamic` setting's choices are fetched once per state of the entry's settings.
    moduleChanged = Signal()
    _entries: Callable[[], list]
    _enable: Callable[[str, bool], Any]
    _choices: Callable[[str, str], Any]
    _set: Callable[[str, str, Any], Any]

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
            self._refresh()

    @Slot(str)
    def load(self, ident):
        self._set_show_advanced(False)
        self._ident = ident
        self._refresh()

    def _refresh(self):
        self._set_rows(*self._build(self._ident, self._entries()))
        self.moduleChanged.emit()

    def _build(self, ident, entries):
        self._module, rows, groups = build_page(self, ident, entries, self._dynamic_choices)
        return rows, groups

    info = Property(QVARIANT, lambda self: dict(self._module), notify=moduleChanged)

    def _write(self, row, payload):
        if row["key"] == "enabled":
            self._enable(row["module"], payload == "true")
            return True
        return self._set(row["module"], row["key"], payload)

    def _reload(self, row):
        self._refresh()


class ModuleForm(ModuleApi, PageForm):
    def __init__(self, client, parent=None):
        super().__init__(client, parent)
        client.modulesChanged.connect(self.reload)


class SourceForm(SourceApi, PageForm):
    def __init__(self, client, parent=None):
        super().__init__(client, parent)
        client.sourcesChanged.connect(self.reload)

    def _refresh(self):
        ident = self._ident

        def done(entries):
            if self._ident == ident:
                self._set_rows(*self._build(ident, entries))
                self.moduleChanged.emit()

        self._client.runAsync(self._entries, done)
