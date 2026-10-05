from collections.abc import Callable

from PySide6.QtCore import QObject

from ..qt import Property
from .achievements import AchievementsList
from .add import AddGameForm
from .addons import AddonsForm
from .artwork import ArtworkForm, ArtworkOverview
from .bluetooth import BluetoothScreen
from .changelog import Changelog
from .components import ComponentsForm
from .controller import ControllerScreen
from .data import GameData, Storage
from .launch import LaunchForm
from .media import JournalList, MediaTimeline, PendingJournals, RecordingsList, ScreenshotsList, Thumbs
from .network import Network, WifiScreen
from .onboarding import Onboarding
from .paths import PathBrowser
from .runners import RunnerForm, RunnersForm
from .search import SettingsSearch
from .sessions import SessionsList
from .settings import GameSettingsForm, ModuleForm, ModulesForm, SourceForm, SourcesForm
from .sources import LoginFlow, SourcesBrowser


class Screens(QObject):
    def __init__(
        self,
        client,
        memory,
        screen_mode: Callable[[], dict],
        games,
        power,
        themes: Callable[[], list] = list,
        network=None,
        busy: Callable[[], bool] = lambda: False,
        parent=None,
    ):
        super().__init__(parent)
        self._network = WifiScreen(client, network if network is not None else Network(parent=self), self)
        self._bluetooth = BluetoothScreen(client, busy, self)
        self._gameSettings = GameSettingsForm(client, screen_mode, self)
        self._addons = AddonsForm(client, self)
        self._modules = ModulesForm(client, self, self._addons)
        self._module = ModuleForm(client, self)
        self._sourceList = SourcesForm(client, self, self._addons)
        self._source = SourceForm(client, self)
        self._launch = LaunchForm(client, screen_mode, self)
        self._sources = SourcesBrowser(client, games, self)
        self._login = LoginFlow(client, self)
        self._login.finished.connect(lambda ok, text: (self._sourceList.load(), self._source.reload()))
        self._thumbs = Thumbs(self)
        self._recordings = RecordingsList(client, self)
        self._journal = JournalList(client, self)
        self._shots = ScreenshotsList(client, self._thumbs, self)
        self._media = MediaTimeline(client, self._recordings, self._thumbs, self)
        self._pendingJournals = PendingJournals(client, self)
        self._sessions = SessionsList(client, self)
        self._achievements = AchievementsList(client, self)
        # The dock's own: the launcher's page may still hold the other, open behind the game.
        self._dockAchievements = AchievementsList(client, self)
        self._paths = PathBrowser(client, self)
        self._controller = ControllerScreen(client, memory, power, self)
        self._components = ComponentsForm(client, self)
        self._runners = RunnersForm(client, self._components, self)
        self._runner = RunnerForm(client, self._components, screen_mode, self)
        self._artwork = ArtworkForm(client, self)
        self._artworkOverview = ArtworkOverview(client, self)
        self._add = AddGameForm(client, self)
        self._search = SettingsSearch(client, screen_mode, themes, self._controller, self)
        self._gameData = GameData(client, self)
        self._storage = Storage(client, self)
        self._onboarding = Onboarding(client, memory, games, self._login, self._controller, self._components, self)
        self._changelog = Changelog(client, self)

    def shutdown(self):
        self._thumbs.shutdown()
        self._recordings.shutdown()
        self._pendingJournals.shutdown()
        self._controller.shutdown()
        self._components.shutdown()
        self._network.shutdown()
        self._bluetooth.shutdown()

    gameSettings = Property(QObject, lambda self: self._gameSettings, constant=True)
    addons = Property(QObject, lambda self: self._addons, constant=True)
    modules = Property(QObject, lambda self: self._modules, constant=True)
    module = Property(QObject, lambda self: self._module, constant=True)
    sourceList = Property(QObject, lambda self: self._sourceList, constant=True)
    source = Property(QObject, lambda self: self._source, constant=True)
    launch = Property(QObject, lambda self: self._launch, constant=True)
    sources = Property(QObject, lambda self: self._sources, constant=True)
    login = Property(QObject, lambda self: self._login, constant=True)
    recordings = Property(QObject, lambda self: self._recordings, constant=True)
    journal = Property(QObject, lambda self: self._journal, constant=True)
    shots = Property(QObject, lambda self: self._shots, constant=True)
    media = Property(QObject, lambda self: self._media, constant=True)
    thumbs = Property(QObject, lambda self: self._thumbs, constant=True)
    pendingJournals = Property(QObject, lambda self: self._pendingJournals, constant=True)
    sessions = Property(QObject, lambda self: self._sessions, constant=True)
    achievements = Property(QObject, lambda self: self._achievements, constant=True)
    dockAchievements = Property(QObject, lambda self: self._dockAchievements, constant=True)
    album = Property(QObject, lambda self: self._recordings, constant=True)
    news = Property(QObject, lambda self: self._journal, constant=True)
    paths = Property(QObject, lambda self: self._paths, constant=True)
    controller = Property(QObject, lambda self: self._controller, constant=True)
    runners = Property(QObject, lambda self: self._runners, constant=True)
    runner = Property(QObject, lambda self: self._runner, constant=True)
    components = Property(QObject, lambda self: self._components, constant=True)
    artwork = Property(QObject, lambda self: self._artwork, constant=True)
    artworkOverview = Property(QObject, lambda self: self._artworkOverview, constant=True)
    add = Property(QObject, lambda self: self._add, constant=True)
    search = Property(QObject, lambda self: self._search, constant=True)
    onboarding = Property(QObject, lambda self: self._onboarding, constant=True)
    gameData = Property(QObject, lambda self: self._gameData, constant=True)
    storage = Property(QObject, lambda self: self._storage, constant=True)
    changelog = Property(QObject, lambda self: self._changelog, constant=True)
    network = Property(QObject, lambda self: self._network, constant=True)
    bluetooth = Property(QObject, lambda self: self._bluetooth, constant=True)
