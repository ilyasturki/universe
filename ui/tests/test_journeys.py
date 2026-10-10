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
from looks import Look, read
from PySide6.QtCore import Qt
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

    def close(self):
        if self.look is not None:
            self.look.close()
        if self.api is not None:
            self.api.shutdown()
        if self.client is not None:
            self.client.shutdown()
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


def menu_ids(look):
    return [i.get("action") for i in read(look.menu(), "items") or []]


def pick(look, action):
    """Down the open menu to `action`, then A."""
    menu = look.menu()
    until(lambda: menu.property("open") is True and action in menu_ids(look), f"no {action} in the menu")
    target = menu_ids(look).index(action)
    look.press(Qt.Key.Key_Down, target - menu.property("index"))
    until(lambda: menu.property("index") == target)
    look.press(Qt.Key.Key_Return)


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
    assert {"resume", "stop"} <= set(menu_ids(look)), "the playing game's menu resumes or quits it"
    pick(look, "stop")
    until(lambda: client.currentSession is None, "Quit ends the session")
    rows = until(lambda: journey.sessions(game))
    assert [(r["session"], r["unit"], r["stopped"], r["crashed"]) for r in rows] == [(session, started[0]["unit"], True, False)], rows
    ended = [u for u in journey.units() if "stop_post" in u]
    assert [u["stop_post"] for u in ended] == [0], ended
    sessions = journey.api.screens.sessions
    sessions.load(game)
    assert [(r["session"], r["end"], r["live"]) for r in sessions.rows] == [(session, "stopped", False)]
