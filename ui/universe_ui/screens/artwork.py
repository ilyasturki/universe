from PySide6.QtCore import Property, QTimer, Signal, Slot

from ..models import file_url
from .settings import AsyncScreen

SLOTS = [
    ("box_front", "Box front", 600 / 900, "Details, launch, settings"),
    ("square", "Square", 1.0, "Home rail, Switch 2 tiles"),
    ("banner", "Banner", 920 / 430, "Wide; no view yet"),
    ("background", "Background", 16 / 9, "Behind the home and details"),
    ("logo", "Logo", 3.0, "Over the background, launch"),
]
SLOT_LABELS = {slot: label for slot, label, _, _ in SLOTS}
SLOT_ASPECTS = {slot: aspect for slot, _, aspect, _ in SLOTS}
SLOT_USES = {slot: use for slot, _, _, use in SLOTS}
ORIGIN_LABELS = {"picked": "your pick", "sgdb": "SteamGridDB", "steam": "Steam", "pegasus": "Pegasus", "lutris": "Lutris"}
RELOAD_MS = 300


# One label per slot, carrying its state and where the art came from: the pill both looks draw.
def _kind_label(kind, origin_label):
    return "Your pick" if kind == "picked" else "Missing" if kind == "missing" else origin_label or "Default"


def _slot_row(raw):
    slot = str(raw.get("slot") or "")
    kind = str(raw.get("kind") or "missing")
    origin = str(raw.get("origin") or "")
    default_origin = str(raw.get("default_origin") or "")
    origin_label = ORIGIN_LABELS.get(origin, origin)
    return {
        "slot": slot, "label": SLOT_LABELS.get(slot, slot), "aspect": SLOT_ASPECTS.get(slot, 1.0), "use": SLOT_USES.get(slot, ""),
        "url": file_url(raw.get("path")).toString(), "defaultUrl": file_url(raw.get("default")).toString(), "overrideUrl": file_url(raw.get("override")).toString(),
        "origin": origin, "originLabel": origin_label,
        "defaultOriginLabel": ORIGIN_LABELS.get(default_origin, default_origin) or "Default",
        "kind": kind, "kindLabel": _kind_label(kind, origin_label), "hasOverride": bool(raw.get("override")),
        "hasDefault": bool(raw.get("default")),
    }


def _candidate_row(raw, index):
    return {
        "index": index, "id": int(raw.get("id") or 0), "url": str(raw.get("url") or ""), "thumb": file_url(raw.get("thumb") or raw.get("url")).toString(),
        "votes": int(raw.get("score") or 0) // 1000, "slot": str(raw.get("slot") or ""),
    }


def _hit_row(raw):
    return {
        "id": int(raw.get("id") or 0), "name": str(raw.get("name") or ""), "year": int(raw.get("year") or 0),
        "verified": bool(raw.get("verified")), "current": bool(raw.get("current")),
    }


class ArtworkForm(AsyncScreen):
    slotsChanged = Signal()
    candidatesChanged = Signal()
    hitsChanged = Signal()
    gameIdChanged = Signal()
    message = Signal(str)
    applied = Signal(str)
    searchErrorChanged = Signal()

    def __init__(self, client, parent=None):
        super().__init__(client, parent)
        self._game_id = ""
        self._title = ""
        self._sgdb_id = 0
        self._sgdb_name = ""
        self._sgdb_year = 0
        self._slots = []
        self._candidates = []
        self._candidates_slot = ""
        self._page = 0
        self._more = False
        self._candidates_busy = False
        self._hits = []
        self._search_busy = False
        self._search_error = ""
        self._fetch_seq = 0
        client.mediaChanged.connect(self._on_media_changed)
        client.libraryChanged.connect(self._on_library_changed)

    @Slot(str)
    def load(self, game_id):
        if game_id != self._game_id:
            self._candidates = []
            self._candidates_slot = ""
            self._hits = []
            self.candidatesChanged.emit()
            self.hitsChanged.emit()
        self._game_id = game_id
        self.gameIdChanged.emit()
        self.reload()

    @Slot()
    def unload(self):
        self._game_id = ""
        self.gameIdChanged.emit()

    def reload(self):
        if not self._game_id:
            return
        rows = self._client.mediaStatus(self._game_id)
        status = rows[0] if rows else {}
        self._title = str(status.get("title") or self._game_id)
        sgdb_id = int(status.get("sgdb_id") or 0)
        # A pin names the entry before the core caches its name; keep it until the status knows.
        if sgdb_id != self._sgdb_id or status.get("sgdb_name"):
            self._sgdb_name = str(status.get("sgdb_name") or "")
            self._sgdb_year = int(status.get("sgdb_year") or 0)
        self._sgdb_id = sgdb_id
        self._slots = [_slot_row(s) for s in status.get("slots") or []]
        self.slotsChanged.emit()

    def _on_media_changed(self, ident):
        if ident == self._game_id:
            self.reload()

    def _on_library_changed(self, ids):
        ids = list(ids or [])
        if self._game_id and (not ids or self._game_id in ids):
            self.reload()

    def slot(self, name):
        return next((dict(s) for s in self._slots if s["slot"] == name), {})

    @Slot(str)
    def loadCandidates(self, slot):
        if slot == self._candidates_slot and (self._candidates or self._candidates_busy):
            return
        self._candidates = []
        self._candidates_slot = slot
        self._page = 0
        self._more = False
        self.candidatesChanged.emit()
        self._fetch(slot, 0)

    @Slot()
    def moreCandidates(self):
        if self._more and not self._candidates_busy and self._candidates_slot:
            self._fetch(self._candidates_slot, self._page + 1)

    def _fetch(self, slot, page):
        self._fetch_seq += 1
        seq = self._fetch_seq
        game_id = self._game_id
        self._candidates_busy = True
        self.candidatesChanged.emit()

        def done(data, error):
            if seq != self._fetch_seq:
                return
            self._candidates_busy = False
            if not data:
                self.candidatesChanged.emit()
                return
            items = [_candidate_row(c, len(self._candidates) + i) for i, c in enumerate(data.get("items") or [])]
            self._candidates = self._candidates + items
            self._page = int(data.get("page") or page)
            self._more = bool(data.get("more"))
            entry = data.get("entry") or {}
            if entry and (int(entry.get("id") or 0) != self._sgdb_id or entry.get("name") != self._sgdb_name):
                self._sgdb_id = int(entry.get("id") or 0)
                self._sgdb_name = str(entry.get("name") or "")
                self._sgdb_year = int(entry.get("year") or 0)
                self.slotsChanged.emit()
            self.candidatesChanged.emit()

        self._run(lambda: self._client.mediaCandidates(game_id, slot, page), done)

    @Slot(str, str)
    def apply(self, slot, url):
        game_id = self._game_id
        label = SLOT_LABELS.get(slot, slot)

        def done(placed, error):
            if not placed:
                return
            self.applied.emit(slot)
            self.message.emit(f"{label} picked for {self._title}")

        self._run(lambda: self._client.mediaSetUrl(game_id, slot, url), done)

    @Slot(str, str)
    def useFile(self, slot, path):
        game_id = self._game_id
        label = SLOT_LABELS.get(slot, slot)
        name = path.rstrip("/").rsplit("/", 1)[-1]

        def done(placed, error):
            if not placed:
                return
            self.applied.emit(slot)
            self.message.emit(f"{label} picked for {self._title}: {name}")

        self._run(lambda: self._client.mediaSetSlot(game_id, slot, path), done)

    @Slot(str, result=bool)
    def removeOverride(self, slot):
        label = SLOT_LABELS.get(slot, slot)
        gone = self._client.mediaUnset(self._game_id, slot)
        if gone:
            row = self.slot(slot)
            self.message.emit(f"{label}: back to the default" + (f" from {row['originLabel']}" if row.get("originLabel") else "") if row.get("hasDefault") else f"{label}: pick removed, nothing under it")
        return gone

    # The client toasts the core's refusal and answers empty; the page shows the reason instead.
    @Slot(str)
    def search(self, query):
        game_id = self._game_id
        self._search_busy = True
        self.hitsChanged.emit()
        failures = []
        failed = lambda kind, message: failures.append(message)  # noqa: E731
        self._client.error.connect(failed)

        def done(hits, error):
            self._client.error.disconnect(failed)
            self._search_busy = False
            self._search_error = failures[0] if failures else ""
            self._hits = [] if failures else [_hit_row(h) for h in hits or []]
            self.searchErrorChanged.emit()
            self.hitsChanged.emit()

        self._run(lambda: self._client.mediaSearch(game_id, query), done)

    @Slot(int)
    def pin(self, sgdb_id):
        if not self._client.mediaPin(self._game_id, "sgdb", str(int(sgdb_id))):
            return
        self._sgdb_id = int(sgdb_id)
        self._hits = [dict(h, current=h["id"] == self._sgdb_id) for h in self._hits]
        hit = next((h for h in self._hits if h["current"]), {"name": "", "year": 0})
        self._sgdb_name, self._sgdb_year = hit["name"], hit["year"]
        self.hitsChanged.emit()
        self.slotsChanged.emit()
        self._client.libraryChanged.emit([self._game_id])
        slot = self._candidates_slot
        self._candidates_slot = ""
        if slot:
            self.loadCandidates(slot)
        self.message.emit(f"{self._title} now takes its art from {self._sgdb_name or sgdb_id}")

    @Slot()
    def refresh(self):
        self._client.mediaRefresh(self._game_id, False)
        self.message.emit(f"Fetching the missing art of {self._title}…")

    gameId = Property(str, lambda self: self._game_id, notify=gameIdChanged)
    title = Property(str, lambda self: self._title, notify=slotsChanged)
    sgdbId = Property(int, lambda self: self._sgdb_id, notify=slotsChanged)
    entry = Property(str, lambda self: (self._sgdb_name + (f" ({self._sgdb_year})" if self._sgdb_year else "")) if self._sgdb_name else (f"entry {self._sgdb_id}" if self._sgdb_id else ""), notify=slotsChanged)
    entryDiffers = Property(bool, lambda self: bool(self._sgdb_name) and self._sgdb_name.casefold() != self._title.casefold(), notify=slotsChanged)
    slots = Property("QVariantList", lambda self: [dict(s) for s in self._slots], notify=slotsChanged)
    candidates = Property("QVariantList", lambda self: [dict(c) for c in self._candidates], notify=candidatesChanged)
    candidatesSlot = Property(str, lambda self: self._candidates_slot, notify=candidatesChanged)
    candidatesBusy = Property(bool, lambda self: self._candidates_busy, notify=candidatesChanged)
    more = Property(bool, lambda self: self._more, notify=candidatesChanged)
    hits = Property("QVariantList", lambda self: [dict(h) for h in self._hits], notify=hitsChanged)
    searchBusy = Property(bool, lambda self: self._search_busy, notify=hitsChanged)
    searchError = Property(str, lambda self: self._search_error, notify=searchErrorChanged)


class ArtworkOverview(AsyncScreen):
    rowsChanged = Signal()
    jobChanged = Signal()
    message = Signal(str)

    def __init__(self, client, parent=None):
        super().__init__(client, parent)
        self._rows = []
        self._loaded = False
        self._job = None
        self._reload = QTimer(self)
        self._reload.setSingleShot(True)
        self._reload.setInterval(RELOAD_MS)
        self._reload.timeout.connect(self.load)
        client.mediaChanged.connect(lambda ident: self._stale())
        client.libraryChanged.connect(lambda ids: self._stale())
        client.progress.connect(self._on_progress)
        client.jobFinished.connect(self._on_job_finished)

    def _stale(self):
        if self._loaded:
            self._reload.start()

    @Slot()
    def load(self):
        if self._busy:
            self._reload.start()
            return

        def work():
            status = self._client.mediaStatus("")
            hidden = {str(g.get("id") or "") for g in self._client.list() if g.get("hidden") or g.get("removed")}
            rows = []
            for g in status:
                ident = str(g.get("id") or "")
                if ident in hidden:
                    continue
                by_slot = {str(s.get("slot") or ""): _slot_row(s) for s in g.get("slots") or []}
                slots = [by_slot.get(slot) or _slot_row({"slot": slot}) for slot, *_ in SLOTS]
                rows.append({"id": ident, "title": str(g.get("title") or ident), "slots": slots})
            return sorted(rows, key=lambda r: r["title"].casefold())

        def done(rows, error):
            self._loaded = True
            if error:
                self.message.emit(error)
            else:
                self._rows = rows
            self.rowsChanged.emit()

        self._run(work, done)

    @Slot()
    def unload(self):
        self._loaded = False
        self._reload.stop()

    @Slot()
    def refreshAll(self):
        if self._job and self._job.get("ok") is None:
            self.message.emit("Already fetching")
            return
        job_id = self._client.mediaRefresh("", False)
        if not job_id:
            return
        self._job = {"id": job_id, "message": "Fetching the missing art of every game…", "done": 0, "total": 0, "ok": None, "cancelled": False}
        self.jobChanged.emit()

    @Slot(result=bool)
    def cancelRefresh(self):
        """Stops the library fetch after the game in hand; what was fetched stays."""
        if not self._job or self._job["ok"] is not None or self._job["cancelled"] or not self._client.cancel(self._job["id"]):
            return False
        self._job.update({"cancelled": True, "message": "Stopping…"})
        self.jobChanged.emit()
        return True

    def _on_progress(self, job_id, done, total, text):
        if self._job and self._job["id"] == job_id and not self._job["cancelled"]:
            self._job.update({"done": int(done), "total": int(total), "message": text or self._job["message"]})
            self.jobChanged.emit()

    def _on_job_finished(self, job_id, ok, text):
        if self._job and self._job["id"] == job_id:
            if self._job["cancelled"] and ok:
                text = f"Stopped after {self._job['done'] + 1} of {self._job['total']} games"
            else:
                text = ("Artwork fetched: " if ok else "Artwork fetch failed: ") + text
            self._job.update({"ok": bool(ok), "message": text})
            self.jobChanged.emit()
            self.message.emit(text)
            self._stale()

    rows = Property("QVariantList", lambda self: [dict(r, slots=[dict(s) for s in r["slots"]]) for r in self._rows], notify=rowsChanged)
    columns = Property("QVariantList", lambda self: [{"slot": slot, "label": label, "aspect": aspect, "use": use} for slot, label, aspect, use in SLOTS], constant=True)
    missingGames = Property(int, lambda self: sum(any(s["kind"] == "missing" for s in r["slots"]) for r in self._rows), notify=rowsChanged)
    job = Property("QVariant", lambda self: dict(self._job) if self._job else None, notify=jobChanged)
