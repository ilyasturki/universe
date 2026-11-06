import logging

from PySide6.QtCore import QCoreApplication, QObject, Signal, Slot

from .qt import Property

log = logging.getLogger("universe.host")


def reason_of(error):
    reason = str(error.args[-1] if error.args else error).strip()
    log.error("the core did not open: %s", reason)
    return reason


class Failure(QObject):
    changed = Signal()

    def __init__(self, error, attempt, session=False, fullscreen=False, parent=None):
        super().__init__(parent)
        self._reason = reason_of(error)
        self._attempt = attempt
        self._tries = 0
        self._session = session
        self._fullscreen = fullscreen
        self.client = None

    @Slot()
    def retry(self):
        self._tries += 1
        try:
            self.client = self._attempt()
        except Exception as error:  # noqa: BLE001  as host.run: any of them stays on the page
            self._reason = reason_of(error)
            self.changed.emit()
        else:
            QCoreApplication.quit()

    @Slot()
    def leave(self):
        QCoreApplication.quit()

    reason = Property(str, lambda self: self._reason, notify=changed)
    retried = Property(bool, lambda self: self._tries > 0, notify=changed)
    session = Property(bool, lambda self: self._session, constant=True)
    fullscreen = Property(bool, lambda self: self._fullscreen, constant=True)
