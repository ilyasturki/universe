import os
import time

import pytest

# One environment for `just test` and the flake's checks: the fixtures carry Paris timestamps, Qt has no display
# and no sound server.
os.environ["TZ"] = "Europe/Paris"
os.environ["LC_ALL"] = "C.UTF-8"
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
os.environ.setdefault("QT_FORCE_STDERR_LOGGING", "1")
os.environ["PIPEWIRE_REMOTE"] = "/nonexistent"
os.environ["PULSE_SERVER"] = "unix:/nonexistent"
time.tzset()


# The flake's checks set 0: a shared builder's load says nothing of a test's own time.
BUDGET_S = float(os.environ.get("UNIVERSE_TEST_BUDGET_S", "2"))


# pytest-qt ships with the UI suite only; the other suites read the same pyproject.
def pytest_addoption(parser, pluginmanager):
    if not pluginmanager.hasplugin("pytestqt"):
        for name in ("qt_api", "qt_log_level_fail", "qt_log_ignore"):
            parser.addini(name, "pytest-qt, unused here")


# Only the test's own fixtures count: the first test on a worker pays for the session's.
@pytest.hookimpl(hookwrapper=True)
def pytest_fixture_setup(fixturedef, request):
    start = time.perf_counter()
    yield
    if fixturedef.scope == "function":
        request.node.fixture_s = getattr(request.node, "fixture_s", 0.0) + time.perf_counter() - start


@pytest.hookimpl(hookwrapper=True)
def pytest_runtest_makereport(item, call):
    outcome = yield
    report = outcome.get_result()
    if call.when != "call" or not report.passed or item.get_closest_marker("slow"):
        return
    spent = getattr(item, "fixture_s", 0.0) + report.duration
    if BUDGET_S and spent > BUDGET_S:
        report.outcome = "failed"
        report.longrepr = f"took {spent:.2f} s, over the {BUDGET_S:.0f} s budget: make it faster or mark it @pytest.mark.slow"


@pytest.fixture(scope="session")
def xdg(tmp_path_factory):
    root = tmp_path_factory.mktemp("xdg")
    for name in ("XDG_STATE_HOME", "XDG_CACHE_HOME", "XDG_DATA_HOME", "XDG_CONFIG_HOME"):
        kind = name.split("_")[1].lower()
        os.environ[name] = str(root / kind)
        # a justfile or shell may point the core at a dev library; the tests get their own
        os.environ[f"UNIVERSE_{kind.upper()}_HOME"] = str(root / kind / "universe")
    return root
