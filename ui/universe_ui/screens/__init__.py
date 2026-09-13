"""`api.screens`: one support object per new screen, all data through the client."""

from PySide6.QtCore import Property, QObject

from .controller import ControllerScreen
from .media import JournalList, PendingJournals, RecordingsList
from .paths import PathBrowser
from .runners import RunnersForm
from .settings import GameSettingsForm, ModulesForm
from .sources import LoginFlow, SourcesBrowser


class Screens(QObject):
    def __init__(self, client, screen_hz=lambda: 0, parent=None, memory=None):
        super().__init__(parent)
        self._gameSettings = GameSettingsForm(client, self)
        self._modules = ModulesForm(client, screen_hz, self)
        self._sources = SourcesBrowser(client, self)
        self._login = LoginFlow(client, self)
        self._recordings = RecordingsList(client, self)
        self._journal = JournalList(client, self)
        self._pendingJournals = PendingJournals(client, self)
        self._paths = PathBrowser(client, self)
        self._controller = ControllerScreen(client, memory, self)
        self._runners = RunnersForm(client, self)

    def shutdown(self):
        self._recordings.shutdown()
        self._pendingJournals.shutdown()
        self._controller.shutdown()

    gameSettings = Property(QObject, lambda self: self._gameSettings, constant=True)
    modules = Property(QObject, lambda self: self._modules, constant=True)
    sources = Property(QObject, lambda self: self._sources, constant=True)
    login = Property(QObject, lambda self: self._login, constant=True)
    recordings = Property(QObject, lambda self: self._recordings, constant=True)
    journal = Property(QObject, lambda self: self._journal, constant=True)
    pendingJournals = Property(QObject, lambda self: self._pendingJournals, constant=True)
    paths = Property(QObject, lambda self: self._paths, constant=True)
    controller = Property(QObject, lambda self: self._controller, constant=True)
    runners = Property(QObject, lambda self: self._runners, constant=True)
