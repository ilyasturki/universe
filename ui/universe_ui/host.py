import argparse
import logging
import os
import signal
import socket
import sys
from pathlib import Path

QML_DIR = Path(__file__).parent / "qml"


def parse_args(argv):
    parser = argparse.ArgumentParser(prog="universe-ui", description="Universe game launcher UI")
    parser.add_argument("--fake", action="store_true", help="fixture library, no core")
    parser.add_argument("--fake-launch", action="store_true", help="fake session runs `sleep 2` (implies --fake)")
    parser.add_argument("--windowed", action="store_true", help="a window instead of fullscreen (--size implies it)")
    parser.add_argument("--quit-after", type=int, default=0, metavar="MS", help="quit after MS (0 = never)")
    parser.add_argument("--no-gamepad", action="store_true")
    parser.add_argument("--keys", default="", metavar="LIST",
                        help="key names to post once loaded, e.g. 'Right Right Return' (Wait idles one gap)")
    parser.add_argument("--key-gap", type=int, default=120, metavar="MS")
    parser.add_argument("--key-delay", type=int, default=1200, metavar="MS", help="delay before the first key")
    parser.add_argument("--size", metavar="WxH", help="window size, implies --windowed (default 1920x1080)")
    parser.add_argument("--theme", default="", metavar="ID", help="the look for this run: reprise or switch2")
    args = parser.parse_args(argv)
    args.fullscreen = not (args.windowed or args.size)
    args.size = args.size or "1920x1080"
    return args


def build_client(args):
    from .universe_client import CoreClient

    if args.fake or args.fake_launch:
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
    notifier.sockets = (reader, writer)

    def drain():
        try:
            reader.recv(64)
        except OSError:
            pass

    notifier.activated.connect(drain)
    for sig in (signal.SIGINT, signal.SIGTERM):
        signal.signal(sig, lambda signum, frame: on_signal())
    return notifier


def run(argv=None):
    args = parse_args(sys.argv[1:] if argv is None else argv)
    os.environ.setdefault("QT_FORCE_STDERR_LOGGING", "1")
    # With a desktop file name set, Qt's portal app-id registration warns when the process already has one.
    os.environ.setdefault("QT_LOGGING_RULES", "qt.multimedia.ffmpeg.info=false;qt.qpa.services.warning=false")
    # Probing VDPAU makes libvdpau try its nvidia fallback and complain on stderr when no driver is installed.
    os.environ.setdefault("QT_FFMPEG_DECODING_HW_DEVICE_TYPES", "vaapi")
    os.environ.setdefault("QT_FFMPEG_ENCODING_HW_DEVICE_TYPES", "vaapi")
    logging.basicConfig(level=logging.INFO, format="%(name)s: %(message)s")

    from PySide6.QtCore import Qt, QTimer, QUrl
    from PySide6.QtGui import QGuiApplication
    from PySide6.QtQml import QQmlApplicationEngine
    from PySide6.QtQuick import QQuickWindow  # noqa: F401  (down-casts rootObjects() so grabWindow exists)

    app = QGuiApplication(sys.argv[:1])
    app.setApplicationName("universe-ui")
    app.setOrganizationName("universe")
    app.setDesktopFileName("universe-ui")

    from . import models  # noqa: F401  (registers the Universe QML module)
    from .api import Api

    client = build_client(args)
    if not (args.fake or args.fake_launch):
        client.adoptScope()
    api = Api(client, fullscreen=args.fullscreen, theme=args.theme, parent=app)
    quit_on_signals(app)

    engine = QQmlApplicationEngine()
    engine.rootContext().setContextProperty("api", api)
    engine.load(QUrl.fromLocalFile(str(QML_DIR / "main.qml")))
    if not engine.rootObjects():
        print("universe-ui: main.qml failed to load", file=sys.stderr)
        return 1
    window = engine.rootObjects()[0]
    api.attachWindow(window)
    if not args.fullscreen:
        try:
            w, h = (int(v) for v in args.size.lower().split("x"))
            window.setWidth(w)
            window.setHeight(h)
        except ValueError:
            pass

    gamepad = None
    if not args.no_gamepad:
        from .gamepad import GamepadThread
        from .screens.controller import FakeWatcher, Watcher

        gamepad = GamepadThread(app, pad=api.pad)
        gamepad.stick.connect(api.pad.set, Qt.ConnectionType.QueuedConnection)
        gamepad.start()
        if args.fake or args.fake_launch:
            unbound = [s for s in os.environ.get("UNIVERSE_FAKE_UNBOUND", "").split(",") if s]
            watcher = FakeWatcher(os.environ.get("UNIVERSE_FAKE_PAD") or "dualsense-edge", unbound, parent=app)
        else:
            watcher = Watcher(app)
        api.screens.controller.start(watcher)

    if args.keys:
        from .gamepad import KeyScript

        # Keys only reach an active window; a bare X server hands focus to nobody by itself.
        window.requestActivate()
        fake_pad = watcher if gamepad is not None and (args.fake or args.fake_launch) else None
        KeyScript(args.keys, args.key_gap, window, pad=api.pad, watcher=fake_pad, parent=app).start(args.key_delay)

    if args.quit_after > 0:
        QTimer.singleShot(args.quit_after, app.quit)

    rc = app.exec()
    if gamepad is not None:
        gamepad.stop()
    # The scope is ours now: the game goes with the launcher, and stopping it first lets session-end run.
    if client.currentSession:
        client.stopNow("")
    api.shutdown()
    return rc


def main():
    sys.exit(run())
