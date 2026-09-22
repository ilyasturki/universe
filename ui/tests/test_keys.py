from PySide6.QtCore import QEvent, QPointF, Qt
from PySide6.QtGui import QKeyEvent, QMouseEvent, QWindow

from universe_ui import gamepad
from universe_ui.api import KEYS, Keys


def key_event(key, pressed=True, text=""):
    return QKeyEvent(QEvent.Type.KeyPress if pressed else QEvent.Type.KeyRelease, key, Qt.KeyboardModifier.NoModifier, text)


def mouse_move():
    p = QPointF(10, 10)
    return QMouseEvent(QEvent.Type.MouseMove, p, p, p, Qt.MouseButton.NoButton, Qt.MouseButton.NoButton, Qt.KeyboardModifier.NoModifier)


def test_posted_keys_remember_who_posted_them(app):
    window = QWindow()
    gamepad.POSTED.clear()
    assert gamepad.post_key(Qt.Key.Key_Return, True, window=window)
    assert gamepad.post_key(Qt.Key.Key_Return, True, window=window, source="pointer")
    assert gamepad.posted_source(Qt.Key.Key_Return, True) == "pad"
    assert gamepad.posted_source(Qt.Key.Key_Return, True) == "pointer"
    assert gamepad.posted_source(Qt.Key.Key_Return, True) is None, "the keyboard's own presses are not on the list"
    assert gamepad.posted_source(Qt.Key.Key_Return, False) is None
    app.processEvents()


def test_the_mode_follows_the_last_device(app):
    keys = Keys()
    window = QWindow()
    keys.watch(window)
    modes = []
    keys.modeChanged.connect(lambda: modes.append(keys.mode))
    assert keys.mode == "pad"
    assert window.cursor().shape() == Qt.CursorShape.BlankCursor

    keys.eventFilter(window, key_event(Qt.Key.Key_I, text="i"))
    assert keys.mode == "keyboard"

    keys.eventFilter(window, mouse_move())
    assert keys.mode == "mouse"
    assert window.cursor().shape() == Qt.CursorShape.ArrowCursor

    gamepad.POSTED.clear()
    gamepad.POSTED[(int(Qt.Key.Key_Return), True)] = ["pad"]
    keys.eventFilter(window, key_event(Qt.Key.Key_Return))
    assert keys.mode == "pad", "a key the pad posted is the pad's"
    assert window.cursor().shape() == Qt.CursorShape.BlankCursor

    gamepad.POSTED[(int(Qt.Key.Key_Down), True)] = ["pointer"]
    keys.eventFilter(window, key_event(Qt.Key.Key_Down))
    assert keys.mode == "pad", "a key a click or the wheel posted changes nothing"
    assert modes == ["keyboard", "mouse", "pad"]
    assert not gamepad.POSTED


def test_a_key_without_a_keysym_is_not_the_keyboard(app):
    keys = Keys()
    window = QWindow()
    keys.watch(window)
    gamepad.POSTED.clear()
    for key in (Qt.Key(0), Qt.Key.Key_unknown):
        keys.eventFilter(window, key_event(key))
        keys.eventFilter(window, key_event(key, pressed=False))
        assert keys.mode == "pad", "InputPlumber's keyboard sends KEY_UNKNOWN for every pad button: it is not typing"
    keys.eventFilter(window, key_event(Qt.Key.Key_I, text="i"))
    assert keys.mode == "keyboard"


def test_backspace_cancels_but_holding_it_does_not_ask_to_quit(app):
    keys = Keys()
    window = QWindow()
    keys.watch(window)
    assert Qt.Key.Key_Backspace in KEYS["Cancel"]
    keys.eventFilter(window, key_event(Qt.Key.Key_Backspace))
    assert not keys._hold.isActive()
    keys.eventFilter(window, key_event(Qt.Key.Key_Escape))
    assert keys._hold.isActive()
    keys.eventFilter(window, key_event(Qt.Key.Key_Escape, pressed=False))
    assert not keys._hold.isActive()


def test_hint_labels_name_the_keys(app):
    labels = Keys().labels
    assert labels["A"] == "Enter"
    assert labels["B"] == "Esc"
    assert labels["X"] == "I"
    assert labels["LB"] == "Q"
    assert labels["LT"] == "PgUp"
    assert labels["Start"] == "F1"
