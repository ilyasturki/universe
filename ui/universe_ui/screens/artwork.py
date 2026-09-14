"""Artwork: one game's slots, each with its default under the pick over it and SteamGridDB's
candidates for it; and the library-wide gallery of one slot across every game."""

import json

from PySide6.QtCore import Property, QObject, QTimer, Signal, Slot

from ..models import file_url
from ..universe_client import UniverseError

# slot, label, width / height as the theme draws it, where the themes show it
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
KIND_LABELS = {"picked": "Picked", "default": "Default", "missing": "Missing"}
ORIGIN_LABELS = {"picked": "your pick", "sgdb": "SteamGridDB", "steam": "Steam", "pegasus": "Pegasus", "lutris": "Lutris"}
FILTERS = ["all", "missing", "picked", "default"]
FILTER_LABELS = {"all": "All", "missing": "Missing", "picked": "Picked", "default": "Default"}
RELOAD_MS = 300


def _url(value):
    value = str(value or "")
    if not value:
        return ""
    if "://" in value:
        return value
    return file_url(value).toString()


def _slot_row(raw):
    slot = str(raw.get("slot") or "")
    kind = str(raw.get("kind") or "missing")
    origin = str(raw.get("origin") or "")
    default_origin = str(raw.get("default_origin") or "")
    return {
        "slot": slot, "label": SLOT_LABELS.get(slot, slot), "aspect": SLOT_ASPECTS.get(slot, 1.0), "use": SLOT_USES.get(slot, ""),
        "url": _url(raw.get("path")), "defaultUrl": _url(raw.get("default")), "overrideUrl": _url(raw.get("override")),
        "origin": origin, "originLabel": ORIGIN_LABELS.get(origin, origin),
        "defaultOriginLabel": ORIGIN_LABELS.get(default_origin, default_origin) or "Default",
        "kind": kind, "kindLabel": KIND_LABELS.get(kind, kind), "hasOverride": bool(raw.get("override")),
        "hasDefault": bool(raw.get("default")),
    }


def _candidate_row(raw, index):
    score = int(raw.get("score") or 0)
    # `url` stays the provider's, for the core to download; `thumb` is what the grid shows.
    return {
        "index": index, "id": int(raw.get("id") or 0), "url": str(raw.get("url") or ""), "thumb": _url(raw.get("thumb") or raw.get("url")),
        "score": score, "votes": score // 1000, "slot": str(raw.get("slot") or ""),
    }


def _hit_row(raw):
    return {
        "id": int(raw.get("id") or 0), "name": str(raw.get("name") or ""), "year": int(raw.get("year") or 0),
        "verified": bool(raw.get("verified")), "current": bool(raw.get("current")),
    }


class ArtworkForm(QObject):
    """`api.screens.artwork`: the slots of one game and the candidates of the one in focus."""

    slotsChanged = Signal()
    candidatesChanged = Signal()
    hitsChanged = Signal()
    busyChanged = Signal()
    gameIdChanged = Signal()
    message = Signal(str)
    applied = Signal(str)
    # The last search's failure, empty when it went through.
    searchErrorChanged = Signal()

    def __init__(self, client, parent=None):
        super().__init__(parent)
        self._client = client
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
        self._busy = 0
        # One in-flight candidates fetch at a time; a slot change while it runs drops its result.
        self._fetch_seq = 0
        client.mediaChanged.connect(self._on_media_changed)
        client.libraryChanged.connect(self._on_library_changed)

    # -- loading ---------------------------------------------------------------------------

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

    @Slot()
    def reload(self):
        if not self._game_id:
            return
        rows = self._client.mediaStatus(self._game_id) or []
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

    @Slot(str, result="QVariant")
    def slot(self, name):
        return next((dict(s) for s in self._slots if s["slot"] == name), {})

    # -- work off the UI thread -------------------------------------------------------------

    def _run(self, work, done):
        self._busy += 1
        self.busyChanged.emit()

        def guarded():
            try:
                return work(), ""
            except UniverseError as e:
                return None, e.message or e.kind

        def finish(result):
            self._busy -= 1
            done(*result)
            self.busyChanged.emit()

        self._client.runAsync(guarded, finish)

    # -- candidates --------------------------------------------------------------------------

    @Slot(str)
    def loadCandidates(self, slot):
        """The first page of a slot's candidates; the same slot again is a no-op while it holds."""
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

        def work():
            return self._client._call("Media1", "Candidates", game_id, slot, page)

        def done(result, error):
            if seq != self._fetch_seq:
                return
            self._candidates_busy = False
            if error:
                self.message.emit(error)
                self.candidatesChanged.emit()
                return
            data = _decode(result, {})
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

        self._run(work, done)

    # -- picking -----------------------------------------------------------------------------

    @Slot(str, str)
    def apply(self, slot, url):
        """Downloads the candidate over the slot as its override."""
        game_id = self._game_id
        label = SLOT_LABELS.get(slot, slot)

        def work():
            return self._client._call("Media1", "SetUrl", game_id, slot, url)

        def done(result, error):
            if error:
                self.message.emit(f"{label}: {error}")
                return
            self._client.mediaChanged.emit(game_id)
            self.applied.emit(slot)
            self.message.emit(f"{label} picked for {self._title}")

        self._run(work, done)

    @Slot(str, result=bool)
    def removeOverride(self, slot):
        label = SLOT_LABELS.get(slot, slot)
        try:
            gone = bool(self._client._call("Media1", "Unset", self._game_id, slot))
        except UniverseError as e:
            self.message.emit(f"{label}: {e.message or e.kind}")
            return False
        if gone:
            self._client.mediaChanged.emit(self._game_id)
            row = self.slot(slot)
            self.message.emit(f"{label}: back to the default" + (f" from {row['originLabel']}" if row.get("originLabel") else "") if row.get("hasDefault") else f"{label}: pick removed, nothing under it")
        return gone

    # -- the wrong game ----------------------------------------------------------------------

    @Slot(str)
    def search(self, query):
        game_id = self._game_id
        self._search_busy = True
        self.hitsChanged.emit()

        def work():
            return self._client._call("Media1", "Search", game_id, query)

        def done(result, error):
            self._search_busy = False
            self._search_error = error or ""
            self._hits = [] if error else [_hit_row(h) for h in _decode(result, [])]
            self.searchErrorChanged.emit()
            self.hitsChanged.emit()

        self._run(work, done)

    @Slot(int)
    def pin(self, sgdb_id):
        """Pins the game to a SteamGridDB id: the candidates and the next refresh follow it."""
        try:
            self._client._call("Media1", "Pin", self._game_id, "sgdb", str(int(sgdb_id)))
        except UniverseError as e:
            self.message.emit(e.message or e.kind)
            return
        self._sgdb_id = int(sgdb_id)
        self._hits = [dict(h, current=h["id"] == self._sgdb_id) for h in self._hits]
        hit = next((h for h in self._hits if h["current"]), None)
        self._sgdb_name = hit["name"] if hit else ""
        self._sgdb_year = hit["year"] if hit else 0
        self.hitsChanged.emit()
        self.slotsChanged.emit()
        self._client.libraryChanged.emit([self._game_id])
        slot = self._candidates_slot
        self._candidates_slot = ""
        if slot:
            self.loadCandidates(slot)
        name = next((h["name"] for h in self._hits if h["current"]), str(sgdb_id))
        self.message.emit(f"{self._title} now takes its art from {name}")

    @Slot()
    def refresh(self):
        self._client.mediaRefresh(self._game_id, False)
        self.message.emit(f"Fetching the missing art of {self._title}…")

    gameId = Property(str, lambda self: self._game_id, notify=gameIdChanged)
    title = Property(str, lambda self: self._title, notify=slotsChanged)
    sgdbId = Property(int, lambda self: self._sgdb_id, notify=slotsChanged)
    # The SteamGridDB entry the candidates come from, "Name (year)"; empty until a fetch named it.
    entry = Property(str, lambda self: (self._sgdb_name + (f" ({self._sgdb_year})" if self._sgdb_year else "")) if self._sgdb_name else (f"entry {self._sgdb_id}" if self._sgdb_id else ""), notify=slotsChanged)
    slots = Property("QVariantList", lambda self: [dict(s) for s in self._slots], notify=slotsChanged)
    candidates = Property("QVariantList", lambda self: [dict(c) for c in self._candidates], notify=candidatesChanged)
    candidatesSlot = Property(str, lambda self: self._candidates_slot, notify=candidatesChanged)
    candidatesBusy = Property(bool, lambda self: self._candidates_busy, notify=candidatesChanged)
    more = Property(bool, lambda self: self._more, notify=candidatesChanged)
    hits = Property("QVariantList", lambda self: [dict(h) for h in self._hits], notify=hitsChanged)
    searchBusy = Property(bool, lambda self: self._search_busy, notify=hitsChanged)
    searchError = Property(str, lambda self: self._search_error, notify=searchErrorChanged)
    busy = Property(bool, lambda self: self._busy > 0, notify=busyChanged)


def _decode(value, default):
    if isinstance(value, (dict, list)):
        return value
    try:
        return json.loads(value) if value else default
    except (TypeError, ValueError):
        return default


class ArtworkOverview(QObject):
    """`api.screens.artworkOverview`: one slot across the library, filtered by how each game stands."""

    rowsChanged = Signal()
    slotChanged = Signal()
    filterChanged = Signal()
    busyChanged = Signal()
    jobChanged = Signal()
    message = Signal(str)

    def __init__(self, client, parent=None):
        super().__init__(parent)
        self._client = client
        self._rows = []
        self._slot = "box_front"
        self._filter = "all"
        self._busy = False
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
        self._busy = True
        self.busyChanged.emit()

        def work():
            try:
                return self._client._call("Media1", "Status", ""), ""
            except UniverseError as e:
                return None, e.message or e.kind

        def done(result):
            raw, error = result
            self._busy = False
            self._loaded = True
            if error:
                self.message.emit(error)
            else:
                hidden = {str(g.get("id") or "") for g in self._client.list() or [] if g.get("hidden") or g.get("removed")}
                rows = []
                for g in _decode(raw, []):
                    ident = str(g.get("id") or "")
                    if ident in hidden:
                        continue
                    slots = {str(s.get("slot") or ""): _slot_row(s) for s in g.get("slots") or []}
                    rows.append({"id": ident, "title": str(g.get("title") or ident), "slots": slots})
                rows.sort(key=lambda r: r["title"].casefold())
                self._rows = rows
            self.rowsChanged.emit()
            self.busyChanged.emit()

        self._client.runAsync(work, done)

    @Slot()
    def unload(self):
        self._loaded = False
        self._reload.stop()

    @Slot(str)
    def setSlot(self, slot):
        if slot in SLOT_LABELS and slot != self._slot:
            self._slot = slot
            self.slotChanged.emit()
            self.rowsChanged.emit()

    @Slot(str)
    def setFilter(self, name):
        if name in FILTERS and name != self._filter:
            self._filter = name
            self.filterChanged.emit()
            self.rowsChanged.emit()

    def _tiles(self):
        out = []
        for row in self._rows:
            slot = row["slots"].get(self._slot) or {}
            kind = slot.get("kind") or "missing"
            if self._filter != "all" and kind != self._filter:
                continue
            out.append({"id": row["id"], "title": row["title"], "url": slot.get("url") or "", "kind": kind,
                        "kindLabel": KIND_LABELS.get(kind, kind), "originLabel": slot.get("originLabel") or ""})
        return out

    def _counts(self):
        counts = {name: 0 for name in FILTERS}
        for row in self._rows:
            kind = (row["slots"].get(self._slot) or {}).get("kind") or "missing"
            counts["all"] += 1
            counts[kind] = counts.get(kind, 0) + 1
        return counts

    @Slot()
    def refreshAll(self):
        """Fetches the missing art of every game; `job` follows it until it ends."""
        if self._job and self._job.get("ok") is None:
            self.message.emit("Already fetching")
            return
        job_id = self._client.mediaRefresh("", False)
        if not job_id:
            return
        self._job = {"id": job_id, "message": "Fetching the missing art of every game…", "done": 0, "total": 0, "ok": None}
        self.jobChanged.emit()

    def _on_progress(self, job_id, done, total, text):
        if self._job and self._job["id"] == job_id:
            self._job.update({"done": int(done), "total": int(total), "message": text or self._job["message"]})
            self.jobChanged.emit()

    def _on_job_finished(self, job_id, ok, text):
        if self._job and self._job["id"] == job_id:
            self._job.update({"ok": bool(ok), "message": text or self._job["message"]})
            self.jobChanged.emit()
            self.message.emit(("Artwork fetched: " if ok else "Artwork fetch failed: ") + text)
            self._stale()

    rows = Property("QVariantList", lambda self: [dict(r) for r in self._rows], notify=rowsChanged)
    tiles = Property("QVariantList", _tiles, notify=rowsChanged)
    counts = Property("QVariantMap", _counts, notify=rowsChanged)
    slot = Property(str, lambda self: self._slot, setSlot, notify=slotChanged)
    slotLabel = Property(str, lambda self: SLOT_LABELS.get(self._slot, self._slot), notify=slotChanged)
    aspect = Property(float, lambda self: SLOT_ASPECTS.get(self._slot, 1.0), notify=slotChanged)
    filter = Property(str, lambda self: self._filter, setFilter, notify=filterChanged)
    slotUse = Property(str, lambda self: SLOT_USES.get(self._slot, ""), notify=slotChanged)
    slotNames = Property("QVariantList", lambda self: [{"slot": s, "label": l, "use": u} for s, l, _, u in SLOTS], constant=True)
    filterNames = Property("QVariantList", lambda self: [{"filter": f, "label": FILTER_LABELS[f]} for f in FILTERS], constant=True)
    busy = Property(bool, lambda self: self._busy, notify=busyChanged)
    # {id, message, done, total, ok (None while running)} or None
    job = Property("QVariant", lambda self: dict(self._job) if self._job else None, notify=jobChanged)
