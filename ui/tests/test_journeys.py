"""What a user does, end to end: the UI driven by keys on the real in-process core, over a sealed profile. The stores are their
shims, the user's systemd a private bus's, the game tools/sample's."""

import contextlib
import json
import os
import signal
import socket
import subprocess
import tomllib
from pathlib import Path

import pytest
from dbusmock import BusType, PrivateDBus, SpawnedMock
from looks import Look, current_row, read, settle
from PySide6.QtCore import QObject, Qt
from PySide6.QtTest import QTest
from uitest import until

universe_core = pytest.importorskip("universe_core")

ROOT = Path(__file__).resolve().parents[2]
TESTS = Path(__file__).parent
needs_tree = pytest.mark.skipif(not (ROOT / "tools" / "sample").is_file(), reason="the flake's check copies no tools/: no sample game")

CONFIG = """schema = 1
[desktop]
profile = "none"
keep_awake = false
[saves]
auto_backup = false
[modules]
enabled = []
[launch]
gamescope = false
hide_cursor = false
mangohud = false
[sources]
enabled = ["gog"]
[sources.gog]
games_dir = "{root}/games"
scan_dirs = "{root}/GOG Games"
"""
# What the launcher's own environment would hand a game it starts outside the profile's control.
HOST_VARS = ("DISPLAY", "WAYLAND_DISPLAY", "SWAYSOCK", "HYPRLAND_INSTANCE_SIGNATURE", "NIRI_SOCKET", "GAMESCOPE_WAYLAND_DISPLAY", "SteamGameId")
HOST_VARS += ("UNIVERSE_SESSION", "UNIVERSE_OWN_GAMESCOPE", "XDG_CURRENT_DESKTOP", "NO_PROXY", "no_proxy")


def closed_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


class Journey:
    """A sealed profile under `root` and, once `start`ed, the real core with the UI on it."""

    def __init__(self, root):
        self.root = root
        self.data, self.config, self.state = root / "data", root / "config", root / "state"
        self.home = root / "home"
        self.units_log = root / "units.jsonl"
        self.look = self.api = self.client = None

    def write(self, rel, content):
        path = self.root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        (path.write_bytes if isinstance(content, bytes) else path.write_text)(content)
        return path

    def game(self, ident):
        return tomllib.loads((self.data / "games" / ident / "game.toml").read_text())

    def sessions(self, ident):
        path = self.data / "games" / ident / "sessions.jsonl"
        return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []

    def units(self):
        """What the user's systemd was asked to start, and how each game unit's ExecStopPost ended."""
        return [json.loads(line) for line in self.units_log.read_text().splitlines()] if self.units_log.exists() else []

    def games_started(self):
        return [u for u in self.units() if "argv" in u and u["unit"].startswith("universe-game-")]

    def sample(self):
        """tools/sample's game in the library, as `just sample` puts it there."""
        subprocess.run([ROOT / "tools" / "sample", "install"], check=True, capture_output=True)
        return "sample-game"

    def start(self, onboarded=True, look="reprise"):
        from universe_ui.api import Api
        from universe_ui.screens.network import FAKE as FAKE_NET
        from universe_ui.screens.power import FAKE
        from universe_ui.universe_client import CoreClient

        core = universe_core.Core()
        if onboarded:
            core.mark_onboarded()
        self.client = CoreClient(core)
        self.api = Api(self.client, memory_path=str(self.root / "memory.json"), power_root=FAKE, net_root=FAKE_NET)
        self.look = Look(self.api, look)
        return self.look

    def quit_ui(self):
        """The launcher closed, the profile and the user's systemd left as they are."""
        if self.look is not None:
            self.look.close()
        if self.api is not None:
            self.api.shutdown()
        if self.client is not None:
            self.client.shutdown()
        self.look = self.api = self.client = None

    def close(self):
        self.quit_ui()
        for unit in self.games_started():
            with contextlib.suppress(ProcessLookupError, PermissionError):
                os.killpg(unit["pid"], signal.SIGKILL)


@pytest.fixture
def journey(app, xdg, rendered_looks, tmp_path, monkeypatch):
    from universe_ui import universe_client

    root = tmp_path
    if not os.environ.get("UNIVERSE_BIN"):
        pytest.fail("UNIVERSE_BIN names the universe build a game's session-end runs (`just test` sets it)")
    bin_dir = root / "bin"
    bin_dir.mkdir()
    (bin_dir / "gogdl").symlink_to(ROOT / "sources" / "gog" / "tests" / "shims" / "gogdl")
    for name in HOST_VARS:
        monkeypatch.delenv(name, raising=False)
    for kind in ("data", "config", "state", "cache"):
        monkeypatch.setenv(f"UNIVERSE_{kind.upper()}_HOME", str(root / kind))
    monkeypatch.setenv("UNIVERSE_MODULES_PATH", str(ROOT / "modules"))
    monkeypatch.setenv("UNIVERSE_SOURCES_PATH", str(ROOT / "sources"))
    catalogue = root / "catalogue.json"
    catalogue.write_text('{"schema": 1, "components": {}}')
    monkeypatch.setenv("UNIVERSE_CATALOGUE", catalogue.as_uri())
    index = root / "extensions.json"
    index.write_text('{"schema": 1, "extensions": []}')
    monkeypatch.setenv("UNIVERSE_EXTENSIONS_INDEX", str(index))
    home = root / "home"
    monkeypatch.setenv("HOME", str(home))
    monkeypatch.setenv("XDG_CONFIG_HOME", str(home / ".config"))
    monkeypatch.setenv("XDG_DATA_HOME", str(home / ".local" / "share"))
    (root / "run").mkdir(mode=0o700)
    monkeypatch.setenv("XDG_RUNTIME_DIR", str(root / "run"))
    monkeypatch.setenv("PATH", f"{TESTS / 'shims'}{os.pathsep}{bin_dir}{os.pathsep}{os.environ['PATH']}")
    monkeypatch.setenv("SHIM_LOG", str(root / "shim.log"))
    # The stores, the art providers and the catalogue all fail at once instead of reaching the internet.
    proxy = f"http://127.0.0.1:{closed_port()}"
    for name in ("http_proxy", "https_proxy", "HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "all_proxy"):
        monkeypatch.setenv(name, proxy)
    monkeypatch.setenv("DBUS_SYSTEM_BUS_ADDRESS", "unix:path=/dev/null/bus")
    monkeypatch.setattr(universe_client, "SESSION_POLL_MS", 50)
    shown = Journey(root)
    shown.write("config/config.toml", CONFIG.format(root=root))
    monkeypatch.delenv("DBUS_SESSION_BUS_ADDRESS", raising=False)
    with PrivateDBus(BusType.SESSION) as bus:
        monkeypatch.setenv("DBUS_SESSION_BUS_ADDRESS", bus.address)
        params = {"log": str(shown.units_log)}
        with open(root / "systemd.log", "w") as out, SpawnedMock.spawn_with_template(str(TESTS / "user_systemd.py"), params, BusType.SESSION, out, out):
            try:
                yield shown
            finally:
                shown.close()


def menu_ids(menu):
    return [i.get("action") for i in read(menu, "items") or []]


def pick(look, menu, *actions):
    """Down the open menu to each of `actions` in turn, then A: a submenu's actions follow its parent's."""
    for action in actions:
        until(lambda action=action: menu.property("open") is True and action in menu_ids(menu), f"no {action} in {menu.objectName()}")
        target = menu_ids(menu).index(action)
        look.press(Qt.Key.Key_Down, target - menu.property("index"))
        until(lambda target=target: menu.property("index") == target)
        look.press(Qt.Key.Key_Return)


def down_to(look, row, **want):
    """Down the rows until `row()` holds `want`, whatever their order."""

    def mark(r):
        return tuple(r.get(k) for k in ("key", "field", "module", "label", "storeId"))

    seen = None
    while (current := row() or {}) and mark(current) != seen:
        if all(current.get(k) == v for k, v in want.items()):
            return current
        seen = mark(current)
        look.press(Qt.Key.Key_Down)
    raise AssertionError(f"no row holds {want}")


def walk(look, page, key):
    """The cursor onto `key` on a page of cards, whatever their order: each card of the sidebar in turn, down its rows."""
    for _ in read(page, "sections"):
        if not page.property("inRows"):
            look.press(Qt.Key.Key_Right)
        with contextlib.suppress(AssertionError):
            return down_to(look, lambda: current_row(page), key=key)
        look.press(Qt.Key.Key_Left)
        look.press(Qt.Key.Key_Down)
    raise AssertionError(f"no {key} row on any card")


def settings(look, section):
    """Settings by keys: across the tabs, down the sidebar to `section`, into its rows; the page and its rows' cursor."""
    root = look.root
    for _ in range(4):
        if root.property("tabIndex") == root.property("settingsTab"):
            break
        look.press(Qt.Key.Key_E)
    page = until(lambda: (p := root.property("activePage")) is not None and p.objectName() == "settingsPage" and p, "Settings is shown")
    ids = [s["id"] for s in read(page, "sections")]
    step = ids.index(section) - page.property("section")
    look.press(Qt.Key.Key_Down if step > 0 else Qt.Key.Key_Up, abs(step))
    until(lambda: page.property("sectionId") == section)
    look.press(Qt.Key.Key_Right)
    cards = next(c for c in page.findChildren(QObject) if c.metaObject().className().startswith("SettingsCards"))
    until(lambda: cards.property("activeFocus"), f"the cursor is on {section}'s rows")
    return page, lambda: read(cards, "currentRow") or {}


def type_text(look, text):
    for ch in text:
        QTest.keyClick(look.window, ch)


def back_home(look):
    """B until Home has the focus again: a card's rows, then its page, give way one press each."""
    for _ in range(4):
        if look.root.property("returnLaunches") not in (None, ""):
            return
        look.press(Qt.Key.Key_Escape)
        settle(look.window)
    until(lambda: look.root.property("returnLaunches") not in (None, ""), "B is back on Home")


def quit_game(look, client):
    """Quit from the playing game's menu on Home, until its session is over."""
    until(lambda: not look.root.property("launching") and look.root.property("playingId"))
    look.press(Qt.Key.Key_F1)
    pick(look, look.menu(), "stop")
    until(lambda: client.currentSession is None, "Quit ends the session")


@needs_tree
@pytest.mark.slow
def test_a_game_launched_from_home_and_quit_from_its_menu_leaves_a_session_row(journey):
    game = journey.sample()
    look = journey.start()
    client = journey.client
    until(lambda: look.root.property("returnLaunches") == game, "Home lands on the one game")
    look.press(Qt.Key.Key_Return)
    started = until(journey.games_started, "A launches the game in a unit of its own")
    assert started[0]["argv"][-1].endswith("/sample/sample-game"), started[0]
    until(lambda: (client.currentSession or {}).get("id") == game)
    session = client.currentSession["session_id"]
    until(lambda: look.root.property("playingId") == game and not look.root.property("launching"))
    look.press(Qt.Key.Key_F1)
    assert {"resume", "stop"} <= set(menu_ids(look.menu())), "the playing game's menu resumes or quits it"
    look.press(Qt.Key.Key_Escape)
    quit_game(look, client)
    rows = until(lambda: journey.sessions(game))
    assert [(r["session"], r["unit"], r["stopped"], r["crashed"]) for r in rows] == [(session, started[0]["unit"], True, False)], rows
    ended = until(lambda: [u for u in journey.units() if "stop_post" in u])
    assert [u["stop_post"] for u in ended] == [0], ended
    sessions = journey.api.screens.sessions
    sessions.load(game)
    assert [(r["session"], r["end"], r["live"]) for r in sessions.rows] == [(session, "stopped", False)]


@needs_tree
@pytest.mark.slow
def test_arguments_set_on_the_games_page_reach_it_on_every_launch_after_a_restart(journey):
    game = journey.sample()
    look = journey.start()
    until(lambda: look.root.property("returnLaunches") == game)
    look.press(Qt.Key.Key_F1)
    pick(look, look.menu(), "manage", "settings")
    page = look.page("gameSettingsPage")
    until(lambda: page.property("activeFocus") and read(page, "sections"))
    look.press(Qt.Key.Key_F)
    until(lambda: journey.api.screens.gameSettings.showAdvanced, "Y shows the advanced rows")
    walk(look, page, "launch.args")
    look.press(Qt.Key.Key_Return)
    sheet = until(lambda: (s := page.findChild(QObject, "textSheet")) is not None and s.property("open") is True and s)
    type_text(look, "--delay,0")
    look.press(Qt.Key.Key_Return)
    until(lambda: journey.game(game)["launch"].get("args") == ["--delay", "0"], "game.toml holds the game's own arguments")
    until(lambda: sheet.property("open") is False)
    back_home(look)
    look.press(Qt.Key.Key_Return)
    quit_game(look, journey.client)
    journey.quit_ui()
    look = journey.start()
    form = journey.api.screens.gameSettings
    form.load(game)
    assert next(r for r in form.rows if r["key"] == "launch.args")["value"] == "--delay,0", "the page shows them after a restart"
    until(lambda: look.root.property("returnLaunches") == game)
    look.press(Qt.Key.Key_Return)
    quit_game(look, journey.client)
    assert [u["argv"][-3:] for u in journey.games_started()] == [[str(journey.root / "sample" / "sample-game"), "--delay", "0"]] * 2


MINI_METRO = "1434554947"
LIBRARY = [{"id": MINI_METRO, "title": "Mini Metro", "owned": True, "installed": False, "release_year": 2015}]


def gog_info(folder, game_id, name, exe):
    folder.mkdir(parents=True, exist_ok=True)
    (folder / exe).write_bytes(b"MZ")
    info = {"gameId": game_id, "rootGameId": game_id, "name": name, "buildId": "B1", "playTasks": [{"isPrimary": True, "type": "FileTask", "path": exe}]}
    (folder / f"goggame-{game_id}.info").write_text(json.dumps(info))


def store(journey):
    """The GOG account's listing as the core cached it, and umu's id for its game: nothing a store verb needs is fetched."""
    journey.write("data/sources/gog/library.json", json.dumps(LIBRARY))
    journey.write("data/sources/gog/umu.json", json.dumps({MINI_METRO: "umu-287980"}))


def store_row(journey, store_id):
    return next((r for r in journey.api.screens.sources.rows if r["id"] == store_id), {})


@pytest.mark.slow
def test_a_store_game_installed_from_settings_joins_the_library_installed(journey):
    store(journey)
    look = journey.start()
    page, row = settings(look, "install")
    until(lambda: store_row(journey, MINI_METRO), "the cached listing, no store reached")
    assert store_row(journey, MINI_METRO)["installed"] is False
    down_to(look, row, storeId=MINI_METRO)
    look.press(Qt.Key.Key_Return)
    pick(look, page.findChild(QObject, "settingsMenu"), "install")
    pick(look, page.findChild(QObject, "dialog"), "yes")
    until(lambda: store_row(journey, MINI_METRO).get("game_id") == "mini-metro", "the row is the library game's once installed", 10000)
    assert store_row(journey, MINI_METRO)["installed"] is True
    folder = journey.root / "games" / "Mini Metro"
    assert (folder / f"goggame-{MINI_METRO}.info").is_file(), "gogdl downloaded into the source's games folder"
    game = journey.game("mini-metro")
    assert (game["source"]["kind"], game["source"]["id"], game["source"]["dir"]) == ("gog", MINI_METRO, str(folder))
    until(lambda: journey.api.allGames.byId("mini-metro") is not None, "Home lists it")


@pytest.mark.slow
def test_uninstall_from_settings_trashes_the_folder_and_keeps_the_game(journey):
    store(journey)
    folder = journey.root / "games" / "Mini Metro"
    gog_info(folder, MINI_METRO, "Mini Metro", "Mini Metro.exe")
    journey.write(
        "data/games/mini-metro/game.toml",
        f'schema = 1\nid = "mini-metro"\ntitle = "Mini Metro"\n[source]\nkind = "gog"\nid = "{MINI_METRO}"\ndir = "{folder}"\nbuild_id = "B1"\n'
        f'[launch]\nrunner = "proton"\nexe = "{folder}/Mini Metro.exe"\n',
    )
    look = journey.start()
    page, row = settings(look, "install")
    until(lambda: store_row(journey, MINI_METRO).get("game_id") == "mini-metro")
    down_to(look, row, storeId=MINI_METRO)
    look.press(Qt.Key.Key_Return)
    pick(look, page.findChild(QObject, "settingsMenu"), "uninstall")
    pick(look, page.findChild(QObject, "settingsMenu"), "yes")
    until(lambda: not folder.exists(), "the install folder goes")
    assert (journey.home / ".local" / "share" / "Trash" / "files" / "Mini Metro").is_dir(), "to the trash, not deleted"
    until(lambda: store_row(journey, MINI_METRO).get("installed") is False)
    game = journey.game("mini-metro")
    assert "dir" not in game["source"] and "exe" not in game["launch"] and not game.get("removed_at"), "still in the library, not installed"


@needs_tree
@pytest.mark.slow
def test_remove_from_the_games_menu_takes_it_off_home_and_keeps_its_files(journey):
    game = journey.sample()
    look = journey.start()
    until(lambda: look.root.property("returnLaunches") == game)
    look.press(Qt.Key.Key_F1)
    pick(look, look.menu(), "manage", "remove", "yes")
    until(lambda: journey.api.allGames.byId(game) is None, "Home drops it")
    toml = journey.game(game)
    assert toml["removed_at"] and toml["hidden"] is True, "archived in game.toml"
    assert (journey.root / "sample" / "sample-game").is_file(), "its files stay"
