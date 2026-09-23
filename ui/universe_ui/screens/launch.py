from collections.abc import Callable

from PySide6.QtCore import Signal, Slot

from ..qt import Property
from .settings import HIDE_CURSOR, KEEP_AWAKE, RowsForm, _add, _dig, _row, global_launch_rows, screen_label

# config.toml keys with no launch key of their own; every one is advanced: (section, key, label, type, choices, detail).
CONFIG_ROWS = [
    ("Folders", "paths.games_root", "Games", "path", (), "Where sources install games."),
    ("Folders", "paths.prefixes_root", "Wine prefixes", "path", (), "Where a game's prefix is made when it names none."),
    ("Folders", "paths.recordings_root", "Recordings", "path", (), "Where the capture module files its videos."),
    ("Folders", "paths.journal_root", "Journal notes", "path", (), "Where the journal module renders its Markdown notes."),
    ("Folders", "paths.overrides", "Picked artwork", "path", (), "Where your own art picks live, shown over the fetched media."),
    ("API keys", "keys.sgdb", "SteamGridDB key", "secret", (), "Artwork comes from SteamGridDB with a key from steamgriddb.com."),
    ("API keys", "keys.sgdb_file", "SteamGridDB key file", "path", (), "A file holding the key, read when the key above is empty."),
    ("API keys", "keys.rawg", "RAWG key", "secret", (), "Descriptions and metadata come from RAWG with a key from rawg.io."),
    ("API keys", "keys.rawg_file", "RAWG key file", "path", (), "A file holding the key, read when the key above is empty."),
    (
        "Desktop",
        "desktop.profile",
        "Desktop",
        "enum",
        ("auto", "gnome", "none"),
        "What the launcher integrates with: auto detects GNOME, none skips the shell extension and cursor hiding.",
    ),
    (
        "Desktop",
        "desktop.cursor_extension",
        "Cursor extension",
        "string",
        (),
        "The GNOME Shell extension toggled to hide the cursor, restored to its prior state after the session.",
    ),
]

CONFIG_DEFAULTS = {"desktop.profile": "auto", "desktop.cursor_extension": "hide-cursor@elcste.com"}


def config_row(config, section, key, label, kind, choices, detail):
    value = _dig(config, key)
    if value in (None, ""):
        value = CONFIG_DEFAULTS.get(key, "")
    row = _row(section, key, label, "string" if kind == "secret" else kind, str(value or ""), choices, detail=detail, advanced=True)
    if kind == "secret":
        row["display"] = "Set" if value else "—"
        row["secret"] = True
    return row


def build_launch(client, screen_mode):
    """The global Launch page: Display and Overlay, then behind the gate the scaling, environment, programs, folders, keys and desktop cards."""
    config = client.config()
    mode = screen_mode()
    screen = " ".join(p for p in (str(mode.get("screen") or ""), screen_label(mode)) if p)
    rows, groups = [], []
    # A key tied to a runner is set on that runner's page.
    global_launch_rows(rows, groups, client, config, mode, lambda spec: not spec["runners"])
    _add(
        rows,
        groups,
        "Overlay",
        _row("Overlay", "desktop.hide_cursor", "Hide the cursor while playing", "bool", bool(_dig(config, "desktop.hide_cursor", True)), detail=HIDE_CURSOR),
        caps=True,
    )
    _add(
        rows,
        groups,
        "Overlay",
        _row("Overlay", "desktop.keep_awake", "Keep the screen awake", "bool", bool(_dig(config, "desktop.keep_awake", True)), detail=KEEP_AWAKE),
        caps=True,
    )
    for section, key, label, kind, choices, detail in CONFIG_ROWS:
        _add(rows, groups, section, config_row(config, section, key, label, kind, choices, detail), caps=True)
    return rows, groups, screen


class LaunchForm(RowsForm):
    screenChanged = Signal()

    def __init__(self, client, screen_mode: Callable[[], dict] = dict, parent=None):
        super().__init__(client, parent)
        self._screen_mode = screen_mode
        self._screen = ""

    @Slot()
    def load(self):
        rows, groups, self._screen = build_launch(self._client, self._screen_mode)
        self.screenChanged.emit()
        self._set_rows(rows, groups)

    def _write(self, row, payload):
        return self._client.setConfig(row["key"], payload)

    def _reload(self, row):
        self.load()

    screen = Property(str, lambda self: self._screen, notify=screenChanged)
