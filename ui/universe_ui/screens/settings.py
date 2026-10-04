# A row's `type` is bool, enum, string, path, int, info or action; a group's `rows` and `control` index the flat row list.
# An `advanced` row sits in an `advanced` group, shown while the form's `showAdvanced` is set (`load` clears it): folded into
# the basic group titled like it or like its `home` — its rows after the group's `divider`, each folded group ruled off by
# one of `dividers` — or, with no such group, as a group of its own after the basic ones. A gated form (the controller's)
# appends an Advanced action row that opens them; the other pages flip `showAdvanced` from a button.
# A settings form's rows are the core's fields (`Core.form`), laid out and worded here: `field` is the key `set_field`
# takes, `resettable` and `promotable` what a reset and "every game" may do. On a game's or a runner's page `origin` is
# where the value comes from (game, runner, global, default, found; empty when nothing is inherited), `inherited` that it
# comes from elsewhere, and `changed` that the page sets it itself: a shown group's `changed` says it holds one, its
# advanced rows counted, and with `showAdvanced` off the advanced rows holding one still fold in. A map field is one row per
# entry, `entry` naming the map, then an `action` row with `map` set that adds one; an entry's empty value removes it.
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
# The pages whose rows tell what they set themselves from what they inherit.
SCOPED = ("game", "runner")


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
        "inherited": inherited,
        "advanced": advanced,
        "origin": origin,
    }


def _plural(n, word):
    return f"{n} {word}{'' if n == 1 else 's'}"


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
        "changed": False,
    }


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


ROW_TYPES = {"resolution": "string", "refresh": "int", "fps": "string", "proton": "enum", "list": "string", "toggle": "enum", "secret": "string"}
# The pickers that open on a choice clearing the page's own value: `Global · <what that comes to>`.
CLEARING = ("enum", "int", "toggle")
ORIGIN_WORDS = {"game": "This game", "runner": "Runner", "global": "Global", "default": "Default", "found": "Found"}


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


UPSCALING = "Upscaling"

# The basic card an advanced card folds into when the page has it; the runner's card stands in for Proton.
HOMES = {"Scaling": "Display", "Environment": "Launch", "Sync": "Proton", "Upscaling": "Proton", "Logs": "Proton", "Artwork": "Desktop and library"}

MAP_NOUNS = {"env": "a variable", "dll_overrides": "an override"}
MAP_FIELDS = {"env": ["Variable", "Value"], "dll_overrides": ["DLL", "Override"]}


def _label_of(field, value):
    return next((c["label"] for c in field["choices"] if c["value"] == value), value)


def _reads(field, value: str) -> str:
    """`value` as a row reads it: `on` is On, a resolution's `x` a `×`."""
    if field["type"] == "toggle":
        return {"on": "On", "off": "Off"}.get(value, value)
    return value.replace("x", "×") if field["type"] == "resolution" else value


def _shown(field, page):
    """What the row's value line reads: a choice by its label, `auto` with what it comes to after a dot, a scaling key left
    unset by what gamescope does then (`default · linear` where nothing tags the row's origin)."""
    kind, value, resolved = field["type"], field["value"], field["resolved"]
    if kind == "bool":
        return "On" if value == "true" else "Off"
    if kind == "secret":
        return "Set" if value else "—"
    if value == "auto" and resolved:
        return "auto · " + _reads(field, resolved)
    if value == "" and resolved:
        return resolved if page in SCOPED else "default · " + resolved
    if kind == "list":
        return ", ".join(v for v in value.split(",") if v) or "—"
    return _display(kind, _label_of(field, value))


def _clearing(field):
    """The picker's first choice, which clears the page's own value: where the value then comes from, and what it reads."""
    word = ORIGIN_WORDS.get(field.get("fallback") or "default", "Default")
    shown = _label_of(field, field["inherited"]) if field["inherited"] else field.get("inherited_resolved") or ""
    return word + (f" · {shown}" if shown else "")


def _detail(field):
    fits = field.get("fits")
    if fits is None:
        return field["description"]
    return field["description"] + (" Works on your GPU." if fits else " Not for your GPU.")


def field_row(field, section, page, module=""):
    """A field's row on a `page` (game, runner, launch, module, source): `module` names a module's or source's own page; on
    a game's page a module's setting carries its module and its own key."""
    key = field["key"]
    if page == "game" and key.startswith("modules."):
        _, module, key = key.split(".", 2)
    kind = ROW_TYPES.get(field["type"], field["type"])
    scoped = page in SCOPED
    origin = (field.get("origin") or "") if scoped else ""
    value = field["value"] == "true" if kind == "bool" else field["value"]
    row = _row(section, key, field["label"], kind, value, module=module, detail=_detail(field), advanced=field["advanced"], origin=origin)
    row.update(
        field=field["key"],
        display=_shown(field, page),
        inherited=bool(origin) and not field["own"],
        originLabel=ORIGIN_WORDS.get(origin, "") if origin and not field["own"] else "",
        changed=scoped and bool(field["resettable"]),
        resettable=bool(field["resettable"]),
        promotable=bool(field.get("promotable")),
        reach=str(field.get("reach") or ""),
        keywords=list(field.get("keywords") or []),
    )
    if field["type"] == "secret":
        row["secret"] = True
    labels, values = [c["label"] for c in field["choices"]], [c["value"] for c in field["choices"]]
    if labels:
        clears = field.get("origin") is not None and field["type"] in CLEARING
        if clears:
            labels, values = [_clearing(field), *labels], ["", *values]
        row["choices"] = labels
        if values != labels:
            row["choiceValues"] = values
        row["value"] = labels[0] if clears and not field["own"] else _label_of(field, field["value"])
    return row


def entry_rows(field, section, page, module=""):
    """A map field's rows: one per entry (on a game's page the game's own or the global's), then the row that adds one."""
    short = field["key"].rsplit(".", 1)[-1]
    rows = []
    for entry in field["entries"]:
        origin = entry["origin"] if page in SCOPED else ""
        row = _row(
            section,
            f"{field['key']}.{entry['name']}",
            entry["name"],
            "string",
            entry["value"],
            module=module,
            detail=field["description"],
            advanced=field["advanced"],
            origin=origin,
        )
        row.update(
            field=row["key"],
            entry=field["key"],
            inherited=origin == "global",
            originLabel=ORIGIN_WORDS["global"] if origin == "global" else "",
            changed=origin == "game",
            resettable=bool(entry["resettable"]),
            promotable=bool(entry["promotable"]),
            reach=str(field.get("reach") or ""),
        )
        rows.append(row)
    add = _row(
        section,
        field["key"],
        "Add " + MAP_NOUNS.get(short, "an entry") + "…",
        "action",
        "",
        module=module,
        detail=field["description"],
        advanced=field["advanced"],
    )
    add.update(
        field=field["key"],
        display="",
        action="Add",
        icon="plus",
        map=True,
        fields=MAP_FIELDS.get(short, ["Name", "Value"]),
        reach=str(field.get("reach") or ""),
    )
    rows.append(add)
    return rows


def form_rows(fields, page, place, module=""):
    """Every field's rows, each through `place(section, row)` in field order; `module` as `field_row` takes it."""
    for field in fields:
        if field["type"] == "map":
            for row in entry_rows(field, field["section"], page, module):
                place(field["section"], row)
        else:
            place(field["section"], field_row(field, field["section"], page, module))


def launch_cards(fields, rows, groups, mode, gpu, homes=HOMES, page="launch", name=""):
    """The launch keys' cards: caps titles, the screen on Display, the GPU on Upscaling, each advanced card's home. `name`
    titles the Proton card and its home."""

    def place(section, row):
        title = name if name and section == "Proton" else section
        home = homes.get(section, "")
        _add(
            rows,
            groups,
            title,
            {**row, "section": title},
            caps=True,
            meta=_card_meta(section, mode, gpu or {}),
            home=name if name and home == "Proton" else home,
        )

    form_rows(fields, page, place)


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

    def _changed(self, group):
        return any(self._row_at(i).get("changed") for i in group["rows"])

    def _shown_groups(self):
        if not self._has_advanced:
            return [{**g, "changed": self._changed(g)} for g in self._groups]
        basic = [{**g, "changed": self._changed(g)} for g in self._groups if not g["advanced"]]
        every = [g for g in self._groups if g["advanced"]]
        homes = [next((b for b in basic if b["title"] and b["title"] == (g["home"] or g["title"])), None) for g in every]
        for group, home in zip(every, homes, strict=True):
            if home is not None and self._changed(group):
                home["changed"] = True
        advanced = list(zip(every, homes, strict=True))
        if not self._show_advanced:
            changes = [({**g, "rows": [i for i in g["rows"] if self._row_at(i).get("changed")]}, home) for g, home in advanced]
            advanced = [(g, home) for g, home in changes if g["rows"]]
        more = [{**g, "changed": self._changed(g)} for g, home in advanced if home is None]
        folded = [(g, home) for g, home in advanced if home is not None]
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
        key = row.get("entry") or (row.get("field") or row["key"] if row.get("map") else "")
        if not key or not name:
            return False
        ok = self._write({**row, "key": key + "." + name, "field": key + "." + name, "type": "string"}, str(value or ""))
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
        return bool(row.get("resettable"))

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


def _launch_last(fields):
    """A game's Launch card follows the launch keys' cards, ahead of the library's."""
    launch = [f for f in fields if f["section"] == "Launch"]
    rest = [f for f in fields if f["section"] != "Launch"]
    at = next((i for i, f in enumerate(rest) if f["section"] == "Desktop and library"), len(rest))
    return [*rest[:at], *launch, *rest[at:]]


def build_game(client, game_id, screen_mode):
    """A game's settings rows: the launch cards, the runner's, the program, the library flags, then each module's game
    settings and its source's, as the core's game form lists them."""
    game = client.game(game_id)
    title = str(game.get("title") or game_id)
    mode = screen_mode()
    fields = client.form("game", game_id, mode)
    if not fields:
        return [], [], title
    gpu = client.gpu()
    effective = game.get("effective") or {}
    runner = effective.get("runner") or ""
    name = str(effective.get("runner_name") or runner)
    owners = {f"modules.{m['id']}.": m for m in client.modules()}
    owners.update({f"sources.{s['id']}.": s for s in client.sources()})
    rows, groups = [], []
    for field in _launch_last(fields):
        owner = next((o for prefix, o in owners.items() if field["key"].startswith(prefix)), None)
        if owner is None:
            launch_cards([field], rows, groups, mode, gpu, page="game", name=name)
            continue
        row = field_row(field, field["section"], "game")
        _add(rows, groups, field["section"], row, meta=_meta(owner))
    picker = next((r for r in rows if r["key"] == "launch.runner"), None)
    if picker is not None:
        picker["valueIcon"] = runner_logo(runner)
        picker["icons"] = [runner_logo(v) for v in picker.get("choiceValues") or picker["choices"]]
    return rows, groups, title


class GameSettingsForm(RowsForm):
    gameIdChanged = Signal()
    titleChanged = Signal()
    message = Signal(str)

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
        return self._client.setField("game", self._game_id, row["field"], payload)

    def _reload(self, row):
        self._refresh()

    @Slot("QVariant", result=bool)
    def promotable(self, row):
        row = (row.toVariant() if hasattr(row, "toVariant") else row) or {}
        return bool(row.get("promotable"))

    # The game's value becomes the global one and the game's own goes; other games keep theirs.
    @Slot(int, result=bool)
    def promote(self, index):
        row = self.row(index)
        if not self.promotable(row):
            return False
        return self._for_all(row, lambda: self._client.promoteField("game", self._game_id, row["field"]))

    # The value for every game the row's key reaches (its `reach`), this one included; games that set their own keep theirs.
    @Slot(int, "QVariant", result=bool)
    def setValueAll(self, index, value):
        row = self.row(index)
        if not row.get("reach"):
            return False
        return self._for_all(row, lambda: self._client.setFieldAll("game", self._game_id, row["field"], _to_bus(row, value)))

    @Slot(int, result=bool)
    def toggleAll(self, index):
        row = self.row(index)
        return row.get("type") == "bool" and self.setValueAll(index, not row.get("value"))

    @Slot(int, str, str, result=bool)
    def setMapEntryAll(self, index, name, value):
        row = self.row(index)
        name = re.sub(r"[^A-Za-z0-9_\-]", "", str(name or ""))
        key = row.get("entry") or (row.get("field") if row.get("map") else "")
        if not key or not name or not row.get("reach"):
            return False
        entry = {**row, "label": name, "field": f"{key}.{name}"}
        return self._for_all(entry, lambda: self._client.setFieldAll("game", self._game_id, entry["field"], str(value or "")))

    def _for_all(self, row, write):
        ok = bool(write())
        self._reload(row)
        shown = next((r["display"] for r in self._rows if r.get("field") == row["field"]), "")
        if ok and shown:
            shown = shown.lower() if row.get("type") == "bool" else shown
            self.message.emit(f"{row['label']} is now {shown} for all {row['reach']} without their own")
        return ok

    gameId = Property(str, lambda self: self._game_id, notify=gameIdChanged)
    title = Property(str, lambda self: self._title, notify=titleChanged)


def _state(entry):
    if not entry.get("available", True):
        if entry.get("incompatible"):
            return "unavailable: " + str(entry["incompatible"])
        missing = ", ".join(entry.get("missing") or [])
        return "unavailable" + (f": missing {missing}" if missing else "")
    return ""


def _setup(entry):
    """On, but a setting it cannot guess is still empty: its hooks run and do nothing."""
    if not entry.get("enabled") or not entry.get("unset"):
        return ""
    labels = {s["key"]: s.get("label", s["key"]) for s in entry.get("settings") or []}
    return "waiting on " + ", ".join(labels.get(k, k).lower() for k in entry["unset"])


class ModuleApi:
    source = False
    kind = "Modules"
    form = "module"
    _client: "CoreClient"

    def _entries(self):
        return self._client.modules()

    def _choices(self, ident, key):
        return self._client.settingChoices(ident, key)


class SourceApi:
    source = True
    kind = "Sources"
    form = "source"
    _client: "CoreClient"

    def _entries(self):
        return self._client.sources()

    def _choices(self, ident, key):
        return self._client.sourceSettingChoices(ident, key)


class ListForm(RowsForm):
    section = "Modules"
    source: bool
    _entries: Callable[[], list]

    def _show(self, entries):
        rows, on, off = [], [], []
        for entry in entries:
            ident = entry["id"]
            name = entry.get("name", ident)
            enabled = bool(entry.get("enabled"))
            warning = _state(entry)
            setup = _setup(entry)
            row = _row(self.section, "module", name, "action", enabled, module=ident)
            row.update(
                display="Unavailable" if warning else "Set it up" if setup else "On" if enabled else "Off",
                action="Open",
                runner="",
                switch=True,
                meta=_meta(entry),
                warning=warning,
                source=self.source,
                detail=warning.replace("unavailable", "On, but its hooks are skipped" if enabled else "Cannot be enabled", 1)
                if warning
                else f"On, {setup}"
                if setup
                else _meta(entry),
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
        self._client.setField("source" if self.source else "module", row["module"], "enabled", "false" if row["value"] else "true")
        self.load()


DOCTOR_GROUPS = {"core": "Core", "runners": "Runners", "media": "Media", "controller": "Controller"}


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
        names = {**DOCTOR_GROUPS, **names}
        rows, homes = [], {}
        for check in checks:
            ident = check.get("module") or "core"
            name = names.get(ident, ident)
            row = _row(
                name, "", check.get("label") or check.get("check", ""), "info", bool(check.get("ok")), detail=str(check.get("detail") or ""), module=ident
            )
            row["fix"] = str(check.get("fix") or "")
            row["component"] = str(check.get("component") or "")
            homes.setdefault(name, []).append(len(rows))
            rows.append(row)
        failing = [i for i in range(len(rows)) if not rows[i]["value"]]
        for i in failing:
            rows[i]["path"] = rows[i]["section"]
        groups = []
        if failing:
            groups.append(_group("Needs attention", failing, warning=f"{len(failing)} of {len(rows)} checks fail"))
        for name in sorted(homes, key=lambda n: n != "Core"):
            passing = [i for i in homes[name] if rows[i]["value"]]
            if passing:
                groups.append(_group(name, passing, meta=f"{len(passing)} of {len(homes[name])} checks pass"))
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
        "setup": _setup(entry),
        "enabled": bool(entry.get("enabled")),
        "source": api.source,
        "logged_in": bool(entry.get("logged_in")),
        "user": str(entry.get("user") or ""),
    }


def login_words(source):
    """How a source's sign-in reads, from its `login` table: an API key as one, a code as a sign-in."""
    login = source.get("login") or {}
    key = login.get("kind") == "key"
    name = source.get("name") or source.get("id", "")
    return {
        "kind": "key" if key else "code",
        "link": "Get an API key" if key else "Get a sign-in link",
        "go": "Open" if key else "Sign in",
        "enter": "Enter the key" if key else "Enter the code",
        "prompt": f"API key from {name}" if key else f"Code from {name}",
        "checking": "Checking the key…" if key else "Checking the code…",
        "signed_out": ("Add an API key to " if key else "Sign in to ") + str(login.get("purpose") or "install games"),
        "hint": str(login.get("hint") or ""),
    }


def login_rows(source, section):
    """The link and code rows of a source's sign-in, worded by `login_words`; `login` is the kind, for the looks and tests."""
    words = login_words(source)
    link = _row(section, "link", words["link"], "action", "", module=source["id"])
    code = _row(section, "code", words["enter"], "action", "", module=source["id"])
    return (
        {**link, "action": words["go"], "display": "", "login": words["kind"]},
        {**code, "action": "Enter", "display": "", "login": words["kind"], "prompt": words["prompt"]},
    )


def signin_rows(entry, name, rows, groups):
    logged_in = bool(entry.get("logged_in"))
    user = str(entry.get("user") or "")
    signin = _group("Sign-in", [], caps=True)
    for row in (
        _row(name, "logged_in", "Signed in", "info", logged_in, module=entry["id"], detail=user or ("yes" if logged_in else "no")),
        *login_rows(entry, name),
    ):
        signin["rows"].append(len(rows))
        rows.append(row)
    groups.append(signin)


def build_page(api, ident, entries, choices_of=lambda ident, key, values: None, fields=None):
    """A module's or a source's page: the switch, a source's sign-in, then the settings of the core's form, the advanced and
    config-only ones behind the gate. `choices_of` answers a dynamic setting's choices, or None while they are not known;
    `fields` is the form when the caller has it."""
    entry = next((m for m in entries if m["id"] == ident), None)
    if entry is None:
        return {}, [], []
    info = page_info(api, entry, ident)
    name, enabled = info["name"], info["enabled"]
    control = _row(name, "enabled", "Enabled", "bool", enabled, module=ident)
    control.update(field="enabled", disabled=bool(info["warning"]) and not enabled)
    rows = [control]
    groups = [_group("Settings", [0], caps=True)]
    if not enabled:
        return info, rows, groups
    if fields is None:
        fields = api._client.form(api.form, ident, None)
    values = {f["key"]: f["value"] for f in fields}
    for field in fields:
        if field["key"] == "enabled":
            continue
        known = choices_of(ident, field["key"], values) if field.get("dynamic") else None
        if known is not None:
            labels = {c["value"]: c["label"] for c in field["choices"]}
            known = [{"value": c, "label": labels.get(c, c)} for c in known]
        _add(rows, groups, field["section"], field_row({**field, "choices": known or field["choices"]}, name, api.form, ident), caps=True)
    if api.source:
        signin_rows(entry, name, rows, groups)
    return info, rows, groups


class PageForm(RowsForm):
    # A `dynamic` setting's choices are fetched once per state of the entry's settings.
    moduleChanged = Signal()
    form: str
    _entries: Callable[[], list]
    _choices: Callable[[str, str], Any]

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

    def _build(self, ident, entries, fields=None):
        self._module, rows, groups = build_page(self, ident, entries, self._dynamic_choices, fields)
        return rows, groups

    info = Property(QVARIANT, lambda self: dict(self._module), notify=moduleChanged)

    def _write(self, row, payload):
        return self._client.setField(self.form, row["module"], row["field"], payload)

    @Slot(result=int)
    def setupIndex(self):
        """Row of the first setting this module is waiting on, so switching it on lands the cursor there; -1 when it needs nothing."""
        entry = next((m for m in self._entries() if m.get("id") == self._ident), None)
        for key in (entry or {}).get("unset") or []:
            index = self._reveal(key, self._ident)
            if index >= 0:
                return index
        return -1

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

        def done(result):
            entries, fields = result
            if self._ident == ident:
                self._set_rows(*self._build(ident, entries, fields))
                self.moduleChanged.emit()

        self._client.runAsync(lambda: (self._entries(), self._client.form("source", ident, None)), done)
