"""Recordings (with frames sampled by ffmpeg) and journal entries for one game."""

import hashlib
import json
import os
import re
import shutil
from datetime import datetime

import shiboken6
from PySide6.QtCore import Property, QObject, QProcess, Qt, QUrl, Signal, Slot


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
    n = float(n or 0)
    for unit in ("B", "KB", "MB", "GB"):
        if n < 1024 or unit == "GB":
            return f"{n:.0f} {unit}" if unit == "B" else f"{n:.1f} {unit}"
        n /= 1024
    return ""


def _cache_dir():
    base = os.environ.get("UNIVERSE_CACHE_HOME")
    if not base:
        xdg = os.environ.get("XDG_CACHE_HOME") or os.path.join(os.path.expanduser("~"), ".cache")
        base = os.path.join(xdg, "universe")
    path = os.path.join(base, "frames")
    os.makedirs(path, exist_ok=True)
    return path


FRAME_COUNT = 16
FRAME_WIDTH = 640
# Which frame stands for the recording: the first of these that is not black or a flat fade.
THUMB_ORDER = (3, 7, 11, 15, 1, 5, 9, 13, 0, 2, 4, 6, 8, 10, 12, 14)
# 8x8 area-averaged gray: below this the frame is black or a flat fade (images.py's threshold).
FLAT_STDDEV = 4.0
WORKERS = 2


def frame_stddev(path):
    from PySide6.QtGui import QImage

    image = QImage(path)
    if image.isNull():
        return 0.0
    small = image.scaled(8, 8, Qt.AspectRatioMode.IgnoreAspectRatio, Qt.TransformationMode.SmoothTransformation)
    grays = [small.pixelColor(x, y).value() for y in range(8) for x in range(8)]
    mean = sum(grays) / len(grays)
    return (sum((g - mean) ** 2 for g in grays) / len(grays)) ** 0.5


class Frames:
    """The cached frames of one recording: `<cache>/frames/<sha1 of path>/NN.jpg` plus stats.json."""

    def __init__(self, path):
        self.path = path
        self.dir = os.path.join(_cache_dir(), hashlib.sha1(path.encode()).hexdigest())
        self.duration = 0.0
        self.stddev = {}
        try:
            with open(os.path.join(self.dir, "stats.json")) as f:
                data = json.load(f)
            self.duration = float(data.get("duration") or 0)
            self.stddev = {int(k): float(v) for k, v in (data.get("stddev") or {}).items()}
        except (OSError, ValueError, TypeError):
            pass
        self.stddev = {i: v for i, v in self.stddev.items() if os.path.exists(self.file(i))}

    def file(self, index):
        return os.path.join(self.dir, f"{index:02d}.jpg")

    def save(self):
        os.makedirs(self.dir, exist_ok=True)
        with open(os.path.join(self.dir, "stats.json"), "w") as f:
            json.dump({"duration": self.duration, "stddev": {str(k): v for k, v in self.stddev.items()}}, f)

    def seconds(self, index):
        return (index + 0.5) / FRAME_COUNT * self.duration

    def thumbnail(self):
        for i in THUMB_ORDER:
            if self.stddev.get(i, 0.0) >= FLAT_STDDEV:
                return i
        if len(self.stddev) == FRAME_COUNT:
            return max(self.stddev, key=self.stddev.get)
        return None

    def next_candidate(self):
        for i in THUMB_ORDER:
            if i not in self.stddev:
                return i
        return None

    def complete(self):
        return len(self.stddev) == FRAME_COUNT


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
        client.recordingFiled.connect(lambda session, ident, path: ident == self._game_id and self.load(ident))

    @Slot(str)
    def load(self, game_id):
        self._game_id = game_id
        self.gameIdChanged.emit()
        self._queue.clear()
        journal = {str(e.get("session") or "") for e in self._client.journal(game_id) or []}
        rows = []
        for rec in self._client.recordings(game_id) or []:
            path = str(rec.get("path") or "")
            session = str(rec.get("session") or "")
            rows.append({
                "session": session, "path": path,
                "url": QUrl.fromLocalFile(path).toString() if path else "",
                "size": rec.get("size") or 0, "sizeText": _size(rec.get("size")),
                "duration_s": rec.get("duration_s") or 0, "durationText": _duration(rec.get("duration_s")),
                "dateText": _when(rec.get("created_at")), "hasJournal": session in journal,
            })
            if path and session and session not in self._frames:
                self._frames[session] = Frames(path)
        self._rows = rows
        self.rowsChanged.emit()
        self.framesChanged.emit()
        for row in rows:
            self._want_thumbnail(row["session"])
        self._pump()

    # The picked row gets all its frames, ahead of the other rows' thumbnails. In thumbnail order,
    # so the one shown in the list is settled before the rest of the mosaic arrives.
    @Slot(str)
    def select(self, session):
        frames = self._frames.get(session)
        if frames is None:
            return
        jobs = [(session, i) for i in THUMB_ORDER if i not in frames.stddev and (session, i) not in self._running]
        self._queue = jobs + [j for j in self._queue if j[0] != session]
        self._pump()

    def _want_thumbnail(self, session):
        frames = self._frames.get(session)
        if frames is None or frames.thumbnail() is not None:
            return
        index = frames.next_candidate()
        if index is not None and (session, index) not in self._queue and (session, index) not in self._running:
            self._queue.append((session, index))

    def _pump(self):
        # Frames of a file still being probed wait for its duration, in place.
        waiting = []
        while self._queue and len(self._running) < WORKERS:
            job = self._queue.pop(0)
            if job in self._running:
                continue
            frames = self._frames.get(job[0])
            if frames is None or not os.path.exists(frames.path):
                continue
            if frames.duration > 0:
                self._extract(job, frames)
            elif any(j[0] == job[0] for j in self._running):
                waiting.append(job)
            else:
                self._probe(job, frames)
        self._queue = waiting + self._queue

    def _start(self, job, program, args, done):
        exe = shutil.which(program)
        if not exe:
            return
        proc = QProcess(self)
        self._running[job] = proc
        proc.finished.connect(lambda code, status: done(job, proc, code))
        proc.start(exe, args)

    def _probe(self, job, frames):
        self._start(job, "ffprobe", ["-v", "error", "-show_entries", "format=duration",
                                     "-of", "default=noprint_wrappers=1:nokey=1", frames.path], self._probed)

    def _probed(self, job, proc, code):
        self._finish(job, proc)
        frames = self._frames.get(job[0])
        if frames is not None and code == 0:
            try:
                frames.duration = float(bytes(proc.readAllStandardOutput()).decode().strip())
            except ValueError:
                frames.duration = 0.0
            if frames.duration > 0:
                frames.save()
                self._queue.insert(0, job)
        self._pump()

    def _extract(self, job, frames):
        index = job[1]
        os.makedirs(frames.dir, exist_ok=True)
        self._start(job, "ffmpeg", ["-loglevel", "error", "-y", "-ss", f"{frames.seconds(index):.3f}", "-i", frames.path,
                                    "-frames:v", "1", "-vf", f"scale={FRAME_WIDTH}:-2", "-q:v", "4", frames.file(index)],
                    self._extracted)

    def _extracted(self, job, proc, code):
        self._finish(job, proc)
        session, index = job
        frames = self._frames.get(session)
        if frames is not None and code == 0 and os.path.exists(frames.file(index)):
            frames.stddev[index] = frame_stddev(frames.file(index))
            frames.save()
            self._want_thumbnail(session)
            self.framesChanged.emit()
        self._pump()

    def _finish(self, job, proc):
        self._running.pop(job, None)
        if shiboken6.isValid(proc):
            proc.deleteLater()

    def shutdown(self):
        self._queue.clear()
        for proc in self._running.values():
            proc.finished.disconnect()
            proc.kill()
            proc.waitForFinished(1000)
        self._running.clear()

    def _frame_map(self):
        out = {}
        for session, frames in self._frames.items():
            thumb = frames.thumbnail()
            out[session] = {
                "thumbnail": QUrl.fromLocalFile(frames.file(thumb)).toString() if thumb is not None else "",
                "frames": [QUrl.fromLocalFile(frames.file(i)).toString() if i in frames.stddev else "" for i in range(FRAME_COUNT)],
                "complete": frames.complete(),
                "duration": frames.duration,
            }
        return out

    rows = Property("QVariantList", lambda self: [dict(r) for r in self._rows], notify=rowsChanged)
    count = Property(int, lambda self: len(self._rows), notify=rowsChanged)
    # session → { thumbnail, frames[16] ("" until extracted), complete, duration }
    frameMap = Property("QVariantMap", _frame_map, notify=framesChanged)
    gameId = Property(str, lambda self: self._game_id, notify=gameIdChanged)


LIST_ITEM = re.compile(r"^\s*(?:[-*+]|\d+[.)])\s+")


def markdown_blocks(paragraphs):
    """Paragraphs as Markdown blocks: list items that follow each other become one list."""
    blocks = []
    for p in paragraphs:
        if blocks and LIST_ITEM.match(p) and LIST_ITEM.match(blocks[-1].rsplit("\n", 1)[-1]):
            blocks[-1] += "\n" + p
        else:
            blocks.append(p)
    return blocks


def _journal_dir(game_id):
    data = os.environ.get("XDG_DATA_HOME") or os.path.join(os.path.expanduser("~"), ".local", "share")
    return os.path.join(data, "universe", "games", game_id, "journal")


class JournalList(QObject):
    rowsChanged = Signal()
    gameIdChanged = Signal()

    def __init__(self, client, parent=None):
        super().__init__(parent)
        self._client = client
        self._game_id = ""
        self._rows = []
        client.entryWritten.connect(lambda session, ident: ident == self._game_id and self.load(ident))

    @Slot(str)
    def load(self, game_id):
        self._game_id = game_id
        self.gameIdChanged.emit()
        game_dir = (self._client.game(game_id) or {}).get("dir") or ""
        base = os.path.join(game_dir, "journal") if game_dir else _journal_dir(game_id)
        recorded = {str(r.get("session") or "") for r in self._client.recordings(game_id) or []}
        rows = []
        for entry in self._client.journal(game_id) or []:
            images = []
            for rel in entry.get("images") or []:
                path = rel if os.path.isabs(str(rel)) else os.path.join(base, str(rel))
                images.append(QUrl.fromLocalFile(path).toString())
            paragraphs = [str(p) for p in entry.get("paragraphs") or []]
            rows.append({
                "session": str(entry.get("session") or ""), "title": str(entry.get("title") or "Untitled"),
                "dateText": _when(entry.get("written_at")), "lang": str(entry.get("lang") or ""),
                "provider": str(entry.get("provider") or ""),
                "paragraphs": paragraphs, "blocks": markdown_blocks(paragraphs),
                "next_up": str(entry.get("next_up") or ""), "images": images,
                "hasRecording": str(entry.get("session") or "") in recorded,
            })
        self._rows = rows
        self.rowsChanged.emit()

    @Slot(result=str)
    def render(self):
        return self._client.renderJournal(self._game_id) if self._game_id else ""

    rows = Property("QVariantList", lambda self: [dict(r) for r in self._rows], notify=rowsChanged)
    count = Property(int, lambda self: len(self._rows), notify=rowsChanged)
    gameId = Property(str, lambda self: self._game_id, notify=gameIdChanged)
