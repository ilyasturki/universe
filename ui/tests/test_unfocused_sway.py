"""The launcher on the real core in a headless sway, beside another app's window: what the pad does while that window has the focus."""

import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

import pytest

LAUNCHER = Path(sys.executable).with_name("universe-ui")
ESCAPE = 0x01000000
OTHER = """
import sys
from PySide6.QtGui import QColor, QGuiApplication, QPainter, QRasterWindow

class Window(QRasterWindow):
    def paintEvent(self, event):
        QPainter(self).fillRect(0, 0, self.width(), self.height(), QColor("teal"))

app = QGuiApplication(sys.argv[:1])
app.setDesktopFileName("other")
window = Window()
window.setTitle("other")
window.resize(640, 480)
window.show()
app.exec()
"""
# One gap is 100 ms: B held a second while the other window has the focus, HOME, then B held again once the launcher has it.
SCRIPT = "PadHold:B Wait:10 PadRelease:B Press:guide Unpress:guide Wait:30 PadHold:B Wait:10 PadRelease:B Wait:5"
KEY_DELAY_MS = 6000
QUIT_AFTER_MS = KEY_DELAY_MS + 8000


def render_node():
    for card in sorted(Path("/sys/class/drm").glob("card[0-9]*")):
        if (card / "device/boot_vga").is_file() and (card / "device/boot_vga").read_text().strip() == "1":
            nodes = sorted((card / "device/drm").glob("renderD*"))
            if nodes:
                return f"/dev/dri/{nodes[-1].name}"
    return None


class Sway:
    def __init__(self, env):
        self.env = env

    def msg(self, *args):
        return subprocess.run(["swaymsg", *args], env=self.env, capture_output=True, text=True, timeout=5).stdout

    def windows(self):
        found = []

        def walk(node):
            if node.get("pid") and node.get("type") in ("con", "floating_con"):
                found.append({"app_id": node.get("app_id") or "", "name": node.get("name") or "", "focused": bool(node.get("focused"))})
            for child in node.get("nodes", []) + node.get("floating_nodes", []):
                walk(child)

        walk(json.loads(self.msg("-r", "-t", "get_tree") or "{}"))
        return found

    def focused(self):
        return next((w["app_id"] for w in self.windows() if w["focused"]), "")

    def wait(self, predicate, what, timeout=20):
        deadline = time.monotonic() + timeout
        while not predicate():
            if time.monotonic() > deadline:
                raise AssertionError(f"{what}: {self.windows()}")
            time.sleep(0.05)


@pytest.fixture
def sway(tmp_path):
    if not shutil.which("sway") or not LAUNCHER.exists() or not os.environ.get("XDG_RUNTIME_DIR"):
        pytest.skip("needs sway, the installed universe-ui and a runtime dir")
    ready = tmp_path / "ready"
    config = tmp_path / "sway.cfg"
    config.write_text(f'output HEADLESS-1 resolution 1920x1080\nexec printf \'%s %s\\n\' "$WAYLAND_DISPLAY" "$SWAYSOCK" > {ready}\n')
    gone = ("WAYLAND_DISPLAY", "DISPLAY", "SWAYSOCK", "HYPRLAND_INSTANCE_SIGNATURE", "NIRI_SOCKET", "XDG_CURRENT_DESKTOP", "GAMESCOPE_WAYLAND_DISPLAY")
    env = {k: v for k, v in os.environ.items() if k not in gone}
    env.update(WLR_BACKENDS="headless", WLR_LIBINPUT_NO_DEVICES="1")
    node = render_node()
    if node:
        env["WLR_RENDER_DRM_DEVICE"] = node
    else:
        env["WLR_RENDERER"] = "pixman"
    with open(tmp_path / "sway.log", "w") as log:
        proc = subprocess.Popen(["sway", "-c", str(config)], env=env, stdout=log, stderr=subprocess.STDOUT)
    deadline = time.monotonic() + 10
    while not (ready.exists() and ready.read_text().strip()):
        if proc.poll() is not None or time.monotonic() > deadline:
            proc.kill()
            pytest.skip(f"no headless sway here: {(tmp_path / 'sway.log').read_text()[-400:]}")
        time.sleep(0.05)
    display, sock = ready.read_text().split()
    env.update(WAYLAND_DISPLAY=display, SWAYSOCK=sock, QT_QPA_PLATFORM="wayland")
    session = Sway(env)
    session.gpu = node
    yield session
    session.msg("exit")
    try:
        proc.wait(5)
    except subprocess.TimeoutExpired:
        proc.kill()


def profile(tmp_path):
    env = {}
    for name in ("DATA", "CONFIG", "STATE", "CACHE"):
        path = tmp_path / "profile" / name.lower()
        path.mkdir(parents=True)
        env[f"UNIVERSE_{name}_HOME"] = str(path)
    (tmp_path / "profile/config/config.toml").write_text("schema = 1\n[modules]\nenabled = []\n[sources]\nenabled = []\n")
    # No watcher reads the machine's pads: HOME comes from the script.
    env.update(UNIVERSE_UI_INPUT_LOG="1", UNIVERSE_BIN=shutil.which("true") or "/bin/true", PIPEWIRE_REMOTE="/nonexistent")
    return env


@pytest.mark.parametrize("nested", [False, True], ids=["desktop", "nested"])
def test_a_pad_press_reaches_neither_the_launcher_nor_its_power_menu_while_another_app_has_the_focus(sway, tmp_path, nested):
    if nested and not (sway.gpu and shutil.which("gamescope")):
        pytest.skip("gamescope needs a GPU and its binary")
    other = subprocess.Popen([sys.executable, "-c", OTHER], env=sway.env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    launcher = None
    try:
        sway.wait(lambda: any(w["name"] == "other" for w in sway.windows()), "the other window maps")
        flags = [] if nested else ["--windowed"]
        script = ["--key-delay", str(KEY_DELAY_MS), "--key-gap", "100", "--keys", SCRIPT, "--quit-after", str(QUIT_AFTER_MS)]
        launcher = subprocess.Popen(
            [str(LAUNCHER), *flags, *script], env={**sway.env, **profile(tmp_path)}, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True
        )
        app_id = "gamescope" if nested else "universe-ui"
        sway.wait(lambda: sway.focused() == app_id, "the launcher maps and takes the focus", timeout=30)
        sway.msg('[title="^other$"] focus')
        sway.wait(lambda: sway.focused() == "other", "the other window takes the focus back")
        sway.wait(lambda: sway.focused() == app_id, "HOME brings the launcher up", timeout=QUIT_AFTER_MS / 1000)
        out, _ = launcher.communicate(timeout=30)
    finally:
        for proc in (launcher, other):
            if proc is not None and proc.poll() is None:
                proc.kill()
    lines = out.splitlines()
    mode = "host" if nested else "qt"
    if nested and not any(line.endswith("focus: host") for line in lines):
        pytest.skip("gamescope did not come up here: " + " | ".join(lines[-5:]))
    assert any(line.endswith(f"focus: {mode}") for line in lines), out
    dropped = [i for i, line in enumerate(lines) if f"pad key {ESCAPE} dropped" in line]
    held = [i for i, line in enumerate(lines) if line.endswith("cancel held")]
    assert dropped, f"B never reached the gate while the other window had the focus:\n{out}"
    assert len(held) == 1 and held[0] > dropped[-1], f"the power menu opened once, for the B held after HOME:\n{out}"
