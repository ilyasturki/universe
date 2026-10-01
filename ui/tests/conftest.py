import inspect
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


@pytest.fixture(autouse=True)
def fast_clock(monkeypatch):
    """FakeCore's delays and Home's poll of it in milliseconds, and a session that runs until the test stops it or calls `fake.core.end_session()`."""
    from universe_ui import fake_core, home

    for name, seconds in {"STEP_S": 0.01, "WINDOW_S": 0.02, "UNLOCK_S": 0.05, "SESSION_S": None}.items():
        monkeypatch.setattr(fake_core, name, seconds)
    monkeypatch.setattr(home, "POLL_MS", 30)


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
    from universe_ui.screens.network import FAKE as FAKE_NET
    from universe_ui.screens.power import FAKE

    api = Api(fake, memory_path=str(tmp_path / "memory.json"), power_root=FAKE, net_root=FAKE_NET)
    yield api
    api.shutdown()


def pump(ms):
    from PySide6.QtCore import QEventLoop, QTimer

    loop = QEventLoop()
    QTimer.singleShot(ms, loop.quit)
    loop.exec()


def until(predicate, message="", timeout_ms=5000):
    """Runs the event loop until `predicate()` is truthy and returns that value, failing once `timeout_ms` passes without it."""
    from PySide6.QtCore import QDeadlineTimer

    deadline = QDeadlineTimer(timeout_ms)
    while not (value := predicate()):
        if deadline.hasExpired():
            raise AssertionError(message or f"never held within {timeout_ms} ms: {_source(predicate)}")
        pump(5)
    return value


def _source(fn):
    try:
        return inspect.getsource(fn).strip()
    except (OSError, TypeError):
        return repr(fn)


def record(signal):
    """Every emission of `signal` from here on, as its argument tuples."""
    seen = []
    signal.connect(lambda *args: seen.append(args))
    return seen


def settle(screen, timeout_ms=5000):
    until(lambda: not screen.busy, "still busy", timeout_ms)


def index_of(form, key):
    return next(i for i, r in enumerate(form.rows) if r["key"] == key)


def rows_by_key(form, module=None):
    return {r["key"]: r for r in form.rows if module is None or r["module"] == module}
