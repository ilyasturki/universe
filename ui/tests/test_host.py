import os
import sys

import pytest
from PySide6 import QtGui

from universe_ui import host


@pytest.fixture
def execv(monkeypatch):
    calls = []
    monkeypatch.setattr(os, "execv", lambda program, argv: calls.append((program, argv)))
    monkeypatch.delenv("LD_LIBRARY_PATH", raising=False)
    return calls


GAMESCOPE = ["/bin/gamescope", "-f"]


def test_re_execs_the_launcher_itself(monkeypatch, execv, tmp_path):
    launcher = tmp_path / "universe-ui"
    launcher.write_text("#!/bin/sh\n")
    launcher.chmod(0o755)
    monkeypatch.setattr(sys, "argv", [str(launcher), "--theme", "switch2"])

    host.exec_in_gamescope(GAMESCOPE, ["--theme", "switch2"])
    assert execv == [("/bin/gamescope", ["/bin/gamescope", "-f", "--", str(launcher), "--theme", "switch2"])], (
        "a wrapper binary is not a script the interpreter can read"
    )


def test_falls_back_to_the_interpreter_when_argv0_is_not_executable(monkeypatch, execv, tmp_path):
    script = tmp_path / "host.py"
    script.write_text("")
    monkeypatch.setattr(sys, "argv", [str(script)])

    host.exec_in_gamescope(GAMESCOPE, [])
    assert execv[0][1] == ["/bin/gamescope", "-f", "--", sys.executable, str(script)]


def test_the_library_path_is_put_back_past_the_wrapper(monkeypatch, execv, tmp_path):
    launcher = tmp_path / "universe-ui"
    launcher.write_text("#!/bin/sh\n")
    launcher.chmod(0o755)
    monkeypatch.setattr(sys, "argv", [str(launcher)])
    monkeypatch.setenv("LD_LIBRARY_PATH", "/nix/store/pipewire/lib")
    monkeypatch.setattr(host.shutil, "which", lambda name: {"env": "/usr/bin/env"}.get(name, name))

    host.exec_in_gamescope(GAMESCOPE, [])
    assert execv[0][1] == ["/bin/gamescope", "-f", "--", "/usr/bin/env", "LD_LIBRARY_PATH=/nix/store/pipewire/lib", str(launcher)], (
        "the CAP_SYS_NICE wrapper's loader drops it; the launcher inside gamescope needs it for pipewire"
    )


def test_without_gamescope_it_stays_on_the_desktop(monkeypatch, execv):
    host.exec_in_gamescope(None, [])
    assert execv == []


class Execed(Exception):
    pass


def test_fullscreen_re_execs_before_the_core_opens(app, monkeypatch, tmp_path):
    import types

    opened = []

    class Core:
        def __init__(self):
            opened.append("core")

    stub = types.ModuleType("universe_core")
    stub.Core = Core
    stub.host_gamescope = lambda screen: GAMESCOPE
    monkeypatch.setitem(sys.modules, "universe_core", stub)
    monkeypatch.setattr(QtGui, "QGuiApplication", lambda argv: app)
    monkeypatch.delenv("GAMESCOPE_WAYLAND_DISPLAY", raising=False)
    monkeypatch.setattr(sys, "argv", [str(tmp_path / "universe-ui")])

    def execv(program, argv):
        raise Execed(program)

    monkeypatch.setattr(os, "execv", execv)
    with pytest.raises(Execed):
        host.run([])
    assert opened == [], "the library is loaded once, inside gamescope, not on each side of the exec"


def test_a_lost_display_ends_the_process_at_once_from_any_thread(monkeypatch):
    import ctypes

    installed = []

    class X11:
        def XSetIOErrorHandler(self, handler):
            installed.append(handler)

    monkeypatch.setattr(ctypes, "CDLL", lambda name: X11())
    monkeypatch.setattr(host, "_io_error_handlers", [])
    exits = []

    monkeypatch.setattr(os, "_exit", exits.append)
    host.exit_with_the_display()
    host.exit_with_the_display()
    assert installed == host._io_error_handlers and len(installed) == 1, "set once, the callback kept alive by the module"
    installed[0](None)
    assert exits == [0], "no exit(): no destructors run on the thread that hit the error"
