from PySide6.QtCore import QObject, Qt, QTimer, Signal, Slot

from .qt import Property

# From its first frame, the most the intro holds the input whether or not its layer ever says it is done.
CAP_MS = 4000


class Boot(QObject):
    changed = Signal()
    started = Signal()
    skipped = Signal()
    landed = Signal(bool, arguments=["skipped"])

    def __init__(self, running=False, parent=None):
        super().__init__(parent)
        self._running = bool(running)
        self._started = False
        self._landed = False
        self._skipping = False
        self._held = set()
        self._window = None
        self._cap = QTimer(self)
        self._cap.setSingleShot(True)
        self._cap.setInterval(CAP_MS)
        self._cap.timeout.connect(self.finish)
        if self._running:
            self._cap.start()

    def watch(self, window):
        if self._running and self._window is None:
            self._window = window
            window.frameSwapped.connect(self._shown, Qt.ConnectionType.QueuedConnection)

    @Slot()
    def _shown(self):
        if self._window is not None:
            self._window.frameSwapped.disconnect(self._shown)
            self._window = None
        if self._started or not self._running:
            return
        self._started = True
        self._cap.start()
        self.started.emit()

    @Slot()
    def skip(self):
        if not self._running or self._skipping:
            return
        self._skipping = True
        self.skipped.emit()
        self.land()

    # The mark is done: the look builds its home under the fading layer, at once after a skip.
    @Slot()
    def land(self):
        if self._running and not self._landed:
            self._landed = True
            self.landed.emit(self._skipping)

    @Slot()
    def finish(self):
        if not self._running:
            return
        self.land()
        self._running = False
        self._cap.stop()
        self.changed.emit()

    def takes(self, ident, pressed):
        """Whether the intro keeps this press or release (`ident`: a key code, "guide", "mouse" or "touch") from the launcher."""
        if self._running and pressed:
            self._held.add(ident)
            self.skip()
            return True
        if ident in self._held:
            if not pressed:
                self._held.discard(ident)
            return True
        return self._running

    running = Property(bool, lambda self: self._running, notify=changed)
