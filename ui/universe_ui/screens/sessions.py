from datetime import datetime

from PySide6.QtCore import QObject, Signal, Slot

from ..qt import Property
from .media import _duration, _when

# Lines the pages show: a crash's last words, not a whole evening of gamescope
TAIL = 400

END_TEXT = {"quit": "Quit", "stopped": "Stopped", "crashed": "Crashed", "killed": "Killed", "ended": "Ended"}


def _clock(value):
    try:
        return datetime.fromisoformat(str(value)).strftime("%H:%M:%S")
    except (TypeError, ValueError):
        return ""


def _session_row(line):
    end = str(line.get("end") or "quit")
    exit_code = int(line.get("exit") or 0)
    return {
        "session": str(line.get("session") or ""),
        "gameId": str(line.get("game") or ""),
        "gameTitle": str(line.get("title") or ""),
        "started_at": str(line.get("started_at") or ""),
        "dateText": _when(line.get("started_at")),
        "durationText": _duration(line.get("duration_s")),
        "end": end,
        "endText": f"Crashed (exit {exit_code})" if end == "crashed" else END_TEXT.get(end, end),
        "bad": end in ("crashed", "killed"),
        "exit": exit_code,
        "source": str(line.get("source") or ""),
        "hasRecording": bool(line.get("recording")),
        "hasJournal": bool(line.get("journal")),
        "debugLog": str(line.get("debug_log") or ""),
        "live": False,
    }


def _log_row(line):
    return {
        "time": _clock(line.get("time")),
        "source": str(line.get("source") or ""),
        "message": str(line.get("message") or ""),
        "error": int(line.get("priority") or 6) <= 3,
        "warning": int(line.get("priority") or 6) == 4,
    }


class SessionsList(QObject):
    rowsChanged = Signal()
    logChanged = Signal()
    gameIdChanged = Signal()

    def __init__(self, client, parent=None):
        super().__init__(parent)
        self._client = client
        self._game_id = ""
        self._rows = []
        self._log_session = ""
        self._log = []
        self._log_loading = False
        self._log_error = ""
        self._generation = 0
        client.libraryChanged.connect(self._changed)
        client.currentSessionChanged.connect(lambda: self._changed([]))

    def _changed(self, ids):
        if self._game_id and (not ids or self._game_id in ids):
            self._reload()

    @Slot(str)
    def load(self, game_id):
        if game_id != self._game_id:
            self._log_session, self._log, self._log_error = "", [], ""
            self.logChanged.emit()
        self._game_id = game_id
        self.gameIdChanged.emit()
        self._reload()

    # The running session is not in sessions.jsonl yet: its row comes from the marker, its log under `""`.
    def _reload(self):
        lines = self._client.sessions(self._game_id) if self._game_id else []
        rows = [_session_row(line) for line in lines if line.get("unit")]
        current = self._client.currentSession
        if current and current.get("id") == self._game_id:
            rows.insert(
                0,
                {
                    **_session_row({"session": "", "game": self._game_id, "started_at": current.get("started_at")}),
                    "live": True,
                    "endText": "Playing now",
                    "durationText": "",
                },
            )
        self._rows = rows
        self.rowsChanged.emit()

    @Slot()
    def unload(self):
        self._game_id = ""
        self._rows = []
        self.rowsChanged.emit()
        self.closeLog()

    @Slot(str)
    def openLog(self, session):
        if not self._game_id:
            return
        self._generation += 1
        generation = self._generation
        self._log_session, self._log_loading, self._log_error = session, True, ""
        self.logChanged.emit()

        def landed(lines):
            if generation != self._generation:
                return
            self._log, self._log_loading = [_log_row(line) for line in lines or []], False
            self.logChanged.emit()

        def missed(e):
            if generation != self._generation:
                return
            self._log, self._log_loading, self._log_error = [], False, e.message
            self.logChanged.emit()

        self._client.sessionLogAsync(self._game_id, session, TAIL, landed, missed)

    @Slot()
    def closeLog(self):
        self._generation += 1
        self._log_session, self._log, self._log_loading, self._log_error = "", [], False, ""
        self.logChanged.emit()

    rows = Property(list, lambda self: [dict(r) for r in self._rows], notify=rowsChanged)
    count = Property(int, lambda self: len(self._rows), notify=rowsChanged)
    gameId = Property(str, lambda self: self._game_id, notify=gameIdChanged)
    logSession = Property(str, lambda self: self._log_session, notify=logChanged)
    log = Property(list, lambda self: [dict(r) for r in self._log], notify=logChanged)
    logLoading = Property(bool, lambda self: self._log_loading, notify=logChanged)
    logError = Property(str, lambda self: self._log_error, notify=logChanged)
