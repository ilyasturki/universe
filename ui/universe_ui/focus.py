import logging
import os
from typing import cast

from PySide6.QtCore import QObject, Qt, QTimer, Signal, Slot
from PySide6.QtGui import QGuiApplication

from .host import OWN_ENV
from .qt import Property

POLL_MS = 500

log = logging.getLogger("universe.focus")


def mode_of(client):
    """`always` where nothing else can have the focus, `host` inside the launcher's nested gamescope (which passes no focus change on to it), else `qt`."""
    if client.underSteam or os.environ.get(OWN_ENV) == "drm":
        return "always"
    if client.nested:
        return "host"
    if QGuiApplication.platformName() in ("offscreen", "minimal"):
        return "always"
    return "qt"


class Focus(QObject):
    changed = Signal()

    def __init__(self, client, mode=None, parent=None):
        super().__init__(parent)
        self._client = client
        self._mode = mode or mode_of(client)
        self._qt_active = True
        # The desktop's last answer, (launcher, session); None while it cannot tell.
        self._host = None
        self._polling = False
        self._generation = 0
        self._timer = QTimer(self)
        self._timer.setInterval(POLL_MS)
        self._timer.timeout.connect(self._poll)
        self.changed.connect(self._log)
        log.info("focus: %s", self._mode)
        if self._mode == "host":
            self._timer.start()
            self._poll()
        elif self._mode == "qt":
            app = cast("QGuiApplication", QGuiApplication.instance())
            app.applicationStateChanged.connect(self._on_state)
            self._on_state(app.applicationState())

    # The desktop is asked only while Qt says another window has the focus: whether it is the launcher's game's.
    def _on_state(self, state):
        active = state == Qt.ApplicationState.ApplicationActive
        if active == self._qt_active:
            return
        self._qt_active = active
        self._generation += 1
        self._polling = False
        self._host = None
        if active:
            self._timer.stop()
        else:
            self._timer.start()
            self._poll()
        self.changed.emit()

    @Slot()
    def refresh(self):
        if self._mode == "host" or (self._mode == "qt" and not self._qt_active):
            self._poll()

    def _poll(self):
        if self._polling:
            return
        self._polling = True
        generation = self._generation

        def landed(focus):
            if generation != self._generation:
                return
            self._polling = False
            self._land(None if focus is None else (bool(focus.get("launcher")), bool(focus.get("session"))))

        self._client.hostFocusAsync(landed)

    def _land(self, host):
        before = (self._active(), self._elsewhere())
        self._host = host
        if (self._active(), self._elsewhere()) != before:
            self.changed.emit()

    def _log(self):
        if os.environ.get("UNIVERSE_UI_INPUT_LOG"):
            log.info("focus: %s", "active" if self._active() else "elsewhere" if self._elsewhere() else "inactive")

    def summon(self):
        self._client.summon(self.refresh)

    def _active(self):
        if self._mode == "qt":
            return self._qt_active
        if self._mode == "host":
            return self._host is None or self._host[0]
        return True

    def _elsewhere(self):
        return not self._active() and self._host is not None and not any(self._host)

    def shutdown(self):
        self._generation += 1
        self._timer.stop()

    # The launcher has the focus, or nobody can tell: what it shows may move and its pad acts.
    active = Property(bool, _active, notify=changed)
    # Another app has the focus, neither the launcher nor the game it runs: HOME summons it or does nothing.
    elsewhere = Property(bool, _elsewhere, notify=changed)
    mode = Property(str, lambda self: self._mode, constant=True)
