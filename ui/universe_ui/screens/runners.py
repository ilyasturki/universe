import os
import re
from collections.abc import Callable

from PySide6.QtCore import Signal, Slot

from ..models import file_url
from ..qt import QVARIANT, Property
from .settings import HOMES, RowsForm, _group, _row, _to_bus, global_launch_rows, runner_logo


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


class RunnersForm(RowsForm):
    def __init__(self, client, parent=None):
        super().__init__(client, parent)
        client.libraryChanged.connect(lambda ids: self.load() if self._rows else None)

    @Slot()
    def load(self):
        usage = {}
        for game in self._client.list():
            ident = _runner_of(game)
            if not ident:
                continue
            count, hours = usage.get(ident, (0, 0.0))
            usage[ident] = (count + 1, hours + float((game.get("stats") or {}).get("hours") or 0))
        runners = sorted(
            self._client.runners(),
            key=lambda r: (not _found(r), -usage.get(r["id"], (0, 0.0))[0], -usage.get(r["id"], (0, 0.0))[1], r.get("name", r["id"]).lower()),
        )
        rows, found, missing = [], [], []
        for runner in runners:
            ident = runner["id"]
            count = usage.get(ident, (0, 0.0))[0]
            row = _row("Runners", "runner", runner.get("name", ident), "action", "", module=ident)
            row.update(display=f"{count} game{'' if count == 1 else 's'}" if count else "", icon=runner_logo(ident), iconSlot=True, runner=ident, action="Open")
            (found if _found(runner) else missing).append(len(rows))
            rows.append(row)
        groups = [_group("", found)]
        if missing:
            groups.append(_group("Not found", missing, caps=True, off=True))
        self._set_rows(rows, groups)

    @Slot(str, result=str)
    def logo(self, ident):
        return runner_logo(ident)


def build_runner(client, ident, screen_mode):
    """A runner's page: its program and gamescope, the global launch keys of its kind (the advanced ones folded into the Proton card,
    else the runner's), its options, its games."""
    runner = next((r for r in client.runners() if r["id"] == ident), None)
    if runner is None:
        return {}, [], []
    name = runner.get("name", ident)
    found = runner.get("path") or ""
    source = runner.get("source") or ""
    platforms = ", ".join(runner.get("platforms") or [])
    if runner.get("kind") == "linux":
        meta, warning = platforms, ""
    elif found:
        meta = f"{platforms} · {found}" + (f" ({source})" if source and source != "path" else "")
        warning = "" if runner.get("available", True) else "Proton not found"
    else:
        meta, warning = platforms, "not found"
    info = {"id": ident, "name": name, "meta": meta, "warning": warning, "icon": runner_logo(ident)}
    rows, groups = [], []
    if runner.get("kind") != "linux":
        own = runner.get("exe") or ""
        where = {"path": "Found on PATH", "lutris": "Found in Lutris's runners"}.get(source, "Found") if found and not own else ""
        rows.append(
            _row(name, "exe", "Program", "path", own or found, module=ident, detail=where, inherited=not own and bool(found), origin="runner" if own else "")
        )
        rows.append(_row(name, "args", "Arguments", "string", runner.get("args") or "", module=ident))
    config = client.config()
    launch = config.get("launch") or {}
    own = runner.get("gamescope")
    rows.append(
        _row(
            name,
            "gamescope",
            "Gamescope",
            "bool",
            bool(launch.get("gamescope", True)) if own is None else bool(own),
            module=ident,
            origin="global" if own is None else "runner",
        )
    )
    groups.append(_group("Runner", list(range(len(rows))), caps=True))
    kind = runner.get("kind") or ""
    home = "Proton" if kind == "proton" else "Runner"
    homes = {**HOMES, "Sync": home, "Upscaling": home, "Logs": home}
    global_launch_rows(rows, groups, client, config, screen_mode(), lambda spec: kind in spec["runners"], client.gpu(), homes)
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
    groups.append(_group("Games", list(range(first, len(rows))), caps=True, meta=f"{len(games)} game{'' if len(games) == 1 else 's'}" if games else ""))
    return info, rows, groups


class RunnerForm(RowsForm):
    message = Signal(str)
    runnerChanged = Signal()

    def __init__(self, client, screen_mode: Callable[[], dict] = dict, parent=None):
        super().__init__(client, parent)
        self._screen_mode = screen_mode
        self._runner = {}
        self._pending = None
        client.libraryChanged.connect(lambda ids: self._refresh() if self._runner else None)

    @Slot(str)
    def load(self, ident):
        self._set_show_advanced(False)
        self._runner = {"id": ident}
        self._refresh()

    def _refresh(self):
        self._runner, rows, groups = build_runner(self._client, self._runner["id"], self._screen_mode)
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
