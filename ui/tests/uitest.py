import inspect


def pump(ms):
    from PySide6.QtCore import QEventLoop, QTimer

    loop = QEventLoop()
    QTimer.singleShot(ms, loop.quit)
    loop.exec()


def until(predicate, message="", timeout_ms=5000):
    """Runs the event loop until `predicate()` is truthy and returns that value, failing once `timeout_ms` passes without it."""
    from PySide6.QtCore import QDeadlineTimer

    deadline = QDeadlineTimer(timeout_ms)
    while not (value := predicate()):
        if deadline.hasExpired():
            raise AssertionError(message or f"never held within {timeout_ms} ms: {_source(predicate)}")
        pump(5)
    return value


def _source(fn):
    try:
        return inspect.getsource(fn).strip()
    except (OSError, TypeError):
        return repr(fn)


def record(signal):
    """Every emission of `signal` from here on, as its argument tuples."""
    seen = []
    signal.connect(lambda *args: seen.append(args))
    return seen


def settle(screen, timeout_ms=5000):
    until(lambda: not screen.busy, "still busy", timeout_ms)


def own(fake, key, game="the-technomancer"):
    """The game's own launch value for `key`, None while it follows every game's."""
    value = (fake.game(game).get("launch") or {}).get(key)
    return None if value in ("", None) else value


def index_of(form, key):
    return next(i for i, r in enumerate(form.rows) if r["key"] == key)


def rows_by_key(form, module=None):
    return {r["key"]: r for r in form.rows if module is None or r["module"] == module}
