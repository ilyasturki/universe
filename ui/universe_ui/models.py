"""Game objects, the library model and the proxies the theme sorts and filters with.

QtQml.Models' SortFilterProxyModel has no get(), no ExpressionFilter and a FunctionFilter
that segfaults (phase 0, T2), so every proxy the theme needs lives here and is
registered under `import Universe`.
"""

from PySide6.QtCore import (
    Property,
    QAbstractListModel,
    QDateTime,
    QModelIndex,
    QObject,
    QSortFilterProxyModel,
    Qt,
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


def _file_url(path):
    if not path:
        return QUrl()
    if isinstance(path, str) and "://" in path:
        return QUrl(path)
    return QUrl.fromLocalFile(str(path))


def _source_kind(value):
    if isinstance(value, dict):
        return str(value.get("kind") or value.get("id") or value.get("name") or "")
    return str(value or "")


# The theme reads Pegasus extras as lists (`extra["metacritic"][0]`); the daemon keeps them as
# scalars under metadata, so every unknown metadata key becomes an extra with a list value.
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
        self._box = QUrl()
        self._tile = QUrl()
        self._background = QUrl()
        self._logo = QUrl()
        self._shots = []

    def update(self, media):
        media = media or {}
        self._box = _file_url(media.get("box_front"))
        self._tile = _file_url(media.get("tile"))
        self._background = _file_url(media.get("background"))
        self._logo = _file_url(media.get("logo"))
        self._shots = [_file_url(p) for p in (media.get("screenshots") or [])]
        self.changed.emit()

    boxFront = Property(QUrl, lambda self: self._box, notify=changed)
    tile = Property(QUrl, lambda self: self._tile, notify=changed)
    background = Property(QUrl, lambda self: self._background, notify=changed)
    logo = Property(QUrl, lambda self: self._logo, notify=changed)
    screenshotList = Property("QVariantList", lambda self: list(self._shots), notify=changed)


class Game(QObject):
    """One library entry, decoded from Library1's Game JSON. Missing keys fall back to defaults."""

    changed = Signal()
    favoriteChanged = Signal()

    def __init__(self, data, library, parent=None):
        super().__init__(parent)
        self._library = library
        self._assets = GameAssets(self)
        self._collections = None
        self._raw = {}
        self.update(data)

    def update(self, data):
        self._raw = data or {}
        raw = self._raw
        meta = raw.get("metadata") or {}
        stats = raw.get("stats") or {}
        self._id = str(raw.get("id") or "")
        self._title = str(raw.get("title") or self._id)
        self._sortTitle = str(raw.get("sort_title") or self._title)
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
        self._tags = _as_list(raw.get("tags"))
        self._extra = _extras(meta)
        self._assets.update(raw.get("media"))
        self.changed.emit()
        self.favoriteChanged.emit()

    def rawData(self):
        return self._raw

    def setCollections(self, model):
        self._collections = model
        self.changed.emit()

    def _get_favorite(self):
        return self._favorite

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

    id = Property(str, lambda self: self._id, notify=changed)
    title = Property(str, lambda self: self._title, notify=changed)
    sortTitle = Property(str, lambda self: self._sortTitle, notify=changed)
    favorite = Property(bool, _get_favorite, _set_favorite, notify=favoriteChanged)
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
    tags = Property("QVariantList", lambda self: list(self._tags), notify=changed)
    assets = Property(QObject, lambda self: self._assets, constant=True)
    collections = Property(QObject, lambda self: self._collections, notify=changed)
    extra = Property("QVariantMap", lambda self: dict(self._extra), notify=changed)
    raw = Property("QVariantMap", lambda self: dict(self._raw), notify=changed)


class ObjectListModel(QAbstractListModel):
    """A list of QObjects with `count` and `get(i)`, the shape Pegasus gave `api.collections`."""

    countChanged = Signal()

    def __init__(self, objects=None, parent=None):
        super().__init__(parent)
        self._objects = list(objects or [])
        for signal in (self.rowsInserted, self.rowsRemoved, self.modelReset):
            signal.connect(self.countChanged)

    def setObjects(self, objects):
        self.beginResetModel()
        self._objects = list(objects)
        self.endResetModel()

    def objects(self):
        return list(self._objects)

    def rowCount(self, parent=QModelIndex()):
        return 0 if parent.isValid() else len(self._objects)

    def data(self, index, role=Qt.ItemDataRole.DisplayRole):
        if not index.isValid() or not (0 <= index.row() < len(self._objects)):
            return None
        obj = self._objects[index.row()]
        if role == MODEL_DATA_ROLE:
            return obj
        name = self.roleNames().get(role)
        return obj.property(bytes(name).decode()) if name else None

    def roleNames(self):
        return {MODEL_DATA_ROLE: b"modelData", MODEL_DATA_ROLE + 1: b"name"}

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


class GameListModel(QAbstractListModel):
    """`api.allGames`: every visible game, one role per Game property plus `modelData`."""

    countChanged = Signal()

    def __init__(self, parent=None):
        super().__init__(parent)
        self._games = []
        self._roles = {MODEL_DATA_ROLE: b"modelData"}
        for i, name in enumerate(GAME_ROLES):
            self._roles[MODEL_DATA_ROLE + 1 + i] = name.encode()
        for signal in (self.rowsInserted, self.rowsRemoved, self.modelReset):
            signal.connect(self.countChanged)

    def setGames(self, games):
        self.beginResetModel()
        for game in self._games:
            try:
                game.changed.disconnect(self._on_game_changed)
            except (RuntimeError, TypeError):
                pass
        self._games = list(games)
        for game in self._games:
            game.changed.connect(self._on_game_changed)
        self.endResetModel()

    def games(self):
        return list(self._games)

    def _on_game_changed(self):
        game = self.sender()
        row = self.rowOf(game)
        if row >= 0:
            index = self.index(row, 0)
            self.dataChanged.emit(index, index)

    def rowOf(self, game):
        for i, g in enumerate(self._games):
            if g is game:
                return i
        return -1

    def rowCount(self, parent=QModelIndex()):
        return 0 if parent.isValid() else len(self._games)

    def data(self, index, role=Qt.ItemDataRole.DisplayRole):
        if not index.isValid() or not (0 <= index.row() < len(self._games)):
            return None
        game = self._games[index.row()]
        if role == MODEL_DATA_ROLE:
            return game
        name = self._roles.get(role)
        return game.property(name.decode()) if name else None

    def roleNames(self):
        return dict(self._roles)

    @Slot(int, result=QObject)
    def get(self, row):
        return self._games[row] if 0 <= row < len(self._games) else None

    @Slot(str, result=QObject)
    def byId(self, ident):
        for game in self._games:
            if game.id == ident:
                return game
        return None

    count = Property(int, lambda self: len(self._games), notify=countChanged)


def _game_at(model, row):
    return model.data(model.index(row, 0), MODEL_DATA_ROLE)


class GameProxy(QSortFilterProxyModel):
    """Base for the theme's proxies: `count`, `get(i)`, `mapToSource(i)`, sort by a Game property."""

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

    def _get_sort_name(self):
        return self._sort_name

    def _set_sort_name(self, name):
        if name == self._sort_name:
            return
        self._sort_name = name
        self.sortRoleNameChanged.emit()
        self._resort()

    def _get_descending(self):
        return self._descending

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
        game = _game_at(self.sourceModel(), source_row)
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
    sortRoleName = Property(str, _get_sort_name, _set_sort_name, notify=sortRoleNameChanged)
    descending = Property(bool, _get_descending, _set_descending, notify=descendingChanged)


@QmlElement
class SortedGames(GameProxy):
    """Sort only: `sortRoleName` + `descending`, the RoleSorter/StringSorter case."""


@QmlElement
class RecentGames(GameProxy):
    """Played games, last played first (HomePage's rail)."""

    def __init__(self, parent=None):
        super().__init__(parent)
        self._sort_name = "lastPlayed"
        self._descending = True

    def acceptsGame(self, game, source_row):
        return game.playCount > 0


@QmlElement
class LimitedGames(GameProxy):
    """The first `limit` source rows; chained after a sorted proxy, like IndexFilter was."""

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
    """favorite || pinned: `pinned` holds source rows kept in place after Y removed them."""

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
    """Titles containing `query`, case-insensitive, last played first."""

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


LIBRARY_SORTS = [("lastPlayed", True), ("title", False), ("playTime", True), ("releaseYear", True)]


@QmlElement
class LibraryGames(GameProxy):
    """LibraryPage's grid: `sortMode` 0..3 = last played, title, playtime, released."""

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
    """One collection's games: those sharing a platform (or a source, when none is known)."""

    def __init__(self, key, parent=None):
        super().__init__(parent)
        self._key = key
        self._sort_name = "sortTitle"

    def acceptsGame(self, game, source_row):
        return collection_key(game) == self._key
