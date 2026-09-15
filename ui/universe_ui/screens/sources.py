import time

import qrcode
from PySide6.QtCore import Property, QObject, Signal, Slot

from .settings import AsyncScreen


def _source_row(game, updates, art):
    pending = game.get("id") in updates
    if game.get("installed"):
        status = "Update available" if pending else "Installed"
    elif game.get("owned"):
        status = "Owned"
    else:
        status = "Not owned"
    return {
        "id": str(game.get("id") or ""), "title": str(game.get("title") or ""),
        "game_id": str(game.get("game_id") or ""), "image": art(game.get("game_id")) or str(game.get("image") or ""),
        "installed": bool(game.get("installed")), "pending": pending, "status": status,
        "action": "Update" if pending else ("Play from library" if game.get("installed") else "Install"),
    }


# The fetched library and updates stay until a job, a request, or this age.
STALE_S = 15 * 60


class SourcesBrowser(AsyncScreen):
    sourcesChanged = Signal()
    sourceChanged = Signal()
    rowsChanged = Signal()
    updatesChanged = Signal()
    jobChanged = Signal()
    queryChanged = Signal()
    message = Signal(str)

    def __init__(self, client, games, parent=None):
        super().__init__(client, parent)
        self._games_model = games
        self._sources = []
        self._source = ""
        self._games = []
        self._shown = []
        self._rows = []
        self._updates = []
        self._query = ""
        self._job = None
        self._loaded_at = 0.0
        client.progress.connect(self._on_progress)
        client.jobFinished.connect(self._on_job_finished)
        client.mediaChanged.connect(lambda ident: self._show(self._shown))

    # The library's own art when the game is in it, else the store's picture.
    def _art(self, game_id):
        game = self._games_model.byId(game_id) if game_id else None
        if game is None:
            return ""
        return next((u.toString() for u in (game.assets.square, game.assets.boxFront) if not u.isEmpty()), "")

    def _show(self, games):
        pending = {u.get("id") for u in self._updates}
        self._shown = games
        self._rows = [_source_row(g, pending, self._art) for g in games]
        self.rowsChanged.emit()

    @Slot()
    def load(self):
        if self._busy or (self._loaded_at and time.monotonic() - self._loaded_at < STALE_S):
            return
        self.refresh()

    @Slot()
    def refresh(self):
        if self._busy:
            return

        def work():
            sources = list(self._client.sources() or [])
            source = self._source or (sources[0]["id"] if sources else "")
            updates = list(self._client.updates() or []) if source else []
            games = list(self._client.sourceLibrary(source) or []) if source else []
            return sources, source, updates, games

        def done(result, error):
            if error:
                self.message.emit(error)
                return
            sources, source, updates, games = result
            self._sources = sources
            self.sourcesChanged.emit()
            self._source = source
            self.sourceChanged.emit()
            self._updates = updates
            self.updatesChanged.emit()
            self._games = games
            self._loaded_at = time.monotonic()
            if not self._query:
                self._rebuild()

        self._run(work, done)

    def _rebuild(self):
        self._show(sorted(self._games, key=lambda g: (not g.get("installed"), str(g.get("title", "")).casefold())))

    @Slot(str)
    def search(self, query):
        self._query = query
        self.queryChanged.emit()
        if not query:
            self._rebuild()
            return
        source = self._source

        def done(found, error):
            if error:
                self.message.emit(error)
            if self._query != query or error:
                return
            self._show(list(found or []))

        self._run(lambda: self._client.search(source, query), done)

    @Slot(int, result=str)
    def install(self, index):
        if not (0 <= index < len(self._rows)):
            return ""
        row = self._rows[index]
        if row["pending"]:
            return self._begin(self._client.update(self._source, row["id"]), f"Updating {row['title']}")
        if row["installed"]:
            return ""
        return self._begin(self._client.install(self._source, row["id"]), f"Installing {row['title']}")

    @Slot(int, result=str)
    def update(self, index):
        if not (0 <= index < len(self._updates)):
            return ""
        item = self._updates[index]
        return self._begin(self._client.update(self._source, item.get("id", "")), f"Updating {item.get('title', '')}")

    @Slot(result=str)
    def updateAll(self):
        return self._begin(self._client.update(self._source, ""), "Updating everything")

    # By library id: a confirmation outlives a refresh that reorders the rows.
    def _title(self, game_id):
        return next((r["title"] for r in self._rows if r["game_id"] == game_id), game_id)

    @Slot(str)
    def uninstall(self, game_id):
        if not game_id:
            return
        title = self._title(game_id)

        def done(ok, error):
            self.message.emit(error or (f"Uninstalled {title}" if ok else f"Could not uninstall {title}"))
            self.refresh()

        self._run(lambda: self._client.uninstall(game_id), done)

    @Slot(str)
    def remove(self, game_id):
        if not game_id:
            return
        title = self._title(game_id)

        def done(ok, error):
            self.message.emit(error or (f"Removed {title} from the library" if ok else f"Could not remove {title}"))
            self.refresh()

        self._run(lambda: self._client.remove(game_id, False), done)

    def _begin(self, job_id, label):
        if not job_id:
            return ""
        self._job = {"id": job_id, "message": label, "done": 0, "total": 0, "ok": None}
        self.jobChanged.emit()
        return job_id

    def _on_progress(self, job_id, done, total, message):
        if self._job and self._job["id"] == job_id:
            self._job.update({"done": int(done), "total": int(total), "message": message or self._job["message"]})
            self.jobChanged.emit()

    def _on_job_finished(self, job_id, ok, text):
        if self._job and self._job["id"] == job_id:
            self._job.update({"ok": bool(ok), "message": text or self._job["message"]})
            self.jobChanged.emit()
            self.message.emit(text)
            self.refresh()
            if self._query:
                self.search(self._query)

    sources = Property("QVariantList", lambda self: list(self._sources), notify=sourcesChanged)
    source = Property(str, lambda self: self._source, notify=sourceChanged)
    current = Property("QVariant", lambda self: next((dict(s) for s in self._sources if s.get("id") == self._source), None), notify=sourceChanged)
    rows = Property("QVariantList", lambda self: list(self._rows), notify=rowsChanged)
    updates = Property("QVariantList", lambda self: list(self._updates), notify=updatesChanged)
    query = Property(str, lambda self: self._query, notify=queryChanged)
    job = Property("QVariant", lambda self: dict(self._job) if self._job else None, notify=jobChanged)


def qr_matrix(text):
    qr = qrcode.QRCode(border=0, error_correction=qrcode.constants.ERROR_CORRECT_L)
    qr.add_data(text)
    qr.make(fit=True)
    return [[bool(cell) for cell in row] for row in qr.get_matrix()]


class LoginFlow(QObject):
    changed = Signal()
    finished = Signal(bool, str)

    def __init__(self, client, parent=None):
        super().__init__(parent)
        self._client = client
        self._source = ""
        self._url = ""
        self._matrix = []
        self._job = ""
        self._status = ""
        client.jobFinished.connect(self._on_job_finished)

    @Slot(str)
    def begin(self, source):
        self._source = source
        self._url = self._client.loginUrl(source) or ""
        self._matrix = qr_matrix(self._url) if self._url else []
        self._status = "Open the link, sign in, then enter the code it shows." if self._url else "This source has no login."
        self.changed.emit()

    @Slot(str)
    def submit(self, code):
        code = code.strip()
        if not code:
            return
        self._job = self._client.login(self._source, code) or ""
        self._status = "Checking the code…" if self._job else "Login could not start."
        self.changed.emit()

    def _on_job_finished(self, job_id, ok, text):
        if job_id != self._job:
            return
        self._job = ""
        self._status = text or ("Logged in." if ok else "Login failed.")
        self.changed.emit()
        self.finished.emit(bool(ok), self._status)

    source = Property(str, lambda self: self._source, notify=changed)
    url = Property(str, lambda self: self._url, notify=changed)
    matrix = Property("QVariantList", lambda self: [list(r) for r in self._matrix], notify=changed)
    size = Property(int, lambda self: len(self._matrix), notify=changed)
    status = Property(str, lambda self: self._status, notify=changed)
    busy = Property(bool, lambda self: bool(self._job), notify=changed)
