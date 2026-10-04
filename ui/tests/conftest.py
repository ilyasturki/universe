import inspect
import os
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


# A key posted and never delivered would make the next test's own press of that key count as the pad's.
@pytest.fixture(autouse=True)
def posted_keys_cleared():
    from universe_ui import gamepad

    gamepad.POSTED.clear()
    yield
    gamepad.POSTED.clear()


@pytest.fixture(scope="session")
def app(xdg):
    from PySide6.QtGui import QGuiApplication

    application = QGuiApplication.instance() or QGuiApplication([])
    from universe_ui import models  # noqa: F401  (registers the Universe QML module)

    return application


# A FakeCore paints the fixture's art into the cache when it finds none: once a run, each xdist worker taking a copy, rather than in a first test's 2 s.
@pytest.fixture(scope="session")
def fixture_art(app, xdg, tmp_path_factory):
    import fcntl
    import json
    import shutil

    from universe_ui.fake_core import FIXTURE
    from universe_ui.fixtures.art import paint_library

    base = tmp_path_factory.getbasetemp()
    painted = (base.parent if os.environ.get("PYTEST_XDIST_WORKER") else base) / "fake-art"
    with open(f"{painted}.lock", "w") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        with open(FIXTURE) as f:
            paint_library(json.load(f)["games"], str(painted))
    shutil.copytree(painted, os.path.join(os.environ["XDG_CACHE_HOME"], "universe", "fake-art"), dirs_exist_ok=True)


# A process's first window on a look costs about a second its next ones do not: paid here, rather than in the first test's 2 s.
@pytest.fixture(scope="session")
def rendered_looks(app, fixture_art, tmp_path_factory):
    from looks import LOOKS, Look

    from universe_ui.api import Api
    from universe_ui.fake_core import FIXTURE, FakeCore
    from universe_ui.screens.network import FAKE as FAKE_NET
    from universe_ui.screens.power import FAKE
    from universe_ui.universe_client import CoreClient

    root = tmp_path_factory.mktemp("warm")
    client = CoreClient(FakeCore(FIXTURE, root / "core"))
    api = Api(client, memory_path=str(root / "memory.json"), power_root=FAKE, net_root=FAKE_NET)
    for name in LOOKS:
        Look(api, name).close()
    api.shutdown()
    client.shutdown()


@pytest.fixture
def fake(app, xdg, fixture_art, tmp_path):
    from universe_ui.fake_core import FIXTURE, FakeCore
    from universe_ui.universe_client import CoreClient

    client = CoreClient(FakeCore(FIXTURE, tmp_path / "core"))
    yield client
    client.shutdown()


@pytest.fixture(params=[False, True], ids=["desktop", "session"])
def universe_session(request, monkeypatch):
    """Outside, then inside the Universe session a display manager started; asked for before `api`, which reads it once."""
    monkeypatch.setenv("UNIVERSE_FAKE_SESSION", "1" if request.param else "0")
    return request.param


def pytest_generate_tests(metafunc):
    if "look" in metafunc.fixturenames and not any("look" in m.args[0] for m in metafunc.definition.iter_markers("parametrize")):
        from looks import LOOKS

        metafunc.parametrize("look", LOOKS, indirect=True)


@pytest.fixture
def look(request, api):
    """A window on each look in turn, or on the looks a test's own `parametrize("look", …, indirect=True)` names."""
    from looks import Look

    shown = Look(api, request.param)
    yield shown
    shown.close()


@pytest.fixture
def api(fake, rendered_looks, tmp_path):
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


def own(fake, key, game="the-technomancer"):
    """The game's own launch value for `key`, None while it follows every game's."""
    value = (fake.game(game).get("launch") or {}).get(key)
    return None if value in ("", None) else value


def index_of(form, key):
    return next(i for i, r in enumerate(form.rows) if r["key"] == key)


def rows_by_key(form, module=None):
    return {r["key"]: r for r in form.rows if module is None or r["module"] == module}
