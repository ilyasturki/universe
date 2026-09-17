import os
import re

from PySide6.QtCore import Property, Signal, Slot

from .settings import RowsForm, _add, _card_meta, _group, _row, _to_bus, auto_rate, launch_row, proton_choices, runner_logo


def suggested_title(path):
    base = os.path.basename(str(path or "").rstrip("/"))
    stem = base if os.path.isdir(str(path or "")) else os.path.splitext(base)[0]
    stem = re.sub(r"\[[^\]]*\]|\([^)]*\)", " ", stem.replace("_", " "))
    words = [w for w in stem.split() if not re.fullmatch(r"v\d[\d.]*", w)]
    return " ".join(words).rstrip("- ").strip()


def _found(runner):
    return runner.get("kind") == "linux" or bool(runner.get("path"))


class RunnersForm(RowsForm):
    @Slot()
    def load(self):
        usage = {}
        for game in self._client.list():
            ident = str((game.get("effective") or {}).get("runner") or (game.get("launch") or {}).get("runner") or "")
            if not ident:
                continue
            count, hours = usage.get(ident, (0, 0.0))
            usage[ident] = (count + 1, hours + float((game.get("stats") or {}).get("hours") or 0))
        runners = sorted(self._client.runners(),
                         key=lambda r: (not _found(r), -usage.get(r["id"], (0, 0.0))[0], -usage.get(r["id"], (0, 0.0))[1],
                                        r.get("name", r["id"]).lower()))
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


class RunnerForm(RowsForm):
    message = Signal(str)
    runnerChanged = Signal()

    def __init__(self, client, screen_mode=lambda: {}, parent=None):
        super().__init__(client, parent)
        self._screen_mode = screen_mode
        self._runner = {}
        self._pending = None

    @Slot(str)
    def load(self, ident):
        runner = next((r for r in self._client.runners() if r["id"] == ident), None)
        if runner is None:
            self._runner = {}
            self._set_rows([], [])
            self.runnerChanged.emit()
            return
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
        self._runner = {"id": ident, "name": name, "meta": meta, "warning": warning, "icon": runner_logo(ident)}
        rows, groups = [], []
        if runner.get("kind") != "linux":
            own = runner.get("exe") or ""
            origin = {"path": "Found on PATH", "lutris": "Found in Lutris's runners"}.get(source, "Found") if found and not own else ""
            rows.append(_row(name, "exe", "Program", "path", own or found, module=ident, detail=origin, inherited=not own and bool(found)))
            rows.append(_row(name, "args", "Arguments", "string", runner.get("args") or "", module=ident))
            groups.append(_group("", list(range(len(rows)))))
        config = self._client.config()
        launch = config.get("launch") or {}
        own = runner.get("gamescope")
        rows.append(_row(name, "gamescope", "Gamescope", "bool", bool(launch.get("gamescope", True)) if own is None else bool(own), module=ident, inherited=own is None))
        groups.append(_group("", [len(rows) - 1]))
        # The launch keys tied to this runner's kind: what every game through it starts with, config.toml's [launch].
        kind = runner.get("kind") or ""
        mode = self._screen_mode()
        protons = proton_choices(config)
        hz = auto_rate(mode, launch.get("gamescope", True), launch.get("gamescope_refresh"))
        gpu = self._client.gpu()
        for spec in self._client.launchKeys("global", mode):
            if kind not in spec["runners"]:
                continue
            value = launch.get(spec["key"])
            if value in (None, ""):
                value = spec["default"]
            section = spec["section"]
            _add(rows, groups, section, launch_row(section, spec, value, protons=protons, auto_hz=hz, gpu=gpu), caps=True, meta=_card_meta(section, mode, gpu))
        options = runner.get("options") or []
        if options:
            first = len(rows)
            for option in options:
                rows.append(_row(name, option["key"], option.get("label", option["key"]), option.get("type", "string"),
                                 option.get("value", option.get("default")), option.get("choices"), ident))
            groups.append(_group("Options", list(range(first, len(rows))), caps=True))
        rows.append({**_row(name, "add_file", "Add a game…", "action", "", module=ident), "display": "", "action": "Pick a file", "runner": ident})
        groups.append(_group("", [len(rows) - 1]))
        self._set_rows(rows, groups)
        self.runnerChanged.emit()

    info = Property("QVariant", lambda self: dict(self._runner), notify=runnerChanged)

    @Slot(int, "QVariant", result=bool)
    def setValue(self, index, value):
        row = self.row(index)
        if not row:
            return False
        if row["key"] == "add_file":
            self._pending = {"runner": row["module"], "name": row["section"], "file": str(value or "")}
            return bool(self._pending["file"])
        if row["key"].startswith("launch."):
            ok = self._client.setConfig(row["key"], _to_bus(row, value))
        else:
            ok = self._client.setRunnerSetting(row["module"], row["key"], _to_bus(row, value))
        if ok:
            self.load(self._runner["id"])
        return bool(ok)

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
