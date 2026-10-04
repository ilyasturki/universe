import importlib.machinery
import importlib.util
import json
import os
import signal
import subprocess
import sys
import urllib.request

import pytest


@pytest.fixture(autouse=True)
def offline(monkeypatch):
    def refuse(*args, **kwargs):
        raise OSError("no network in tests")

    monkeypatch.setattr(urllib.request, "urlopen", refuse)


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
