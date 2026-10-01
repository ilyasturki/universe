from datetime import UTC, datetime

from PySide6.QtCore import QObject, Signal, Slot

from ..qt import QVARIANT, Property
from .media import _when


def _moment(value):
    try:
        at = datetime.fromisoformat(str(value))
    except (TypeError, ValueError):
        return None
    return at if at.tzinfo else at.replace(tzinfo=UTC)


def _rarity(value):
    if value is None:
        return ""
    value = float(value)
    return f"{value:.1f}% of players" if value < 10 else f"{round(value)}% of players"


def _row(item):
    unlocked = bool(item.get("unlocked_at"))
    return {
        "key": str(item.get("key") or ""),
        "name": str(item.get("name") or item.get("key") or ""),
        "description": str(item.get("description") or ""),
        "unlocked": unlocked,
        "hidden": 1 if item.get("hidden") and not unlocked else 0,
        "unlockedAt": str(item.get("unlocked_at") or ""),
        "dateText": _when(item.get("unlocked_at")) if unlocked else "",
        "icon": str(item.get("icon") or "") if unlocked else str(item.get("icon_locked") or item.get("icon") or ""),
        "rarity": float(item["rarity"]) if item.get("rarity") is not None else -1.0,
        "rarityText": _rarity(item.get("rarity")),
    }


# A hidden locked one is no row of its own: its icon_locked is a greyed copy of the art, so the lot fold into one row with no icon.
def _folded(count):
    return {
        "key": "hidden",
        "name": f"{count} hidden achievement" + ("" if count == 1 else "s"),
        "description": "Keep playing to find out.",
        "unlocked": False,
        "hidden": count,
        "unlockedAt": "",
        "dateText": "",
        "icon": "",
        "rarity": -1.0,
        "rarityText": "",
    }


# Unlocked first, newest on top; then the locked ones, the most common first; the hidden ones, folded, close the list.
def _order(rows):
    unlocked = sorted((r for r in rows if r["unlocked"]), key=lambda r: _moment(r["unlockedAt"]) or datetime.min.replace(tzinfo=UTC), reverse=True)
    locked = sorted((r for r in rows if not r["unlocked"] and not r["hidden"]), key=lambda r: -r["rarity"])
    hidden = sum(1 for r in rows if r["hidden"])
    return unlocked + locked + ([_folded(hidden)] if hidden else [])


class AchievementsList(QObject):
    rowsChanged = Signal()
    stateChanged = Signal()
    gameIdChanged = Signal()

    def __init__(self, client, parent=None):
        super().__init__(parent)
        self._client = client
        self._game_id = ""
        self._rows = []
        self._total = 0
        self._unlocked = 0
        self._fetched_at = ""
        self._loading = False
        self._error = ""
        self._generation = 0
        self._primed = set()
        client.libraryChanged.connect(self._changed)

    # Once a run, for a game with no list yet: the first look fills the cache, and the game's menu offers the page from then on.
    @Slot(str, str, int)
    def prime(self, game_id, source, total):
        if not game_id or source in ("", "manual") or total > 0 or game_id in self._primed:
            return
        self._primed.add(game_id)
        self._client.achievementsAsync(game_id, False, lambda listing: None, lambda e: None)

    # An unlock filed mid-session rewrites the cache: the page follows it.
    def _changed(self, ids):
        if self._game_id and not self._loading and (not ids or self._game_id in ids):
            self._fetch(False)

    @Slot(str)
    def load(self, game_id):
        if game_id != self._game_id:
            self._rows, self._total, self._unlocked, self._fetched_at, self._error = [], 0, 0, "", ""
            self.rowsChanged.emit()
        self._game_id = game_id
        self.gameIdChanged.emit()
        self._fetch(False)

    @Slot()
    def refresh(self):
        if self._game_id:
            self._fetch(True)

    @Slot()
    def unload(self):
        self._generation += 1
        self._game_id = ""
        self._rows, self._total, self._unlocked, self._fetched_at, self._error, self._loading = [], 0, 0, "", "", False
        self.rowsChanged.emit()
        self.stateChanged.emit()

    def _fetch(self, refresh):
        self._generation += 1
        generation = self._generation
        self._loading, self._error = True, ""
        self.stateChanged.emit()

        def landed(listing):
            if generation != self._generation:
                return
            listing = listing or {}
            self._rows = _order([_row(a) for a in listing.get("items") or []])
            self._total = int(listing.get("total") or 0)
            self._unlocked = int(listing.get("unlocked") or 0)
            self._fetched_at = str(listing.get("fetched_at") or "")
            self._loading = False
            self.rowsChanged.emit()
            self.stateChanged.emit()

        def missed(e):
            if generation != self._generation:
                return
            self._loading, self._error = False, e.message or e.kind
            self.stateChanged.emit()

        self._client.achievementsAsync(self._game_id, refresh, landed, missed)

    rows = Property(list, lambda self: [dict(r) for r in self._rows], notify=rowsChanged)
    count = Property(int, lambda self: len(self._rows), notify=rowsChanged)
    total = Property(int, lambda self: self._total, notify=rowsChanged)
    unlocked = Property(int, lambda self: self._unlocked, notify=rowsChanged)
    fetchedText = Property(str, lambda self: _when(self._fetched_at) if self._fetched_at else "", notify=rowsChanged)
    gameId = Property(str, lambda self: self._game_id, notify=gameIdChanged)
    loading = Property(bool, lambda self: self._loading, notify=stateChanged)
    error = Property(str, lambda self: self._error, notify=stateChanged)


class UnlockWatch(QObject):
    """The running game's unlocks as its source files them: each once, and none from before the session."""

    unlocked = Signal(QVARIANT)

    def __init__(self, client, parent=None):
        super().__init__(parent)
        self._client = client
        self._game = ""
        self._title = ""
        self._since = None
        self._known = set()
        self._replayed = None
        client.currentSessionChanged.connect(self._on_session)
        client.libraryChanged.connect(self._on_library)
        self._on_session()

    def _on_session(self):
        current = self._client.currentSession or {}
        ident = str(current.get("id") or "") if current.get("session_id") else ""
        if ident == self._game:
            return
        self._game, self._title, self._known, self._replayed = ident, str(current.get("title") or ""), set(), None
        self._since = _moment(current.get("started_at"))
        if ident:
            self._look()

    def _on_library(self, ids):
        if self._game and (not ids or self._game in ids):
            self._look()

    # A game with no list yet is not asked: the ask would reach the store on every library change.
    def _look(self):
        game = self._game
        if not int((self._client.game(game).get("achievements") or {}).get("total") or 0):
            return

        def landed(listing):
            if game != self._game:
                return
            items = [a for a in (listing or {}).get("items") or [] if a.get("unlocked_at")]
            for item in items:
                at = _moment(item.get("unlocked_at"))
                if item.get("key") in self._known or (self._since is not None and (at is None or at < self._since)):
                    continue
                self._emit(game, item)
            self._known.update(str(a.get("key") or "") for a in items)
            # The stamp found on the first look was for an earlier run of the UI.
            replay = (listing or {}).get("replay") or {}
            stamp = str(replay.get("at") or "")
            if self._replayed is not None and stamp and stamp != self._replayed:
                by_key = {str(a.get("key") or ""): a for a in items}
                for key in replay.get("keys") or []:
                    if key in by_key:
                        self._emit(game, by_key[key])
            self._replayed = stamp

        self._client.achievementsAsync(game, False, landed, lambda e: None)

    def _emit(self, game, item):
        self.unlocked.emit(
            {
                "gameId": game,
                "gameTitle": self._title,
                "key": str(item.get("key") or ""),
                "name": str(item.get("name") or item.get("key") or ""),
                "description": str(item.get("description") or ""),
                "icon": str(item.get("icon") or ""),
                "rarityText": _rarity(item.get("rarity")),
            }
        )
