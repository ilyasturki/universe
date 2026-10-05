import os
import sys
from pathlib import Path

import pytest
from PySide6 import QtGui

from universe_ui import host

GAMESCOPE = ["/bin/gamescope", "-f"]


@pytest.fixture
def launcher(monkeypatch, tmp_path):
    path = tmp_path / "universe-ui"
    path.write_text("#!/bin/sh\n")
    path.chmod(0o755)
    monkeypatch.setattr(sys, "argv", [str(path)])
    monkeypatch.delenv("LD_LIBRARY_PATH", raising=False)
    return path


def test_gamescope_runs_the_launcher_itself(launcher):
    assert host.gamescope_argv(GAMESCOPE, ["--theme", "switch2"]) == ["/bin/gamescope", "-f", "--", str(launcher), "--theme", "switch2"], (
        "a wrapper binary is not a script the interpreter can read"
    )


def test_falls_back_to_the_interpreter_when_argv0_is_not_executable(monkeypatch, tmp_path):
    script = tmp_path / "host.py"
    script.write_text("")
    monkeypatch.setattr(sys, "argv", [str(script)])
    monkeypatch.delenv("LD_LIBRARY_PATH", raising=False)
    assert host.gamescope_argv(GAMESCOPE, []) == ["/bin/gamescope", "-f", "--", sys.executable, str(script)]


def test_the_library_path_is_put_back_past_the_wrapper(monkeypatch, launcher):
    monkeypatch.setenv("LD_LIBRARY_PATH", "/nix/store/pipewire/lib")
    monkeypatch.setattr(host.shutil, "which", lambda name: {"env": "/usr/bin/env"}.get(name, name))
    assert host.gamescope_argv(GAMESCOPE, []) == ["/bin/gamescope", "-f", "--", "/usr/bin/env", "LD_LIBRARY_PATH=/nix/store/pipewire/lib", str(launcher)], (
        "the CAP_SYS_NICE wrapper's loader drops it; the launcher inside gamescope needs it for pipewire"
    )


@pytest.fixture
def gamescope(monkeypatch, tmp_path, launcher):
    """A gamescope whose launcher comes up (`up`), quits on purpose with 3 (`quits`), dies after `FAKE_CRASHES` runs (`crashes`),
    that dies at start (`dies`) or that hangs showing nothing (`hangs`). Each run's session flag and launcher argv land in `runs`."""
    monkeypatch.setenv("XDG_RUNTIME_DIR", str(tmp_path))
    script = tmp_path / "gamescope"
    script.write_text(
        '#!/bin/sh\necho "$UNIVERSE_OWN_GAMESCOPE" > "$XDG_RUNTIME_DIR/own"\n'
        'shift 2; echo "${UNIVERSE_SESSION:-0} $*" >> "$XDG_RUNTIME_DIR/runs"\n'
        'case "$FAKE_GAMESCOPE" in\n'
        '  up) touch "$UNIVERSE_HOST_READY"; exit 7;;\n'
        '  quits) touch "$UNIVERSE_HOST_READY"; echo 3 > "$UNIVERSE_HOST_DONE"; exit 0;;\n'
        '  crashes) touch "$UNIVERSE_HOST_READY"\n'
        '    [ "$(wc -l < "$XDG_RUNTIME_DIR/runs")" -gt "${FAKE_CRASHES:-99}" ] && echo 0 > "$UNIVERSE_HOST_DONE"; exit 0;;\n'
        "  dies) exit 1;;\n  hangs) exec sleep 30;;\nesac\n"
    )
    script.chmod(0o755)
    return [str(script)]


def test_the_exit_code_of_a_gamescope_that_came_up_is_the_launchers(monkeypatch, gamescope):
    monkeypatch.setenv("FAKE_GAMESCOPE", "up")
    assert host.run_in_gamescope(gamescope, []) == 7, "quitting gamescope after the launcher came up quits, not a restart on the desktop"
    monkeypatch.setenv("FAKE_GAMESCOPE", "quits")
    assert host.run_in_gamescope(gamescope, []) == 3, "gamescope exits 0 whatever the launcher quit with"


@pytest.mark.parametrize(("display", "own"), [("wayland-0", "nested"), ("", "drm")])
def test_the_launchers_own_gamescope_is_marked_so_steams_is_told_apart(monkeypatch, gamescope, tmp_path, display, own):
    monkeypatch.setenv("FAKE_GAMESCOPE", "up")
    monkeypatch.delenv("DISPLAY", raising=False)
    monkeypatch.setenv("WAYLAND_DISPLAY", display)
    host.run_in_gamescope(gamescope, [])
    assert (tmp_path / "own").read_text().strip() == own, "straight on the screen (a session of its own) or in a desktop's window"


@pytest.mark.parametrize("mode", ["dies", "hangs", "none"])
def test_a_gamescope_that_fails_leaves_the_launcher_on_the_desktop(monkeypatch, gamescope, tmp_path, mode):
    monkeypatch.setenv("FAKE_GAMESCOPE", mode)
    (tmp_path / f"universe-ui-ready-{os.getpid()}").touch()
    assert host.run_in_gamescope(None if mode == "none" else gamescope, [], ready_s=0.1) is None, "a mark left by a crashed run is no launcher up"


def runs(tmp_path):
    return (tmp_path / "runs").read_text().splitlines()


def test_a_session_ends_with_the_code_the_launcher_quit_with(monkeypatch, gamescope, tmp_path):
    monkeypatch.setenv("FAKE_GAMESCOPE", "quits")
    assert host.run_session(gamescope, ["--session"]) == 3, "gamescope's own 0 says nothing of its child"
    assert [r.split()[0] for r in runs(tmp_path)] == ["1"], "the launcher inside knows it is the session"


@pytest.mark.parametrize("mode", ["dies", "hangs", "none"])
def test_a_session_whose_gamescope_fails_ends_instead_of_falling_back_to_a_desktop(monkeypatch, gamescope, mode):
    monkeypatch.setenv("FAKE_GAMESCOPE", mode)
    assert host.run_session(None if mode == "none" else gamescope, [], ready_s=0.1) == 1, "the display manager shows its greeter again"


def test_a_launcher_that_dies_comes_back_without_the_intro(monkeypatch, gamescope, tmp_path):
    monkeypatch.setenv("FAKE_GAMESCOPE", "crashes")
    monkeypatch.setenv("FAKE_CRASHES", str(host.SHORT_RUNS + 1))
    assert host.run_session(gamescope, ["--session"], short_s=0) == 0, "runs that lasted are no crash loop"
    launchers = [r.split()[1:] for r in runs(tmp_path)]
    assert len(launchers) == host.SHORT_RUNS + 2
    assert launchers[0] == ["--session"] and all(argv == ["--session", "--no-boot"] for argv in launchers[1:])


def test_a_launcher_that_keeps_dying_at_once_ends_the_session(monkeypatch, gamescope, tmp_path):
    monkeypatch.setenv("FAKE_GAMESCOPE", "crashes")
    assert host.run_session(gamescope, []) == 1
    assert len(runs(tmp_path)) == host.SHORT_RUNS


class Supervised(Exception):
    pass


def test_fullscreen_starts_gamescope_before_the_core_and_the_app_open(monkeypatch, tmp_path):
    import types

    opened = []

    class Core:
        def __init__(self):
            opened.append("core")

    stub = types.ModuleType("universe_core")
    stub.Core = Core
    stub.host_gamescope = lambda screen: GAMESCOPE
    monkeypatch.setitem(sys.modules, "universe_core", stub)
    monkeypatch.setattr(QtGui, "QGuiApplication", lambda argv: opened.append("app"))
    monkeypatch.delenv("GAMESCOPE_WAYLAND_DISPLAY", raising=False)
    monkeypatch.setattr(sys, "argv", [str(tmp_path / "universe-ui")])

    def run_in_gamescope(command, argv):
        raise Supervised(command)

    monkeypatch.setattr(host, "run_in_gamescope", run_in_gamescope)
    with pytest.raises(Supervised):
        host.run([])
    assert opened == [], "the library is loaded once, inside gamescope, and no display is held while it runs"


def test_the_launcher_inside_says_it_is_up_once(monkeypatch, tmp_path):
    mark = tmp_path / "ready"
    monkeypatch.setenv(host.READY_ENV, str(mark))
    host.mark_ready()
    assert mark.exists() and host.READY_ENV not in os.environ, "a game or module started from here inherits no mark to set"


def test_the_launcher_inside_says_it_quit_even_after_restarting_in_place(monkeypatch, tmp_path):
    mark = tmp_path / "done"
    monkeypatch.setenv(host.DONE_ENV, str(mark))
    host.mark_done(0)
    assert mark.read_text() == "0" and os.environ[host.DONE_ENV] == str(mark)


def test_a_session_takes_the_screen_from_inside_a_desktop_too(monkeypatch, tmp_path):
    import types

    stub = types.ModuleType("universe_core")
    stub.host_gamescope = lambda screen: GAMESCOPE
    monkeypatch.setitem(sys.modules, "universe_core", stub)
    monkeypatch.delenv("GAMESCOPE_WAYLAND_DISPLAY", raising=False)
    monkeypatch.delenv("XDG_CURRENT_DESKTOP", raising=False)
    monkeypatch.setenv("WAYLAND_DISPLAY", "wayland-0")
    monkeypatch.setenv("DISPLAY", ":0")
    seen = []
    monkeypatch.setattr(host, "run_session", lambda command, argv: seen.append((command, argv, dict(os.environ))) or 0)
    monkeypatch.setattr(host, "run_in_gamescope", lambda command, argv: pytest.fail("a session has no desktop to come back to"))
    assert host.run(["--session"]) == 0
    command, argv, env = seen[0]
    assert (command, argv) == (GAMESCOPE, ["--session"])
    assert "WAYLAND_DISPLAY" not in env and "DISPLAY" not in env, "gamescope drives the screen itself"
    assert env["XDG_CURRENT_DESKTOP"] == "Universe", "the desktop detection inside finds none"


def test_a_session_cannot_be_windowed():
    with pytest.raises(SystemExit):
        host.parse_args(["--session", "--windowed"])


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


WEDGE = """
import os, subprocess, sys
sys.argv[0] = {relaunch!r}
from universe_ui import host
from PySide6.QtCore import QCoreApplication, QSocketNotifier, QThread, QTimer
host.restart_when_wedged(["--theme", "switch2"])
app = QCoreApplication([])
child = subprocess.Popen(["sleep", "30"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
print("child", child.pid, flush=True)
reader, writer = os.pipe()
notifier = QSocketNotifier(reader, QSocketNotifier.Type.Read)

class Stale(QThread):
    def run(self):
        notifier.setEnabled(False)
        os.close(reader)

stale = Stale()
QTimer.singleShot(0, stale.start)
# The app's loop wakes for every frame; an idle one would see the dead socket again only at the next timer.
tick = QTimer()
tick.start(20)
QTimer.singleShot(5000, lambda: (print("still here", flush=True), app.quit()))
app.exec()
"""


def run_script(tmp_path, script):
    import subprocess

    relaunch = tmp_path / "universe-ui"
    relaunch.write_text('#!/bin/sh\necho "relaunched $$ $* $UNIVERSE_UI_RESTARTS"\n')
    relaunch.chmod(0o755)
    env = {**os.environ, "QT_QPA_PLATFORM": "offscreen", "PYTHONPATH": os.pathsep.join(sys.path)}
    env.pop(host.RESTARTS_ENV, None)
    proc = subprocess.Popen([sys.executable, "-c", script.format(relaunch=str(relaunch))], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    out, err = proc.communicate(timeout=20)
    return proc.pid, out, err, int(out.split()[out.split().index("child") + 1])


def test_a_loop_wedged_on_a_dead_notifier_restarts_in_place(tmp_path):
    pid, out, err, child = run_script(tmp_path, WEDGE)
    assert out.rstrip().endswith(f"relaunched {pid} --theme switch2 1"), "the same process, the same arguments, one restart counted\n" + out + err
    assert "Socket notifiers cannot be enabled or disabled from another thread" in err and "libQt6Core" in err, (
        "the thread that left the notifier behind is logged with its native stack"
    )
    assert not Path(f"/proc/{child}").exists(), "the old image's children are killed and reaped, not left as zombies of the new one"


def test_a_dead_socket_its_notifier_still_owns_is_no_wedge(tmp_path):
    healed = WEDGE.replace("QTimer.singleShot(0, stale.start)", "QTimer.singleShot(0, lambda: os.close(reader))").replace("5000", "500")
    _, out, err, child = run_script(tmp_path, healed)
    os.kill(child, 9)
    assert err.count("QSocketNotifier: Invalid socket") == 1 and "still here" in out and "relaunched" not in out, (
        "Qt disables it on the owning thread and the loop carries on\n" + out + err
    )


def test_a_loop_that_keeps_wedging_quits_instead_of_restarting(monkeypatch):
    monkeypatch.setenv(host.RESTARTS_ENV, str(host.MAX_RESTARTS))
    exits, execs = [], []
    monkeypatch.setattr(os, "_exit", exits.append)
    monkeypatch.setattr(os, "execvp", lambda *a: execs.append(a))
    monkeypatch.setattr(host, "stop_children", lambda: pytest.fail("a run that quits kills nothing on the way"))
    host.relaunch([])
    assert exits == [1] and execs == []


@pytest.mark.parametrize(
    ("argv", "session", "steam", "restarts", "wanted"),
    [
        ([], None, False, 0, True),
        (["--windowed"], None, False, 0, False),
        (["--size", "1280x800"], None, False, 0, False),
        (["--fake"], None, False, 0, False),
        (["--keys", "Right"], None, False, 0, False),
        ([], {"id": "control"}, False, 0, False),
        ([], None, True, 0, False),
        ([], None, False, 1, False),
        (["--windowed", "--fake", "--boot"], None, False, 0, True),
        (["--no-boot"], None, False, 0, False),
    ],
    ids=["fullscreen", "windowed", "size", "fake", "keys", "session", "steam", "relaunch", "forced", "off"],
)
def test_only_a_plain_fullscreen_start_opens_on_the_intro(argv, session, steam, restarts, wanted):
    from types import SimpleNamespace

    client = SimpleNamespace(currentSession=session, underSteam=steam)
    assert host.boot_wanted(host.parse_args(argv), client, restarts) is wanted
