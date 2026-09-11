"""Offscreen Qt, fixtures for the fake client, and XDG dirs pointed away from the user's state.

Files are named test_ui_* because the repo's pytest.ini collects ui/tests and modules/*/tests
in one run without packages, so basenames must be unique across all of them.
"""

import os
import sys
from pathlib import Path

import pytest

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
os.environ.setdefault("QT_FORCE_STDERR_LOGGING", "1")
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))


@pytest.fixture(scope="session")
def xdg(tmp_path_factory):
    root = tmp_path_factory.mktemp("xdg")
    for name in ("XDG_STATE_HOME", "XDG_CACHE_HOME", "XDG_DATA_HOME", "XDG_CONFIG_HOME"):
        os.environ[name] = str(root / name.split("_")[1].lower())
    return root


@pytest.fixture(scope="session")
def app(xdg):
    from PySide6.QtCore import QCoreApplication
    from PySide6.QtGui import QGuiApplication

    from universe_ui import host

    if host.qt_paths_unset():
        for p in host.qt_plugin_paths():
            QCoreApplication.addLibraryPath(p)
    application = QGuiApplication.instance() or QGuiApplication([])
    from universe_ui import models  # noqa: F401  (registers the Universe QML module)

    return application


@pytest.fixture
def fake(app, tmp_path):
    from universe_ui.universe_client import FakeClient

    client = FakeClient(art_dir=str(tmp_path / "art"))
    yield client
    client.shutdown()


@pytest.fixture
def api(fake, tmp_path):
    from universe_ui.api import Api

    api = Api(fake, memory_path=str(tmp_path / "memory.json"))
    yield api
    api.shutdown()


def wait_for(signal, timeout_ms=5000):
    """Runs the event loop until `signal` fires; returns its arguments, or None on timeout."""
    from PySide6.QtCore import QEventLoop, QTimer

    loop = QEventLoop()
    got = []

    def capture(*args):
        got.append(args)
        loop.quit()

    signal.connect(capture)
    QTimer.singleShot(timeout_ms, loop.quit)
    loop.exec()
    signal.disconnect(capture)
    return got[0] if got else None


def pump(ms):
    from PySide6.QtCore import QEventLoop, QTimer

    loop = QEventLoop()
    QTimer.singleShot(ms, loop.quit)
    loop.exec()
