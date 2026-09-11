"""QGuiApplication + QQmlApplicationEngine around the theme, with the flags the tests and the
nix check drive it by."""

import argparse
import glob
import logging
import os
import subprocess
import sys
from pathlib import Path

QML_DIR = Path(__file__).parent / "qml"
STORE_QT_MODULES = ("QtQml", "QtQuick", "QtMultimedia", "QtGui", "QtCore")


def _store_dirs_linked_by(so_path):
    try:
        out = subprocess.run(["ldd", so_path], capture_output=True, text=True).stdout
    except OSError:
        return set()
    dirs = set()
    for line in out.splitlines():
        if "=>" not in line:
            continue
        target = line.split("=>")[1].strip().split(" ")[0]
        if target.startswith("/nix/store/") and "/lib/" in target:
            dirs.add(target.split("/lib/")[0])
    return dirs


def qt_store_dirs():
    """Nix store derivations PySide6 links against, plus the qt5compat and qtsvg builds that
    share its qtbase (several coexist in the store; a plugin against another qtbase is refused)."""
    import PySide6

    pydir = os.path.dirname(PySide6.__file__)
    dirs = set()
    for mod in STORE_QT_MODULES:
        so = os.path.join(pydir, f"{mod}.abi3.so")
        if os.path.exists(so):
            dirs |= _store_dirs_linked_by(so)
    bases = {d for d in dirs if "-qtbase-" in d}
    for name, lib in (("qt5compat", "libQt6Core5Compat.so.6"), ("qtsvg", "libQt6Svg.so.6")):
        for d in glob.glob(f"/nix/store/*-{name}-6.*"):
            base = os.path.basename(d)
            if base.endswith(("-dev", "-debug", ".drv")) or "src" in base:
                continue
            so = os.path.join(d, "lib", lib)
            if os.path.exists(so) and _store_dirs_linked_by(so) & bases:
                dirs.add(d)
    return sorted(dirs)


def qt_paths_unset():
    return not os.environ.get("QML2_IMPORT_PATH") and not os.environ.get("QML_IMPORT_PATH")


def qml_import_paths():
    return [p for p in (os.path.join(d, "lib", "qt-6", "qml") for d in qt_store_dirs()) if os.path.isdir(p)]


def qt_plugin_paths():
    return [p for p in (os.path.join(d, "lib", "qt-6", "plugins") for d in qt_store_dirs()) if os.path.isdir(p)]


def parse_args(argv):
    parser = argparse.ArgumentParser(prog="universe-ui", description="Universe game launcher UI")
    parser.add_argument("--fake", action="store_true", help="fixture library, no daemon")
    parser.add_argument("--fake-launch", action="store_true", help="fake session runs `sleep 2` (implies --fake)")
    parser.add_argument("--fullscreen", action="store_true")
    parser.add_argument("--screenshot", metavar="PATH", help="grab the window to PATH, then quit")
    parser.add_argument("--after", type=int, default=3000, metavar="MS", help="delay before --screenshot")
    parser.add_argument("--quit-after", type=int, default=0, metavar="MS", help="quit after MS (0 = never)")
    parser.add_argument("--no-gamepad", action="store_true")
    parser.add_argument("--keys", default="", metavar="LIST",
                        help="key names to post once loaded, e.g. 'Right Right Return' (Wait idles one gap)")
    parser.add_argument("--key-gap", type=int, default=120, metavar="MS")
    parser.add_argument("--key-delay", type=int, default=1200, metavar="MS", help="delay before the first key")
    parser.add_argument("--size", default="1920x1080", help="window size when not fullscreen")
    return parser.parse_args(argv)


def build_client(args):
    if args.fake or args.fake_launch:
        from .universe_client import FakeClient

        return FakeClient(fake_launch=args.fake_launch)
    from .universe_client import UniverseClient

    return UniverseClient()


def run(argv=None):
    args = parse_args(sys.argv[1:] if argv is None else argv)
    os.environ.setdefault("QT_FORCE_STDERR_LOGGING", "1")
    logging.basicConfig(level=logging.INFO, format="%(name)s: %(message)s")

    from PySide6.QtCore import QCoreApplication, QTimer, QUrl
    from PySide6.QtGui import QGuiApplication
    from PySide6.QtQml import QQmlApplicationEngine
    from PySide6.QtQuick import QQuickWindow  # noqa: F401  (down-casts rootObjects() so grabWindow exists)

    # The nix shell sets no Qt paths at all; the flake's wrapQtAppsHook will, so only fill the gap.
    if qt_paths_unset():
        for p in qt_plugin_paths():
            QCoreApplication.addLibraryPath(p)
        import_paths = qml_import_paths()
    else:
        import_paths = []

    app = QGuiApplication(sys.argv[:1])
    app.setApplicationName("universe-ui")
    app.setOrganizationName("universe")

    from . import models  # noqa: F401  (registers the Universe QML module)
    from .api import Api

    client = build_client(args)
    api = Api(client, fullscreen=args.fullscreen, parent=app)

    engine = QQmlApplicationEngine()
    for p in import_paths:
        engine.addImportPath(p)
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

        gamepad = GamepadThread(app)
        gamepad.start()

    exit_code = {"value": 0}

    if args.keys:
        from .gamepad import KeyScript

        KeyScript(args.keys, args.key_gap, window, parent=app).start(args.key_delay)

    def grab():
        image = window.grabWindow()
        ok = image.save(args.screenshot)
        print(f"universe-ui: screenshot {'saved' if ok else 'FAILED'} {image.width()}x{image.height()} {args.screenshot}")
        exit_code["value"] = 0 if ok else 2
        app.quit()

    if args.screenshot:
        QTimer.singleShot(args.after, grab)
    if args.quit_after > 0:
        QTimer.singleShot(args.quit_after, app.quit)

    rc = app.exec()
    if gamepad is not None:
        gamepad.stop()
    api.shutdown()
    return rc or exit_code["value"]
