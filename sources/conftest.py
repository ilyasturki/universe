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
def interrupt(src):
    def interrupt(*argv):
        proc = subprocess.Popen([sys.executable, src.__file__, *argv], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        assert json.loads(proc.stdout.readline())["event"] == "progress"
        proc.send_signal(signal.SIGTERM)
        out, _ = proc.communicate(timeout=15)
        assert proc.returncode == 128 + signal.SIGTERM and '"game"' not in out

    return interrupt
