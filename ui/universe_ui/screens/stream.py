import json
import logging
import os
import shutil

from PySide6.QtCore import QObject, QProcess, Qt, QTimer, Signal, Slot

log = logging.getLogger("universe.stream")
RESTART_MS = 2000
RESTART_MAX_MS = 30000


class Stream(QObject):
    """A `universe … watch --json` child: each JSON line it prints as `received`, commands to its stdin through `send`, and
    `{"event": "off", "code": …}` once it ends."""

    received = Signal(object)

    def __init__(self, args, parent=None):
        super().__init__(parent)
        self._args = list(args)
        self._process = None
        self._buffer = b""

    def start(self, *_):
        program = os.environ.get("UNIVERSE_BIN") or shutil.which("universe")
        if not program:
            log.warning("no universe binary: `%s` is off", " ".join(self._args[:2]))
            return False
        self._process = QProcess(self)
        self._process.setProcessChannelMode(QProcess.ProcessChannelMode.ForwardedErrorChannel)
        self._process.readyReadStandardOutput.connect(self._read)
        self._process.finished.connect(self._finished)
        self._process.start(program, self._args)
        return True

    def _finished(self, code, status):
        log.info("`%s` ended (%s)", " ".join(self._args[:2]), code)
        if self._process is not None:
            self._process.deleteLater()
            self._process = None
        self.received.emit({"event": "off", "code": code})

    def _read(self):
        if self._process is None:
            return
        self._buffer += bytes(self._process.readAllStandardOutput().data())
        while b"\n" in self._buffer:
            line, self._buffer = self._buffer.split(b"\n", 1)
            try:
                payload = json.loads(line)
            except ValueError:
                continue
            if isinstance(payload, dict):
                self.received.emit(payload)

    def send(self, command):
        if self._process is None or self._process.state() == QProcess.ProcessState.NotRunning:
            return False
        self._process.write((json.dumps(command) + "\n").encode())
        return True

    def stop(self):
        if self._process is None:
            return
        process, self._process = self._process, None
        process.finished.disconnect(self._finished)
        process.closeWriteChannel()
        process.terminate()
        if not process.waitForFinished(2000):
            process.kill()
            process.waitForFinished(1000)


class FakeStream(QObject):
    """The fake core's play of a watch (`FakeCore.watch`): its lines reach `received` through the event loop, as a child's do."""

    received = Signal(object)
    _line = Signal(object)

    def __init__(self, open_watch, parent=None):
        super().__init__(parent)
        self._open = open_watch
        self._handle = None
        self._line.connect(self.received.emit, Qt.ConnectionType.QueuedConnection)

    def start(self, *_):
        self._handle = self._open(self._line.emit)
        return True

    def send(self, command):
        if self._handle is None:
            return False
        self._handle.send(dict(command))
        return True

    def stop(self):
        if self._handle is not None:
            self._handle.stop()
            self._handle = None


class Streamed(QObject):
    """A screen over one `universe <kind> watch`: started on first need, started again, later each time, should it end."""

    def __init__(self, client, kind, parent=None):
        super().__init__(parent)
        self._client = client
        self._kind = kind
        self._stream = None
        self._closed = False
        self._scanning = 0
        self._delay = RESTART_MS
        self._restart = QTimer(self)
        self._restart.setSingleShot(True)
        self._restart.timeout.connect(self.start)

    def start(self):
        if self._stream is not None:
            return True
        stream = self._client.stream(self._kind)
        stream.received.connect(self._received)
        if not stream.start():
            stream.deleteLater()
            return False
        self._closed = False
        self._stream = stream
        if self._scanning:
            stream.send({"cmd": "scan", "on": True})
        return True

    def send(self, command):
        if not self.start() or self._stream is None:
            return False
        return self._stream.send(command)

    def shutdown(self):
        self._closed = True
        self._restart.stop()
        if self._stream is not None:
            self._stream.stop()
            self._stream.deleteLater()
            self._stream = None

    def _received(self, line):
        if line.get("event") == "off":
            if self._stream is not None:
                self._stream.deleteLater()
                self._stream = None
            if not self._closed:
                self._restart.start(self._delay)
                self._delay = min(self._delay * 2, RESTART_MAX_MS)
            self.on_line(line)
            return
        if line.get("event") == "ready":
            self._delay = RESTART_MS
        self.on_line(line)

    # A page listing what the watch finds: it searches while one is open.
    @Slot()
    def open(self):
        self._scanning += 1
        if self._scanning > 1:
            return
        if self._stream is None:
            self.start()
        else:
            self._stream.send({"cmd": "scan", "on": True})

    @Slot()
    def close(self):
        self._scanning = max(0, self._scanning - 1)
        if self._scanning == 0 and self._stream is not None:
            self._stream.send({"cmd": "scan", "on": False})

    def on_line(self, line):
        pass
