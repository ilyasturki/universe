import sys
import traceback

import pytest


@pytest.fixture(autouse=True)
def raising_slots_fail(request):
    """PySide6 prints an exception raised inside a slot and carries on; the test that let it happen fails instead."""
    caught = []
    previous = sys.excepthook
    sys.excepthook = lambda *exc: caught.append("".join(traceback.format_exception(*exc)))
    yield
    sys.excepthook = previous
    if caught and request.node.rep_call_passed:
        pytest.fail("an exception escaped a Qt slot:\n" + "\n".join(caught), pytrace=False)


@pytest.hookimpl(hookwrapper=True)
def pytest_runtest_makereport(item, call):
    outcome = yield
    if call.when == "call":
        item.rep_call_passed = outcome.get_result().passed


@pytest.fixture(scope="session")
def app(xdg):
    from PySide6.QtGui import QGuiApplication

    application = QGuiApplication.instance() or QGuiApplication([])
    from universe_ui import models  # noqa: F401  (registers the Universe QML module)

    return application


@pytest.fixture
def fake(app, xdg, tmp_path):
    from universe_ui.fake_core import FIXTURE, FakeCore
    from universe_ui.universe_client import CoreClient

    client = CoreClient(FakeCore(FIXTURE, tmp_path / "core"))
    yield client
    client.shutdown()


@pytest.fixture
def api(fake, tmp_path):
    from universe_ui.api import Api
    from universe_ui.screens.power import FAKE

    api = Api(fake, memory_path=str(tmp_path / "memory.json"), power_root=FAKE)
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
    from PySide6.QtCore import QDeadlineTimer

    deadline = QDeadlineTimer(timeout_ms)
    while screen.busy and not deadline.hasExpired():
        pump(10)
    assert not screen.busy, "still busy"


def index_of(form, key):
    return next(i for i, r in enumerate(form.rows) if r["key"] == key)


def rows_by_key(form, module=None):
    return {r["key"]: r for r in form.rows if module is None or r["module"] == module}
