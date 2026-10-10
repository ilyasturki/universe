import base64
import contextlib
import json
import os
import re
import shutil
import tomllib
from fnmatch import fnmatch
from pathlib import Path

import pytest

from universe_ui.errors import UniverseError
from universe_ui.fake_core import EXTENSIONS, FIXTURE, LAUNCH_KEYS, FakeCore
from universe_ui.models import Game
from universe_ui.themes import THEMES

universe_core = pytest.importorskip("universe_core")

ROOT = Path(__file__).resolve().parents[2]
needs_manifests = pytest.mark.skipif(not (ROOT / "modules").is_dir(), reason="the flake's check copies ui/ alone: the core would list no module or source")

PNG = base64.b64decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==")
SESSION = "20260913-120000"
# The real profile: games the fixture has too, so one call reads both cores.
SEED = {
    "config/config.toml": 'schema = 1\n[modules]\nenabled = ["capture", "journal", "screenshot", "controls"]\n[sources]\nenabled = ["gog"]\n',
    "data/games/the-technomancer/game.toml": 'schema = 1\nid = "the-technomancer"\ntitle = "The Technomancer"\nsort_title = "Technomancer"\nfavorite = true\n'
    'tags = ["rpg"]\nrelease_year = 2016\n[metadata]\ndevelopers = ["Spiders"]\npublishers = ["Focus"]\ngenres = ["RPG"]\nsummary = "Mars."\n'
    'description = "On Mars."\nmetacritic = 61\nplayers = 2\n'
    '[source]\nkind = "gog"\n[launch]\nexe = "/mnt/games/PC/The Technomancer/TheTechnomancer.exe"\n[modules.capture]\ncursor = false\n',
    "data/games/the-technomancer/sessions.jsonl": f'{{"session": "{SESSION}", "game": "the-technomancer", "started_at": "2026-09-13T12:00:00+02:00",'
    ' "ended_at": "2026-09-13T13:00:00+02:00", "duration_s": 3600, "exit": 0, "stopped": false, "recording": "/nowhere/20260913-120000.mkv"}\n',
    f"data/games/the-technomancer/journal/{SESSION}.json": json.dumps(
        {"session": SESSION, "game": "the-technomancer", "written_at": "2026-09-13T13:10:00+02:00", "title": "First", "paragraphs": ["p"]}
    ),
    "data/games/the-technomancer/journal/20260912-120000.deferred.json": json.dumps({"game": "the-technomancer", "until": "2099-01-01T00:00:00+00:00"}),
    "data/games/the-technomancer/screenshots/20260913-121000.png": PNG,
    "data/games/the-technomancer/media/box_front.png": PNG,
    **{
        f"{home}/extensions/theme/dusk/{name}": content
        for home in ("data", "fake/data")
        for name, content in {
            "theme.toml": 'api = 2\nid = "dusk"\nname = "Dusk"\nversion = "1.0.0"\nauthor = "Someone"\nlicense = "MIT"\nscreenshot = "shot.png"\n',
            "theme.qml": "import QtQuick\nItem {}\n",
            "shot.png": PNG,
        }.items()
    },
    "data/games/dead-cells/game.toml": 'schema = 1\nid = "dead-cells"\ntitle = "Dead Cells"\n[source]\nkind = "gog"\n',
    # An emulator game: the emulator-only modules' settings reach its effective.modules alone.
    "data/games/super-mario-strikers/game.toml": 'schema = 1\nid = "super-mario-strikers"\ntitle = "Super Mario Strikers"\nplatform = "Nintendo GameCube"\n'
    '[launch]\nrunner = "dolphin"\nexe = "/roms/strikers.rvz"\n',
    "data/games/dead-cells/achievements.json": json.dumps(
        {"source": "gog", "fetched_at": "2026-09-18T19:02:00+02:00", "items": [{"key": "prisoner", "name": "Prisoner", "unlocked_at": "", "rarity": 91.0}]}
    ),
}
SCREEN = {"screen": "DP-1", "width": 2560, "height": 1440, "refresh": 144}

READS = {
    "list": ("list",),
    "get": ("get", "the-technomancer"),
    "settings": ("settings",),
    "modules": ("modules",),
    "module_settings": ("module_settings", "capture", "the-technomancer"),
    "sources": ("sources",),
    "source_settings": ("source_settings", "gog"),
    "runners": ("runners",),
    "components": ("components",),
    "extensions": ("extensions",),
    "themes": ("themes",),
    "sessions": ("sessions", ""),
    "journal": ("journal", "the-technomancer"),
    "media": ("media", "the-technomancer"),
    "screenshots": ("screenshots", "the-technomancer"),
    "media_status": ("media_status", "the-technomancer"),
    "achievements": ("achievements", "dead-cells"),
    "controller_state": ("controller_state",),
    "discover": ("discover",),
    "import_roms": ("import_roms", False),
    "doctor": ("doctor",),
    "disk_free": ("disk_free",),
    "gpu": ("gpu",),
    "screen_mode": ("screen_mode", "DP-1"),
    "changelog": ("changelog",),
    "network": ("network",),
    "bluetooth": ("bluetooth",),
    "form-launch": ("form", "launch", "", SCREEN),
    "form-game": ("form", "game", "the-technomancer", SCREEN),
    "form-runner": ("form", "runner", "dolphin"),
    "form-module": ("form", "module", "capture"),
    "form-source": ("form", "source", "gog"),
}

# Dicts keyed by ids or option names, not by the schema: compared by their values' shape.
MAPS = {"effective.modules", "effective.options", "effective.env", "launch.options", "launch.env", "launch.dll_overrides", "system", "hooks"}
MAPS |= {"controller.axes", "controller.buttons", "settings.choice_labels"}
# What a file sets as written, the config's or a game's own tables: no schema to hold either side to.
OPAQUE = {"set", "modules", "sources", "runners", "proton"}

# What the fake knowingly answers otherwise, per read, as globs of `<path> only in the fake|core` or `<path>: <core types> | <fake types>`.
KNOWN = {}
# Reads of the machine: without a GPU or the default roots, the core answers nothing to compare.
HOST = {"gpu", "disk_free"}


def shape(value, path=""):
    if path in OPAQUE:
        return None
    if isinstance(value, dict):
        if path in MAPS:
            return {"*": _merged(shape(v, f"{path}.*") for v in value.values())} if value else None
        return {k: shape(v, f"{path}.{k}".lstrip(".")) for k, v in value.items()}
    if isinstance(value, (list, tuple)):
        return [_merged(shape(v, path) for v in value)]
    if value is None:
        return None
    return frozenset({"number" if type(value) in (int, float) else type(value).__name__})


def _merged(shapes):
    out = None
    for s in shapes:
        out = _merge(out, s)
    return out


def _merge(a, b):
    if a is None or b is None:
        return b if a is None else a
    if isinstance(a, dict) and isinstance(b, dict):
        return {k: _merge(a.get(k), b.get(k)) for k in a | b}
    if isinstance(a, list) and isinstance(b, list):
        return [_merge(a[0], b[0])]
    return _types(a) | _types(b)


def _types(s):
    return s if isinstance(s, frozenset) else frozenset({type(s).__name__})


def drift(core, fake, path=""):
    if core is None or fake is None:
        return
    if isinstance(core, dict) and isinstance(fake, dict):
        for key in sorted(core.keys() - fake.keys()):
            yield f"{path}{key} only in the core"
        for key in sorted(fake.keys() - core.keys()):
            yield f"{path}{key} only in the fake"
        for key in sorted(core.keys() & fake.keys()):
            yield from drift(core[key], fake[key], f"{path}{key}.")
    elif isinstance(core, list) and isinstance(fake, list):
        yield from drift(core[0], fake[0], path)
    elif _types(core) != _types(fake):
        yield f"{path.rstrip('.')}: {'|'.join(sorted(_types(core)))} | {'|'.join(sorted(_types(fake)))}"


# The machine's own desktop and session, which a core would read instead of the profile's.
HOST_VARS = ("DISPLAY", "WAYLAND_DISPLAY", "GAMESCOPE_WAYLAND_DISPLAY", "SWAYSOCK", "HYPRLAND_INSTANCE_SIGNATURE", "NIRI_SOCKET", "XDG_CURRENT_DESKTOP")
HOST_VARS += ("SteamGameId", "UNIVERSE_SESSION", "UNIVERSE_OWN_GAMESCOPE")


@pytest.fixture(scope="module")
def cores(app, tmp_path_factory):
    with pytest.MonkeyPatch.context() as env, seeded(tmp_path_factory.mktemp("contract"), env) as both:
        yield both


@pytest.fixture
def profile(app, xdg, fixture_art, tmp_path, monkeypatch):
    """Both cores on a seed of the test's own, for writes; `profile(files)` adds files to the seed."""
    with contextlib.ExitStack() as stack:
        yield lambda files=None: stack.enter_context(seeded(tmp_path, monkeypatch, files))


@contextlib.contextmanager
def seeded(root, env, files=None):
    for name, content in {**SEED, **(files or {})}.items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        (path.write_bytes if isinstance(content, bytes) else path.write_text)(content)
    # The core leaves out a module whose binary is missing: a stub stands in for each one this machine lacks.
    stubs = root / "bin"
    stubs.mkdir()

    def stub(name):
        if shutil.which(name) is None:
            (stubs / name).write_text("#!/bin/sh\nexit 1\n")
            (stubs / name).chmod(0o755)

    for manifest in [*ROOT.glob("modules/*/module.toml"), *ROOT.glob("sources/*/source.toml")]:
        for name in tomllib.loads(manifest.read_text()).get("requires", {}).get("bins", []):
            stub(name)
    index = root / "index.json"
    index.write_text(json.dumps({"schema": 1, "extensions": json.loads(EXTENSIONS.read_text())["extensions"][:1]}))
    env.setenv("PATH", f"{os.environ['PATH']}{os.pathsep}{stubs}")
    for name in HOST_VARS:
        env.delenv(name, raising=False)
    env.setenv("HOME", str(root / "home"))
    env.setenv("DBUS_SYSTEM_BUS_ADDRESS", "unix:path=/dev/null/bus")
    env.setenv("UNIVERSE_EXTENSIONS_INDEX", str(index))
    for kind in ("data", "config", "state", "cache"):
        env.setenv(f"UNIVERSE_{kind.upper()}_HOME", str(root / kind))
    for kind in ("modules", "sources"):
        env.setenv(f"UNIVERSE_{kind.upper()}_PATH", str(ROOT / kind))
    core, fake = universe_core.Core(), FakeCore(FIXTURE, root / "fake")
    # A runner the fixture finds on PATH is on this machine's too, whatever it has installed.
    found = {r["id"] for r in json.loads(FIXTURE.read_text())["runners"] if r.get("source") == "path"}
    for runner in core.runners():
        if runner["id"] in found and runner["binaries"]:
            stub(runner["binaries"][0])
    try:
        yield core, fake
    finally:
        fake.shutdown()


@needs_manifests
@pytest.mark.parametrize("name", READS)
def test_the_fake_answers_in_the_cores_shape(cores, name):
    method, *args = READS[name]
    core, fake = (getattr(c, method)(*args) for c in cores)
    assert fake and (core or name in HOST), "the seed gives both cores something to show"
    found, known = list(drift(shape(core), shape(fake))), KNOWN.get(name, {})
    assert [d for d in found if not any(fnmatch(d, k) for k in known)] == []
    assert [k for k in known if not any(fnmatch(d, k) for d in found)] == [], "the fake answers as the core does now: drop it from KNOWN"


@needs_manifests
def test_a_gamecube_game_s_page_offers_the_layout_and_the_trigger_swap(cores):
    core, _ = cores
    keys = {f["key"] for f in core.form("game", "super-mario-strikers", SCREEN)}
    assert {"modules.controls.layout", "modules.controls.shoulders"} <= keys


ERRORS = [
    ("NotFound", "get", "nope"),
    ("NotFound", "module_settings", "nope", ""),
    ("Invalid", "source_settings", "capture"),
    ("Invalid", "set_module_setting", "capture", "", "codec", "mpeg2"),
    ("NotFound", "set_runner_setting", "nope", "exe", "/x"),
    ("Invalid", "set_runner_setting", "dolphin", "nope", "1"),
    ("Invalid", "launch_keys", "nowhere", None),
    ("NotFound", "remove_journal_entry", "the-technomancer", "20990101-000000"),
    ("Invalid", "set_controller_macro", {"family": "*", "button": "south", "trigger": "press", "action": "nope"}),
    ("Invalid", "add_game", {"runner": "nope", "exe": "/x"}),
    ("NotFound", "session_log", "the-technomancer", "20990101-000000"),
    ("NotFound", "media_set_slot", "the-technomancer", "box_front", "/nowhere/cover.png"),
    ("Unavailable", "achievements", "the-technomancer"),
    ("Invalid", "extension_install", "now-playing"),
    ("NotFound", "extension_remove", "nope"),
    ("Invalid", "extension_remove", "capture"),
    *[("Invalid", "extension_remove", t["id"]) for t in THEMES],
]


@needs_manifests
@pytest.mark.parametrize(("kind", "method", "args"), [(k, m, a) for k, m, *a in ERRORS], ids=[e[1] for e in ERRORS])
def test_the_fake_fails_as_the_core_does(cores, kind, method, args):
    for core in cores:
        with pytest.raises((universe_core.UniverseError, UniverseError)) as raised:
            getattr(core, method)(*args)
        assert raised.value.args[0] == kind, type(core).__name__


def test_the_launch_keys_fixture_is_the_cores_table(cores):
    core, fake = cores
    assert json.loads(LAUNCH_KEYS.read_text()) == core.launch_keys("both", None), (
        "regenerate with `universe launch-keys --json > ui/universe_ui/fixtures/launch_keys.json`"
    )
    assert fake.launch_keys("game", SCREEN) == core.launch_keys("game", SCREEN), "the fake sizes the choices as the core does"
    assert fake.launch_keys("global", {}) == core.launch_keys("global", {})


def test_the_ui_reads_every_field_the_core_sends_of_a_game(cores):
    core, _ = cores
    games = [core.get(g["id"]) for g in core.list()]
    assert {g["id"] for g in games} == {"the-technomancer", "dead-cells", "super-mario-strikers"}
    for raw in games:
        game, meta, stats = Game(raw, None), raw["metadata"], raw["stats"]
        assert (game.id, game.title, game.favorite, game.hidden, game.tags) == (raw["id"], raw["title"], raw["favorite"], raw["hidden"], raw["tags"])
        assert game.sortTitle == (raw["sort_title"] or game.sortTitle)
        assert (game.playTime, game.playCount, game.lastPlayed is not None and game.lastPlayed.isValid()) == (
            round(stats["hours"] * 3600),
            stats["play_count"],
            bool(stats["last_played"]),
        )
        assert (game.releaseYear, game.developerList, game.publisherList, game.genreList) == (
            raw["release_year"],
            meta["developers"],
            meta["publishers"],
            meta["genres"],
        )
        assert (game.summary, game.description, game.players) == (meta["summary"], meta["description"], meta["players"] or 1)
        assert game.extra == ({"metacritic": [meta["metacritic"]]} if meta["metacritic"] else {})
        assert (game.source, game.platform, game.runner) == (raw["source"]["kind"], raw["platform"], raw["effective"]["runner"])
        assert (game.achievementsTotal, game.achievementsUnlocked) == (raw["achievements"]["total"], raw["achievements"]["unlocked"])
        assert game.assets.boxFront.toLocalFile() == (raw["media"]["box_front"] or ""), raw["id"]
    played = next(g for g in games if g["id"] == "the-technomancer")
    assert played["metadata"]["developers"] and played["stats"]["play_count"] and played["release_year"], "the seed fills what the UI reads"


T, D = "the-technomancer", "dead-cells"
TIME = re.compile(r"^\d{4}-\d{2}-\d{2}T")


def plain(value, core):
    """`value` less what differs only by whose profile it is: the data home in paths, the clock's times."""
    if isinstance(value, dict):
        return {k: plain(v, core) for k, v in value.items()}
    if isinstance(value, list):
        return [plain(v, core) for v in value]
    if isinstance(value, str):
        return "<time>" if TIME.match(value) else value.replace(core.data_home(), "<data>")
    return value


def changes(before, after, path=""):
    """Each path the writes changed, with its new value; a key gone reads None."""
    if isinstance(before, dict) and isinstance(after, dict):
        out = {}
        for key in before.keys() | after.keys():
            out.update(changes(before.get(key), after.get(key), f"{path}.{key}".lstrip(".")))
        return out
    return {} if before == after else {path: after}


def by_id(rows, ident):
    return next(r for r in rows if r["id"] == ident)


def fields(rows):
    return {f["key"]: (f.get("value"), f.get("origin")) for f in rows}


def macros(state):
    return {f"{m['family']}/{m['button']}/{m['trigger']}": m["action"] for m in state["macros"]}


def png(core):
    path = Path(core.data_home()).parent / "banner.png"
    path.write_bytes(PNG)
    return str(path)


LAUNCH = [("set_field", "game", T, "launch.fps_limit", "60"), ("set_field", "game", T, "launch.env.DXVK_HUD", "fps")]
# Each: what to read, and the writes between two reads; a write is a call, or a function of the core for one that names its own data.
WRITES = {
    "game flags": (
        lambda c: c.get(T),
        [("set", T, "favorite", "false"), ("set", T, "hidden", "true"), ("set", T, "tags", "racing,mars"), ("set", T, "sort_title", "Techno")],
    ),
    "game launch": (lambda c: c.get(T), LAUNCH),
    "game form": (lambda c: fields(c.form("game", T, SCREEN)), LAUNCH),
    "global launch": (lambda c: {"set": c.settings()["set"], "game": c.get(D)["effective"]}, [("set_setting", "launch.fps_limit", "30")]),
    "promote": (
        lambda c: {"set": c.settings()["set"], "game": c.get(T)["launch"]},
        [("set_field", "game", T, "launch.mangohud", "true"), ("promote_field", "game", T, "launch.mangohud")],
    ),
    "every game": (lambda c: {g: c.get(g)["effective"]["mangohud"] for g in (T, D)}, [("set_field_all", "game", T, "launch.mangohud", "true")]),
    "module": (
        lambda c: {"listed": by_id(c.modules(), "capture"), "settings": c.module_settings("capture", T)},
        [("enable_module", "capture", False), ("set_module_setting", "capture", T, "cursor", "true")],
    ),
    "source": (lambda c: c.source_settings("gog"), [("set_source_setting", "gog", "games_dir", "/games")]),
    "macro": (
        lambda c: macros(c.controller_state()),
        [
            ("set_controller_macro", {"family": "*", "button": "south", "trigger": "hold", "action": "screenshot"}),
            ("remove_controller_macro", "dualsense-edge", "fn_left", "press"),
        ],
    ),
    "remove": (lambda c: {"listed": D in [g["id"] for g in c.list()], "game": c.get(D)}, [("remove", D, False)]),
    "artwork": (lambda c: c.get(T)["media"], [lambda c: c.media_set_slot(T, "banner", png(c))]),
    "runner": (lambda c: by_id(c.runners(), "dolphin"), [("set_runner_setting", "dolphin", "exe", "/opt/dolphin/dolphin-emu")]),
}
# What the fake knowingly writes otherwise, per case, as globs of `<path>: <core> | <fake>`.
KNOWN_WRITES = {}


def written(core, read, writes):
    before = plain(read(core), core)
    for write in writes:
        if callable(write):
            write(core)
        else:
            getattr(core, write[0])(*write[1:])
    return changes(before, plain(read(core), core))


def unlike(core, fake):
    for path in sorted(core.keys() | fake.keys()):
        if core.get(path, "unchanged") != fake.get(path, "unchanged"):
            yield f"{path}: {core.get(path, 'unchanged')!r} | {fake.get(path, 'unchanged')!r}"


@needs_manifests
@pytest.mark.parametrize("name", WRITES)
def test_the_fake_writes_as_the_core_does(profile, name):
    read, writes = WRITES[name]
    core, fake = (written(c, read, writes) for c in profile())
    assert core, "the writes change what the core reads back"
    found, known = list(unlike(core, fake)), KNOWN_WRITES.get(name, {})
    assert [d for d in found if not any(fnmatch(d, k) for k in known)] == []
    assert [k for k in known if not any(fnmatch(d, k) for d in found)] == [], "the fake writes as the core does now: drop it from KNOWN_WRITES"


# A launch that starts nothing but the game: no hooks, no gamescope or HUD of the machine's, no saves backup after.
QUIET = (
    'schema = 1\n[desktop]\nprofile = "none"\nkeep_awake = false\n[saves]\nauto_backup = false\n[modules]\nenabled = []\n'
    "[launch]\ngamescope = false\nmangohud = false\nhide_cursor = false\n[sources]\nenabled = []\n"
)
# The row's own values the two cores fill alike; the rest names the unit, the command and the clock.
SESSION_KEYS = ("game", "title", "end", "stopped", "crashed", "exit", "source", "recording", "journal", "debug_log")
KNOWN_SESSION = {}


@needs_manifests
def test_a_stopped_session_ends_alike_on_both_cores(profile, user_systemd, tmp_path, monkeypatch):
    from uitest import until

    monkeypatch.setenv("PATH", f"{Path(__file__).with_name('shims')}{os.pathsep}{os.environ['PATH']}")
    game = tmp_path / "sleeper"
    game.write_text("#!/bin/sh\nexec sleep 600\n")
    game.chmod(0o755)
    rows = []
    for core in profile({"config/config.toml": QUIET}):
        for module in core.modules():
            core.enable_module(module["id"], False)
        ident = core.add_game({"runner": "linux", "exe": str(game), "title": "Sleeper"})
        session = core.launch(ident, "")
        assert core.current()["session_id"] == session
        core.stop(session)
        until(lambda core=core: core.current() is None, "the stop ends the session")
        core.reload_game(ident)  # as the client does once the session is over
        rows.append({k: plain(v, core) for k, v in core.sessions(ident)[0].items() if k in SESSION_KEYS})
    found = list(unlike(*rows))
    assert [d for d in found if not any(fnmatch(d, k) for k in KNOWN_SESSION)] == []


STARDEW = "1207666883"


@needs_manifests
def test_an_install_lands_alike_on_both_cores(profile, tmp_path, monkeypatch):
    shims = tmp_path / "gogdl"
    shims.mkdir()
    (shims / "gogdl").symlink_to(ROOT / "sources" / "gog" / "tests" / "shims" / "gogdl")
    monkeypatch.setenv("PATH", f"{shims}{os.pathsep}{os.environ['PATH']}")
    monkeypatch.setenv("SHIM_LOG", str(tmp_path / "gogdl.log"))
    for name in ("http_proxy", "https_proxy", "HTTP_PROXY", "HTTPS_PROXY"):
        monkeypatch.setenv(name, "http://127.0.0.1:9")
    listing = [{"id": STARDEW, "title": "Stardew Valley", "owned": True, "installed": False}]
    core, fake = profile({"data/sources/gog/library.json": json.dumps(listing), "data/sources/gog/umu.json": json.dumps({STARDEW: "umu-413150"})})
    core.set_source_setting("gog", "games_dir", str(tmp_path / "games"))
    landed = []
    for c in (core, fake):
        before = {g["id"] for g in c.list()}
        returned = c.install("gog", STARDEW, None)
        new = {g["id"] for g in c.list()} - before
        row = next(r for r in c.library("gog", False) if r["id"] == STARDEW)
        game = c.get(next(iter(new))) if new else {}
        landed.append(
            {
                "returns the new game": [returned] == list(new),
                "listed installed, as it": (row["installed"], row.get("game_id") in new),
                "store": (game.get("source", {}).get("kind"), game.get("source", {}).get("id")),
                "runner": game.get("effective", {}).get("runner"),
            }
        )
    assert landed[0] == landed[1]
