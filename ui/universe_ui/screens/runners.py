import os
import re
from collections.abc import Callable

from PySide6.QtCore import Signal, Slot

from ..models import file_url
from ..qt import QVARIANT, Property
from .components import NO_BUILD, NO_BUILD_META, build_text, catalogue_warning
from .settings import HOMES, RowsForm, _group, _plural, _row, _to_bus, global_launch_rows, launch_row, proton_choices, runner_logo

FOUND = {"path": "Found on PATH"}
PROTON_TOOL = "umu-run"
NOT_INSTALLED = "Not installed"
NOT_INSTALLED_META = "Universe downloads and updates these"
NOT_FOUND = "Not found"
TOOLS = "Tools"
BUILDS = "Builds"
# Launch keys picked on the Builds card: Proton's lists every build found, a family's or not.
BUILD_KEYS = ("proton",)


def suggested_title(path):
    base = os.path.basename(str(path or "").rstrip("/"))
    stem = base if os.path.isdir(str(path or "")) else os.path.splitext(base)[0]
    stem = re.sub(r"\[[^\]]*\]|\([^)]*\)", " ", stem.replace("_", " "))
    words = [w for w in stem.split() if not re.fullmatch(r"v\d[\d.]*", w)]
    return " ".join(words).rstrip("- ").strip()


def _found(runner):
    return runner.get("kind") == "linux" or bool(runner.get("path"))


def _runner_of(game):
    return str((game.get("effective") or {}).get("runner") or (game.get("launch") or {}).get("runner") or "")


def _play_time(hours):
    seconds = float(hours or 0) * 3600
    if seconds <= 0:
        return ""
    if seconds < 3600:
        return f"{max(1, round(seconds / 60))} min"
    return f"{seconds / 3600:.1f} h"


def runner_components(ident, kind, components):
    """What a runner's page installs: Proton's builds and umu-run, which starts them; another runner its own."""
    if kind == "proton":
        return [c for c in components if c["kind"] == "proton"] + [c for c in components if c["id"] == PROTON_TOOL]
    return [c for c in components if c.get("runner") == ident]


def _in_use_first(components):
    return sorted(components, key=lambda c: (not c.get("in_use"), c["kind"] != "proton", not c.get("builds"), c["name"].lower()))


def _state(components, row_of):
    """A runner's row: the build in use, and what to do about its components, the ones in use first."""
    ordered = _in_use_first(components)
    rows = [row_of(c) for c in ordered]
    lead = next((r for r in rows if r["tag"]), rows[0] if rows else None)
    if lead is None:
        return {}
    in_use = next((c["in_use"] for c in ordered if c.get("in_use")), None)
    return {
        "tag": lead["tag"],
        "accent": any(c.get("update") or c.get("proposal") for c in ordered),
        "size": lead["size"],
        "progress": max(r["progress"] for r in rows),
        "detail": build_text(in_use) if in_use else "",
        "component": lead["component"],
    }


def _usage(games):
    usage = {}
    for game in games:
        ident = _runner_of(game)
        if ident:
            count, hours = usage.get(ident, (0, 0.0))
            usage[ident] = (count + 1, hours + float((game.get("stats") or {}).get("hours") or 0))
    return usage


def _needs_doing(c):
    return (c.get("proposal") != "install", not (c.get("update") or c.get("proposal")), not c.get("in_use"), c["name"].lower())


def build_runners(runners, games, listing, row_of):
    """Settings › Runners: the runners found, by the games on them; those Universe can install, the ones a game waits on first;
    those it cannot, dimmed; the tools last."""
    usage = _usage(games)
    components = listing.get("components") or []
    loaded = bool(components)
    runners = sorted(runners, key=lambda r: (-usage.get(r["id"], (0, 0.0))[0], -usage.get(r["id"], (0, 0.0))[1], r.get("name", r["id"]).lower()))
    found, offered, none = [], [], []
    for runner in runners:
        ident = runner["id"]
        own = runner_components(ident, runner.get("kind"), components)
        count = usage.get(ident, (0, 0.0))[0]
        row = _row("Runners", "runner", runner.get("name", ident), "action", "", module=ident)
        row.update(display=_plural(count, "game") if count else "", icon=runner_logo(ident), iconSlot=True, runner=ident, action="Open")
        row.update({"tag": "", "accent": False, "size": "", "progress": 0, "component": "", **_state(own, row_of)})
        if _found(runner):
            found.append(row)
        elif any(c.get("latest") for c in own):
            offered.append((not any(c.get("proposal") == "install" for c in own), row))
        else:
            none.append(row)
    tools = sorted((c for c in components if c["kind"] in ("tool", "system") and c["id"] != PROTON_TOOL), key=_needs_doing)
    rows, groups = [], []

    def card(title, members, **group):
        if members:
            at = len(rows)
            rows.extend(members)
            groups.append(_group(title, range(at, len(rows)), **group))

    card("", found)
    card(NOT_INSTALLED, [row for _, row in sorted(offered, key=lambda o: o[0])], meta=NOT_INSTALLED_META)
    card(NO_BUILD if loaded else NOT_FOUND, none, meta=NO_BUILD_META if loaded else "", off=True)
    card(TOOLS, [{**row_of(c), "section": TOOLS} for c in tools], meta=f"{sum(1 for c in tools if c.get('in_use'))} of {len(tools)} installed")
    if groups:
        groups[0]["warning"] = catalogue_warning(listing)
    return rows, groups


class RunnersForm(RowsForm):
    def __init__(self, client, components, parent=None):
        super().__init__(client, parent)
        self._components = components
        client.libraryChanged.connect(lambda ids: self._build() if self._rows else None)
        components.listingChanged.connect(lambda: self._build() if self._rows else None)

    @Slot()
    def load(self):
        self._build()
        self._components.load()

    # Y: the catalogue fetched again.
    @Slot()
    def refresh(self):
        self._components.refresh()

    def _build(self):
        rows, groups = build_runners(self._client.runners(), self._client.list(), self._components.listing(), self._components.row)
        self._set_rows(rows, groups)

    @Slot(str, result=str)
    def logo(self, ident):
        return runner_logo(ident)

    @Slot(str, result=int)
    def indexOf(self, ident):
        return next((i for i, r in enumerate(self._rows) if (r.get("runner") or r.get("component")) == ident), -1)


def build_runner(client, ident, screen_mode, listing=None, row_of: Callable[[dict], dict] = dict):
    """A runner's page: its program and gamescope, its builds (Proton's default, what Universe installs of it), the global launch
    keys of its kind (the advanced ones folded into the Proton card, else the runner's), its options, its games. Without
    `listing` (the search) the Builds card holds no component."""
    runner = next((r for r in client.runners() if r["id"] == ident), None)
    if runner is None:
        return {}, [], []
    name = runner.get("name", ident)
    found = runner.get("path") or ""
    source = runner.get("source") or ""
    kind = runner.get("kind") or ""
    platforms = ", ".join(runner.get("platforms") or [])
    if kind == "linux":
        meta, warning = platforms, ""
    elif found:
        meta = f"{platforms} · {found}" + (f" ({source})" if source and source != "path" else "")
        warning = "" if runner.get("available", True) else "Proton not found"
    else:
        meta, warning = platforms, "not found"
    info = {"id": ident, "name": name, "meta": meta, "warning": warning, "icon": runner_logo(ident)}
    own = runner_components(ident, kind, listing.get("components") or []) if listing else []
    rows, groups = [], []
    if kind != "linux":
        exe = runner.get("exe") or ""
        where = (f"Installed by Universe ({runner.get('version')})" if source == "universe" else FOUND.get(source, "Found")) if found and not exe else ""
        rows.append(
            _row(name, "exe", "Program", "path", exe or found, module=ident, detail=where, inherited=not exe and bool(found), origin="runner" if exe else "")
        )
        rows.extend({**row_of(c), "section": name} for c in own if c["id"] == PROTON_TOOL)
        rows.append(_row(name, "args", "Arguments", "string", runner.get("args") or "", module=ident))
    config = client.config()
    launch = config.get("launch") or {}
    gamescope = runner.get("gamescope")
    rows.append(
        _row(
            name,
            "gamescope",
            "Gamescope",
            "bool",
            bool(launch.get("gamescope", True)) if gamescope is None else bool(gamescope),
            module=ident,
            origin="global" if gamescope is None else "runner",
        )
    )
    groups.append(_group("Runner", list(range(len(rows))), caps=True))
    mode = screen_mode()
    first = len(rows)
    rows.extend(
        launch_row(spec["section"], spec, launch.get(spec["key"]) or spec["default"], protons=proton_choices(config))
        for spec in client.launchKeys("global", mode)
        if kind in spec["runners"] and spec["key"] in BUILD_KEYS
    )
    rows.extend({**row_of(c), "section": name} for c in _in_use_first(own) if c["id"] != PROTON_TOOL)
    if len(rows) > first:
        groups.append(_group(BUILDS, list(range(first, len(rows))), caps=True, warning=catalogue_warning(listing or {})))
    home = "Proton" if kind == "proton" else "Runner"
    homes = {**HOMES, "Sync": home, "Upscaling": home, "Logs": home}
    global_launch_rows(rows, groups, client, config, mode, lambda spec: kind in spec["runners"] and spec["key"] not in BUILD_KEYS, client.gpu(), homes)
    options = runner.get("options") or []
    if options:
        first = len(rows)
        rows.extend(
            _row(
                name,
                option["key"],
                option.get("label", option["key"]),
                option.get("type", "string"),
                option.get("value", option.get("default")),
                option.get("choices"),
                ident,
            )
            for option in options
        )
        groups.append(_group("Options", list(range(first, len(rows))), caps=True))
    games = sorted((g for g in client.list() if _runner_of(g) == ident), key=lambda g: str(g.get("title") or "").casefold())
    first = len(rows)
    for game in games:
        media, source = game.get("media") or {}, game.get("source")
        art = next((p for p in (media.get("square"), media.get("box_front")) if p), "")
        rows.append(
            {
                **_row(name, "game", str(game.get("title") or game.get("id")), "action", "", module=ident),
                "display": _play_time((game.get("stats") or {}).get("hours")),
                "action": "Options",
                "gameId": str(game.get("id")),
                "image": file_url(art).toString(),
                "installed": isinstance(source, dict) and bool(source.get("dir")),
            }
        )
    rows.append({**_row(name, "add_file", "Add a game…", "action", "", module=ident), "display": "", "action": "Pick a file", "runner": ident})
    groups.append(_group("Games", list(range(first, len(rows))), caps=True, meta=_plural(len(games), "game") if games else ""))
    return info, rows, groups


class RunnerForm(RowsForm):
    message = Signal(str)
    runnerChanged = Signal()

    def __init__(self, client, components, screen_mode: Callable[[], dict] = dict, parent=None):
        super().__init__(client, parent)
        self._components = components
        self._screen_mode = screen_mode
        self._runner = {}
        self._pending = None
        client.libraryChanged.connect(lambda ids: self._refresh() if self._runner else None)
        components.listingChanged.connect(lambda: self._refresh() if self._runner else None)

    @Slot(str)
    def load(self, ident):
        self._set_show_advanced(False)
        self._runner = {"id": ident}
        self._refresh()
        if ident and not self._components.listing():
            self._components.load()

    def _refresh(self):
        self._runner, rows, groups = build_runner(self._client, self._runner["id"], self._screen_mode, self._components.listing(), self._components.row)
        self._set_rows(rows, groups)
        self.runnerChanged.emit()

    info = Property(QVARIANT, lambda self: dict(self._runner), notify=runnerChanged)

    @Slot(int, "QVariant", result=bool)
    def setValue(self, index, value):
        row = self.row(index)
        if not row:
            return False
        if row["key"] == "add_file":
            self._pending = {"runner": row["module"], "name": row["section"], "file": str(value or "")}
            return bool(self._pending["file"])
        ok = self._write(row, _to_bus(row, value))
        if ok:
            self._refresh()
        return bool(ok)

    def _write(self, row, payload):
        if row["key"].startswith("launch."):
            return self._client.setConfig(row["key"], payload)
        return self._client.setRunnerSetting(row["module"], row["key"], payload)

    def _reload(self, row):
        self._refresh()

    def _title(self, game_id):
        return next((r["label"] for r in self._rows if r.get("gameId") == game_id), game_id)

    # The client's `error` signal carries the failure; the toast only says which.
    def _act(self, work, game_id, done_text, failed_text):
        title = self._title(game_id)
        self._client.runAsync(work, lambda ok: self.message.emit((done_text if ok else failed_text).format(title)))

    @Slot(str)
    def uninstall(self, game_id):
        if game_id:
            self._act(lambda: self._client.uninstall(game_id), game_id, "Uninstalled {}", "Could not uninstall {}")

    @Slot(str)
    def remove(self, game_id):
        if game_id:
            self._act(lambda: self._client.remove(game_id, False), game_id, "Removed {} from the library", "Could not remove {}")

    @Slot(result=str)
    def pendingTitle(self):
        return suggested_title(self._pending["file"]) if self._pending else ""

    @Slot(str, result=str)
    def addGame(self, title):
        if not self._pending:
            return ""
        pending, self._pending = self._pending, None
        ident = self._client.addGame(pending["runner"], pending["file"], title.strip())
        if ident:
            self.message.emit(f"Added {title.strip() or ident} through {pending['name']}")
        return ident
