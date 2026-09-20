import os

from PySide6.QtCore import (
    Property,
    QAbstractListModel,
    QDateTime,
    QModelIndex,
    QObject,
    QSortFilterProxyModel,
    Qt,
    QTimer,
    QUrl,
    Signal,
    Slot,
)
from PySide6.QtQml import QmlElement

QML_IMPORT_NAME = "Universe"
QML_IMPORT_MAJOR_VERSION = 1

MODEL_DATA_ROLE = Qt.ItemDataRole.UserRole + 1

GAME_ROLES = [
    "id", "title", "sortTitle", "favorite", "hidden", "playTime", "playCount", "lastPlayed",
    "releaseYear", "developerList", "publisherList", "genreList", "players", "description",
    "summary", "source", "platform", "tags", "assets", "collections", "extra",
]

KNOWN_METADATA = {
    "developers", "developer", "publishers", "publisher", "genres", "genre", "release_year",
    "players", "description", "summary", "extra",
}


def _as_list(value):
    if value is None or value == "":
        return []
    if isinstance(value, str):
        return [v.strip() for v in value.split(",") if v.strip()]
    return [str(v) for v in value]


def sort_title(title):
    first, _, rest = title.partition(" ")
    return rest if rest and first.lower() in ("the", "a", "an") else title


ASSET_SLOTS = ("box_front", "square", "banner", "background", "logo")


def file_url(path):
    if not path:
        return QUrl()
    if "://" in str(path):
        return QUrl(path)
    # The mtime query keys the image cache: a slot replaced in place repaints instead of showing the cached bytes.
    url = QUrl.fromLocalFile(str(path))
    try:
        url.setQuery(f"v={int(os.stat(path).st_mtime_ns // 1_000_000)}")
    except OSError:
        pass
    return url


def _source_kind(value):
    if isinstance(value, dict):
        return str(value.get("kind") or value.get("id") or value.get("name") or "")
    return str(value or "")


# Pegasus extras are lists (`extra["metacritic"][0]`); the core keeps scalars under metadata.
def _extras(meta):
    out = {}
    for key, value in meta.items():
        if key in KNOWN_METADATA or key.endswith(("_id", "_appid")) or value in (None, "", 0, [], {}):
            continue
        out[key.replace("_", "-")] = list(value) if isinstance(value, (list, tuple)) else [value]
    for key, value in (meta.get("extra") or {}).items():
        out[key] = list(value) if isinstance(value, (list, tuple)) else [value]
    return out


def _datetime(value):
    if not value:
        return None
    dt = QDateTime.fromString(str(value), Qt.DateFormat.ISODate)
    return dt if dt.isValid() else None


class GameAssets(QObject):
    changed = Signal()

    def __init__(self, parent=None):
        super().__init__(parent)
        self._urls = dict.fromkeys(ASSET_SLOTS, QUrl())
        self._shots = []

    def update(self, media):
        media = media or {}
        self._urls = {k: file_url(media.get(k)) for k in ASSET_SLOTS}
        self._shots = [file_url(p) for p in (media.get("screenshots") or [])]
        self.changed.emit()

    boxFront = Property(QUrl, lambda self: self._urls["box_front"], notify=changed)
    square = Property(QUrl, lambda self: self._urls["square"], notify=changed)
    banner = Property(QUrl, lambda self: self._urls["banner"], notify=changed)
    background = Property(QUrl, lambda self: self._urls["background"], notify=changed)
    logo = Property(QUrl, lambda self: self._urls["logo"], notify=changed)
    screenshotList = Property("QVariantList", lambda self: list(self._shots), notify=changed)


class Game(QObject):
    changed = Signal()
    favoriteChanged = Signal()

    def __init__(self, data, library, parent=None):
        super().__init__(parent)
        self._library = library
        self._assets = GameAssets(self)
        self._collections = None
        self.update(data)

    def update(self, data):
        raw = data or {}
        meta = raw.get("metadata") or {}
        stats = raw.get("stats") or {}
        self._id = str(raw.get("id") or "")
        self._title = str(raw.get("title") or self._id)
        self._sortTitle = str(raw.get("sort_title") or sort_title(self._title))
        self._favorite = bool(raw.get("favorite", False))
        self._hidden = bool(raw.get("hidden", False))
        self._playTime = int(round(float(stats.get("hours") or 0) * 3600))
        self._playCount = int(stats.get("play_count") or 0)
        self._lastPlayed = _datetime(stats.get("last_played"))
        self._releaseYear = int(meta.get("release_year") or raw.get("release_year") or 0)
        self._developers = _as_list(meta.get("developers") or meta.get("developer"))
        self._publishers = _as_list(meta.get("publishers") or meta.get("publisher"))
        self._genres = _as_list(meta.get("genres") or meta.get("genre"))
        self._players = int(meta.get("players") or 1)
        self._description = str(meta.get("description") or "")
        self._summary = str(meta.get("summary") or "")
        self._source = _source_kind(raw.get("source"))
        self._platform = str(raw.get("platform") or "")
        effective = raw.get("effective") or {}
        self._runner = str(effective.get("runner") or "")
        self._runnerName = str(effective.get("runner_name") or self._runner)
        self._tags = _as_list(raw.get("tags"))
        self._extra = _extras(meta)
        self._assets.update(raw.get("media"))
        self.changed.emit()
        self.favoriteChanged.emit()

    def setCollections(self, model):
        self._collections = model
        self.changed.emit()

    def _set_favorite(self, value):
        value = bool(value)
        if value == self._favorite:
            return
        self._favorite = value
        self.favoriteChanged.emit()
        self.changed.emit()
        if self._library is not None:
            self._library.setGameKey(self._id, "favorite", "true" if value else "false")

    @Slot()
    def launch(self):
        if self._library is not None:
            self._library.launch(self)

    @Slot(QObject)
    def launchWith(self, grab):
        if self._library is not None:
            image = grab.property("image") if grab is not None else None
            self._library.launch(self, image)

    id = Property(str, lambda self: self._id, notify=changed)
    title = Property(str, lambda self: self._title, notify=changed)
    sortTitle = Property(str, lambda self: self._sortTitle, notify=changed)
    favorite = Property(bool, lambda self: self._favorite, _set_favorite, notify=favoriteChanged)
    hidden = Property(bool, lambda self: self._hidden, notify=changed)
    playTime = Property(int, lambda self: self._playTime, notify=changed)
    playCount = Property(int, lambda self: self._playCount, notify=changed)
    lastPlayed = Property("QVariant", lambda self: self._lastPlayed, notify=changed)
    releaseYear = Property(int, lambda self: self._releaseYear, notify=changed)
    developerList = Property("QVariantList", lambda self: list(self._developers), notify=changed)
    publisherList = Property("QVariantList", lambda self: list(self._publishers), notify=changed)
    genreList = Property("QVariantList", lambda self: list(self._genres), notify=changed)
    players = Property(int, lambda self: self._players, notify=changed)
    description = Property(str, lambda self: self._description, notify=changed)
    summary = Property(str, lambda self: self._summary, notify=changed)
    source = Property(str, lambda self: self._source, notify=changed)
    platform = Property(str, lambda self: self._platform, notify=changed)
    runner = Property(str, lambda self: self._runner, notify=changed)
    runnerName = Property(str, lambda self: self._runnerName, notify=changed)
    tags = Property("QVariantList", lambda self: list(self._tags), notify=changed)
    assets = Property(QObject, lambda self: self._assets, constant=True)
    collections = Property(QObject, lambda self: self._collections, notify=changed)
    extra = Property("QVariantMap", lambda self: dict(self._extra), notify=changed)


class ObjectListModel(QAbstractListModel):
    countChanged = Signal()

    def __init__(self, objects=None, parent=None, roles=("name",)):
        super().__init__(parent)
        self._objects = list(objects or [])
        self._roles = {MODEL_DATA_ROLE: b"modelData", **{MODEL_DATA_ROLE + 1 + i: r.encode() for i, r in enumerate(roles)}}
        for signal in (self.rowsInserted, self.rowsRemoved, self.modelReset):
            signal.connect(self.countChanged)

    def setObjects(self, objects):
        self.beginResetModel()
        self._objects = list(objects)
        self.endResetModel()

    def rowCount(self, parent=QModelIndex()):
        return 0 if parent.isValid() else len(self._objects)

    def data(self, index, role=Qt.ItemDataRole.DisplayRole):
        if not index.isValid() or not (0 <= index.row() < len(self._objects)):
            return None
        obj = self._objects[index.row()]
        if role == MODEL_DATA_ROLE:
            return obj
        name = self._roles.get(role)
        return obj.property(name.decode()) if name else None

    def roleNames(self):
        return dict(self._roles)

    @Slot(int, result=QObject)
    def get(self, row):
        return self._objects[row] if 0 <= row < len(self._objects) else None

    count = Property(int, lambda self: len(self._objects), notify=countChanged)


class Collection(QObject):
    def __init__(self, ident, name, games, parent=None):
        super().__init__(parent)
        self._id = ident
        self._name = name
        self._games = games

    id = Property(str, lambda self: self._id, constant=True)
    name = Property(str, lambda self: self._name, constant=True)
    shortName = Property(str, lambda self: self._id, constant=True)
    games = Property(QObject, lambda self: self._games, constant=True)


class GameListModel(ObjectListModel):
    totalPlayTimeChanged = Signal()

    def __init__(self, parent=None):
        super().__init__(parent=parent, roles=GAME_ROLES)
        self.countChanged.connect(self.totalPlayTimeChanged)

    def setGames(self, games):
        for game in self._objects:
            try:
                game.changed.disconnect(self._on_game_changed)
            except (RuntimeError, TypeError):
                pass
        self.setObjects(games)
        for game in self._objects:
            game.changed.connect(self._on_game_changed)

    def _on_game_changed(self):
        row = self.rowOf(self.sender())
        if row >= 0:
            index = self.index(row, 0)
            self.dataChanged.emit(index, index)
            self.totalPlayTimeChanged.emit()

    def rowOf(self, game):
        return next((i for i, g in enumerate(self._objects) if g is game), -1)

    @Slot(str, result=QObject)
    def byId(self, ident):
        return next((g for g in self._objects if g.id == ident), None)

    totalPlayTime = Property(int, lambda self: sum(g.playTime for g in self._objects), notify=totalPlayTimeChanged)


class GameProxy(QSortFilterProxyModel):
    countChanged = Signal()
    sortRoleNameChanged = Signal()
    descendingChanged = Signal()

    def __init__(self, parent=None):
        super().__init__(parent)
        self._sort_name = ""
        self._descending = False
        self.setDynamicSortFilter(True)
        for signal in (self.rowsInserted, self.rowsRemoved, self.modelReset, self.layoutChanged):
            signal.connect(self.countChanged)
        self.sourceModelChanged.connect(self._resort)

    def _resort(self):
        if self._sort_name:
            order = Qt.SortOrder.DescendingOrder if self._descending else Qt.SortOrder.AscendingOrder
            self.sort(0, order)
        else:
            self.sort(-1)
        self.invalidate()

    def _set_sort_name(self, name):
        if name == self._sort_name:
            return
        self._sort_name = name
        self.sortRoleNameChanged.emit()
        self._resort()

    def _set_descending(self, value):
        value = bool(value)
        if value == self._descending:
            return
        self._descending = value
        self.descendingChanged.emit()
        self._resort()

    def sortKey(self, game):
        value = game.property(self._sort_name)
        if isinstance(value, QDateTime):
            return (1, value.toMSecsSinceEpoch())
        if value is None:
            return (0, 0)
        if isinstance(value, str):
            return (1, value.casefold())
        return (1, value)

    def lessThan(self, left, right):
        a = self.sourceModel().data(left, MODEL_DATA_ROLE)
        b = self.sourceModel().data(right, MODEL_DATA_ROLE)
        if a is None or b is None:
            return False
        return self.sortKey(a) < self.sortKey(b)

    def acceptsGame(self, game, source_row):
        return True

    def filterAcceptsRow(self, source_row, source_parent):
        source = self.sourceModel()
        game = source.data(source.index(source_row, 0), MODEL_DATA_ROLE)
        return game is not None and self.acceptsGame(game, source_row)

    @Slot(int, result=QObject)
    def get(self, row):
        if not (0 <= row < self.rowCount()):
            return None
        return self.data(self.index(row, 0), MODEL_DATA_ROLE)

    # Not named mapToSource: that would shadow the QModelIndex virtual the proxy itself calls.
    @Slot(int, result=int)
    def sourceRow(self, row):
        index = self.mapToSource(self.index(row, 0))
        return index.row() if index.isValid() else -1

    count = Property(int, lambda self: self.rowCount(), notify=countChanged)
    sortRoleName = Property(str, lambda self: self._sort_name, _set_sort_name, notify=sortRoleNameChanged)
    descending = Property(bool, lambda self: self._descending, _set_descending, notify=descendingChanged)


@QmlElement
class SortedGames(GameProxy):
    pass


@QmlElement
class RecentGames(GameProxy):
    playingIdChanged = Signal()

    def __init__(self, parent=None):
        super().__init__(parent)
        self._sort_name = "lastPlayed"
        self._descending = True
        self._playing = ""

    def _set_playing(self, ident):
        ident = str(ident or "")
        if ident == self._playing:
            return
        self._playing = ident
        self.playingIdChanged.emit()
        self.invalidate()

    def acceptsGame(self, game, source_row):
        return game.playCount > 0 or game.id == self._playing

    def sortKey(self, game):
        return (2 if game.id == self._playing else 0,) + super().sortKey(game)

    playingId = Property(str, lambda self: self._playing, _set_playing, notify=playingIdChanged)


@QmlElement
class LimitedGames(GameProxy):
    limitChanged = Signal()

    def __init__(self, parent=None):
        super().__init__(parent)
        self._limit = 12

    def _set_limit(self, value):
        if value == self._limit:
            return
        self._limit = int(value)
        self.limitChanged.emit()
        self.invalidate()

    def acceptsGame(self, game, source_row):
        return source_row < self._limit

    limit = Property(int, lambda self: self._limit, _set_limit, notify=limitChanged)


@QmlElement
class FavouriteGames(GameProxy):
    # `pinned`: source rows kept in place after Y unfavourited them.
    pinnedChanged = Signal()

    def __init__(self, parent=None):
        super().__init__(parent)
        self._pinned = []
        self._sort_name = "playTime"
        self._descending = True

    def _set_pinned(self, rows):
        rows = [int(r) for r in (rows or [])]
        if rows == self._pinned:
            return
        self._pinned = rows
        self.pinnedChanged.emit()
        self.invalidate()

    def acceptsGame(self, game, source_row):
        return game.favorite or source_row in self._pinned

    pinned = Property("QVariantList", lambda self: list(self._pinned), _set_pinned, notify=pinnedChanged)


@QmlElement
class SearchGames(GameProxy):
    queryChanged = Signal()

    def __init__(self, parent=None):
        super().__init__(parent)
        self._query = ""
        self._sort_name = "lastPlayed"
        self._descending = True

    def _set_query(self, value):
        value = str(value)
        if value == self._query:
            return
        self._query = value
        self.queryChanged.emit()
        self.invalidate()

    def acceptsGame(self, game, source_row):
        return self._query.casefold() in game.title.casefold()

    query = Property(str, lambda self: self._query, _set_query, notify=queryChanged)


LIBRARY_SORTS = [("lastPlayed", True), ("sortTitle", False), ("playTime", True), ("releaseYear", True)]


@QmlElement
class LibraryGames(GameProxy):
    sortModeChanged = Signal()

    def __init__(self, parent=None):
        super().__init__(parent)
        self._mode = 0
        self._sort_name, self._descending = LIBRARY_SORTS[0]

    def _set_mode(self, mode):
        mode = int(mode) % len(LIBRARY_SORTS)
        if mode == self._mode:
            return
        self._mode = mode
        self._sort_name, self._descending = LIBRARY_SORTS[mode]
        self.sortModeChanged.emit()
        self._resort()

    sortMode = Property(int, lambda self: self._mode, _set_mode, notify=sortModeChanged)


def collection_key(game):
    return game.platform or game.source


class CollectionGames(GameProxy):
    def __init__(self, key, parent=None):
        super().__init__(parent)
        self._key = key
        self._sort_name = "sortTitle"

    def acceptsGame(self, game, source_row):
        return collection_key(game) == self._key


@QmlElement
class GameAnchor(QObject):
    clientChanged = Signal()
    modelChanged = Signal()
    indexChanged = Signal()
    gameChanged = Signal()
    moved = Signal(int)

    def __init__(self, parent=None):
        super().__init__(parent)
        self._client = None
        self._model = None
        self._index = -1
        self._game = None
        self._game_id = ""
        self._settling = False

    def _set_client(self, client):
        if client is self._client:
            return
        if self._client is not None:
            self._client.sessionEnded.disconnect(self._session_ended)
        self._client = client
        if client is not None:
            client.sessionEnded.connect(self._session_ended)
        self.clientChanged.emit()

    def _session_ended(self, session_id, ident, duration):
        self.hold(ident)

    def _set_model(self, model):
        if model is self._model:
            return
        if self._model is not None:
            self._model.countChanged.disconnect(self._changed)
            self._model.modelReset.disconnect(self._on_reset)
        self._model = model
        if model is not None:
            model.countChanged.connect(self._changed)
            model.modelReset.connect(self._on_reset)
        self.modelChanged.emit()
        self._resolve()

    def _set_index(self, index):
        if index == self._index:
            return
        self._index = index
        self.indexChanged.emit()
        if not self._settling:
            self._refresh()

    def _refresh(self, adopt=True):
        game = self._model.get(self._index) if self._model is not None else None
        if adopt or not self._game_id:
            self._game_id = game.id if game is not None else ""
        if game is not self._game:
            self._game = game
            self.gameChanged.emit()

    @Slot(str)
    def hold(self, ident):
        self._game_id = str(ident or "")
        self._resolve()

    # A reset is another list (a collection switched, the library reloaded): the cursor's row stands.
    def _on_reset(self):
        self._game_id = ""

    def _changed(self):
        # The view adjusts its own index while the rows move; settle once the whole change is in.
        if self._settling:
            return
        self._settling = True
        QTimer.singleShot(0, self._resolve)

    def _resolve(self):
        self._settling = False
        ident = self._game_id if self._model is not None else ""
        index = next((i for i in range(self._model.rowCount()) if self._model.get(i).id == ident), -1) if ident else -1
        if index >= 0 and index != self._index:
            self.moved.emit(index)
        self._refresh(adopt=index >= 0)

    client = Property(QObject, lambda self: self._client, _set_client, notify=clientChanged)
    model = Property(QObject, lambda self: self._model, _set_model, notify=modelChanged)
    index = Property(int, lambda self: self._index, _set_index, notify=indexChanged)
    game = Property(QObject, lambda self: self._game, notify=gameChanged)
