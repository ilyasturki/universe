import hashlib
import os
import re
import shutil
from datetime import datetime

import shiboken6
from PySide6.QtCore import Property, QLocale, QObject, QProcess, QTimer, QUrl, Signal, Slot

from .paths import universe_home


def _when(value):
    try:
        return datetime.fromisoformat(str(value)).strftime("%-d %b %Y · %H:%M")
    except (TypeError, ValueError):
        return str(value or "")


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


class RecordingsList(QObject):
    rowsChanged = Signal()
    framesChanged = Signal()
    gameIdChanged = Signal()

    def __init__(self, client, parent=None):
        super().__init__(parent)
        self._client = client
        self._game_id = ""
        self._rows = []
        self._frames = {}
        self._queue = []
        self._running = {}
        self._all = False
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
            rows.append({
                "session": session, "path": path,
                "url": QUrl.fromLocalFile(path).toString() if path else "",
                "size": rec.get("size") or 0, "sizeText": _size(rec.get("size")),
                "duration_s": line.get("duration_s") or 0, "durationText": _duration(line.get("duration_s")),
                "dateText": _when(line.get("ended_at")), "hasJournal": line.get("journal") is not None,
                "created_at": str(line.get("ended_at") or ""), "gameId": str(line.get("game") or ""), "gameTitle": str(line.get("title") or ""),
            })
            if path and session and session not in self._frames:
                self._frames[session] = Frames(path, rec.get("duration_s") or line.get("duration_s"))
        self._rows = rows
        self.rowsChanged.emit()
        self.framesChanged.emit()
        for row in rows:
            self._want_thumbnail(row["session"])
        self._pump()

    @Slot()
    def unload(self):
        self._all = False

    @Slot(str, str, result=bool)
    def remove(self, game_id, session):
        if not self._client.removeRecording(game_id, session):
            return False
        self._queue = [j for j in self._queue if j[0] != session]
        self._kill(session)
        frames = self._frames.pop(session, None)
        if frames is not None:
            shutil.rmtree(frames.dir, ignore_errors=True)
            self.framesChanged.emit()
        return True

    @Slot(str)
    def select(self, session):
        frames = self._frames.get(session)
        if frames is None:
            return
        jobs = [(session, i) for i in range(FRAME_COUNT) if i not in frames.extracted and (session, i) not in self._running]
        self._queue = jobs + [j for j in self._queue if j[0] != session]
        self._pump()

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
            self._extract(job, frames)

    def _extract(self, job, frames):
        ffmpeg = shutil.which("ffmpeg")
        if not ffmpeg:
            return
        index = job[1]
        os.makedirs(frames.dir, exist_ok=True)
        proc = QProcess(self)
        self._running[job] = proc
        proc.finished.connect(lambda code, status: self._extracted(job, proc, code))
        proc.start(ffmpeg, ["-loglevel", "error", "-y", "-ss", f"{frames.seconds(index):.3f}", "-i", frames.path,
                            "-frames:v", "1", "-vf", f"scale={FRAME_WIDTH}:-2", "-q:v", "4", frames.file(index)])

    def _extracted(self, job, proc, code):
        self._finish(job, proc)
        session, index = job
        frames = self._frames.get(session)
        if frames is not None and code == 0 and os.path.exists(frames.file(index)):
            frames.extracted.add(index)
            self.framesChanged.emit()
        self._pump()

    def _finish(self, job, proc):
        self._running.pop(job, None)
        if shiboken6.isValid(proc):
            proc.deleteLater()

    def _kill(self, session=None):
        for job, proc in list(self._running.items()):
            if session is None or job[0] == session:
                proc.finished.disconnect()
                proc.kill()
                proc.waitForFinished(1000)
                self._finish(job, proc)

    def shutdown(self):
        self._queue.clear()
        self._kill()

    def _frame_map(self):
        out = {}
        for session, frames in self._frames.items():
            thumb = frames.thumbnail()
            out[session] = {
                "thumbnail": QUrl.fromLocalFile(frames.file(thumb)).toString() if thumb is not None else "",
                "frames": [QUrl.fromLocalFile(frames.file(i)).toString() if i in frames.extracted else "" for i in range(FRAME_COUNT)],
                "complete": frames.complete(),
                "duration": frames.duration,
            }
        return out

    rows = Property("QVariantList", lambda self: [dict(r) for r in self._rows], notify=rowsChanged)
    count = Property(int, lambda self: len(self._rows), notify=rowsChanged)
    frameMap = Property("QVariantMap", _frame_map, notify=framesChanged)
    gameId = Property(str, lambda self: self._game_id, notify=gameIdChanged)


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
            rows.append({
                "session": session,
                "title": str(entry.get("title") or ("" if state == "pending" else "Journal failed" if state == "failed" else "Untitled")),
                "state": state, "reason": paragraphs[0] if state == "failed" and paragraphs else "",
                "started_at": str(entry.get("started_at") or ""),
                "dateText": _when(entry.get("written_at") or entry.get("started_at")),
                "duration_s": duration, "durationText": _duration(duration) if duration else "",
                "provider": str(entry.get("provider") or ""),
                "paragraphs": paragraphs, "blocks": markdown_blocks(paragraphs),
                "next_up": str(entry.get("next_up") or ""),
                "images": [QUrl.fromLocalFile(str(p)).toString() for p in entry.get("images") or []],
                "hasRecording": (game_id, session) in recorded,
                "written_at": str(entry.get("written_at") or ""),
                "gameId": game_id, "gameTitle": title,
            })
        return rows

    @Slot()
    def unload(self):
        self._all = False

    @Slot(str, str, result=bool)
    def remove(self, game_id, session):
        return bool(self._client.removeJournalEntry(game_id, session))

    rows = Property("QVariantList", lambda self: [dict(r) for r in self._rows], notify=rowsChanged)
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
        self._timer = QTimer(self)
        self._timer.setInterval(POLL_MS)
        self._timer.timeout.connect(self.refresh)
        client.entryWritten.connect(lambda session, ident: self.refresh())
        client.sessionEnded.connect(lambda session, ident, duration: self.refresh())
        self.refresh()

    @Slot()
    def refresh(self):
        rows = []
        for entry in self._client.pendingJournals():
            rows.append({"game": str(entry.get("game") or ""), "title": str(entry.get("title") or ""),
                         "session": str(entry.get("session") or ""), "started_at": str(entry.get("started_at") or "")})
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

    rows = Property("QVariantList", lambda self: [dict(r) for r in self._rows], notify=changed)
    count = Property(int, lambda self: len(self._rows), notify=changed)
