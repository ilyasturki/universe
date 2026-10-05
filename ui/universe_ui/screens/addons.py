import time

from PySide6.QtCore import Signal, Slot

from ..qt import QVARIANT, Property
from .media import _size
from .settings import AsyncScreen

KINDS = {"module": "A module", "source": "A source", "theme": "A theme"}
# The index is fetched again once a list shows past this, or at once after an install or a removal.
FRESH_S = 60


def host(url):
    scheme, _, rest = str(url or "").partition("://")
    return rest.partition("/")[0] if scheme == "https" else rest or scheme


def state(row, busy=False):
    if busy:
        return "Installing…"
    if row["installed"]:
        if row["update"]:
            return f"Update {row['update']}"
        return "Installed · Unlisted" if row["origin"] == "unlisted" else "Installed"
    if row["incompatible"]:
        return "Needs another Universe"
    return " · ".join(p for p in (row["version"], _size(row["size"]) if row.get("size") else "") if p)


def _ask(message, detail, yes, no="Not now", **extra):
    return {"message": message, "detail": detail, "yes": yes, "no": no, **extra}


class AddonsForm(AsyncScreen):
    message = Signal(str)
    listingChanged = Signal()
    # An install, an update or a removal is over, whichever way it went.
    settled = Signal()

    def __init__(self, client, parent=None):
        super().__init__(client, parent)
        self._listing = None
        self._fetched = 0.0
        self._job = None
        client.jobFinished.connect(self._on_job_finished)

    def _fetch(self):
        def done(listing, error):
            self._listing = listing or {"index": {"url": "", "error": error}, "extensions": []}
            self._fetched = time.monotonic()
            self.listingChanged.emit()

        self._run(self._client.extensions, done)

    @Slot()
    def load(self):
        if self.busy or (self._listing is not None and time.monotonic() - self._fetched < FRESH_S):
            return
        self._fetch()

    def _rows(self, kind):
        return [r for r in (self._listing or {}).get("extensions") or [] if r["kind"] == kind]

    def _by_id(self, ident):
        return next((r for r in (self._listing or {}).get("extensions") or [] if r["id"] == ident), None)

    def _busy_on(self, ident):
        return bool(self._job) and self._job["id"] == ident

    @Slot(str, result="QVariant")
    def items(self, kind):
        """The list of `kind`'s add-ons, as menu items whose action is the add-on's id; an empty action picks nothing, its
        `state` says why (`loading`, `error`, `empty`). An open list takes them again on `listingChanged`."""
        if self._listing is None:
            return [{"icon": "refresh", "label": "Loading add-ons…", "detail": "", "action": "", "state": "loading"}]
        out = []
        for r in self._rows(kind):
            icon = "refresh" if r["update"] else "check" if r["installed"] else "download"
            out.append({"icon": icon, "label": r["name"], "detail": state(r, self._busy_on(r["id"])), "action": r["id"], "state": "addon"})
        if self._listing["index"]["error"]:
            out.append({"icon": "", "label": "Couldn't load add-ons", "detail": "", "action": "", "state": "error"})
        elif not out:
            out.append({"icon": "", "label": "No add-ons of this kind yet", "detail": "", "action": "", "state": "empty"})
        return out

    @Slot(str, result="QVariant")
    def actions(self, ident):
        """What a pick of the list does: one action goes straight to its confirmation, more open a menu."""
        r = self._by_id(ident)
        if r is None or self._busy_on(ident) or (not r["installed"] and r["incompatible"]):
            return []
        if not r["installed"]:
            return [{"icon": "download", "label": "Install", "action": "install"}]
        out = [{"icon": "refresh", "label": f"Update to {r['update']}", "action": "update"}] if r["update"] else []
        return [*out, {"icon": "trash", "label": "Remove", "action": "remove", "danger": True}]

    @Slot(str, str, result="QVariant")
    def confirm(self, ident, action):
        r = self._by_id(ident)
        if r is None:
            return None
        if action == "remove":
            if r["kind"] == "theme":
                detail = f"Universe deletes it. {r['name']} cannot be picked until it is installed again."
            else:
                detail = f"Universe deletes it and turns it off. {r['name']} cannot run until it is installed again."
            return _ask(f"Remove {r['name']}?", detail, "Remove", "Keep it", kind=r["kind"], danger=True)
        listed = r["listed"] and r["origin"] != "unlisted"
        origin = f"from the index at {host((self._listing or {})['index']['url'])}" if listed else "Unlisted"
        version = r["update"] if action == "update" else r["version"]
        head = " · ".join(p for p in (KINDS[r["kind"]], version, _size(r["size"]) if r.get("size") else "") if p)
        detail = "\n\n".join(p for p in (f"{head}\n{r['description']}".strip(), f"Runs programs as you · {origin}") if p)
        verb = "Update" if action == "update" else "Install"
        message = f"Update {r['name']} to {version}?" if action == "update" else f"Install {r['name']}?"
        return _ask(message, detail, verb, kind=r["kind"], origin="registry" if listed else "unlisted", runsAsYou=True)

    @Slot(str, str, result=bool)
    def act(self, ident, action):
        r = self._by_id(ident)
        if r is None or action not in ("install", "update", "remove"):
            return False
        if action == "remove":
            self._run(lambda: self._client.extensionRemove(ident), lambda ok, error: self._settled(error or f"Removed {r['name']}"))
            return True
        if self._job:
            self.message.emit(f"{self._job['label']} first: wait for it")
            return False
        label = f"Updating {r['name']}" if action == "update" else f"Installing {r['name']}"
        job = self._client.extensionUpdate(ident) if action == "update" else self._client.extensionInstall(ident)
        self._job = {"job": job, "id": ident, "label": label}
        self.message.emit(f"{label}…")
        self.listingChanged.emit()
        return True

    def _settled(self, text):
        self.message.emit(text)
        self._client.modulesChanged.emit()
        self._client.sourcesChanged.emit()
        self.settled.emit()
        self._fetch()

    def _on_job_finished(self, job_id, ok, text):
        if not self._job or self._job["job"] != job_id:
            return
        label = self._job["label"]
        self._job = None
        self._settled(("Installed " if label.startswith("Installing") else "Updated ") + text if ok else text)

    listing = Property(QVARIANT, lambda self: self._listing, notify=listingChanged)
