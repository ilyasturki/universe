import hashlib
import os
import re
import shutil
from datetime import datetime

import shiboken6
from PySide6.QtCore import QLocale, QObject, QProcess, QTimer, QUrl, Signal, Slot

from ..qt import Property
from .paths import universe_home


def _month(when):
    return QLocale.c().monthName(when.month, QLocale.FormatType.ShortFormat)


def _when(value):
    try:
        when = datetime.fromisoformat(str(value))
    except (TypeError, ValueError):
        return str(value or "")
    return f"{when.day} {_month(when)} {when:%Y · %H:%M}"


def _duration(seconds):
    seconds = int(seconds or 0)
    if seconds < 60:
        return f"{seconds} s"
    if seconds < 3600:
        return f"{seconds // 60} min"
    return f"{seconds // 3600} h {(seconds % 3600) // 60:02d}"


def _size(n):
    return QLocale.c().formattedDataSize(int(n or 0), 1, QLocale.DataSizeFormat.DataSizeTraditionalFormat)


def _cache_dir():
    path = os.path.join(universe_home("CACHE", ".cache"), "frames")
    os.makedirs(path, exist_ok=True)
    return path


FRAME_COUNT = 16
FRAME_WIDTH = 640
# The frame standing for the recording: about a fifth in, past the launch and the menus.
THUMB = 3
WORKERS = 2
VAAPI_DEVICE = "/dev/dri/renderD128"
# The core makes thumbnails in the background; the lists look for them this often while any is missing.
THUMB_POLL_MS = 400
# One watch event emits libraryChanged, recordingFiled and entryWritten in a row: one reload serves them.
RELOAD_MS = 400


class Frames:
    def __init__(self, path, duration):
        self.path = path
        self.dir = os.path.join(_cache_dir(), hashlib.sha1(path.encode()).hexdigest())
        self.duration = float(duration or 0)
        self.extracted = {i for i in range(FRAME_COUNT) if os.path.exists(self.file(i))}

    def file(self, index):
        return os.path.join(self.dir, f"{index:02d}.jpg")

    def seconds(self, index):
        return (index + 0.5) / FRAME_COUNT * self.duration

    def thumbnail(self):
        return THUMB if THUMB in self.extracted else None

    def complete(self):
        return len(self.extracted) == FRAME_COUNT


class Thumbs(QObject):
    """The thumbnails the core is making: `url(path)` is the file's URL once it is there, `version` bumps as they land."""

    versionChanged = Signal()

    def __init__(self, parent=None):
        super().__init__(parent)
        self._version = 0
        self._pending = set()
        self._poll = QTimer(self)
        self._poll.setInterval(THUMB_POLL_MS)
        self._poll.timeout.connect(self._check)

    def want(self, paths):
        new = {p for p in paths if p and not os.path.exists(p)}
        if new:
            self._pending |= new
            self._poll.start()

    def _check(self):
        landed = {p for p in self._pending if os.path.exists(p)}
        if landed:
            self._pending -= landed
            self._version += 1
            self.versionChanged.emit()
        if not self._pending:
            self._poll.stop()

    @Slot(str, result=str)
    def url(self, path):
        return QUrl.fromLocalFile(path).toString() if path and os.path.exists(path) else ""

    def shutdown(self):
        self._poll.stop()

    version = Property(int, lambda self: self._version, notify=versionChanged)
    pending = Property(int, lambda self: len(self._pending), notify=versionChanged)


class RecordingsList(QObject):
    rowsChanged = Signal()
    framesChanged = Signal()
    gameIdChanged = Signal()

    def __init__(self, client, parent=None):
        super().__init__(parent)
        self._client = client
        self._game_id = ""
        self._rows = []
        self._rows_out = None
        self._frames = {}
        self._map_out = None
        self._queue = []
        self._running = {}
        self._all = False
        # None until the first frame says whether the GPU decodes: VAAPI takes a quarter of the time and memory.
        self._hw = None
        client.recordingFiled.connect(lambda session, ident, path: self.loadAll() if self._all else ident == self._game_id and self.load(ident))

    @Slot(str)
    def load(self, game_id):
        self._load(game_id, False)

    @Slot()
    def loadAll(self):
        self._load("", True)

    def _load(self, game_id, all_games):
        self._game_id, self._all = game_id, all_games
        self.gameIdChanged.emit()
        self._queue.clear()
        rows = []
        for line in self._client.sessions(game_id):
            rec = line.get("recording")
            if not rec:
                continue
            path = str(rec.get("path") or "")
            session = str(line.get("session") or "")
            rows.append(
                {
                    "session": session,
                    "path": path,
                    "url": QUrl.fromLocalFile(path).toString() if path else "",
                    "size": rec.get("size") or 0,
                    "sizeText": _size(rec.get("size")),
                    "duration_s": line.get("duration_s") or 0,
                    "durationText": _duration(line.get("duration_s")),
                    "dateText": _when(line.get("ended_at")),
                    "hasJournal": line.get("journal") is not None,
                    "created_at": str(line.get("ended_at") or ""),
                    "gameId": str(line.get("game") or ""),
                    "gameTitle": str(line.get("title") or ""),
                }
            )
            if path and session and session not in self._frames:
                self._frames[session] = Frames(path, rec.get("duration_s") or line.get("duration_s"))
        self._rows = rows
        self._changed()
        for row in rows:
            self._want_thumbnail(row["session"])
        self._pump()

    def _changed(self):
        self._rows_out = None
        self._map_out = None
        self.rowsChanged.emit()
        self.framesChanged.emit()

    @Slot()
    def unload(self):
        self._all = False

    def warm(self, session, path, duration):
        if path and session and session not in self._frames:
            self._frames[session] = Frames(path, duration)
        self._want_thumbnail(session)
        self._pump()

    # Frames built off the UI thread (the media timeline's), taken in for the sessions not known yet.
    def adopt(self, frames):
        for session, entry in frames.items():
            if session not in self._frames:
                self._frames[session] = entry
        for session in frames:
            self._want_thumbnail(session)
        self._pump()

    def thumbnail_url(self, session):
        frames = self._frames.get(session)
        if frames is None or frames.thumbnail() is None:
            return ""
        return QUrl.fromLocalFile(frames.file(THUMB)).toString()

    @Slot(str, str, result=bool)
    def remove(self, game_id, session):
        if not self._client.removeRecording(game_id, session):
            return False
        self._queue = [j for j in self._queue if j[0] != session]
        self._kill(session)
        frames = self._frames.pop(session, None)
        if frames is not None:
            shutil.rmtree(frames.dir, ignore_errors=True)
            self._map_out = None
            self.framesChanged.emit()
        return True

    # The picked recording's frames go first; another session's frames still waiting are dropped, its thumbnails stay.
    @Slot(str)
    def select(self, session):
        frames = self._frames.get(session)
        if frames is None:
            return
        jobs = [(session, i) for i in range(FRAME_COUNT) if i not in frames.extracted and (session, i) not in self._running]
        self._queue = jobs + [j for j in self._queue if j[0] != session and j[1] == THUMB]
        self._kill_frames_of_others(session)
        self._pump()

    def _kill_frames_of_others(self, session):
        for job, proc in list(self._running.items()):
            if job[0] != session and job[1] != THUMB:
                self._stop(job, proc)

    def _want_thumbnail(self, session):
        frames = self._frames.get(session)
        job = (session, THUMB)
        if frames is not None and frames.thumbnail() is None and job not in self._queue and job not in self._running:
            self._queue.append(job)

    def _pump(self):
        while self._queue and len(self._running) < WORKERS:
            job = self._queue.pop(0)
            frames = self._frames.get(job[0])
            if job in self._running or frames is None or frames.duration <= 0 or not os.path.exists(frames.path):
                continue
            self._extract(job, frames, self._hw is not False)

    def _extract(self, job, frames, hw):
        ffmpeg = shutil.which("ffmpeg")
        if not ffmpeg:
            return
        index = job[1]
        os.makedirs(frames.dir, exist_ok=True)
        proc = QProcess(self)
        self._running[job] = proc
        proc.finished.connect(lambda code, status: self._extracted(job, proc, code, hw))
        proc.start(ffmpeg, _ffmpeg_args(frames.path, frames.seconds(index), frames.file(index), hw))

    def _extracted(self, job, proc, code, hw):
        self._finish(job, proc)
        session, index = job
        frames = self._frames.get(session)
        if frames is not None and code == 0 and os.path.exists(frames.file(index)):
            frames.extracted.add(index)
            if hw:
                self._hw = True
            self._map_out = None
            self.framesChanged.emit()
        elif hw and self._hw is not True and frames is not None:
            # The GPU path failed before it ever worked: this run decodes in software, the job goes again.
            self._hw = False
            self._queue.insert(0, job)
        self._pump()

    def _finish(self, job, proc):
        self._running.pop(job, None)
        if shiboken6.isValid(proc):
            proc.deleteLater()

    def _stop(self, job, proc):
        proc.finished.disconnect()
        proc.kill()
        proc.waitForFinished(1000)
        self._finish(job, proc)

    def _kill(self, session=None):
        for job, proc in list(self._running.items()):
            if session is None or job[0] == session:
                self._stop(job, proc)

    def shutdown(self):
        self._queue.clear()
        self._kill()

    # The listed sessions only: the map is read by every row, so it is built once per change and kept small.
    def _frame_map(self):
        if self._map_out is None:
            out = {}
            for row in self._rows:
                frames = self._frames.get(row["session"])
                if frames is None:
                    continue
                thumb = frames.thumbnail()
                out[row["session"]] = {
                    "thumbnail": QUrl.fromLocalFile(frames.file(thumb)).toString() if thumb is not None else "",
                    "frames": [QUrl.fromLocalFile(frames.file(i)).toString() if i in frames.extracted else "" for i in range(FRAME_COUNT)],
                    "complete": frames.complete(),
                    "duration": frames.duration,
                }
            self._map_out = out
        return self._map_out

    def _rows_list(self):
        if self._rows_out is None:
            self._rows_out = [dict(r) for r in self._rows]
        return self._rows_out

    rows = Property(list, _rows_list, notify=rowsChanged)
    count = Property(int, lambda self: len(self._rows), notify=rowsChanged)
    frameMap = Property(dict, _frame_map, notify=framesChanged)
    gameId = Property(str, lambda self: self._game_id, notify=gameIdChanged)
    hardware = Property(bool, lambda self: self._hw is not False, notify=framesChanged)


def _ffmpeg_args(path, seconds, out, hw):
    head = ["-loglevel", "error", "-y"]
    if hw:
        head += ["-hwaccel", "vaapi", "-hwaccel_device", VAAPI_DEVICE, "-hwaccel_output_format", "vaapi"]
    scale = f"scale_vaapi=w={FRAME_WIDTH}:h=-2:format=nv12,hwdownload,format=nv12" if hw else f"scale={FRAME_WIDTH}:-2"
    return [*head, "-ss", f"{seconds:.3f}", "-i", path, "-frames:v", "1", "-vf", scale, "-q:v", "4", out]


class ScreenshotsList(QObject):
    rowsChanged = Signal()
    gameIdChanged = Signal()

    def __init__(self, client, thumbs, parent=None):
        super().__init__(parent)
        self._client = client
        self._thumbs = thumbs
        self._game_id = ""
        self._rows = []
        self._all = False
        client.libraryChanged.connect(self._changed)

    def _changed(self, ids):
        if self._all:
            self.loadAll()
        elif self._game_id and (not ids or self._game_id in ids):
            self.load(self._game_id)

    @Slot(str)
    def load(self, game_id):
        self._load(game_id, False)

    @Slot()
    def loadAll(self):
        self._load("", True)

    def _load(self, game_id, all_games):
        self._game_id, self._all = game_id, all_games
        self.gameIdChanged.emit()
        lines = self._client.sessions(game_id)
        journaled = {(str(line.get("game") or ""), str(line.get("session") or "")) for line in lines if line.get("journal")}
        self._rows = [_shot_row(shot, journaled) for shot in self._client.screenshots(game_id)]
        self._thumbs.want(r["thumb"] for r in self._rows if not r["thumbReady"])
        self.rowsChanged.emit()

    @Slot()
    def unload(self):
        self._all = False

    @Slot(str, str, result=bool)
    def remove(self, game_id, name):
        return bool(self._client.removeScreenshot(game_id, name))

    rows = Property(list, lambda self: [dict(r) for r in self._rows], notify=rowsChanged)
    count = Property(int, lambda self: len(self._rows), notify=rowsChanged)
    gameId = Property(str, lambda self: self._game_id, notify=gameIdChanged)


def _shot_row(shot, journaled):
    path = str(shot.get("path") or "")
    session = str(shot.get("session") or "")
    ident = str(shot.get("game") or "")
    return {
        "name": os.path.basename(path),
        "path": path,
        "url": QUrl.fromLocalFile(path).toString() if path else "",
        "thumb": str(shot.get("thumb") or ""),
        "thumbReady": bool(shot.get("thumb_ready")),
        "taken_at": str(shot.get("taken_at") or ""),
        "dateText": _when(shot.get("taken_at")),
        "session": session,
        "hasJournal": (ident, session) in journaled,
        "gameId": ident,
        "gameTitle": str(shot.get("title") or ""),
    }


# One list row from the core's `media` row; a recording's `image` is filled from the frames cache on the UI thread.
def _media_row(r):
    kind = str(r.get("kind") or "")
    ident = str(r.get("game") or "")
    session = str(r.get("session") or "")
    path = str(r.get("path") or "")
    duration = int(r.get("duration_s") or 0)
    return {
        "kind": kind,
        "key": f"{kind}:{ident}:{session or path}",
        "gameId": ident,
        "gameTitle": str(r.get("title") or ""),
        "when": str(r.get("when") or ""),
        "dateText": _when(r.get("date")),
        "session": session,
        "path": path,
        "name": os.path.basename(path) if kind == "shot" else "",
        "url": QUrl.fromLocalFile(path).toString() if kind == "shot" and path else "",
        "thumb": str(r.get("thumb") or ""),
        "thumbReady": bool(r.get("thumb_ready")),
        "image": "",
        "hasJournal": bool(r.get("has_journal")),
        "title": _duration(duration) if kind == "recording" else str(r.get("heading") or "") if kind == "journal" else "",
        "excerpt": str(r.get("excerpt") or "") if kind == "journal" else "",
        "durationText": _duration(duration) if kind != "shot" and duration else "",
    }


class MediaTimeline(QObject):
    rowsChanged = Signal()
    loadingChanged = Signal()

    def __init__(self, client, recordings, thumbs, parent=None):
        super().__init__(parent)
        self._client = client
        self._recordings = recordings
        self._thumbs = thumbs
        self._rows = []
        self._rows_out = None
        self._loaded = False
        self._loading = False
        self._generation = 0
        self._reload = QTimer(self)
        self._reload.setSingleShot(True)
        self._reload.setInterval(RELOAD_MS)
        self._reload.timeout.connect(self.load)
        client.libraryChanged.connect(lambda ids: self._loaded and self._reload.start())
        client.recordingFiled.connect(lambda session, ident, path: self._loaded and self._reload.start())
        client.entryWritten.connect(lambda session, ident: self._loaded and self._reload.start())
        recordings.framesChanged.connect(lambda: self._loaded and self._thumbnails())

    @Slot()
    def load(self):
        self._loaded = True
        self._generation += 1
        generation = self._generation
        if not self._loading:
            self._loading = True
            self.loadingChanged.emit()
        self._client.mediaAsync("", _build_media, lambda built: self._landed(generation, built))

    def _landed(self, generation, built):
        if generation != self._generation:
            return
        rows, frames = built
        self._loading = False
        self.loadingChanged.emit()
        self._recordings.adopt(frames)
        self._rows = rows
        self._thumbs.want(r["thumb"] for r in rows if r["thumb"] and not r["thumbReady"])
        self._thumbnails(force=True)

    # A recording's picture is its cached frame; the list is announced only when one of them changed.
    def _thumbnails(self, force=False):
        changed = force
        for row in self._rows:
            if row["kind"] != "recording":
                continue
            url = self._recordings.thumbnail_url(row["session"])
            if url != row["image"]:
                row["image"] = url
                changed = True
        if changed:
            self._rows_out = None
            self.rowsChanged.emit()

    @Slot()
    def unload(self):
        self._loaded = False

    def _rows_list(self):
        if self._rows_out is None:
            self._rows_out = [dict(r) for r in self._rows]
        return self._rows_out

    rows = Property(list, _rows_list, notify=rowsChanged)
    count = Property(int, lambda self: len(self._rows), notify=rowsChanged)
    loading = Property(bool, lambda self: self._loading, notify=loadingChanged)


# Off the UI thread: the rows, and a `Frames` (16 stats each) for every recording.
def _build_media(core_rows):
    rows = [_media_row(r) for r in core_rows]
    frames = {}
    for r in core_rows:
        if r.get("kind") == "recording" and r.get("path") and r.get("session"):
            frames[str(r["session"])] = Frames(str(r["path"]), r.get("duration_s"))
    return rows, frames


LIST_ITEM = re.compile(r"^\s*(?:[-*+]|\d+[.)])\s+")


def markdown_blocks(paragraphs):
    blocks = []
    for p in paragraphs:
        if blocks and LIST_ITEM.match(p) and LIST_ITEM.match(blocks[-1].rsplit("\n", 1)[-1]):
            blocks[-1] += "\n" + p
        else:
            blocks.append(p)
    return blocks


class JournalList(QObject):
    rowsChanged = Signal()
    gameIdChanged = Signal()

    def __init__(self, client, parent=None):
        super().__init__(parent)
        self._client = client
        self._game_id = ""
        self._rows = []
        self._all = False
        client.entryWritten.connect(lambda session, ident: self.loadAll() if self._all else ident == self._game_id and self.load(ident))

    @Slot(str)
    def load(self, game_id):
        self._load(game_id, False)

    @Slot()
    def loadAll(self):
        self._load("", True)

    def _load(self, game_id, all_games):
        self._game_id, self._all = game_id, all_games
        self.gameIdChanged.emit()
        lines = self._client.sessions(game_id)
        recorded = {(str(line.get("game") or ""), str(line.get("session") or "")) for line in lines if line.get("recording")}
        if game_id:
            titles = {game_id: str(self._client.game(game_id).get("title") or game_id)}
        else:
            titles = {}
            for line in lines:
                titles.setdefault(str(line.get("game") or ""), str(line.get("title") or ""))
        rows = [row for ident, title in titles.items() for row in self._rows_of(ident, title, recorded)]
        # Session ids are timestamps: a pending entry sorts among the written ones by when it was played.
        rows.sort(key=lambda r: r["session"], reverse=True)
        self._rows = rows
        self.rowsChanged.emit()

    def _rows_of(self, game_id, title, recorded):
        rows = []
        for entry in self._client.journal(game_id):
            session = str(entry.get("session") or "")
            state = str(entry.get("state") or "written")
            paragraphs = [str(p) for p in entry.get("paragraphs") or []]
            duration = int(entry.get("duration_s") or 0)
            rows.append(
                {
                    "session": session,
                    "title": str(entry.get("title") or ("" if state == "pending" else "Journal failed" if state == "failed" else "Untitled")),
                    "state": state,
                    "reason": paragraphs[0] if state == "failed" and paragraphs else "",
                    "started_at": str(entry.get("started_at") or ""),
                    "dateText": _when(entry.get("written_at") or entry.get("started_at")),
                    "duration_s": duration,
                    "durationText": _duration(duration) if duration else "",
                    "provider": str(entry.get("provider") or ""),
                    "paragraphs": paragraphs,
                    "blocks": markdown_blocks(paragraphs),
                    "next_up": str(entry.get("next_up") or ""),
                    "images": [QUrl.fromLocalFile(str(p)).toString() for p in entry.get("images") or []],
                    "hasRecording": (game_id, session) in recorded,
                    "written_at": str(entry.get("written_at") or ""),
                    "gameId": game_id,
                    "gameTitle": title,
                }
            )
        return rows

    @Slot()
    def unload(self):
        self._all = False

    @Slot(str, str, result=bool)
    def remove(self, game_id, session):
        return bool(self._client.removeJournalEntry(game_id, session))

    rows = Property(list, lambda self: [dict(r) for r in self._rows], notify=rowsChanged)
    count = Property(int, lambda self: len(self._rows), notify=rowsChanged)
    gameId = Property(str, lambda self: self._game_id, notify=gameIdChanged)


POLL_MS = 10000


class PendingJournals(QObject):
    changed = Signal()
    appeared = Signal(str, str)
    resolved = Signal(str, str, str, str)

    def __init__(self, client, parent=None):
        super().__init__(parent)
        self._client = client
        self._rows = []
        self._announced = set()
        self._busy = False
        self._again = False
        self._timer = QTimer(self)
        self._timer.setInterval(POLL_MS)
        self._timer.timeout.connect(self.refresh)
        client.entryWritten.connect(lambda session, ident: self.refresh())
        client.sessionEnded.connect(lambda *args: self.refresh())
        self.refresh()

    # The read runs off the UI thread; a refresh asked meanwhile runs once the reply is in.
    @Slot()
    def refresh(self):
        if self._busy:
            self._again = True
            return
        self._busy = True
        self._client.runAsync(self._client.pendingJournals, self._apply)

    def _apply(self, entries):
        self._busy = False
        rows = [
            {
                "game": str(entry.get("game") or ""),
                "title": str(entry.get("title") or ""),
                "session": str(entry.get("session") or ""),
                "started_at": str(entry.get("started_at") or ""),
            }
            for entry in entries or []
        ]
        before = {r["session"]: r for r in self._rows}
        now = {r["session"]: r for r in rows}
        self._rows = rows
        if rows:
            self._timer.start()
        else:
            self._timer.stop()
        self.changed.emit()
        for session, row in now.items():
            if session not in self._announced:
                self._announced.add(session)
                self.appeared.emit(session, row["title"])
        for session, row in before.items():
            if session not in now:
                self._resolve(session, row["game"])
        if self._again:
            self._again = False
            self.refresh()

    def _resolve(self, session, game):
        entry = next((e for e in self._client.journal(game) if str(e.get("session") or "") == session), None)
        if entry is None:
            return
        state = str(entry.get("state") or "written")
        paragraphs = [str(p) for p in entry.get("paragraphs") or []]
        text = (paragraphs[0] if paragraphs else "") if state == "failed" else str(entry.get("title") or "Untitled")
        self.resolved.emit(session, game, state, text)

    def shutdown(self):
        self._timer.stop()

    rows = Property(list, lambda self: [dict(r) for r in self._rows], notify=changed)
    count = Property(int, lambda self: len(self._rows), notify=changed)
