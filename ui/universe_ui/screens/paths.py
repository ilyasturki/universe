import os

from PySide6.QtCore import Property, QObject, Signal, Slot


def _expand(path):
    return os.path.abspath(os.path.expanduser(str(path or "") or "~"))


def universe_home(kind, fallback):
    """$UNIVERSE_<KIND>_HOME, else $XDG_<KIND>_HOME/universe, else ~/<fallback>/universe — as universe(1) resolves them."""
    base = os.environ.get(f"UNIVERSE_{kind}_HOME")
    if base:
        return base
    xdg = os.environ.get(f"XDG_{kind}_HOME") or os.path.join(os.path.expanduser("~"), *fallback.split("/"))
    return os.path.join(xdg, "universe")


def _mounts():
    out = []
    for root in ("/mnt", os.path.join("/run/media", os.environ.get("USER", ""))):
        try:
            names = sorted(os.listdir(root), key=str.casefold)
        except OSError:
            continue
        out.extend(os.path.join(root, n) for n in names if not n.startswith(".") and os.path.isdir(os.path.join(root, n)))
    return out


class PathBrowser(QObject):
    changed = Signal()

    def __init__(self, client, parent=None):
        super().__init__(parent)
        self._client = client
        self._path = _expand("~")
        self._files = False
        self._entries = []
        self._shortcuts = []

    def _load_shortcuts(self):
        config = self._client.config() or {}
        paths = config.get("paths") or {}
        wanted = [("Home", "~"), ("Games", paths.get("games_root")), ("Prefixes", paths.get("prefixes_root"))]
        wanted += [(os.path.basename(m), m) for m in _mounts()]
        wanted.append(("Root", "/"))
        seen, out = set(), []
        for label, path in wanted:
            if not path:
                continue
            full = _expand(path)
            if full in seen or not os.path.isdir(full):
                continue
            seen.add(full)
            out.append({"label": label, "path": full})
        self._shortcuts = out

    @Slot(str, bool)
    def open(self, path, files):
        self._files = bool(files)
        self._load_shortcuts()
        full = _expand(path)
        while full != os.path.dirname(full) and not os.path.isdir(full):
            full = os.path.dirname(full)
        self.go(full if os.path.isdir(full) else _expand("~"))

    @Slot(str)
    def go(self, path):
        full = _expand(path)
        dirs, files = [], []
        try:
            with os.scandir(full) as it:
                for entry in it:
                    if entry.name.startswith("."):
                        continue
                    try:
                        is_dir = entry.is_dir()
                    except OSError:
                        continue
                    if is_dir:
                        dirs.append(entry.name)
                    elif self._files:
                        files.append(entry.name)
        except OSError:
            if full != self._path:
                return
        self._path = full
        self._entries = [{"name": n, "path": os.path.join(full, n), "dir": True} for n in sorted(dirs, key=str.casefold)]
        self._entries += [{"name": n, "path": os.path.join(full, n), "dir": False} for n in sorted(files, key=str.casefold)]
        self.changed.emit()

    @Slot(result=bool)
    def up(self):
        parent = os.path.dirname(self._path)
        if parent == self._path:
            return False
        self.go(parent)
        return True

    @Slot(int)
    def enter(self, index):
        if 0 <= index < len(self._entries) and self._entries[index]["dir"]:
            self.go(self._entries[index]["path"])

    @Slot(str, result=str)
    def display(self, path):
        home = _expand("~")
        full = str(path or "")
        if full == home:
            return "~"
        if full.startswith(home + os.sep):
            return "~" + full[len(home):]
        return full

    path = Property(str, lambda self: self._path, notify=changed)
    entries = Property("QVariantList", lambda self: list(self._entries), notify=changed)
    shortcuts = Property("QVariantList", lambda self: list(self._shortcuts), notify=changed)
    files = Property(bool, lambda self: self._files, notify=changed)
    atRoot = Property(bool, lambda self: os.path.dirname(self._path) == self._path, notify=changed)
