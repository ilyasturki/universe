from PySide6.QtCore import Property, QObject

from .add import AddGameForm
from .artwork import ArtworkForm, ArtworkOverview
from .controller import ControllerScreen
from .launch import LaunchForm
from .media import JournalList, PendingJournals, RecordingsList
from .paths import PathBrowser
from .runners import RunnerForm, RunnersForm
from .settings import GameSettingsForm, ModuleForm, ModulesForm, SourceForm, SourcesForm
from .sources import LoginFlow, SourcesBrowser


class Screens(QObject):
    def __init__(self, client, memory, screen_mode, games, power, parent=None):
        super().__init__(parent)
        self._gameSettings = GameSettingsForm(client, screen_mode, self)
        self._modules = ModulesForm(client, self)
        self._module = ModuleForm(client, self)
        self._sourceList = SourcesForm(client, self)
        self._source = SourceForm(client, self)
        self._launch = LaunchForm(client, screen_mode, self)
        self._sources = SourcesBrowser(client, games, self)
        self._login = LoginFlow(client, self)
        self._login.finished.connect(lambda ok, text: (self._sourceList.load(), self._source.reload()))
        self._recordings = RecordingsList(client, self)
        self._journal = JournalList(client, self)
        self._pendingJournals = PendingJournals(client, self)
        self._paths = PathBrowser(client, self)
        self._controller = ControllerScreen(client, memory, power, self)
        self._runners = RunnersForm(client, self)
        self._runner = RunnerForm(client, screen_mode, self)
        self._artwork = ArtworkForm(client, self)
        self._artworkOverview = ArtworkOverview(client, self)
        self._add = AddGameForm(client, self)

    def shutdown(self):
        self._recordings.shutdown()
        self._pendingJournals.shutdown()
        self._controller.shutdown()

    gameSettings = Property(QObject, lambda self: self._gameSettings, constant=True)
    modules = Property(QObject, lambda self: self._modules, constant=True)
    module = Property(QObject, lambda self: self._module, constant=True)
    sourceList = Property(QObject, lambda self: self._sourceList, constant=True)
    source = Property(QObject, lambda self: self._source, constant=True)
    launch = Property(QObject, lambda self: self._launch, constant=True)
    sources = Property(QObject, lambda self: self._sources, constant=True)
    login = Property(QObject, lambda self: self._login, constant=True)
    recordings = Property(QObject, lambda self: self._recordings, constant=True)
    journal = Property(QObject, lambda self: self._journal, constant=True)
    pendingJournals = Property(QObject, lambda self: self._pendingJournals, constant=True)
    album = Property(QObject, lambda self: self._recordings, constant=True)
    news = Property(QObject, lambda self: self._journal, constant=True)
    paths = Property(QObject, lambda self: self._paths, constant=True)
    controller = Property(QObject, lambda self: self._controller, constant=True)
    runners = Property(QObject, lambda self: self._runners, constant=True)
    runner = Property(QObject, lambda self: self._runner, constant=True)
    artwork = Property(QObject, lambda self: self._artwork, constant=True)
    artworkOverview = Property(QObject, lambda self: self._artworkOverview, constant=True)
    add = Property(QObject, lambda self: self._add, constant=True)
