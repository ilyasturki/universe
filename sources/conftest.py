import importlib.machinery
import importlib.util
import io
import json
import os
import signal
import subprocess
import sys
import urllib.request

import pytest


class Web:
    """The stores' side of urlopen: `answers` maps a URL prefix to a JSON value, bytes, an exception to raise or a function of
    the URL giving one of those; any other URL is refused. `seen` keeps each request's URL and bearer token."""

    def __init__(self):
        self.answers, self.seen = {}, []

    def __call__(self, request, timeout=None):
        url = request.full_url
        auth = request.get_header("Authorization") or ""
        self.seen.append((url, auth.removeprefix("Bearer ") or None))
        answer = next((a for prefix, a in self.answers.items() if url.startswith(prefix)), OSError(f"no network in tests: {url}"))
        if callable(answer) and not isinstance(answer, BaseException):
            answer = answer(url)
        if isinstance(answer, BaseException):
            raise answer
        return io.BytesIO(answer if isinstance(answer, bytes) else json.dumps(answer).encode())


@pytest.fixture(autouse=True)
def web(monkeypatch):
    web = Web()
    monkeypatch.setattr(urllib.request, "urlopen", web)
    return web


@pytest.fixture
def src(request):
    path = request.path.parents[1] / "bin" / "source"
    loader = importlib.machinery.SourceFileLoader(f"{path.parents[1].name}_source", str(path))
    module = importlib.util.module_from_spec(importlib.util.spec_from_loader(loader.name, loader))
    loader.exec_module(module)
    for name, value in getattr(request.module, "SOURCE_PATCHES", {}).items():
        setattr(module, name, value)
    return module


@pytest.fixture
def run(src, capsys):
    def run(*argv):
        code = src.main(list(argv))
        out, err = capsys.readouterr()
        return code, [json.loads(line) for line in out.splitlines()], err

    return run


@pytest.fixture
def shims(request, monkeypatch):
    folder = request.path.parent / "shims"
    monkeypatch.setenv("PATH", f"{folder}{os.pathsep}{os.environ['PATH']}")
    return folder


@pytest.fixture
def settings(monkeypatch):
    def change(**changes):
        monkeypatch.setenv("SOURCE_SETTINGS_JSON", json.dumps({**json.loads(os.environ["SOURCE_SETTINGS_JSON"]), **changes}))

    return change


@pytest.fixture
def core(tmp_path, monkeypatch, shims):
    """The real in-process core on a sealed profile; the trash is the profile's home's."""
    universe_core = pytest.importorskip("universe_core")
    root = tmp_path / "profile"

    def start(source, settings):
        for kind in ("data", "config", "state", "cache"):
            monkeypatch.setenv(f"UNIVERSE_{kind.upper()}_HOME", str(root / kind))
        for name, rel in (("HOME", "home"), ("XDG_DATA_HOME", "home/share"), ("XDG_CONFIG_HOME", "home/config"), ("UNIVERSE_MODULES_PATH", "modules")):
            (root / rel).mkdir(parents=True, exist_ok=True)
            monkeypatch.setenv(name, str(root / rel))
        monkeypatch.setenv("UNIVERSE_SOURCES_PATH", os.path.dirname(__file__))
        monkeypatch.setenv("DBUS_SESSION_BUS_ADDRESS", "unix:path=/dev/null/bus")
        for name in ("http_proxy", "https_proxy", "HTTP_PROXY", "HTTPS_PROXY"):
            monkeypatch.setenv(name, "http://127.0.0.1:9")
        (root / "config").mkdir(parents=True, exist_ok=True)
        lines = [f"{k} = {json.dumps(v)}" for k, v in settings.items()]
        (root / "config" / "config.toml").write_text(
            f'schema = 1\n[desktop]\nprofile = "none"\n[sources]\nenabled = ["{source}"]\n[sources.{source}]\n' + "\n".join(lines) + "\n"
        )
        return universe_core.Core()

    start.data = root / "data" / "sources"
    start.trash = root / "home" / "share" / "Trash" / "files"
    start.stopped = stopped_install
    return start


def stopped_install(core, source, game_id):
    """An install of `game_id` cancelled once its download began: the Paused row a player stopped."""
    started = []

    def progress(done, total, message):
        if not started:
            started.append(core.cancel(source, game_id))

    with pytest.raises(sys.modules["universe_core"].UniverseError):
        core.install(source, game_id, progress)
    assert started == [True]
    return next(r for r in core.library(source, False) if r["id"] == game_id)


@pytest.fixture
def interrupt(src):
    def interrupt(*argv):
        proc = subprocess.Popen([sys.executable, src.__file__, *argv], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        assert json.loads(proc.stdout.readline())["event"] == "progress"
        proc.send_signal(signal.SIGTERM)
        out, _ = proc.communicate(timeout=15)
        assert proc.returncode == 128 + signal.SIGTERM and '"game"' not in out

    return interrupt
