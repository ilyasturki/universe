import copy
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path
from types import SimpleNamespace

import pytest

from universe_ui import host

LAUNCHER = Path(sys.executable).with_name("universe-ui")

# What the real gamescope does with the launcher, as far as the host can tell: FAKE_GAMESCOPE picks how the launcher inside
# fares (`up` then exits 7, `quits` with 3 on purpose, `crashes` until FAKE_CRASHES runs, `dies` before it is up, `hangs`).
GAMESCOPE = r"""#!/usr/bin/env bash
[ "$FAKE_GAMESCOPE" = hangs ] && exec sleep 30
log="$FAKE_GAMESCOPE_LOG"
echo "${UNIVERSE_SESSION:-0} $UNIVERSE_OWN_GAMESCOPE ${WAYLAND_DISPLAY:--} ${DISPLAY:--} ${XDG_CURRENT_DESKTOP:--}" >> "$log/gamescopes"
while [ "$1" != -- ]; do shift; done; shift
# Its CAP_SYS_NICE wrapper runs secure: the loader drops LD_LIBRARY_PATH before the launcher starts.
unset LD_LIBRARY_PATH
wrapper=(); while [ "$1" != "$FAKE_LAUNCHER" ]; do wrapper+=("$1"); shift; done
shift
echo "$*" >> "$log/launchers"
"${wrapper[@]}" sh -c 'echo "${LD_LIBRARY_PATH:--}"' >> "$log/libraries"
case "$FAKE_GAMESCOPE" in
  up) touch "$UNIVERSE_HOST_READY"; exit 7;;
  quits) touch "$UNIVERSE_HOST_READY"; echo 3 > "$UNIVERSE_HOST_DONE"; exit 0;;
  crashes) touch "$UNIVERSE_HOST_READY"
    [ "$(wc -l < "$log/launchers")" -gt "${FAKE_CRASHES:-99}" ] && echo 0 > "$UNIVERSE_HOST_DONE"; exit 0;;
  dies) exit 1;;
esac
"""


@pytest.fixture
def gamescope(tmp_path):
    """`run(mode, *argv, **env)` starts the real universe-ui on a sealed profile with the fake gamescope first on PATH."""
    bindir = tmp_path / "bin"
    bindir.mkdir()
    (bindir / "gamescope").write_text(GAMESCOPE)
    (bindir / "gamescope").chmod(0o755)
    env = {k: v for k, v in os.environ.items() if k not in ("WAYLAND_DISPLAY", "DISPLAY", "XDG_CURRENT_DESKTOP", "GAMESCOPE_WAYLAND_DISPLAY")}
    env.update(
        PATH=f"{bindir}{os.pathsep}{env['PATH']}",
        HOME=str(tmp_path),
        XDG_RUNTIME_DIR=str(tmp_path),
        QT_QPA_PLATFORM="offscreen",
        FAKE_GAMESCOPE_LOG=str(tmp_path),
        FAKE_LAUNCHER=str(LAUNCHER),
        LD_LIBRARY_PATH="/nix/store/pipewire/lib",
        **{f"UNIVERSE_{kind.upper()}_HOME": str(tmp_path / kind) for kind in ("data", "config", "state", "cache")},
    )

    def lines(name):
        path = tmp_path / name
        lines = path.read_text().splitlines() if path.exists() else []
        path.unlink(missing_ok=True)
        return lines

    def run(mode, *argv, **extra):
        code = subprocess.run([str(LAUNCHER), *argv], env={**env, "FAKE_GAMESCOPE": mode, **extra}, capture_output=True, timeout=30, check=False).returncode
        gamescopes = [dict(zip(("session", "own", "wayland", "x11", "desktop"), row.split(), strict=True)) for row in lines("gamescopes")]
        return SimpleNamespace(code=code, gamescopes=gamescopes, launchers=[row.split() for row in lines("launchers")], libraries=lines("libraries"))

    if not LAUNCHER.exists():
        pytest.skip("needs the installed universe-ui")
    run.env, run.command = env, [str(bindir / "gamescope")]
    return run


@pytest.mark.slow
def test_fullscreen_hands_the_screen_to_gamescope_or_falls_back_to_the_desktop(gamescope):
    run = gamescope("up", "--theme", "switch2", WAYLAND_DISPLAY="wayland-0")
    assert run.code == 7, "quitting gamescope after the launcher came up quits, not a restart on the desktop"
    assert run.launchers == [["--theme", "switch2"]] and run.gamescopes[0]["own"] == "nested", "a window on the desktop, told apart from Steam's"
    assert run.libraries == ["/nix/store/pipewire/lib"], "the launcher inside gets back the library path for pipewire"
    run = gamescope("quits")
    assert run.code == 3, "gamescope exits 0 whatever the launcher quit with"
    assert run.gamescopes[0]["own"] == "drm" and run.gamescopes[0]["session"] == "0", "straight on the screen, yet no session"
    run = gamescope("dies", "--no-gamepad", "--quit-after", "200")
    assert (run.code, len(run.gamescopes)) == (0, 1), "the launcher opened on the desktop and quit there"


def test_a_session_takes_the_screen_restarts_a_dead_launcher_without_the_intro_and_ends_a_crash_loop(gamescope):
    desktop = {"WAYLAND_DISPLAY": "wayland-0", "DISPLAY": ":0"}
    run = gamescope("crashes", "--session", FAKE_CRASHES="2", **desktop)
    assert run.code == 0
    assert run.launchers == [["--session"], ["--session", "--no-boot"], ["--session", "--no-boot"]]
    assert {tuple(g.values()) for g in run.gamescopes} == {("1", "drm", "-", "-", "Universe")}, (
        "gamescope drives the screen itself, even from inside a desktop, and the desktop detection inside finds none"
    )
    run = gamescope("crashes", "--session")
    assert (run.code, len(run.launchers)) == (1, host.SHORT_RUNS), "a launcher that keeps dying at once ends the session"
    run = gamescope("dies", "--session")
    assert (run.code, len(run.gamescopes)) == (1, 1), "no desktop to fall back to: the display manager shows its greeter again"
    assert subprocess.run([str(LAUNCHER), "--session", "--windowed"], capture_output=True, check=False).returncode == 2


def test_a_gamescope_that_shows_nothing_is_stopped(gamescope, monkeypatch):
    monkeypatch.setenv("XDG_RUNTIME_DIR", gamescope.env["XDG_RUNTIME_DIR"])
    monkeypatch.setenv("FAKE_GAMESCOPE", "hangs")
    assert host.run_in_gamescope(gamescope.command, [], ready_s=0.1) is None, "the launcher opens on the desktop instead"
    assert host.run_session(gamescope.command, [], ready_s=0.1) == 1


def test_a_lost_display_ends_the_launcher_at_once(gamescope, tmp_path):
    if not shutil.which("Xvfb"):
        pytest.skip("needs Xvfb")
    server = subprocess.Popen(["Xvfb", "-displayfd", "1", "-nolisten", "tcp"], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    display = ":" + server.stdout.readline().decode().strip()
    env = {**gamescope.env, "DISPLAY": display, "QT_QPA_PLATFORM": "xcb"}
    up = tmp_path / "up.png"
    ui = subprocess.Popen([str(LAUNCHER), "--windowed", "--no-gamepad", "--key-delay", "0", "--keys", f"Shot:{up}"], env=env, stderr=subprocess.DEVNULL)
    try:
        deadline = time.monotonic() + 10
        while not up.exists() and ui.poll() is None and time.monotonic() < deadline:
            time.sleep(0.02)
        assert up.exists(), "the launcher came up on the display"
    finally:
        server.kill()
        server.wait()
    assert ui.wait(timeout=5) == 0, "out at once on the X error, with no destructors run"


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


def run_script(tmp_path, script, restarts=0):
    relaunch = tmp_path / "universe-ui"
    relaunch.write_text('#!/bin/sh\necho "relaunched $$ $* $UNIVERSE_UI_RESTARTS"\n')
    relaunch.chmod(0o755)
    env = {**os.environ, "QT_QPA_PLATFORM": "offscreen", "PYTHONPATH": os.pathsep.join(sys.path), host.RESTARTS_ENV: str(restarts)}
    proc = subprocess.Popen([sys.executable, "-c", script.format(relaunch=str(relaunch))], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    out, err = proc.communicate(timeout=20)
    return SimpleNamespace(pid=proc.pid, code=proc.returncode, out=out, err=err, child=int(out.split()[out.split().index("child") + 1]))


def test_a_loop_wedged_on_a_dead_notifier_restarts_in_place_until_it_keeps_wedging(tmp_path):
    run = run_script(tmp_path, WEDGE)
    assert run.out.rstrip().endswith(f"relaunched {run.pid} --theme switch2 1"), (
        "the same process, the same arguments, one restart counted\n" + run.out + run.err
    )
    assert "Socket notifiers cannot be enabled or disabled from another thread" in run.err and "libQt6Core" in run.err, (
        "the thread that left the notifier behind is logged with its native stack"
    )
    assert not Path(f"/proc/{run.child}").exists(), "the old image's children are killed and reaped, not left as zombies of the new one"
    run = run_script(tmp_path, WEDGE, restarts=host.MAX_RESTARTS)
    os.kill(run.child, 9)
    assert run.code == 1 and "relaunched" not in run.out, run.out + run.err


def test_a_dead_socket_its_notifier_still_owns_is_no_wedge(tmp_path):
    healed = WEDGE.replace("QTimer.singleShot(0, stale.start)", "QTimer.singleShot(0, lambda: os.close(reader))").replace("5000", "500")
    run = run_script(tmp_path, healed)
    os.kill(run.child, 9)
    assert run.err.count("QSocketNotifier: Invalid socket") == 1 and "still here" in run.out and "relaunched" not in run.out, (
        "Qt disables it on the owning thread and the loop carries on\n" + run.out + run.err
    )


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
    client = SimpleNamespace(currentSession=session, underSteam=steam)
    assert host.boot_wanted(host.parse_args(argv), client, restarts) is wanted


@pytest.mark.slow
@pytest.mark.speed
def test_a_thousand_game_library_opens_on_home_within_its_budget(tmp_path, fixture_art, speed_limit):
    from universe_ui.fake_core import FIXTURE
    from universe_ui.fixtures.art import SLOTS

    painted = Path(os.environ["XDG_CACHE_HOME"]) / "universe" / "fake-art"
    art = tmp_path / "cache" / "universe" / "fake-art"
    art.mkdir(parents=True)
    for f in painted.iterdir():
        os.link(f, art / f.name)
    library = json.loads(Path(FIXTURE).read_text())
    shown = [g for g in library["games"] if not g.get("hidden")]
    # The fixture's own games keep their ids, and with them their sessions and journal; the rest share their art.
    games = [*library["games"]]
    for n in range(len(games), 1000):
        template = shown[n % len(shown)]
        ident = f"{template['id']}-{n}"
        games.append({**copy.deepcopy(template), "id": ident, "title": f"{template['title']} {n}", "favorite": False})
        for name in [f"-{slot}.png" for slot in SLOTS if slot != "screenshot"] + ["-screenshot1.png", "-screenshot2.png"]:
            os.link(painted / f"{template['id']}{name}", art / f"{ident}{name}")
    (tmp_path / "library.json").write_text(json.dumps({**library, "games": games}))
    env = {
        **os.environ,
        "QT_QPA_PLATFORM": "offscreen",
        "PYTHONPATH": os.pathsep.join(sys.path),
        "HOME": str(tmp_path),
        "XDG_CACHE_HOME": str(tmp_path / "cache"),
        "UNIVERSE_FAKE_LIBRARY": str(tmp_path / "library.json"),
    }
    argv = ["--fake", "--no-gamepad", "--size", "1280x720", "--key-delay", "0", "--key-gap", "10", "--keys", "Ready Mark:home Quit", "--quit-after", "60000"]
    started = time.time()
    run = subprocess.run(
        [sys.executable, "-c", "from universe_ui import host; host.main()", *argv], env=env, capture_output=True, text=True, timeout=90, check=False
    )
    marks = [line.split()[-1] for line in run.stderr.splitlines() if line.startswith("universe.gamepad: mark home ")]
    assert run.returncode == 0 and marks, run.stderr
    took = float(marks[0]) - started
    assert took < speed_limit(5, 15), f"home took {took:.2f} s"
