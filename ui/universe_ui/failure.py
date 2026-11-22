import logging

from PySide6.QtCore import QCoreApplication, QObject, Signal, Slot

from .qt import Property

log = logging.getLogger("universe.host")


def reason_of(error):
    reason = str(error.args[-1] if error.args else error).strip()
    log.error("the core did not open: %s", reason)
    return reason


def map_saved_pads(gamepad, unopened):
    """Each pad here mapped as the controller setup saved it, again as one arrives: with no core, nothing else tells SDL."""
    from .screens.controller import ControllerScreen, pad_mapping

    def mapped():
        try:
            listed = unopened()
        except Exception as error:  # noqa: BLE001  the page stays on the keyboard and the pads SDL knows
            log.warning("the saved pad mappings could not be read: %s", error)
            return
        families = {f["id"]: f for f in listed["state"].get("families") or [] if f.get("id")}
        for line in listed["pads"]:
            mapping = pad_mapping(ControllerScreen._entry(line), families)
            if mapping:
                gamepad.setMapping(*mapping)

    gamepad.added.connect(mapped)
    mapped()


class Failure(QObject):
    changed = Signal()

    # `broken()` names config.toml when the core refuses it, `defaults()` sets it aside and returns where it went.
    def __init__(self, error, attempt, session=False, fullscreen=False, broken=None, defaults=None, parent=None):
        super().__init__(parent)
        self._reason = reason_of(error)
        self._attempt = attempt
        self._tries = 0
        self._session = session
        self._fullscreen = fullscreen
        self._broken = broken or (lambda: None)
        self._defaults = defaults
        self._offered = self._offers()
        self.kept = ""
        self.client = None

    def _offers(self):
        try:
            return bool(self._defaults and self._broken())
        except Exception:  # an offer that cannot be checked is not made
            log.exception("checking config.toml")
            return False

    @Slot()
    def retry(self):
        self._tries += 1
        try:
            self.client = self._attempt()
        except Exception as error:  # noqa: BLE001  as host.run: any of them stays on the page
            self._reason = reason_of(error)
            self._offered = self._offers()
            self.changed.emit()
        else:
            QCoreApplication.quit()

    @Slot()
    def startOnDefaults(self):
        if self._defaults is None:
            return
        try:
            kept = self._defaults()
        except Exception as error:  # noqa: BLE001  the file stays as it was, and the page says why
            self._reason = reason_of(error)
            self.changed.emit()
            return
        if kept:
            self.kept = kept
            log.info("config.toml would not load: kept as %s, started on the defaults", kept)
        self.retry()

    @Slot()
    def leave(self):
        QCoreApplication.quit()

    reason = Property(str, lambda self: self._reason, notify=changed)
    retried = Property(bool, lambda self: self._tries > 0, notify=changed)
    offersDefaults = Property(bool, lambda self: self._offered, notify=changed)
    keptAt = Property(str, lambda self: self.kept, notify=changed)
    session = Property(bool, lambda self: self._session, constant=True)
    fullscreen = Property(bool, lambda self: self._fullscreen, constant=True)
