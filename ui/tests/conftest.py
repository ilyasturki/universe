import os

import pytest

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
os.environ.setdefault("QT_FORCE_STDERR_LOGGING", "1")


@pytest.fixture(scope="session")
def xdg(tmp_path_factory):
    root = tmp_path_factory.mktemp("xdg")
    for name in ("XDG_STATE_HOME", "XDG_CACHE_HOME", "XDG_DATA_HOME", "XDG_CONFIG_HOME"):
        kind = name.split("_")[1].lower()
        os.environ[name] = str(root / kind)
        # a justfile or shell may point the core at a dev library; the tests get their own
        os.environ[f"UNIVERSE_{kind.upper()}_HOME"] = str(root / kind / "universe")
    return root


@pytest.fixture(scope="session")
def app(xdg):
    from PySide6.QtGui import QGuiApplication

    application = QGuiApplication.instance() or QGuiApplication([])
    from universe_ui import models  # noqa: F401  (registers the Universe QML module)

    return application


@pytest.fixture
def fake(app, xdg, tmp_path):
    """`CoreClient` over a `FakeCore` laid out under a fresh root; the core is `fake.core`."""
    from universe_ui.fake_core import FIXTURE, FakeCore
    from universe_ui.universe_client import CoreClient

    client = CoreClient(FakeCore(FIXTURE, tmp_path / "core"))
    yield client
    client.shutdown()


@pytest.fixture
def api(fake, tmp_path):
    from universe_ui.api import Api

    api = Api(fake, memory_path=str(tmp_path / "memory.json"))
    yield api
    api.shutdown()


def wait_for(signal, timeout_ms=5000):
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


def settle(screen, timeout_ms=5000):
    """Pumps until `screen.busy` clears: the client runs every screen's work on a thread."""
    from PySide6.QtCore import QDeadlineTimer

    deadline = QDeadlineTimer(timeout_ms)
    while screen.busy and not deadline.hasExpired():
        pump(10)
    assert not screen.busy, "still busy"


def index_of(form, key):
    return next(i for i, r in enumerate(form.rows) if r["key"] == key)


def rows_by_key(form, module=None):
    return {r["key"]: r for r in form.rows if module is None or r["module"] == module}
