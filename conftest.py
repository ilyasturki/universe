import os
import time

import pytest

# One environment for `just test` and the flake's checks: the fixtures carry Paris timestamps, Qt has no display.
os.environ["TZ"] = "Europe/Paris"
os.environ["LC_ALL"] = "C.UTF-8"
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
os.environ.setdefault("QT_FORCE_STDERR_LOGGING", "1")
time.tzset()


# pytest-qt ships with the UI suite only; the other suites read the same pyproject.
def pytest_addoption(parser, pluginmanager):
    if not pluginmanager.hasplugin("pytestqt"):
        for name in ("qt_api", "qt_log_level_fail", "qt_log_ignore"):
            parser.addini(name, "pytest-qt, unused here")


@pytest.fixture(scope="session")
def xdg(tmp_path_factory):
    root = tmp_path_factory.mktemp("xdg")
    for name in ("XDG_STATE_HOME", "XDG_CACHE_HOME", "XDG_DATA_HOME", "XDG_CONFIG_HOME"):
        kind = name.split("_")[1].lower()
        os.environ[name] = str(root / kind)
        # a justfile or shell may point the core at a dev library; the tests get their own
        os.environ[f"UNIVERSE_{kind.upper()}_HOME"] = str(root / kind / "universe")
    return root
