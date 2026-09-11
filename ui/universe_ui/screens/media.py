"""Recordings (with ffmpeg thumbnails) and journal entries for one game."""

import hashlib
import os
import shutil
from datetime import datetime

import shiboken6
from PySide6.QtCore import Property, QObject, QProcess, QUrl, Signal, Slot


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
    base = os.environ.get("XDG_CACHE_HOME") or os.path.join(os.path.expanduser("~"), ".cache")
    path = os.path.join(base, "universe", "thumbnails")
    os.makedirs(path, exist_ok=True)
    return path


class RecordingsList(QObject):
    rowsChanged = Signal()
    gameIdChanged = Signal()

    def __init__(self, client, parent=None):
        super().__init__(parent)
        self._client = client
        self._game_id = ""
        self._rows = []
        self._pending = {}
        client.recordingFiled.connect(lambda session, ident, path: ident == self._game_id and self.load(ident))

    @Slot(str)
    def load(self, game_id):
        self._game_id = game_id
        self.gameIdChanged.emit()
        rows = []
        for rec in self._client.recordings(game_id) or []:
            path = str(rec.get("path") or "")
            rows.append({
                "session": str(rec.get("session") or ""), "path": path,
                "url": QUrl.fromLocalFile(path).toString() if path else "",
                "size": rec.get("size") or 0, "sizeText": _size(rec.get("size")),
                "duration_s": rec.get("duration_s") or 0, "durationText": _duration(rec.get("duration_s")),
                "dateText": _when(rec.get("created_at")), "thumbnail": "",
            })
        self._rows = rows
        self.rowsChanged.emit()
        for i, row in enumerate(rows):
            self._thumbnail(i, row["path"])

    # One JPEG per clip, taken a few seconds in; cached by path so a second visit is free.
    def _thumbnail(self, index, path):
        if not path or not os.path.exists(path):
            return
        out = os.path.join(_cache_dir(), hashlib.sha1(path.encode()).hexdigest() + ".jpg")
        if os.path.exists(out):
            self._set_thumbnail(index, out)
            return
        ffmpeg = shutil.which("ffmpeg")
        if not ffmpeg or out in self._pending:
            return
        proc = QProcess(self)
        self._pending[out] = proc
        proc.finished.connect(lambda code, status, out=out, index=index: self._thumbnail_done(index, out, code))
        proc.start(ffmpeg, ["-loglevel", "error", "-y", "-ss", "00:00:02", "-i", path, "-frames:v", "1",
                            "-vf", "scale=640:-2", out])

    def shutdown(self):
        for proc in self._pending.values():
            proc.finished.disconnect()
            proc.kill()
            proc.waitForFinished(1000)
        self._pending.clear()

    def _thumbnail_done(self, index, out, code):
        proc = self._pending.pop(out, None)
        if proc is not None and shiboken6.isValid(proc):
            proc.deleteLater()
        if code == 0 and os.path.exists(out):
            self._set_thumbnail(index, out)

    def _set_thumbnail(self, index, out):
        if 0 <= index < len(self._rows):
            self._rows[index]["thumbnail"] = QUrl.fromLocalFile(out).toString()
            self.rowsChanged.emit()

    rows = Property("QVariantList", lambda self: [dict(r) for r in self._rows], notify=rowsChanged)
    count = Property(int, lambda self: len(self._rows), notify=rowsChanged)
    gameId = Property(str, lambda self: self._game_id, notify=gameIdChanged)


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
        rows = []
        for entry in self._client.journal(game_id) or []:
            images = []
            for rel in entry.get("images") or []:
                path = rel if os.path.isabs(str(rel)) else os.path.join(base, str(rel))
                images.append(QUrl.fromLocalFile(path).toString())
            rows.append({
                "session": str(entry.get("session") or ""), "title": str(entry.get("title") or "Untitled"),
                "dateText": _when(entry.get("written_at")), "lang": str(entry.get("lang") or ""),
                "provider": str(entry.get("provider") or ""),
                "paragraphs": [str(p) for p in entry.get("paragraphs") or []],
                "next_up": str(entry.get("next_up") or ""), "images": images,
            })
        self._rows = rows
        self.rowsChanged.emit()

    @Slot(result=str)
    def render(self):
        return self._client.renderJournal(self._game_id) if self._game_id else ""

    rows = Property("QVariantList", lambda self: [dict(r) for r in self._rows], notify=rowsChanged)
    count = Property(int, lambda self: len(self._rows), notify=rowsChanged)
    gameId = Property(str, lambda self: self._game_id, notify=gameIdChanged)
