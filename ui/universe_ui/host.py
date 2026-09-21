import argparse
import contextlib
import logging
import os
import shutil
import signal
import socket
import sys
from pathlib import Path
from typing import TYPE_CHECKING, cast

if TYPE_CHECKING:
    from PySide6.QtQuick import QQuickWindow

QML_DIR = Path(__file__).parent / "qml"


def parse_args(argv):
    parser = argparse.ArgumentParser(prog="universe-ui", description="Universe game launcher UI")
    parser.add_argument("--fake", action="store_true", help="fixture library, no core")
    parser.add_argument("--fake-launch", action="store_true", help="fake session runs `sleep 2` (implies --fake)")
    parser.add_argument("--windowed", action="store_true", help="a window instead of fullscreen (--size implies it)")
    parser.add_argument("--quit-after", type=int, default=0, metavar="MS", help="quit after MS (0 = never)")
    parser.add_argument("--no-gamepad", action="store_true")
    parser.add_argument("--keys", default="", metavar="LIST", help="key names to post once loaded, e.g. 'Right Right Return' (Wait idles one gap)")
    parser.add_argument("--key-gap", type=int, default=120, metavar="MS")
    parser.add_argument("--key-delay", type=int, default=1200, metavar="MS", help="delay before the first key")
    parser.add_argument("--size", metavar="WxH", help="window size, implies --windowed (default 1920x1080)")
    parser.add_argument("--theme", default="", metavar="ID", help="the look for this run: reprise or switch2")
    args = parser.parse_args(argv)
    args.fake = args.fake or args.fake_launch
    args.fullscreen = not (args.windowed or args.size)
    args.size = args.size or "1920x1080"
    return args


def build_client(args):
    from .universe_client import CoreClient

    if args.fake:
        from .fake_core import FakeCore

        return CoreClient(FakeCore(fake_launch=args.fake_launch))
    import universe_core

    return CoreClient(universe_core.Core())


def quit_on_signals(app, on_signal=None):
    from PySide6.QtCore import QSocketNotifier

    on_signal = on_signal or app.quit
    # Python runs a handler only between bytecodes: the wakeup fd makes Qt call into Python at once.
    reader, writer = socket.socketpair()
    reader.setblocking(False)
    writer.setblocking(False)
    signal.set_wakeup_fd(writer.fileno())
    notifier = QSocketNotifier(reader.fileno(), QSocketNotifier.Type.Read, app)
    notifier.setProperty("sockets", (reader, writer))

    def drain():
        with contextlib.suppress(OSError):
            reader.recv(64)

    notifier.activated.connect(drain)
    for sig in (signal.SIGINT, signal.SIGTERM):
        signal.signal(sig, lambda signum, frame: on_signal())
    return notifier


def exec_in_gamescope(command, argv):
    if not command:
        logging.getLogger("universe.host").warning("no gamescope: running on the desktop")
        return
    # argv[0] is the installed launcher: on Nix a compiled wrapper, not a script for the interpreter.
    launcher = shutil.which(sys.argv[0])
    launcher = [launcher] if launcher else [sys.executable, sys.argv[0]]
    # gamescope's CAP_SYS_NICE wrapper runs secure: the loader drops LD_LIBRARY_PATH before its children see it.
    libs = os.environ.get("LD_LIBRARY_PATH")
    if libs:
        launcher = [shutil.which("env") or "env", f"LD_LIBRARY_PATH={libs}", *launcher]
    os.execv(command[0], [*command, "--", *launcher, *argv])


def create_overlay(engine, size):
    from PySide6.QtCore import QUrl

    before = len(engine.rootObjects())
    engine.load(QUrl.fromLocalFile(str(QML_DIR / "overlay.qml")))
    window = engine.rootObjects()[before] if len(engine.rootObjects()) > before else None
    if window is not None:
        window.setGeometry(0, 0, size.width(), size.height())
    return window


def run(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    args = parse_args(argv)
    os.environ.setdefault("QT_FORCE_STDERR_LOGGING", "1")
    # With a desktop file name set, Qt's portal app-id registration warns when the process already has one.
    os.environ.setdefault("QT_LOGGING_RULES", "qt.multimedia.ffmpeg.info=false;qt.qpa.services.warning=false")
    # Probing VDPAU makes libvdpau try its nvidia fallback and complain on stderr when no driver is installed.
    os.environ.setdefault("QT_FFMPEG_DECODING_HW_DEVICE_TYPES", "vaapi")
    os.environ.setdefault("QT_FFMPEG_ENCODING_HW_DEVICE_TYPES", "vaapi")
    # Qt Multimedia 6.11's PipeWire backend can destroy a main-thread QSocketNotifier from its loop thread, which wedges the event loop for good.
    os.environ.setdefault("QT_AUDIO_BACKEND", "pulseaudio")
    logging.basicConfig(level=logging.INFO, format="%(name)s: %(message)s")

    import PySide6.QtQuick  # noqa: F401  before rootObjects(): the wrapper is otherwise a bare QWindow, no grabWindow
    from PySide6.QtCore import Qt, QTimer, QUrl
    from PySide6.QtGui import QGuiApplication
    from PySide6.QtQml import QQmlApplicationEngine

    nested = bool(os.environ.get("GAMESCOPE_WAYLAND_DISPLAY"))
    if nested:
        # gamescope unsets WAYLAND_DISPLAY; a platform list naming wayland first would still try it.
        os.environ["QT_QPA_PLATFORM"] = "xcb"
    app = QGuiApplication(sys.argv[:1])
    app.setApplicationName("universe-ui")
    app.setOrganizationName("universe")
    app.setDesktopFileName("universe-ui")

    from . import models  # noqa: F401  (registers the Universe QML module)
    from .api import Api
    from .screens.power import FAKE as FAKE_POWER

    # Re-exec before the core opens: the library is loaded once, inside gamescope, not once on each side of it.
    client = build_client(args) if args.fake else None
    if args.fullscreen and not nested:
        if client is not None:
            command = client.hostGamescope("")
        else:
            import universe_core

            command = universe_core.host_gamescope("")
        exec_in_gamescope(command, argv)
    if client is None:
        client = build_client(args)
    if not args.fake:
        client.adoptScope()
    api = Api(client, fullscreen=args.fullscreen, theme=args.theme, power_root=FAKE_POWER if args.fake else None, parent=app)
    quit_on_signals(app)

    engine = QQmlApplicationEngine()
    engine.rootContext().setContextProperty("api", api)
    engine.load(QUrl.fromLocalFile(str(QML_DIR / "main.qml")))
    if not engine.rootObjects():
        print("universe-ui: main.qml failed to load", file=sys.stderr)
        return 1
    window = cast("QQuickWindow", engine.rootObjects()[0])
    api.attachWindow(window)
    if not args.fullscreen:
        try:
            w, h = (int(v) for v in args.size.lower().split("x"))
            window.setWidth(w)
            window.setHeight(h)
        except ValueError:
            pass
    if client.nested:
        overlay = create_overlay(engine, window.screen().size())
        if overlay is not None:
            api.keys.watch(overlay)
            api.home.attachOverlay(overlay)

    gamepad = watcher = None
    if not args.no_gamepad:
        from .gamepad import GamepadThread
        from .screens.controller import FakeWatcher, Watcher

        gamepad = GamepadThread(app, pad=api.pad)
        gamepad.stick.connect(api.pad.set, Qt.ConnectionType.QueuedConnection)
        api.home.changed.connect(lambda: gamepad.setCovered(api.home.padCovered))
        gamepad.start()
        if args.fake:
            unbound = [s for s in os.environ.get("UNIVERSE_FAKE_UNBOUND", "").split(",") if s]
            watcher = FakeWatcher(os.environ.get("UNIVERSE_FAKE_PAD") or "dualsense-edge", unbound, parent=app)
        else:
            watcher = Watcher(app)
        api.screens.controller.start(watcher)

    if args.keys:
        from .gamepad import KeyScript

        # Keys only reach an active window; a bare X server hands focus to nobody by itself.
        window.requestActivate()
        KeyScript(args.keys, args.key_gap, window, pad=api.pad, watcher=watcher if args.fake else None, home=api.home, parent=app).start(args.key_delay)

    if args.quit_after > 0:
        QTimer.singleShot(args.quit_after, app.quit)

    rc = app.exec()
    if gamepad is not None:
        gamepad.stop()
    if client.currentSession:
        client.stopNow("")
    api.shutdown()
    return rc


def main():
    sys.exit(run())
