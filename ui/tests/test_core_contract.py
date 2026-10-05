import base64
import json
import os
import shutil
import tomllib
from fnmatch import fnmatch
from pathlib import Path

import pytest

from universe_ui.errors import UniverseError
from universe_ui.fake_core import EXTENSIONS, FIXTURE, LAUNCH_KEYS, FakeCore

universe_core = pytest.importorskip("universe_core")

ROOT = Path(__file__).resolve().parents[2]
needs_manifests = pytest.mark.skipif(not (ROOT / "modules").is_dir(), reason="the flake's check copies ui/ alone: the core would list no module or source")

PNG = base64.b64decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==")
SESSION = "20260913-120000"
# The real profile: games the fixture has too, so one call reads both cores.
SEED = {
    "config/config.toml": 'schema = 1\n[modules]\nenabled = ["capture", "journal", "screenshot", "controls"]\n[sources]\nenabled = ["gog"]\n',
    "data/games/the-technomancer/game.toml": 'schema = 1\nid = "the-technomancer"\ntitle = "The Technomancer"\nfavorite = true\ntags = ["rpg"]\n'
    '[source]\nkind = "gog"\n[launch]\nexe = "/mnt/games/PC/The Technomancer/TheTechnomancer.exe"\n[modules.capture]\ncursor = false\n',
    "data/games/the-technomancer/sessions.jsonl": f'{{"session": "{SESSION}", "game": "the-technomancer", "started_at": "2026-09-13T12:00:00+02:00",'
    ' "ended_at": "2026-09-13T13:00:00+02:00", "duration_s": 3600, "exit": 0, "recording": "/nowhere/20260913-120000.mkv"}\n',
    f"data/games/the-technomancer/journal/{SESSION}.json": json.dumps(
        {"session": SESSION, "game": "the-technomancer", "written_at": "2026-09-13T13:10:00+02:00", "title": "First", "paragraphs": ["p"]}
    ),
    "data/games/the-technomancer/journal/20260912-120000.deferred.json": json.dumps({"game": "the-technomancer", "until": "2099-01-01T00:00:00+00:00"}),
    "data/games/the-technomancer/screenshots/20260913-121000.png": PNG,
    "data/games/the-technomancer/media/box_front.png": PNG,
    "data/games/dead-cells/game.toml": 'schema = 1\nid = "dead-cells"\ntitle = "Dead Cells"\n[source]\nkind = "gog"\n',
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


@pytest.fixture(scope="module")
def cores(app, tmp_path_factory):
    root = tmp_path_factory.mktemp("contract")
    for name, content in SEED.items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        (path.write_bytes if isinstance(content, bytes) else path.write_text)(content)
    # The core leaves out a module whose binary is missing: a stub stands in for each one this machine lacks.
    stubs = root / "bin"
    stubs.mkdir()
    for manifest in [*ROOT.glob("modules/*/module.toml"), *ROOT.glob("sources/*/source.toml")]:
        for name in tomllib.loads(manifest.read_text()).get("requires", {}).get("bins", []):
            if shutil.which(name) is None:
                (stubs / name).write_text("#!/bin/sh\nexit 1\n")
                (stubs / name).chmod(0o755)
    index = root / "index.json"
    index.write_text(json.dumps({"schema": 1, "extensions": json.loads(EXTENSIONS.read_text())["extensions"][:1]}))
    with pytest.MonkeyPatch.context() as env:
        env.setenv("PATH", f"{os.environ['PATH']}{os.pathsep}{stubs}")
        env.setenv("UNIVERSE_EXTENSIONS_INDEX", str(index))
        for kind in ("data", "config", "state", "cache"):
            env.setenv(f"UNIVERSE_{kind.upper()}_HOME", str(root / kind))
        for kind in ("modules", "sources"):
            env.setenv(f"UNIVERSE_{kind.upper()}_PATH", str(ROOT / kind))
        fake = FakeCore(FIXTURE, root / "fake")
        yield universe_core.Core(), fake
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
    ("Unavailable", "achievements", "the-technomancer"),
    ("Invalid", "extension_install", "now-playing"),
    ("NotFound", "extension_remove", "nope"),
    ("Invalid", "extension_remove", "capture"),
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
