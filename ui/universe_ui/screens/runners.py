import os
import re

from PySide6.QtCore import Signal, Slot

from .settings import RowsForm, _group, _row, _to_bus, runner_logo


def suggested_title(path):
    base = os.path.basename(str(path or "").rstrip("/"))
    stem = base if os.path.isdir(str(path or "")) else os.path.splitext(base)[0]
    stem = re.sub(r"\[[^\]]*\]|\([^)]*\)", " ", stem.replace("_", " "))
    words = [w for w in stem.split() if not re.fullmatch(r"v\d[\d.]*", w)]
    return " ".join(words).rstrip("- ").strip()


class RunnersForm(RowsForm):
    message = Signal(str)

    def __init__(self, client, parent=None):
        super().__init__(client, parent)
        self._pending = None

    @Slot()
    def load(self):
        rows = []
        groups = []
        for runner in self._client.runners() or []:
            ident = runner["id"]
            name = runner.get("name", ident)
            platforms = ", ".join(runner.get("platforms") or [])
            found = runner.get("path") or ""
            source = runner.get("source") or ""
            if runner.get("kind") == "linux":
                meta = platforms
                warning = ""
            elif found:
                meta = f"{platforms} · {found}" + (f" ({source})" if source and source != "path" else "")
                warning = "" if runner.get("available", True) else "Proton not found"
            else:
                meta = platforms
                warning = "not found"
            group = _group(name, [], meta=meta, warning=warning, off=not found and runner.get("kind") != "linux")
            group["runner"] = ident
            group["icon"] = runner_logo(ident)
            if runner.get("kind") != "linux":
                group["rows"].append(len(rows))
                rows.append(_row(name, "exe", "Program", "path", runner.get("exe") or "", module=ident,
                                 detail=found if not runner.get("exe") else "", inherited=not runner.get("exe") and bool(found)))
                group["rows"].append(len(rows))
                rows.append(_row(name, "args", "Arguments", "string", runner.get("args") or "", module=ident))
            for option in runner.get("options") or []:
                group["rows"].append(len(rows))
                rows.append(_row(name, option["key"], option.get("label", option["key"]), option.get("type", "string"),
                                 option.get("value", option.get("default")), option.get("choices"), ident))
            group["rows"].append(len(rows))
            rows.append({**_row(name, "add_file", "Add a game…", "action", "", module=ident), "display": "", "action": "Pick a file", "runner": ident})
            groups.append(group)
        self._set_rows(rows, groups)

    @Slot(str, result=str)
    def logo(self, ident):
        return runner_logo(ident)

    @Slot(int, "QVariant", result=bool)
    def setValue(self, index, value):
        if not (0 <= index < len(self._rows)):
            return False
        row = self._rows[index]
        if row["key"] == "add_file":
            self._pending = {"runner": row["module"], "name": row["section"], "file": str(value or "")}
            return bool(self._pending["file"])
        ok = self._client.setRunnerSetting(row["module"], row["key"], _to_bus(row["type"], value))
        if ok:
            self.load()
        return bool(ok)

    @Slot(int)
    def toggle(self, index):
        row = self.row(index)
        if row.get("type") == "bool":
            self.setValue(index, not row.get("value"))

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
