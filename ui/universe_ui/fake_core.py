import contextlib
import copy
import ctypes
import ctypes.util
import json
import os
import re
import shutil
import subprocess
import tempfile
import threading
import time
import tomllib
import zlib
from datetime import UTC, datetime
from pathlib import Path

from .errors import UniverseError
from .fake_radios import FakeBluetooth, FakeWifi

FIXTURE = Path(__file__).parent / "fixtures" / "library.json"
LAUNCH_KEYS = Path(__file__).parent / "fixtures" / "launch_keys.json"
# A rate stays the text config.toml holds it as: `auto` or `60`.
RATE_KEYS = {f"launch.{k['key']}" for k in json.loads(LAUNCH_KEYS.read_text()) if k["type"] in ("refresh", "fps")}
COMPONENTS = Path(__file__).parent / "fixtures" / "components.json"
EXTENSIONS = Path(__file__).parent / "fixtures" / "extensions.json"
EXTENSION_INDEX = "https://raw.githubusercontent.com/ilyasturki/universe-extensions/index/index.json"
EXTENSION_API = 2
BUILTIN_THEMES = ("reprise", "switch2", "ps5")
THEME_ENTRY = "theme.qml"
# What an index theme installs here, where no archive exists to unpack.
STUB_THEME = 'import QtQuick\nRectangle {\n    objectName: "stubTheme"\n    color: "#202024"\n}\n'
GPU = {
    "vendor": "amd",
    "name": "AMD Radeon RX 7900 GRE",
    "rdna": 3,
    "label": "AMD Radeon RX 7900 GRE · RDNA 3",
    "fits": {"dlss_upgrade": False, "fsr4_upgrade": True, "xess_upgrade": True, "optiscaler": True},
    "auto": {"dlss_upgrade": False, "fsr4_upgrade": False, "xess_upgrade": False, "optiscaler": False},
    "vaapi": "/dev/dri/renderD128",
}
# The core's game with nothing set, which a fixture game fills.
GAME = {
    "schema": 1,
    "sort_title": "",
    "release_year": 0,
    "hidden": False,
    "favorite": False,
    "tags": [],
    "added_at": "",
    "removed_at": "",
    "source": {"kind": "", "id": "", "dir": "", "build_id": "", "dlcs": [], "lutris_slug": ""},
    "desktop": {"hide_cursor": None},
    "metadata": dict.fromkeys(("sgdb_id", "gamesdb_id", "steam_appid", "metacritic", "players"), 0)
    | {"developers": [], "publishers": [], "genres": [], "summary": "", "description": ""},
    "modules": {},
    "launch": {
        **dict.fromkeys(
            [
                "runner",
                "runner_exe",
                "runner_build",
                "exe",
                "working_dir",
                "prefix",
                "proton",
                "arch",
                "wrapper",
                "pre_command",
                "post_command",
                "umu_id",
                "store",
            ],
            "",
        ),
        **dict.fromkeys(["gamescope_args", "gamescope_resolution", "gamescope_refresh", "gamescope_scaler", "gamescope_filter", "fps_limit"], ""),
        **dict.fromkeys(
            [
                "esync",
                "fsync",
                "ntsync",
                "wayland",
                "hdr",
                "discrete_gpu",
                "dlss_upgrade",
                "fsr4_upgrade",
                "xess_upgrade",
                "optiscaler",
                "debug_log",
                "mangohud",
            ],
            None,
        ),
        **dict.fromkeys(["pause_on_home", "gamescope", "gamescope_sharpness", "gamescope_adaptive_sync"], None),
        "args": [],
        "dll_overrides": {},
        "env": {},
        "options": {},
    },
}
# The config sections the fixture leaves out, at the core's defaults.
CONFIG = {
    "components": {"auto_update": True, "catalogue": ""},
    "extensions": {"index": ""},
    "controller": {"enabled": True, "hold_ms": 600, "home_summons": True, "volume_step": 2, "axes": {}, "buttons": {}, "macros": None},
    "desktop": {"hide_cursor": True, "cursor_extension": "", "keep_awake": True, "profile": "auto", "whats_new": False},
    "keys": {"prefer_sgdb": False, "sgdb": "", "sgdb_file": "~/.config/steamgriddb/api_key"},
    "system": {"tdp": "", "gpu": "", "refresh": "", "fan": ""},
    "saves": {"auto_backup": True, "keep": 5},
    "paths": {"saves_root": "~/.local/share/universe/saves", "recordings_root": "~/Videos/universe"},
}
REFRESH_RATES = [240, 165, 144, 120, 100, 90, 75, 60, 50, 48, 40, 30]
RESOLUTION_HEIGHTS = [2160, 1800, 1440, 1080, 720]
STEP_S = 0.15
# None: the session runs until stopped or `end_session()`.
SESSION_S = 2.0
# Before the session exists, as the real core's pre-launch hooks.
START_S = 0.0
WINDOW_S = 0.4
FRAME_S = 0.0
JOURNAL_S = 8.0
UNLOCK_S = 1.0
CLIP_S = 20

SLOTS = ("box_front", "square", "banner", "background", "logo")
FAKE_ORIGINS = {"box_front": "steam", "square": "generated", "banner": "steam", "background": "gamesdb", "logo": "libretro"}
CHANGELOG = [
    {
        "version": "0.0.3",
        "date": "2026-09-20",
        "sections": [
            {
                "title": "Added",
                "items": ["A changelog under Settings › About, and what's new after an update.", "`universe doctor` names the module behind each check."],
            },
            {"title": "Fixed", "items": ["The dock reads the pad again over a game."]},
        ],
    },
    {"version": "0.0.2", "date": "2026-09-16", "sections": [{"title": "Changed", "items": ["Settings mark the values you changed."]}]},
    {"version": "0.0.1", "date": "2026-09-13", "sections": [{"title": "Added", "items": ["The first release."]}]},
]


class X11Cards:
    """CARDINAL properties on gamescope's X server through libX11, what the core's nest does with x11rb. Any other server is left alone."""

    XA_CARDINAL = 6

    def __init__(self):
        self._lib = self._display = None

    def set(self, window, name, value):
        if self._display is None:
            self._display = self._open() or 0
        if not self._display:
            return
        assert self._lib is not None
        # Format 32 takes long-sized elements, not uint32.
        self._lib.XChangeProperty(self._display, window, self._atom(name), self.XA_CARDINAL, 32, 0, ctypes.byref(ctypes.c_ulong(value)), 1)
        self._lib.XFlush(self._display)

    def _atom(self, name):
        assert self._lib is not None
        return self._lib.XInternAtom(self._display, name.encode(), False)

    def _open(self):
        path = ctypes.util.find_library("X11")
        if not path:
            return None
        lib = self._lib = ctypes.CDLL(path)
        lib.XOpenDisplay.restype = ctypes.c_void_p
        lib.XOpenDisplay.argtypes = [ctypes.c_char_p]
        lib.XDefaultRootWindow.restype = ctypes.c_ulong
        lib.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
        lib.XInternAtom.restype = ctypes.c_ulong
        lib.XInternAtom.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
        lib.XChangeProperty.argtypes = [
            ctypes.c_void_p,
            ctypes.c_ulong,
            ctypes.c_ulong,
            ctypes.c_ulong,
            ctypes.c_int,
            ctypes.c_int,
            ctypes.c_void_p,
            ctypes.c_int,
        ]
        lib.XGetWindowProperty.argtypes = [
            ctypes.c_void_p,
            ctypes.c_ulong,
            ctypes.c_ulong,
            ctypes.c_long,
            ctypes.c_long,
            ctypes.c_int,
            ctypes.c_ulong,
            ctypes.POINTER(ctypes.c_ulong),
            ctypes.POINTER(ctypes.c_int),
            ctypes.POINTER(ctypes.c_ulong),
            ctypes.POINTER(ctypes.c_ulong),
            ctypes.POINTER(ctypes.c_void_p),
        ]
        lib.XFree.argtypes = [ctypes.c_void_p]
        lib.XFlush.argtypes = [ctypes.c_void_p]
        lib.XCloseDisplay.argtypes = [ctypes.c_void_p]
        display = self._display = lib.XOpenDisplay(None)
        if not display:
            return None
        actual_type, fmt, nitems, after, prop = ctypes.c_ulong(), ctypes.c_int(), ctypes.c_ulong(), ctypes.c_ulong(), ctypes.c_void_p()
        lib.XGetWindowProperty(
            display,
            lib.XDefaultRootWindow(display),
            self._atom("GAMESCOPE_FOCUSED_WINDOW"),
            0,
            1,
            False,
            self.XA_CARDINAL,
            ctypes.byref(actual_type),
            ctypes.byref(fmt),
            ctypes.byref(nitems),
            ctypes.byref(after),
            ctypes.byref(prop),
        )
        if prop.value:
            lib.XFree(prop)
        if nitems.value:
            return display
        lib.XCloseDisplay(display)
        return None


def connected_outputs():
    """Connector names from DRM sysfs, sorted, as the core picks its screen."""
    names = []
    for entry in Path("/sys/class/drm").glob("card*-*"):
        try:
            if (entry / "status").read_text().strip() == "connected":
                names.append(entry.name.split("-", 1)[1])
        except OSError:
            pass
    return sorted(names)


def screen_mode(screen):
    """`(width, height, hz)` of `screen`: Mutter's current mode, else the preferred DRM mode at 60 Hz, else None."""
    if not screen:
        return None
    return _mutter_current_mode(screen) or _drm_preferred_mode(screen)


def _mutter_current_mode(screen):
    try:
        out = subprocess.run(
            [
                "busctl",
                "--user",
                "--timeout=5",
                "--json=short",
                "call",
                "org.gnome.Mutter.DisplayConfig",
                "/org/gnome/Mutter/DisplayConfig",
                "org.gnome.Mutter.DisplayConfig",
                "GetCurrentState",
            ],
            capture_output=True,
            text=True,
            timeout=6,
            check=False,
        )
        monitors = json.loads(out.stdout)["data"][1]
    except (OSError, subprocess.TimeoutExpired, ValueError, LookupError, TypeError):
        return None
    wanted = screen.replace("-A-", "-")
    for info, modes, _ in monitors:
        if info[0] != screen and info[0].replace("-A-", "-") != wanted:
            continue
        for _, w, h, hz, _, _, props in modes:
            current = props.get("is-current")
            if (current.get("data") if isinstance(current, dict) else current) and w > 0 and h > 0:
                return (w, h, round(hz))
    return None


def _drm_preferred_mode(screen):
    for entry in Path("/sys/class/drm").glob(f"card*-{screen}"):
        try:
            first = (entry / "modes").read_text().split("\n", 1)[0].strip()
            w, h = (int(v) for v in first.split("x"))
            return (w, h, 60)
        except (OSError, ValueError):
            continue
    return None


def _end_of(line):
    exit_code, stopped = int(line.get("exit") or 0), line.get("stopped")
    if exit_code == 0:
        return "quit"
    if stopped:
        return "stopped"
    if exit_code < 0:
        return "killed" if stopped is False else "ended"
    return "crashed"


def _now():
    return datetime.now(UTC).astimezone().replace(microsecond=0).isoformat()


def _slug(title):
    return re.sub(r"[^a-z0-9]+", "-", title.casefold()).strip("-")


def _semver(version):
    parts = version.split(".")
    return tuple(int(p) for p in parts) if len(parts) == 3 and all(p.isdigit() for p in parts) else None


def _epoch(value):
    if not value:
        return 0
    try:
        return datetime.fromisoformat(str(value)).timestamp()
    except ValueError:
        return 0


# The core's: the first prose paragraph, emphasis dropped.
_BLOCK = re.compile(r"^(?:[-*+]\s|\d+[.)]\s|!\[|#)")


def _excerpt(paragraphs):
    prose = next((p.strip() for p in paragraphs if p.strip() and not _BLOCK.match(p.strip())), "")
    return " ".join(prose.replace("*", "").replace("`", "").split())


def _filled(value, defaults):
    out = {**copy.deepcopy(defaults), **value}
    for key, default in defaults.items():
        if default and isinstance(default, dict) and isinstance(out[key], dict):
            out[key] = _filled(out[key], default)
    return out


def _place(src, dest):
    os.makedirs(os.path.dirname(dest), exist_ok=True)
    if os.path.exists(dest):
        os.remove(dest)
    try:
        os.link(src, dest)
    except OSError:
        shutil.copy2(src, dest)
    return dest


def _coerce(schema, owner, key, value):
    if key not in schema:
        raise UniverseError("Invalid", f"{owner} has no setting '{key}'")
    kind = schema[key].get("type")
    if kind == "bool":
        return str(value).lower() in ("1", "true", "yes", "on")
    if kind == "int":
        if value in (schema[key].get("choices") or []):
            return value
        try:
            return int(value)
        except ValueError:
            raise UniverseError("Invalid", f"{key} must be an integer") from None
    if kind == "enum" and value not in (schema[key].get("choices") or []):
        raise UniverseError("Invalid", f"'{value}' is not a choice of {key}")
    return value


# The environment's, `us` bare: the core probes the desktop.
def keyboard_layout():
    return {"layout": os.environ.get("XKB_DEFAULT_LAYOUT", "").split(",")[0] or "us", "variant": os.environ.get("XKB_DEFAULT_VARIANT", "").split(",")[0]}


def host_gamescope(screen):
    gamescope = shutil.which("gamescope")
    if not gamescope:
        return None
    mode = screen_mode(screen or next(iter(connected_outputs()), ""))
    size = ["-W", str(mode[0]), "-H", str(mode[1]), "-w", str(mode[0]), "-h", str(mode[1]), "-r", str(mode[2])] if mode else []
    layout = keyboard_layout()
    return [
        shutil.which("env") or "env",
        f"XKB_DEFAULT_LAYOUT={layout['layout']}",
        f"XKB_DEFAULT_VARIANT={layout['variant']}",
        gamescope,
        "-f",
        "--force-composition",
        *size,
        "--mangoapp",
    ]


class FakeCore:
    def __init__(self, fixture=FIXTURE, root=None, fake_launch=False):
        with open(fixture) as f:
            self._data = json.load(f)
        self._config = dict(self._data.get("config") or {})
        # What the "file" sets itself, apart from the defaults filled below: `settings()["set"]`.
        self._set = copy.deepcopy(self._config)
        with open(LAUNCH_KEYS) as f:
            self._launch_keys = json.load(f)
        self._config["launch"] = {
            **{k["key"]: ({} if k["type"] == "map" else k["default"]) for k in self._launch_keys if k["scope"] != "game"},
            **(self._config.get("launch") or {}),
        }
        for section, defaults in CONFIG.items():
            self._config[section] = {**defaults, **(self._config.get(section) or {})}
        self._tmp = tempfile.TemporaryDirectory(prefix="universe-fake-") if root is None else None
        self._root = Path(self._tmp.name if self._tmp is not None else str(root))
        self._cache = os.path.join(os.environ.get("XDG_CACHE_HOME") or os.path.expanduser("~/.cache"), "universe", "fake-art")
        self._fake_launch = fake_launch
        self._lock = threading.Lock()
        self._timers = []
        self._process = None
        self._session = None
        self._session_started = None
        # The controls the running game's own changed, as they were before: its end puts them back.
        self._system_before = {}
        self._stopped = False
        self._closed = False
        self._installing, self._cancel = "", ""
        self._media_stop = False
        self.library_calls = []
        self._backups = {}
        self._cloud = {
            "the-technomancer": {
                "enabled": True,
                "state": "conflict",
                "message": "The saves changed on this device and in the cloud since they last synced.",
                "at": "2026-09-30T21:04:00+02:00",
                "locations": [{"name": "saves", "path": "/mnt/games/gog/the-technomancer/pfx/drive_c/users/steamuser/Documents/The Technomancer"}],
            }
        }
        self.cloud_calls = []
        self.prefix_tools, self.restored, self.trashed = [], [], []
        self.failing_tools = {}
        self._leftovers = [
            {"kind": "prefix", "path": "/mnt/games/prefixes/cyberpunk-2077-bak", "id": "cyberpunk-2077-bak", "title": "", "bytes": 9_100_000_000},
            {"kind": "recordings", "path": os.path.expanduser("~/Videos/universe/.archive/hades"), "id": "hades", "title": "Hades", "bytes": 3_400_000_000},
            {"kind": "game", "path": str(self._root / "data" / "games" / "hades"), "id": "hades", "title": "Hades", "bytes": 48_000_000},
        ]
        self.last_splash = ""
        self.game_shown = False
        self.window_misses = 0
        self.focus = ""
        self.summons = 0
        self.frozen = False
        self._runtime = {}
        self.fps_limit_writes = 0
        self.frames = 0
        self.filter = None
        self._filter_changed = False
        self.level, self.muted = 62, False
        self.outputs_list = [
            {"id": "alsa_card.pci-0000_00_1f.3/analog-output-speaker", "label": "Speakers", "device": "Built-in Audio", "current": True},
            {"id": "alsa_card.pci-0000_00_1f.3/analog-output-headphones", "label": "Headphones", "device": "Built-in Audio", "current": False},
            {"id": "alsa_card.pci-0000_01_00.1/hdmi-output-0", "label": "HDMI / DisplayPort", "device": "TV", "current": False},
        ]
        self.wifi = FakeWifi(self._later, lambda: STEP_S)
        self.bt = FakeBluetooth(self._later, lambda: STEP_S, self.session)
        self.power_list = ["suspend", "reboot", "power_off"]
        self.powered = []
        self.power_error = ""
        # An OLED Deck's, as the core lists them on its own gamescope; listed under UNIVERSE_DECK only.
        self.system = [
            {
                "id": "brightness",
                "label": "Brightness",
                "detail": "The screen's backlight.",
                "kind": "range",
                "value": "60",
                "min": 5,
                "max": 100,
                "step": 5,
                "unit": "%",
                "choices": [],
            },
            {
                "id": "refresh",
                "label": "Refresh rate",
                "detail": "The panel's refresh: lower saves battery.",
                "kind": "range",
                "value": "90",
                "min": 45,
                "max": 90,
                "step": 1,
                "unit": "Hz",
                "choices": [],
            },
            {
                "id": "tdp",
                "label": "Power limit",
                "detail": "The most the APU may draw, sustained: lower runs cooler and longer.",
                "kind": "range",
                "value": "15",
                "min": 3,
                "max": 15,
                "step": 1,
                "unit": "W",
                "choices": [],
            },
            {
                "id": "gpu",
                "label": "GPU clock",
                "detail": "Auto lets the driver choose; a fixed clock trades power for steady frame times.",
                "kind": "choice",
                "value": "auto",
                "min": 200,
                "max": 1600,
                "step": 100,
                "unit": "MHz",
                "choices": ["auto", *[str(m) for m in range(200, 1700, 100)]],
            },
            {
                "id": "fan",
                "label": "SteamOS fan curve",
                "detail": "Off leaves the fan to the firmware.",
                "kind": "toggle",
                "value": "on",
                "min": 0,
                "max": 0,
                "step": 0,
                "unit": "",
                "choices": [],
            },
        ]
        self.system_error = ""
        self.system_applied = 0
        self._cards = X11Cards()
        with open(COMPONENTS) as f:
            self._components = json.load(f)
        self._component_cancel = ""
        with open(EXTENSIONS) as f:
            self._index = json.load(f)["extensions"]
        # Installed extensions by id, as their sidecars hold them.
        self._extensions = {}
        for c in self._components["components"]:
            if c.get("recent"):
                c["recent"]["at"] = _now()
        self._lay_out()

    def data_home(self):
        return str(self._root / "data")

    def state_home(self):
        return str(self._root / "state")

    def onboarded(self):
        return (self._root / "state" / "onboarded").exists()

    def mark_onboarded(self):
        (self._root / "state").mkdir(parents=True, exist_ok=True)
        (self._root / "state" / "onboarded").touch()

    def version(self):
        return "0.0.0-fake"

    def changelog(self):
        return copy.deepcopy(CHANGELOG)

    def whats_new(self):
        state = self._root / "state"
        state.mkdir(parents=True, exist_ok=True)
        recorded = state / "last-version"
        last = recorded.read_text().strip() if recorded.exists() else None
        recorded.write_text(CHANGELOG[0]["version"])
        if last is None or not (self._config.get("desktop") or {}).get("whats_new"):
            return []
        seen = _semver(last)
        return [r for r in self.changelog() if seen is not None and (_semver(r["version"]) or seen) > seen]

    def _game_dir(self, ident):
        return self._root / "data" / "games" / ident

    def _lay_out(self):
        from .fixtures.art import paint_library, paint_store

        (self._root / "state").mkdir(parents=True, exist_ok=True)
        paint_library(self._data["games"], self._cache)
        paint_store(self._data, self._cache)
        for game in self._data["games"]:
            self._write_game(game)
            media = game.setdefault("media", {})
            for slot in SLOTS:
                if media.get(slot):
                    media[slot] = _place(media[slot], str(self._game_dir(game["id"]) / "media" / f"{slot}.png"))
            media["screenshots"] = [
                _place(p, str(self._game_dir(game["id"]) / "media" / f"screenshot{n + 1}.png")) for n, p in enumerate(media.get("screenshots") or [])
            ]
            self._lay_out_shots(game)
        for ident, entries in self._data.get("journal", {}).items():
            for entry in entries:
                self._write_entry(ident, entry)

    def _lay_out_shots(self, game):
        from datetime import datetime, timedelta

        shots = game.get("media", {}).get("screenshots") or []
        if not shots:
            return
        directory = self._game_dir(game["id"]) / "screenshots"
        for n, line in enumerate(self._data.get("sessions", {}).get(game["id"], [])[:3]):
            try:
                start = datetime.fromisoformat(line["started_at"])
            except (KeyError, ValueError):
                continue
            for k, minutes in enumerate((1, 10)):
                name = (start + timedelta(minutes=minutes)).strftime("%Y%m%d-%H%M%S") + ".png"
                _place(shots[(n + k) % len(shots)], str(directory / name))

    def screenshots(self, ident):
        idents = [self._game(ident)["id"]] if ident else [g["id"] for g in self._data["games"] if not (g.get("removed") or g.get("hidden"))]
        out = []
        for i in idents:
            directory = self._game_dir(i) / "screenshots"
            sessions = self._data.get("sessions", {}).get(i, [])
            for p in sorted(directory.glob("*.png"), reverse=True) if directory.is_dir() else []:
                taken = datetime.strptime(p.stem, "%Y%m%d-%H%M%S").astimezone().isoformat()
                session = next((s["session"] for s in sessions if s.get("started_at", "") <= taken <= s.get("ended_at", "")), "")
                # The fixture's shots are small: the picture stands as its own thumbnail.
                out.append(
                    {"game": i, "title": self._game(i)["title"], "path": str(p), "taken_at": taken, "session": session, "thumb": str(p), "thumb_ready": True}
                )
        out.sort(key=lambda r: os.path.basename(r["path"]), reverse=True)
        return out

    def media(self, ident):
        lines = self.sessions(ident)
        journaled = {(r["game"], r["session"]) for r in lines if r.get("journal")}
        rows = [
            {
                "kind": "shot",
                "game": shot["game"],
                "title": shot["title"],
                "session": shot["session"],
                "when": shot["taken_at"],
                "date": shot["taken_at"],
                "path": shot["path"],
                "thumb": shot["thumb"],
                "thumb_ready": True,
                "has_journal": bool(shot["session"]) and (shot["game"], shot["session"]) in journaled,
                "heading": "",
                "excerpt": "",
                "duration_s": 0,
            }
            for shot in self.screenshots(ident)
        ]
        for line in lines:
            rec = line.get("recording")
            if not rec:
                continue
            rows.append(
                {
                    "kind": "recording",
                    "game": line["game"],
                    "title": line["title"],
                    "session": line["session"],
                    "when": line["ended_at"],
                    "date": line["ended_at"],
                    "path": rec["path"],
                    "thumb": "",
                    "thumb_ready": False,
                    "has_journal": line.get("journal") is not None,
                    "heading": "",
                    "excerpt": "",
                    "duration_s": line.get("duration_s") or 0,
                }
            )
        idents = [self._game(ident)["id"]] if ident else [g["id"] for g in self._data["games"] if not (g.get("removed") or g.get("hidden"))]
        for i in idents:
            for e in self.journal(i):
                if (e.get("state") or "written") != "written":
                    continue
                images = [str(p) for p in e.get("images") or []]
                rows.append(
                    {
                        "kind": "journal",
                        "game": i,
                        "title": self._game(i)["title"],
                        "session": e.get("session") or "",
                        "when": e.get("written_at") or e.get("started_at") or "",
                        "date": e.get("started_at") or e.get("written_at") or "",
                        "path": images[0] if images else "",
                        "thumb": images[0] if images else "",
                        "thumb_ready": bool(images),
                        "has_journal": True,
                        "heading": e.get("title") or "Untitled",
                        "excerpt": _excerpt(e.get("paragraphs") or []),
                        "duration_s": e.get("duration_s") or 0,
                    }
                )
        rows.sort(key=lambda r: r["when"], reverse=True)
        return rows

    def remove_screenshot(self, ident, name):
        p = self._game_dir(ident) / "screenshots" / name
        if not p.is_file():
            raise UniverseError("NotFound", str(p))
        p.unlink()

    # The timestamp line makes every `set` a change the directory watch sees.
    def _write_game(self, game):
        d = self._game_dir(game["id"])
        for sub in ("media", "journal"):
            (d / sub).mkdir(parents=True, exist_ok=True)
        # A rename, as the core's atomic write: the directory watch sees it, a rewrite in place it would not.
        (d / "game.toml.tmp").write_text(
            f'schema = 1\nid = "{game["id"]}"\ntitle = {json.dumps(game.get("title", game["id"]))}\n# {_now()} {json.dumps(game.get("launch") or {})}\n'
        )
        os.replace(d / "game.toml.tmp", d / "game.toml")

    def _write_sessions(self, ident):
        lines = list(reversed(self._data.get("sessions", {}).get(ident, [])))
        (self._game_dir(ident) / "sessions.jsonl").write_text("".join(json.dumps(line) + "\n" for line in lines))

    def _write_entry(self, ident, entry):
        state = entry.get("state") or "written"
        journal = self._game_dir(ident) / "journal"
        journal.mkdir(parents=True, exist_ok=True)
        for old in journal.glob(f"{entry['session']}*.json"):
            old.unlink()
        name = f"{entry['session']}.json" if state == "written" else f"{entry['session']}.{state}.json"
        (journal / name).write_text(json.dumps(entry))

    def shutdown(self):
        self._closed = True
        for timer in self._timers:
            timer.cancel()
        self._timers.clear()
        if self._process is not None:
            self._process.kill()
            self._process = None
        if self._tmp is not None:
            self._tmp.cleanup()
            self._tmp = None

    def _later(self, seconds, fn):
        timer = threading.Timer(seconds, fn)
        timer.daemon = True
        self._timers.append(timer)
        timer.start()

    def _game(self, ident):
        for game in self._data["games"]:
            if game["id"] == ident:
                return game
        raise UniverseError("NotFound", f"no game '{ident}'")

    def _resolved(self, game):
        out = copy.deepcopy(game)
        if not isinstance(out.get("source"), dict):
            out["source"] = {"kind": str(out.get("source") or "")}
        out.setdefault("stats", {"hours": 0, "play_count": 0, "last_played": None})
        out.setdefault("removed", False)
        out["media"] = self._effective_media(game)
        out.pop("overrides", None)
        launch = out.setdefault("launch", {})
        effective = out["effective"] = {}
        for spec in (k for k in self._launch_keys if k["scope"] == "both"):
            own = launch.get(spec["key"])
            value = own if own not in (None, "") else self._config["launch"].get(spec["key"], spec["default"])
            if spec["type"] == "toggle" and spec["key"] != "gamescope_adaptive_sync":
                # An upgrade resolves through the GPU, as the core does; adaptive sync waits for the screen at launch.
                value = bool((self.gpu() or {}).get("auto", {}).get(spec["key"])) if value == "auto" else value in (True, "on", "true")
            effective[spec["key"]] = value
        effective["gamescope_args"] = launch.get("gamescope_args") or ""
        effective["hide_cursor"] = out.setdefault("desktop", {}).get("hide_cursor", self._config.get("desktop", {}).get("hide_cursor", True))
        runner = self._runner_of(launch)
        spec = self._runner(runner) or {"id": runner, "name": runner, "kind": "", "platforms": [], "path": "", "options": []}
        options = {o["key"]: o.get("value", o.get("default")) for o in spec.get("options") or []}
        options.update(launch.get("options") or {})
        effective.update(
            {
                "runner": spec["id"],
                "runner_name": spec.get("name", runner),
                "runner_kind": spec.get("kind", ""),
                "runner_path": launch.get("runner_exe") or spec.get("path") or "",
                "platform": out.get("platform") or (spec.get("platforms") or [""])[0],
                "options": options,
            }
        )
        out.setdefault("platform", effective["platform"])
        exe = str(launch.get("exe") or "")
        effective["working_dir"] = launch.get("working_dir") or (os.path.dirname(exe) if exe else "")
        prefixes = str(self._config.get("paths", {}).get("prefixes_root") or "~/.local/share/universe/prefixes")
        effective["prefix"] = (launch.get("prefix") or os.path.join(prefixes, game["id"])) if spec.get("kind") in ("proton", "wine") else ""
        effective["modules"] = {m["id"]: self.module_settings(m["id"], game["id"]) for m in self._data.get("modules", []) if m.get("enabled")}
        effective["proton_path"] = ""
        items = self._data.get("achievements", {}).get(game["id"], {}).get("items") or []
        out["achievements"] = {"total": len(items), "unlocked": sum(1 for a in items if a.get("unlocked_at"))}
        meta = out.setdefault("metadata", {})
        out["release_year"] = int(meta.pop("release_year", 0) or out.get("release_year") or 0)
        meta.update({k: int(meta[k] or 0) for k in ("sgdb_id", "gamesdb_id", "steam_appid") if k in meta})
        out.update(
            dir=str(self._game_dir(game["id"])),
            installed=bool(exe),
            journal_count=sum(1 for e in self._data.get("journal", {}).get(game["id"], []) if (e.get("state") or "written") == "written"),
            recording_count=sum(1 for line in self._data.get("sessions", {}).get(game["id"], []) if line.get("recording")),
        )
        return _filled(out, GAME)

    def achievements(self, ident, refresh=False):
        game = self._game(ident)
        cache = self._data.get("achievements", {}).get(game["id"])
        if cache is None:
            raise UniverseError("Unavailable", f"{game['title']}: no source lists its achievements")
        if refresh:
            self._tick(None, "Asking the store", 3)
            cache["fetched_at"] = _now()
        items = copy.deepcopy(cache.get("items") or [])
        return {
            "replay": None,
            **{k: v for k, v in cache.items() if k != "items"},
            "total": len(items),
            "unlocked": sum(1 for a in items if a.get("unlocked_at")),
            "items": items,
        }

    # What the source's watcher files mid-session: the first locked one, written as the core does (a rename the watch sees).
    def _unlock_one(self, ident):
        items = self._data.get("achievements", {}).get(ident, {}).get("items") or []
        locked = next((a for a in items if not a.get("unlocked_at")), None)
        if locked is None or self._closed:
            return
        locked["unlocked_at"] = _now()
        path = self._game_dir(ident) / "achievements.json"
        path.with_suffix(".tmp").write_text(json.dumps(self._data["achievements"][ident]))
        os.replace(path.with_suffix(".tmp"), path)

    def _runner_of(self, launch):
        runner = str(launch.get("runner") or "") or "proton"
        for spec in self._data.get("runners", []):
            if runner == spec["id"] or runner in (spec.get("aliases") or []):
                return spec["id"]
        return runner

    def _runner(self, ident):
        return next((r for r in self._data.get("runners", []) if r["id"] == ident), None)

    def list(self):
        games = [self._resolved(g) for g in self._data["games"] if not g.get("removed")]
        games.sort(key=lambda g: (g.get("hidden", False), -(_epoch(g["stats"].get("last_played")))))
        return games

    def get(self, ident):
        return self._resolved(self._game(ident))

    def resolve(self, query):
        q = query.casefold()
        exact = [g["id"] for g in self._data["games"] if g["id"] == q or g["title"].casefold() == q]
        if exact:
            return exact
        return [g["id"] for g in self._data["games"] if q in g["title"].casefold() or q in g["id"]]

    def set(self, ident, key, value):
        game = self._game(ident)
        node = game
        parts = key.split(".")
        if parts[0] == "capture":
            parts = ["modules", "capture", *parts[1:]]
        # The core checks a module's or a source's setting against its manifest.
        if parts[0] == "modules" and len(parts) == 3:
            return self.set_module_setting(parts[1], ident, parts[2], value)
        if parts[0] == "sources" and len(parts) == 3:
            return self.set_source_setting(parts[1], parts[2], value, ident)
        for part in parts[:-1]:
            node = node.setdefault(part, {})
        leaf = parts[-1]
        if value == "":
            node.pop(leaf, None)
        elif value in ("true", "false"):
            node[leaf] = value == "true"
        elif leaf == "tags":
            node[leaf] = [v.strip() for v in value.split(",") if v.strip()]
        else:
            node[leaf] = value
        self._write_game(game)

    def remove(self, ident, purge):
        game = self._game(ident)
        game["removed"] = True
        game["hidden"] = True
        if purge:
            shutil.rmtree(self._game_dir(ident), ignore_errors=True)
        else:
            self._write_game(game)

    def uninstall_via(self, ident):
        source = self._game(ident).get("source")
        kind = str(source.get("kind") or "") if isinstance(source, dict) else str(source or "")
        store = next((s for s in self._data.get("sources", []) if s["id"] == kind), None)
        return store["name"] if store and store.get("enabled") and store.get("available") and "uninstall" in store.get("capabilities", []) else None

    def uninstall(self, ident):
        self._game(ident)
        entries = [e for entries in self._data.get("source_library", {}).values() for e in entries if e.get("game_id") == ident]
        if not any(e.get("installed") for e in entries):
            raise UniverseError("Invalid", f"{ident} has no install folder")
        for entry in entries:
            entry["installed"] = False
            entry["dir"] = None

    # The data page's sizes: steady per game and part, so the shots stay the same run to run.
    @staticmethod
    def _bytes(ident, part, scale):
        return (zlib.crc32(f"{ident}:{part}".encode()) % 900 + 100) * scale

    def _path(self, key, fallback):
        return os.path.expanduser(str(self._config.get("paths", {}).get(key) or fallback))

    def game_data(self, ident):
        game, r = self._game(ident), self.get(ident)
        effective = r["effective"]
        kind, exe = effective["runner_kind"], str(r["launch"].get("exe") or "")
        prefixes, saves_root = self._path("prefixes_root", "~/.local/share/universe/prefixes"), self._path("saves_root", "~/.local/share/universe/saves")
        install = None
        if exe and kind != "emulator":
            install = {
                "path": os.path.dirname(exe),
                "bytes": self._bytes(ident, "install", 40_000_000),
                "exists": True,
                "owner": self.uninstall_via(ident) or "universe",
            }
        prefix = None
        if effective["prefix"]:
            path = effective["prefix"]
            source = str(game.get("source") or "")
            owner = "universe" if path.startswith(prefixes) else "lutris" if source == "lutris" else "elsewhere"
            target = os.path.join(prefixes, ident)
            shared = [g["id"] for g in self._data["games"] if g["id"] != ident and not g.get("removed") and self.get(g["id"])["effective"]["prefix"] == path]
            prefix = {
                "path": path,
                "bytes": self._bytes(ident, "prefix", 1_000_000),
                "exists": True,
                "owner": owner,
                "shared_with": shared,
                "movable": owner != "universe",
                "target": target,
            }
        backups = self._backups.setdefault(
            ident,
            [
                {
                    "id": f"backup-2026090{n}T200000Z",
                    "name": game["title"],
                    "when": f"2026-09-0{n}T20:00:00Z",
                    "bytes": self._bytes(ident, f"backup{n}", 9_000),
                    "path": "",
                }
                for n in (8, 6)
            ],
        )
        if kind == "emulator" and effective["runner"] == "eden":
            saves = {"engine": "", "folder": os.path.expanduser("~/.local/share/eden/nand/user/save"), "title_id": "", "files": [], "bytes": 0, "error": ""}
            backups = []
        elif kind == "emulator":
            folder = os.path.expanduser("~/.local/share/dolphin-emu/Wii/title/00010000")
            files = [
                {"path": os.path.join(folder, "524c4245/data/banner.bin"), "bytes": 24_576},
                {"path": os.path.join(folder, "524c4245/data/save.dat"), "bytes": 61_440},
            ]
            saves = {"engine": "emulator", "folder": folder, "title_id": "RLBE", "files": files, "bytes": sum(f["bytes"] for f in files), "error": ""}
        else:
            base = os.path.join(effective["prefix"] or "~", "drive_c/users/steamuser/Saved Games", game["title"])
            files = [{"path": os.path.join(base, f"slot{n}.sav"), "bytes": self._bytes(ident, f"slot{n}", 4_000)} for n in (1, 2, 3)]
            saves = {"engine": "ludusavi", "folder": "", "title_id": "", "files": files, "bytes": sum(f["bytes"] for f in files), "error": ""}
        saves.update(name=game["title"], dir=os.path.join(saves_root, ident), backups=backups, backups_bytes=sum(b["bytes"] for b in backups))
        saves.update(auto=bool(self._config.get("saves", {}).get("auto_backup", True)), keep=int(self._config.get("saves", {}).get("keep", 5)))
        saves["cloud"] = self._cloud_of(ident) if kind in ("proton", "wine") and self._syncs(ident) else None
        parts = {
            "media": self._bytes(ident, "media", 30_000),
            "screenshots": self._bytes(ident, "shots", 40_000),
            "journal": self._bytes(ident, "journal", 2_000),
            "sessions": 4_096,
        }
        recordings = sum(self._data.get("recordings", {}).get(ident, {}).values())
        data = {
            "id": ident,
            "title": game["title"],
            "runner_kind": kind,
            "install": install,
            "prefix": prefix,
            "saves": saves,
            "universe": {"path": str(self._game_dir(ident)), "bytes": sum(parts.values()), "parts": parts},
            "recordings": {"path": str(self._root / "recordings" / ident), "bytes": recordings, "exists": recordings > 0, "archived": False},
            "logs": {"path": str(self._root / "state" / "logs" / ident), "bytes": 0, "exists": False},
        }
        data["total"] = sum((data[k] or {}).get("bytes", 0) for k in ("install", "prefix", "universe", "recordings", "logs")) + saves["backups_bytes"]
        return data

    def storage(self):
        roots = [
            ("games", self._path("games_root", "~/Games"), 412_000_000_000, 1_200_000_000_000),
            ("prefixes", self._path("prefixes_root", "~/.local/share/universe/prefixes"), 28_000_000_000, 1_200_000_000_000),
            ("saves", self._path("saves_root", "~/.local/share/universe/saves"), 310_000_000, 380_000_000_000),
            ("recordings", self._path("recordings_root", "~/Videos/universe"), 96_000_000_000, 380_000_000_000),
            ("library", str(self._root / "data" / "games"), 1_400_000_000, 380_000_000_000),
            ("components", str(self._root / "data" / "components"), 6_800_000_000, 380_000_000_000),
            ("logs", str(self._root / "state" / "logs"), 12_000_000, 380_000_000_000),
        ]
        games = []
        for g in self._data["games"]:
            if g.get("removed"):
                continue
            d = self.game_data(g["id"])
            part = lambda k, d=d: (d[k] or {}).get("bytes", 0)
            games.append(
                {
                    "id": g["id"],
                    "title": g["title"],
                    "bytes": d["total"],
                    "install": part("install"),
                    "prefix": part("prefix"),
                    "universe": part("universe"),
                    "recordings": part("recordings"),
                    "saves": d["saves"]["backups_bytes"],
                    "logs": 0,
                }
            )
        games.sort(key=lambda g: -g["bytes"])
        return {
            "roots": [{"id": i, "path": p, "bytes": used, "free": free, "size": used + free, "exists": True} for i, p, used, free in roots],
            "games": games,
            "leftovers": copy.deepcopy(self._leftovers),
            "leftover_bytes": sum(item["bytes"] for item in self._leftovers),
        }

    def disk_free(self):
        return [{k: v for k, v in r.items() if k != "bytes"} for r in self.storage()["roots"]]

    def trash_leftover(self, path):
        if not any(item["path"] == path for item in self._leftovers):
            raise UniverseError("Invalid", f"{path} is no leftover: only what the Storage view lists goes to the trash")
        self._leftovers = [item for item in self._leftovers if item["path"] != path]
        self.trashed.append(path)

    def move_prefix(self, ident):
        prefix = self.game_data(ident)["prefix"]
        if not prefix or not prefix["movable"]:
            raise UniverseError("Invalid", f"{ident}'s prefix stays where it is")
        moved = [ident, *prefix["shared_with"]]
        for gid in moved:
            self._game(gid).setdefault("launch", {})["prefix"] = prefix["target"]
        return {"from": prefix["path"], "to": prefix["target"], "copied": False, "left": None, "games": moved, "owner": prefix["owner"]}

    def reset_prefix(self, ident):
        prefix = self.game_data(ident)["prefix"]
        if not prefix or prefix["owner"] != "universe" or prefix["shared_with"]:
            raise UniverseError("Invalid", f"{ident}'s prefix is not Universe's alone")
        return {"trashed": prefix["path"], "backup": self.saves_backup(ident)}

    def prefix_tool(self, ident, tool, args=()):
        if not self.game_data(ident)["prefix"]:
            raise UniverseError("Invalid", f"{ident} keeps no Wine prefix")
        self.prefix_tools.append((ident, tool, list(args)))
        if tool in self.failing_tools:
            raise UniverseError("Io", self.failing_tools[tool])
        if tool == "kill":
            return {"tool": tool, "stopped": 0}
        return {"tool": tool, "unit": f"universe-prefix-{ident}-{tool}-20260911-120000"}

    def saves_backup(self, ident):
        data = self.game_data(ident)
        if not data["saves"]["engine"]:
            raise UniverseError("Unavailable", "Eden keeps no saves of one game apart")
        stamp = datetime.now(UTC)
        self._backups[ident].insert(
            0, {"id": f"backup-{stamp:%Y%m%dT%H%M%SZ}", "name": data["title"], "when": stamp.isoformat(), "bytes": data["saves"]["bytes"], "path": ""}
        )
        del self._backups[ident][data["saves"]["keep"] :]
        return {"change": "different", "files": data["saves"]["files"], "bytes": data["saves"]["bytes"]}

    def saves_restore(self, ident, backup=""):
        data = self.game_data(ident)
        if not any(b["id"] == backup or not backup for b in data["saves"]["backups"]):
            raise UniverseError("NotFound", f"{ident} has no backup {backup}".strip())
        self.restored.append((ident, backup))
        return {"change": "same", "files": data["saves"]["files"], "bytes": data["saves"]["bytes"]}

    def _syncs(self, ident):
        source = self._game(ident).get("source")
        return (source.get("kind") if isinstance(source, dict) else source) in ("gog", "epic")

    def _cloud_of(self, ident):
        return copy.deepcopy(self._cloud.get(ident) or {"enabled": False, "state": "", "message": "", "at": "", "locations": []})

    def saves_cloud(self, ident, action="status"):
        if action not in ("status", "download", "upload", "keep-local", "keep-cloud"):
            raise UniverseError("Invalid", f"{action}: not a cloud saves action")
        if not self._syncs(ident):
            raise UniverseError("Unavailable", f"{ident}: its store keeps no cloud saves Universe syncs")
        if action != "status":
            self.cloud_calls.append((ident, action))
            self._cloud[ident] = {**self._cloud_of(ident), "state": "synced", "message": "", "at": datetime.now(UTC).isoformat(timespec="seconds")}
        return self._cloud_of(ident)

    def saves_export(self, ident, to):
        if not self.game_data(ident)["saves"]["backups"]:
            raise UniverseError("NotFound", f"{ident} has no backup to export")
        return os.path.join(os.path.expanduser(to), f"{ident}-saves-20260911-120000.zip")

    def reload(self):
        return None

    def reload_settings(self):
        return None

    def reload_game(self, ident):
        return None

    def import_lutris(self, apply):
        report = self._data.get("lutris")
        if report is None:
            raise UniverseError("NotFound", "~/.local/share/lutris/pga.db")
        if apply:
            for ident in report.get("imported", []):
                if any(g["id"] == ident for g in self._data["games"]):
                    continue
                game = {
                    "id": ident,
                    "title": ident.replace("-", " ").title(),
                    "source": "lutris",
                    "favorite": False,
                    "hidden": False,
                    "platform": "windows",
                    "launch": {"runner": "proton", "exe": f"/games/{ident}/{ident}.exe"},
                    "stats": {"hours": report.get("hours_imported", {}).get(ident, 0)},
                    "metadata": {},
                    "media": {"screenshots": []},
                }
                self._data["games"].append(game)
                self._write_game(game)
        return {"runners": [], "skipped": [], "updated": [], "media_imported": [], "env_diffs": [], **copy.deepcopy(report), "applied": bool(apply)}

    def import_roms(self, apply):
        report = copy.deepcopy(self._data.get("roms") or {"folders": [], "imported": [], "skipped": []})
        known = {g["id"] for g in self._data["games"]}
        report["imported"] = [f for f in report["imported"] if f["id"] not in known]
        if apply:
            for found in report["imported"]:
                self.add_game({"runner": found["runner"], "exe": found["path"], "title": found["title"]})
        return {**report, "applied": bool(apply)}

    def rescan(self):
        return self.import_roms(True)

    def add_game(self, spec):
        runner = self._runner_of({"runner": spec.get("runner", "")})
        runner_spec = self._runner(runner)
        if runner_spec is None:
            raise UniverseError("Invalid", f"unknown runner '{spec.get('runner')}'")
        path = str(spec.get("exe") or "")
        if not path:
            raise UniverseError("Invalid", "a game file is needed")
        title = str(spec.get("title") or "").strip() or os.path.splitext(os.path.basename(path))[0]
        ident = _slug(title)
        if any(g["id"] == ident for g in self._data["games"]):
            raise UniverseError("Invalid", f"{ident} is already in the library")
        game = {
            "id": ident,
            "title": title,
            "source": "manual",
            "favorite": False,
            "hidden": False,
            "platform": spec.get("platform") or (runner_spec.get("platforms") or [""])[0],
            "added_at": _now(),
            "launch": {"runner": runner, "exe": path},
            "metadata": {},
            "media": {"screenshots": []},
        }
        self._data["games"].append(game)
        self._write_game(game)
        return ident

    def runners(self):
        out = copy.deepcopy(self._data.get("runners", []))
        with self._lock:
            used = {c["id"]: next((b for b in c["builds"] if b.get("in_use")), None) for c in self._components["components"]}
        for runner in out:
            build = used.get(runner["id"])
            if not runner.get("path") and build:
                runner.update(path=build["program"], source="universe" if build["managed"] else "path", version=build["version"], available=True)
            runner.pop("detected", None)
            for option in runner.get("options") or []:
                option.pop("advanced", None)
                option.setdefault("choices", [])
            runner.update({"binaries": [], "build": "", "builds": [], "gamescope": None, **runner})
        return out

    def set_runner_setting(self, runner, key, value):
        spec = self._runner(self._runner_of({"runner": runner}))
        if spec is None:
            raise UniverseError("NotFound", f"runner {runner}")
        for root in (self._config, self._set):
            self._write_setting(root, f"runners.{spec['id']}.{key}", value)
        if key == "exe":
            spec["exe"] = value
            spec["path"] = value or spec.get("detected", "")
            spec["source"] = "config" if value else ("path" if spec.get("detected") else "")
            spec["available"] = bool(spec["path"])
            return
        if key == "args":
            spec["args"] = value
            return
        if key == "gamescope":
            spec["gamescope"] = None if value == "" else value == "true"
            return
        option = next((o for o in spec.get("options", []) if o["key"] == key), None)
        if option is None:
            raise UniverseError("Invalid", f"{spec['id']}: unknown option {key}")
        if option.get("type") == "bool":
            if value not in ("true", "false", ""):
                raise UniverseError("Invalid", f"{key} must be true or false")
            option["value"] = option.get("default") if value == "" else value == "true"
        else:
            option["value"] = value if value != "" else option.get("default")

    def _marker(self):
        return self._root / "state" / "current-session.json"

    def current(self):
        try:
            with open(self._marker()) as f:
                m = json.load(f)
        except (OSError, ValueError):
            return None
        return {k: m.get(k, "") for k in ("session_id", "id", "title", "unit", "screen", "started_at")}

    def launch(self, ident, screen, splash=""):
        self.last_splash = splash
        time.sleep(START_S)
        game = self._game(ident)
        with self._lock:
            running = self.current()
            if running:
                raise UniverseError("Busy", f"{running['title']} is running")
            session_id = time.strftime("%Y%m%d-%H%M%S")
            current = {
                "session_id": session_id,
                "id": ident,
                "title": game["title"],
                "unit": f"universe-game-{ident}-{session_id}.scope",
                "screen": screen,
                "started_at": _now(),
            }
            self._session = current
            self._session_started = time.monotonic()
            self._stopped = False
            effective = self._resolved(game)["effective"]
            sharpness = effective.get("gamescope_sharpness")
            self._runtime = {
                "mangohud": bool(effective.get("mangohud")),
                "fps_limit": str(effective.get("fps_limit") or ""),
                "gamescope_filter": str(effective.get("gamescope_filter") or ""),
                "gamescope_sharpness": None if sharpness in (None, "") else int(sharpness),
            }
            own = {k: v for k, v in (game.get("system") or {}).items() if v}
            self._system_before = {c["id"]: c["value"] for c in self.system if c["id"] in own}
            for c in self.system:
                c["value"] = own.get(c["id"], c["value"])
            self._marker().write_text(json.dumps({**current, "hook_env": [], "undo": []}))
        self._later(UNLOCK_S, lambda: self._unlock_one(ident))
        if self._fake_launch and shutil.which("sleep"):
            self._process = subprocess.Popen(["sleep", str(int(SESSION_S))])
            process = self._process

            def wait():
                code = process.wait()
                if self._process is process:
                    self._end_session(code)

            threading.Thread(target=wait, daemon=True, name="fake-session").start()
        elif SESSION_S is not None:
            self._later(SESSION_S, lambda: self._end_session(0))
        return session_id

    def end_session(self, exit_code=0):
        """The game exits by itself: `quit` on 0, where a `stop` is `stopped`."""
        process = self._process
        self._end_session(exit_code)
        if process is not None:
            process.kill()

    # As `session-end`: the session line first, the marker last.
    def _end_session(self, exit_code):
        with self._lock:
            current, self._session, self._process = self._session, None, None
            if not current or self._closed:
                return
            self.game_shown = self.frozen = False
            if self._filter_changed:
                launch = self._config["launch"]
                self.filter = (str(launch.get("gamescope_filter") or ""), launch.get("gamescope_sharpness"))
            self._runtime, self._filter_changed = {}, False
            for c in self.system:
                c["value"] = self._system_before.get(c["id"], c["value"])
            self._system_before = {}
            duration = max(1, round(time.monotonic() - (self._session_started or time.monotonic())))
            game = self._game(current["id"])
            stats = game.setdefault("stats", {"hours": 0, "play_count": 0, "last_played": None})
            stats["hours"] = float(stats.get("hours") or 0) + duration / 3600
            stats["play_count"] = int(stats.get("play_count") or 0) + 1
            stats["last_played"] = _now()
            self._data.setdefault("sessions", {}).setdefault(current["id"], []).insert(
                0,
                {
                    "session": current["session_id"],
                    "game": current["id"],
                    "started_at": current["started_at"],
                    "ended_at": _now(),
                    "duration_s": duration,
                    "source": "daemon",
                    "unit": current["unit"],
                    "screen": current["screen"],
                    "exit": exit_code,
                    "stopped": self._stopped,
                    "command": f"gamescope -f -- universe splash -- {game.get('launch', {}).get('exe') or current['id']}",
                    "recording": None,
                },
            )
            self._write_sessions(current["id"])
            self._pend_journal(current["id"], current["session_id"])
            with contextlib.suppress(OSError):
                self._marker().unlink()

    def stop(self, session_id):
        self._stopped = True
        if self._process is not None:
            self._process.kill()
        elif self._session:
            self._end_session(-15)
        else:
            raise UniverseError("NotFound", "no session running")

    def adopt_scope(self):
        return ""

    def _window(self):
        return {"id": "1", "pid": os.getpid(), "focused": True}

    def session_window(self):
        if not self.current():
            raise UniverseError("NotFound", "no session running")
        return self._window()

    def wait_session_window(self, session_id, timeout_ms):
        time.sleep(min(WINDOW_S, timeout_ms / 1000))
        current = self.current()
        if not current or current["session_id"] != session_id:
            return None
        if self.window_misses > 0:
            self.window_misses -= 1
            return None
        self.game_shown = True
        return self._window()

    def focus_session(self):
        if not self.current():
            raise UniverseError("NotFound", "no session running")
        self.game_shown = True

    def focus_pid(self, pid):
        if pid == os.getpid():
            self.game_shown = False

    # `UNIVERSE_FAKE_FOCUS` plays the desktop's focus: `launcher` (the default), `session` (the game's window), `other` or `unknown`.
    def host_focus(self):
        focus = self.focus or os.environ.get("UNIVERSE_FAKE_FOCUS") or "launcher"
        if focus == "unknown":
            raise UniverseError("Unavailable", "no window list")
        return {"launcher": focus == "launcher", "session": focus == "session"}

    def summon(self):
        self.summons += 1
        self.focus = "launcher"

    def freeze(self, on):
        if not self.current():
            raise UniverseError("NotFound", "no session running")
        self.frozen = bool(on)

    def nested(self):
        return bool(os.environ.get("GAMESCOPE_WAYLAND_DISPLAY"))

    # `UNIVERSE_FAKE_STEAM=1` plays Game Mode.
    def under_steam(self):
        return self.nested() and os.environ.get("UNIVERSE_FAKE_STEAM") == "1"

    # `UNIVERSE_FAKE_SESSION=1` plays the Universe session.
    def session(self):
        return os.environ.get("UNIVERSE_FAKE_SESSION") == "1"

    def deck_model(self):
        model = os.environ.get("UNIVERSE_DECK", "")
        return model if model in ("lcd", "oled") else ""

    def nest_game_shown(self):
        return bool(self.current()) and self.game_shown

    def nest_overlay(self, window, input, opacity):
        self._cards.set(window, "STEAM_OVERLAY", 1)
        self._cards.set(window, "STEAM_INPUT_FOCUS", int(bool(input)))
        self._cards.set(window, "_NET_WM_WINDOW_OPACITY", int(opacity))

    def nest_frame(self):
        self.frames += 1
        time.sleep(FRAME_S)
        return os.path.join(self._cache, "screenshot.png")

    def host_gamescope(self, screen):
        return host_gamescope(screen)

    def keyboard_layout(self):
        return keyboard_layout()

    def runtime(self):
        if not self.current():
            raise UniverseError("NotFound", "no session running")
        return dict(self._runtime)

    def set_fps_limit(self, value):
        if not self.current():
            raise UniverseError("NotFound", "no session running")
        value = value.strip() or "auto"
        if value not in ("auto", "none") and not (value.isascii() and value.isdigit() and int(value) > 0):
            raise UniverseError("Invalid", f"fps_limit must be auto, none or frames per second, not '{value}'")
        self._runtime["fps_limit"] = value
        self.fps_limit_writes += 1

    def set_mangohud(self, on=None):
        if not self.current():
            raise UniverseError("NotFound", "no session running")
        if on is None:
            on = not self._runtime["mangohud"]
        self._runtime["mangohud"] = on
        return on

    def nest_filter(self, filter, sharpness=None):
        self.filter = (filter, sharpness)
        if self.current():
            self._runtime.update({"gamescope_filter": filter, "gamescope_sharpness": sharpness})
            self._filter_changed = True

    def volume(self, change, value=0):
        step = int((self._config.get("controller") or {}).get("volume_step") or 2)
        if change == "up":
            self.level = min(100, self.level + step)
            self.muted = False
        elif change == "down":
            self.level = max(0, self.level - step)
        elif change == "mute":
            self.muted = not self.muted
        elif change == "set":
            self.level = max(0, min(100, int(value)))
        elif change != "get":
            raise UniverseError("Invalid", f"volume: up, down, mute, set or get, not '{change}'")
        output = next((o["label"] for o in self.outputs_list if o["current"]), "")
        return {"percent": self.level, "muted": self.muted, "output": output}

    def outputs(self):
        return [dict(o) for o in self.outputs_list]

    def set_output(self, id):
        if not any(o["id"] == id for o in self.outputs_list):
            raise UniverseError("Invalid", f"output: no '{id}' (universe output lists them)")
        for o in self.outputs_list:
            o["current"] = o["id"] == id
        return self.volume("get")

    def network(self):
        return self.wifi.state()

    def bluetooth(self):
        return self.bt.state()

    # `universe network|bluetooth watch --json` as the fixture plays them: each line to `emit`, commands to the handle's `send`.
    def watch(self, kind, emit):
        return (self.wifi if kind == "network" else self.bt).watch(emit)

    def power_actions(self):
        return [] if self.under_steam() else list(self.power_list)

    def system_controls(self):
        return [dict(c) for c in self.system] if self.deck_model() and not self.under_steam() else []

    def _control(self, ident):
        control = next((c for c in self.system if c["id"] == ident), None)
        if control is None:
            raise UniverseError("Invalid", f"no system control '{ident}'")
        if self.system_error:
            raise UniverseError("Unavailable", self.system_error)
        return control

    def set_system(self, ident, value):
        control = self._control(ident)
        running = self._session and self._game(self._session["id"])
        if not (running and (running.get("system") or {}).get(ident)):
            control["value"] = value
        if ident in self._system_before:
            self._system_before[ident] = value

    def set_system_for(self, game, ident, value):
        control = self._control(ident)
        own = self._game(game)
        if self._session and self._session["id"] == game:
            self._system_before.setdefault(ident, control["value"])
            control["value"] = value
        own.setdefault("system", {})[ident] = value
        self._write_game(own)

    def set_system_all(self, game, ident, value):
        self._control(ident)
        own = self._game(game)
        (own.get("system") or {}).pop(ident, None)
        self._write_game(own)
        self.set_system(ident, value)

    def apply_system(self):
        self.system_applied += 1

    def power(self, action):
        if action not in ("suspend", "reboot", "power_off"):
            raise UniverseError("Invalid", f"power: suspend, reboot or power_off, not '{action}'")
        if self.power_error:
            raise UniverseError("Unavailable", self.power_error)
        self.powered.append(action)

    # A shot during a session lands in the game's screenshots dir, named by the moment, as the capture module's does.
    def screenshot(self):
        current = self.current()
        if not current:
            return os.path.join(self._cache, "screenshot.png")
        shots = self._game(current["id"]).get("media", {}).get("screenshots") or []
        name = time.strftime("%Y%m%d-%H%M%S") + ".png"
        dest = str(self._game_dir(current["id"]) / "screenshots" / name)
        src = shots[int(time.time()) % len(shots)] if shots else os.path.join(self._cache, "screenshot.png")
        return _place(src, dest) if os.path.exists(src) else src

    def sessions(self, ident):
        idents = [self._game(ident)["id"]] if ident else [g["id"] for g in self._data["games"] if not (g.get("removed") or g.get("hidden"))]
        rows = [self._session_row(i, line) for i in idents for line in self._data.get("sessions", {}).get(i, [])]
        rows.sort(key=lambda r: r["ended_at"], reverse=True)
        return rows

    # A few lines shaped like a Proton game's journal; a session nobody played is NotFound, as the core says.
    def session_log(self, ident, session_id="", tail=0):
        game = self._game(ident)
        lines = self._data.get("sessions", {}).get(game["id"], [])
        current = self._session if self._session and self._session["id"] == game["id"] else None
        line = None
        if session_id:
            line = next((row for row in lines if row["session"] == session_id), None)
            if line is None and not (current and current["session_id"] == session_id):
                raise UniverseError("NotFound", f"session {session_id} of {game['id']}")
        elif current is None:
            if not lines:
                raise UniverseError("NotFound", f"{game['id']}: never played")
            line = lines[0]
        picked = line if line is not None else {"session": (current or {}).get("session_id", ""), "started_at": (current or {}).get("started_at", "")}
        start = picked["started_at"]
        exe = game.get("launch", {}).get("exe") or game["id"]
        out = [
            ("universe", 6, f"launch {picked['session']}: gamescope -f -W 3840 -H 2160 -- universe splash -- umu-run {exe}"),
            ("gamescope", 6, "[gamescope] [Info]  console: gamescope version 3.16.23"),
            ("gamescope", 6, "[gamescope] [Info]  vulkan: selecting physical device 'AMD Radeon RX 9070 XT'"),
            ("umu-run", 6, "umu: ProtonPath: GE-Proton10-4"),
            ("pressure-vessel-wrap", 4, "W: Unable to find a session bus for the container"),
            ("gamescope", 6, "[gamescope] [Info]  xwm: got the primary child window 0x2000004"),
            ("mangohud", 6, "[MANGOHUD] [info] [config.cpp:171] parsing config: /home/user/.local/state/universe/MangoHud.conf"),
            ("wine", 3, f"wine: Unhandled page fault on read access to 0000000000000000 at address 00007FF6D2A1B3C4 in {os.path.basename(str(exe))}"),
            ("gamescope", 6, "[gamescope] [Info]  launch: Primary child shut down!"),
        ]
        if line is not None:
            out.append(("systemd", 6, f"{line['unit']}: Deactivated successfully."))
        else:
            out = out[:-2]
        rows = [{"time": start, "source": s, "priority": p, "message": m} for s, p, m in out]
        return rows[-tail:] if tail and len(rows) > tail else rows

    def _session_row(self, ident, line):
        row = copy.deepcopy(line)
        for key in ("recording_duration_s", "command", "stopped"):
            row.pop(key, None)
        row["title"] = self._game(ident)["title"]
        row["end"] = _end_of(line)
        row["debug_log"] = None
        row["recording"] = None
        if line.get("recording"):
            path = line["recording"] if os.path.isfile(line["recording"]) else self._fake_clip(ident, line["session"])
            exists = os.path.isfile(path)
            size = self._data.get("recordings", {}).get(ident, {}).get(line["session"]) or (os.path.getsize(path) if exists else 0)
            duration = CLIP_S if path.startswith(self._cache) and exists else line.get("recording_duration_s") or 0
            row["recording"] = {"path": path, "size": size, "exists": exists, "duration_s": duration, "started_at": "", "pauses": []}
        entry = next((e for e in self._data.get("journal", {}).get(ident, []) if e.get("session") == line.get("session")), None)
        row["journal"] = (
            None
            if entry is None
            else {"state": entry.get("state") or "written", "title": entry.get("title") or "", "written_at": entry.get("written_at") or ""}
        )
        return row

    def _tick(self, progress, message, steps):
        for done in range(1, steps + 1):
            if self._closed:
                raise UniverseError("Io", "the core is closed")
            time.sleep(STEP_S)
            if progress:
                progress(done, steps, f"{message} ({done}/{steps})")

    def sources(self):
        out = self._manifests("sources")
        for source in out:
            source["library_cached"] = len(self._data.get("source_library", {}).get(source["id"], []))
        return out

    def _manifests(self, kind):
        out = [{"dir": "", "hooks": {}, "incompatible": "", "origin": "", **entry} for entry in copy.deepcopy(self._data.get(kind, []))]
        for entry in out:
            entry["settings"] = [{"platforms": [], "required": False, "runners": [], **s} for s in entry.get("settings", [])]
            for setting in entry["settings"]:
                setting.pop("dynamic_choices", None)
        return out

    def _source(self, ident):
        for source in self._data.get("sources", []):
            if source["id"] == ident:
                return source
        if any(m["id"] == ident for m in self._data.get("modules", [])):
            raise UniverseError("Invalid", f"{ident} is a module, not a source")
        raise UniverseError("NotFound", f"no source '{ident}'")

    def enable_source(self, ident, enabled):
        self._source(ident)["enabled"] = bool(enabled)

    def source_settings(self, ident, game_id=""):
        source = self._source(ident)
        merged = {s["key"]: s.get("default") for s in source.get("settings", [])}
        merged.update(self._config.get("sources", {}).get(ident, {}))
        if game_id:
            game_keys = {s["key"] for s in source.get("settings", []) if s.get("scope") == "game"}
            merged.update({k: v for k, v in (self._game(game_id).get("sources") or {}).get(ident, {}).items() if k in game_keys})
        return merged

    def source_setting_choices(self, ident, key):
        for setting in self._source(ident).get("settings", []):
            if setting["key"] == key:
                return list(setting.get("dynamic_choices") or setting.get("choices") or [])
        raise UniverseError("Invalid", f"{ident} has no setting '{key}'")

    def set_source_setting(self, ident, key, value, game_id=""):
        schema = {s["key"]: s for s in self._source(ident).get("settings", [])}
        if game_id and (schema.get(key) or {}).get("scope") != "game":
            raise UniverseError("Invalid", f"{ident}.{key} is a global setting")
        owners = [self._game(game_id)] if game_id else [self._config, self._set]
        for owner in owners:
            table = owner.setdefault("sources", {}).setdefault(ident, {})
            if value == "":
                table.pop(key, None)
            else:
                table[key] = _coerce(schema, ident, key, value)
        if game_id:
            self._write_game(owners[0])

    def login_url(self, source):
        return self._data.get("login_url", "https://example.invalid/login")

    def login(self, source, code):
        self._tick(None, f"Logging in to {source}", 3)
        for s in self._data.get("sources", []):
            if s["id"] == source:
                s["logged_in"] = True
        return f"Logging in to {source}: done"

    def library(self, source, refresh):
        """`refresh` is the store: games under `source_store` bought since the cache join the listing."""
        self.library_calls.append(refresh)
        library = self._data.setdefault("source_library", {}).setdefault(source, [])
        if refresh:
            known = {g["id"] for g in library}
            library.extend(dict(g) for g in self._data.get("source_store", {}).get(source, []) if g["id"] not in known)
            self._source(source)["library_at"] = time.strftime("%Y-%m-%dT%H:%M:%S%z")
        return copy.deepcopy(library)

    def search(self, source, query):
        q = query.casefold()
        catalog = self._data.get("source_library", {}).get(source, []) + self._data.get("catalog", [])
        return [dict(g) for g in catalog if q in g["title"].casefold()]

    def _source_game(self, source, game_id):
        return next((g for g in self._data.get("source_library", {}).get(source, []) if g["id"] == game_id), None)

    def info(self, source, game_id):
        """The sizes come from `sizes` in the fixture and are remembered in the listing, as the core does."""
        game = self._source_game(source, game_id)
        if game is None:
            return None
        sizes = dict(self._data.get("sizes", {}).get(game_id) or {})
        game.update(sizes)
        return {"folder_name": game["title"], **sizes}

    def cancel(self, source, game_id):
        if self._installing != game_id:
            return False
        self._cancel = game_id
        return True

    def install(self, source, game_id, progress=None):
        game = self._source_game(source, game_id)
        total = int((game or {}).get("disk_size") or self._data.get("sizes", {}).get(game_id, {}).get("disk_size") or 20)
        start = int((game or {}).get("partial_bytes") or 0)
        steps = 20
        self._installing, self._cancel = game_id, ""
        try:
            for step in range(int(start * steps / total) + 1, steps + 1):
                if self._closed:
                    raise UniverseError("Io", "the core is closed")
                time.sleep(STEP_S)
                done = total * step // steps
                if self._cancel == game_id:
                    if game is not None:
                        game.update({"partial_dir": f"/mnt/games/PC/{game['title']}", "partial_bytes": done})
                    raise UniverseError("Io", "gog install failed (143): stopped")
                if progress:
                    progress(done, total, f"{100 * step // steps}%")
        finally:
            self._installing = ""
        if game is not None:
            game.update({"installed": True, "dir": f"/mnt/games/PC/{game['title']}", "disk_size": total})
            game.pop("partial_dir", None)
            game.pop("partial_bytes", None)
            game["game_id"] = self._land(source, game)
        return f"Installing {game_id}: done"

    # The core's apply_source_game: the install creates the library game, or brings an archived one back.
    def _land(self, source, entry):
        ident = str(entry.get("game_id") or _slug(entry["title"]))
        game = next((g for g in self._data["games"] if g["id"] == ident), None)
        if game is None:
            game = {
                "id": ident,
                "title": entry["title"],
                "source": {"kind": source, "id": entry["id"], "dir": entry["dir"]},
                "favorite": False,
                "hidden": False,
                "platform": "PC",
                "launch": {"runner": "proton", "exe": f"{entry['dir']}/{ident}.exe"},
                "metadata": {},
                "media": {"screenshots": []},
            }
            self._data["games"].append(game)
        if not game.get("added_at") or game.get("removed"):
            game.update({"added_at": _now(), "removed": False, "hidden": False})
        self._write_game(game)
        return ident

    def update(self, source, game_id, progress=None):
        self._tick(progress, "Updating" + (f" {game_id}" if game_id else " everything"), 12)
        before = len(self._data.get("updates", []))
        self._data["updates"] = [u for u in self._data.get("updates", []) if game_id and u["id"] != game_id]
        return before - len(self._data["updates"])

    def updates(self):
        return copy.deepcopy(self._data.get("updates", []))

    def scan(self, source, progress=None):
        self._tick(progress, "Scanning", 4)
        return []

    def media_refresh(self, ident, force, progress=None):
        games = [self._game(ident)] if ident else [g for g in self._data["games"] if not g.get("removed")]
        return self._refresh_media(games, progress, stoppable=not ident)

    def media_refresh_many(self, ids, force, progress=None):
        return self._refresh_media([g for g in self._data["games"] if g["id"] in ids], progress, stoppable=True)

    def _refresh_media(self, games, progress, stoppable):
        if stoppable:
            self._media_stop = False
        for i, game in enumerate(games):
            if stoppable and self._media_stop:
                break
            if progress:
                progress(i, len(games), game.get("title", game["id"]))
            time.sleep(STEP_S)
            media = game.get("media") or {}
            for path in (media.get(slot) for slot in SLOTS):
                if path and os.path.exists(path):
                    os.replace(path, path + ".part")
                    os.replace(path + ".part", path)
        return (0, len(games))

    def media_cancel(self):
        self._media_stop = True

    def _effective_media(self, game):
        media = dict(game.get("media") or {})
        media.update({k: v for k, v in (game.get("overrides") or {}).items() if v})
        return media

    def _sgdb_key(self):
        return bool((self._config.get("keys") or {}).get("sgdb"))

    # SteamGridDB's games with a key, else GOG GamesDB's, whose ids pass 2^53.
    def _catalogue_hits(self, game):
        title = game.get("title", game["id"])
        provider, base = ("sgdb", 5000) if self._sgdb_key() else ("gamesdb", 51152975252476092)
        base += 1000 * len(game["id"])
        verified = provider == "sgdb"
        return [
            {"provider": provider, "id": base, "name": title, "year": 2016, "verified": verified},
            {"provider": provider, "id": base + 1, "name": f"{title} Remastered", "year": 2021, "verified": False},
            {"provider": provider, "id": base + 2, "name": f"{title} II", "year": 2019, "verified": verified},
        ]

    def _entry(self, game):
        hits = self._catalogue_hits(game)
        provider = hits[0]["provider"]
        pinned = int((game.get("metadata") or {}).get(f"{provider}_id") or 0) or hits[0]["id"]
        return next((h for h in hits if h["id"] == pinned), {**hits[0], "id": pinned})

    def _status_of(self, game):
        media = game.get("media") or {}
        overrides = game.get("overrides") or {}
        slots = []
        for slot in SLOTS:
            default, over = media.get(slot) or "", overrides.get(slot) or ""
            kind = "picked" if over else "default" if default else "missing"
            slots.append(
                {
                    "slot": slot,
                    "path": over or default,
                    "default": default,
                    "override": over,
                    "origin": "picked" if over else FAKE_ORIGINS[slot] if default else "",
                    "default_origin": FAKE_ORIGINS[slot] if default else "",
                    "kind": kind,
                }
            )
        return {
            "id": game["id"],
            "title": game.get("title", game["id"]),
            "entry": {**self._entry(game), "current": True},
            "sgdb_key": self._sgdb_key(),
            "slots": slots,
        }

    def media_status(self, ident):
        games = [self._game(ident)] if ident else [g for g in self._data["games"] if not g.get("removed")]
        return [self._status_of(g) for g in games]

    def media_set_slot(self, ident, slot, path):
        game = self._game(ident)
        placed = _place(path, str(self._game_dir(ident) / "media" / "picked" / f"{slot}.png"))
        game.setdefault("overrides", {})[slot] = placed
        return placed

    def media_set_url(self, ident, slot, url):
        from .fixtures.art import paint_candidate

        path = url if os.path.isfile(url) else paint_candidate(self._cache, ident, slot, url)
        return self.media_set_slot(ident, slot, path)

    def media_unset(self, ident, slot):
        overrides = self._game(ident).setdefault("overrides", {})
        gone = overrides.pop(slot, None)
        if gone:
            with contextlib.suppress(OSError):
                os.remove(gone)
        return bool(gone)

    def media_candidates(self, ident, slot, page=0):
        from .fixtures.art import paint_candidates

        game = self._game(ident)
        key = self._sgdb_key()
        items = [] if int(page) > 0 else paint_candidates(self._cache, ident, slot, game.get("title", ident), key)
        return {"items": items, "page": int(page), "more": False, "entry": {**self._entry(game), "current": True}, "sgdb_key": key}

    def media_search(self, ident, query):
        game = self._game(ident)
        current = self._entry(game)["id"]
        return [{**h, "current": h["id"] == current} for h in self._catalogue_hits(game) if query.casefold() in h["name"].casefold()]

    def media_pin(self, ident, provider, provider_id):
        game = self._game(ident)
        game.setdefault("metadata", {})[f"{provider}_id"] = provider_id
        self._write_game(game)

    def _game_of_session(self, session_id):
        current = self.current()
        if current and current["session_id"] == session_id:
            return current["id"]
        for ident, lines in self._data.get("sessions", {}).items():
            if any(s.get("session") == session_id for s in lines):
                return ident
        raise UniverseError("NotFound", f"session {session_id}")

    def file_recording(self, session_id, path):
        ident = self._game_of_session(session_id)
        dest = str(self._root / "recordings" / ident / f"{session_id}{os.path.splitext(path)[1] or '.mkv'}")
        if os.path.isfile(path):
            _place(path, dest)
        line = next((s for s in self._data.get("sessions", {}).get(ident, []) if s.get("session") == session_id), None)
        if line is not None:
            line["recording"] = dest
            line["recording_duration_s"] = line.get("duration_s") or 0
            self._data.get("recordings", {}).get(ident, {}).pop(session_id, None)
        self._write_sessions(ident)
        return dest

    def remove_recording(self, ident, session_id):
        line = next((s for s in self._data.get("sessions", {}).get(ident, []) if s.get("session") == session_id and s.get("recording")), None)
        if line is None:
            raise UniverseError("NotFound", f"session {session_id} has no recording")
        line["recording"] = None
        self._data.get("recordings", {}).get(ident, {}).pop(session_id, None)
        self._write_sessions(ident)

    def _fake_clip(self, ident, session):
        out = os.path.join(self._cache, f"{ident}-{session}.mkv")
        if os.path.exists(out):
            return out
        ffmpeg = shutil.which("ffmpeg")
        if ffmpeg:
            subprocess.run(
                [
                    ffmpeg,
                    "-loglevel",
                    "error",
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    f"testsrc=size=640x360:rate=30:duration={CLIP_S}",
                    "-f",
                    "lavfi",
                    "-i",
                    f"sine=frequency=440:duration={CLIP_S}",
                    "-c:v",
                    "libx264",
                    "-preset",
                    "ultrafast",
                    "-pix_fmt",
                    "yuv420p",
                    "-c:a",
                    "aac",
                    "-shortest",
                    out,
                ],
                capture_output=True,
                timeout=30,
                check=False,
            )
        return out

    def journal(self, ident):
        entries = self._data.get("journal", {}).get(ident, [])
        shots = self._game(ident).get("media", {}).get("screenshots") or []
        sessions = {s.get("session"): s for s in self._data.get("sessions", {}).get(ident, [])}
        out = []
        for e in entries:
            line = sessions.get(e.get("session")) or {}
            out.append(
                {
                    "state": "written",
                    "started_at": line.get("started_at") or "",
                    "ended_at": line.get("ended_at") or "",
                    "duration_s": line.get("duration_s") or 0,
                    **e,
                    "images": e.get("images") or (shots if e.get("state", "written") == "written" else []),
                }
            )
        return out

    def pending_journals(self):
        return [
            {"game": game, "title": self._game(game)["title"], "session": e.get("session"), "started_at": e.get("started_at")}
            for game, entries in self._data.get("journal", {}).items()
            for e in entries
            if e.get("state") == "pending"
        ]

    def _pend_journal(self, ident, session_id):
        entries = self._data.setdefault("journal", {}).setdefault(ident, [])
        entry = {
            "session": session_id,
            "game": ident,
            "state": "pending",
            "started_at": _now(),
            "written_at": "",
            "lang": "en",
            "title": "",
            "provider": "fake",
            "paragraphs": [],
            "next_up": "",
            "images": [],
        }
        entries.insert(0, entry)
        self._write_entry(ident, entry)

        def write():
            with self._lock:
                if self._closed or entry not in entries:
                    return
                entry.update(state="written", written_at=_now(), title="A short session", paragraphs=["A quick look around, nothing decided yet."])
                self._write_entry(ident, entry)

        self._later(JOURNAL_S, write)

    def journal_write(self, ident, session_id, rewrite=False):
        lines = self._data.get("sessions", {}).get(ident, [])
        if not any(s.get("session") == session_id for s in lines):
            raise UniverseError("NotFound", f"session {session_id} of {ident}")
        entries = self._data.setdefault("journal", {}).setdefault(ident, [])
        written = next((e for e in entries if e.get("session") == session_id and e.get("state", "written") == "written"), None)
        if written and not rewrite:
            raise UniverseError("Invalid", f"{session_id} already has an entry; ask for a rewrite to replace it")
        self._data["journal"][ident] = [e for e in entries if e.get("session") != session_id]
        self._pend_journal(ident, session_id)
        return f"universe-journal-post-process-{session_id}.service"

    def sweep_journals(self, ident=""):
        waiting = [
            (game, e) for game, entries in self._data.get("journal", {}).items() if not ident or game == ident for e in entries if e.get("state") == "deferred"
        ]
        nxt = min((e.get("retry_at") or "" for _, e in waiting), default="")
        due = [(game, e) for game, e in waiting if (e.get("retry_at") or "") <= _now()]
        if not due:
            return {"started": None, "due": 0, "next": nxt}
        game, entry = due[0]
        self.journal_write(game, entry["session"], True)
        return {"started": {"game": game, "session": entry["session"]}, "due": len(due) - 1, "next": nxt}

    def remove_journal_entry(self, ident, session_id):
        entries = self._data.get("journal", {}).get(ident, [])
        kept = [e for e in entries if e.get("session") != session_id]
        if len(kept) == len(entries):
            raise UniverseError("NotFound", f"journal entry {session_id}")
        self._data["journal"][ident] = kept
        for old in (self._game_dir(ident) / "journal").glob(f"{session_id}*.json"):
            old.unlink()

    def add_entry(self, session_id, entry):
        entry = dict(entry)
        entry.setdefault("session", session_id)
        ident = entry.get("game") or self._game_of_session(session_id)
        entry["game"] = ident
        self._data.setdefault("journal", {}).setdefault(ident, []).insert(0, entry)
        self._write_entry(ident, entry)

    def modules(self):
        out = self._manifests("modules")
        for module in out:
            values = self.module_settings(module["id"], "")
            module["unset"] = [s["key"] for s in module.get("settings", []) if s.get("required") and not values.get(s["key"])]
        return out

    def _module(self, ident):
        for module in self._data.get("modules", []):
            if module["id"] == ident:
                return module
        if any(s["id"] == ident for s in self._data.get("sources", [])):
            raise UniverseError("Invalid", f"{ident} is a source, not a module")
        raise UniverseError("NotFound", f"no module '{ident}'")

    def enable_module(self, ident, enabled):
        self._module(ident)["enabled"] = bool(enabled)

    def module_settings(self, module_id, game_id):
        module = self._module(module_id)
        merged = {s["key"]: s.get("default") for s in module.get("settings", [])}
        merged.update(self._config.get("modules", {}).get(module_id, {}))
        if game_id:
            merged.update(self._game(game_id).get("modules", {}).get(module_id, {}))
        return merged

    def module_setting_choices(self, module_id, key):
        for setting in self._module(module_id).get("settings", []):
            if setting["key"] == key:
                return list(setting.get("dynamic_choices") or setting.get("choices") or [])
        raise UniverseError("Invalid", f"{module_id} has no setting '{key}'")

    def set_module_setting(self, module_id, game_id, key, value):
        module = self._module(module_id)
        schema = {s["key"]: s for s in module.get("settings", [])}
        if key not in schema:
            raise UniverseError("Invalid", f"{module_id} has no setting '{key}'")
        for owner in [self._game(game_id)] if game_id else [self._config, self._set]:
            table = owner.setdefault("modules", {}).setdefault(module_id, {})
            if value == "":
                table.pop(key, None)
            else:
                table[key] = _coerce(schema, module_id, key, value)
        if game_id:
            self._write_game(owner)

    def doctor(self):
        checks = [{"module": "", "component": "", **c} for c in copy.deepcopy(self._data.get("doctor", []))]
        for c in self.components()["components"]:
            if c["proposal"] == "install" and c["kind"] in ("emulator", "wine"):
                fix = f"universe component install {c['id']} (Settings › Runners), or install it, or set runners.{c['id']}.exe"
                if c.get("notice"):
                    fix = f"{fix}. {c['notice']}"
                checks.append(
                    {
                        "check": f"runner-{c['id']}",
                        "label": c["name"],
                        "ok": False,
                        "detail": f"{c['name']} not found",
                        "fix": fix,
                        "module": "runners",
                        "component": c["id"],
                    }
                )
        return checks

    def _component(self, ident):
        found = next((c for c in self._components["components"] if c["id"] == ident), None)
        if found is None:
            raise UniverseError("NotFound", f"{ident} is not in the catalogue")
        return found

    def _settle(self, c):
        if c["kind"] in ("emulator", "wine"):
            c["used_by"] = sum(1 for g in self._data["games"] if self._runner_of(g.get("launch") or {}) == c["id"])
            if c["used_by"] and not c["builds"] and c.get("latest"):
                c["proposal"] = "install"
        managed = [b for b in c["builds"] if b["managed"]]
        c["builds"] = sorted(managed, key=lambda b: b["date"], reverse=True) + [b for b in c["builds"] if not b["managed"]]
        have = {b["version"] for b in managed}
        c["in_use"] = next((b for b in c["builds"] if b["in_use"]), None)
        for a in c["available"]:
            a.update(installed=a["version"] in have, skipped=a["version"] in c["skipped"])
        latest = (c.get("latest") or {}).get("version", "")
        c["update"] = latest if managed and latest and latest not in have and latest not in c["skipped"] else ""
        if (c["builds"] and c["proposal"] == "install") or (latest in have and c["proposal"] == "newer"):
            c["proposal"] = ""

    def components(self, refresh=False):
        with self._lock:
            for c in self._components["components"]:
                self._settle(c)
            out = copy.deepcopy(self._components)
        out["auto_update"] = (self._config.get("components") or {}).get("auto_update", True)
        out.setdefault("catalogue", {"url": "", "generated_at": "", "fetched_at": "", "error": ""})
        return out

    def component_install(self, ident, version="", accepted=False, progress=None):
        c = self._component(ident)
        if c.get("notice") and not accepted:
            raise UniverseError("Invalid", f"{c['name']} waits for its notice to be accepted: {c['notice']}")
        if c["kind"] == "system":
            for step in range(1, 5):
                if progress is not None:
                    progress(step * 25, 100, f"Installing {c['name']} · {step * 25}%")
                time.sleep(0.05)
            with self._lock:
                c["builds"] = [{"version": "1.0", "origin": "system", "program": f"/usr/bin/{c['bin']}", "managed": False, "in_use": True}]
                self._settle(c)
            return "1.0"
        version = version or (c.get("latest") or {}).get("version", "")
        build = next((a for a in c["available"] if a["version"] == version), None)
        if build is None:
            raise UniverseError("NotFound", f"{c['name']} {version}: no such build in the catalogue")
        total = build["size"] or 1
        for step in range(1, 5):
            if self._component_cancel == ident:
                self._component_cancel = ""
                raise UniverseError("Busy", f"{c['name']} {version}: cancelled")
            if progress is not None:
                progress(total * step // 4, total, f"Downloading {c['name']} {version}")
            time.sleep(0.05)
        with self._lock:
            if not any(b["managed"] and b["version"] == version for b in c["builds"]):
                first = not any(b["in_use"] for b in c["builds"])
                c["builds"].append(
                    {
                        "version": version,
                        "origin": "universe",
                        "program": f"/fake/components/{ident}/{version}",
                        "managed": True,
                        "in_use": first,
                        "pinned": False,
                        "disk": build["size"] * 2,
                        "date": build["date"],
                    }
                )
            if version in c["skipped"]:
                c["skipped"].remove(version)
            self._settle(c)
        return version

    def component_update(self, ident="", progress=None):
        out = []
        for c in self.components()["components"]:
            if (ident and c["id"] != ident) or not c["update"]:
                continue
            follows = bool(c["in_use"] and c["in_use"]["managed"])
            version = self.component_install(c["id"], c["update"], True, progress)
            with self._lock:
                live = self._component(c["id"])
                if follows:
                    for b in live["builds"]:
                        b["in_use"] = b["managed"] and b["version"] == version
                live["recent"] = {"version": version, "at": _now()}
                self._settle(live)
            out.append({"id": c["id"], "name": c["name"], "version": version})
        return out

    def component_remove(self, ident, version):
        with self._lock:
            c = self._component(ident)
            b = next((b for b in c["builds"] if b["managed"] and b["version"] == version), None)
            if b is None:
                raise UniverseError("NotFound", f"{ident} {version} is not installed")
            if b["in_use"] or b["pinned"]:
                raise UniverseError("Busy", f"{ident} {version} is in use: pick another build for what names it first")
            c["builds"].remove(b)
            self._settle(c)

    def component_uninstall(self, ident):
        with self._lock:
            c = self._component(ident)
            self._settle(c)
            managed = [b for b in c["builds"] if b["managed"]]
            if not managed:
                raise UniverseError("NotFound", f"Universe holds no build of {ident}")
            pinned = next((b for b in managed if b.get("pinned")), None)
            if pinned is not None:
                raise UniverseError("Busy", f"{ident} {pinned['version']} is in use: pick another build for what names it first")
            found = [b for b in c["builds"] if not b["managed"]]
            if found and not any(b["in_use"] for b in found):
                found[0]["in_use"] = True
            if c["kind"] in ("emulator", "wine") or c["setting"] in {b["version"] for b in managed}:
                c["setting"] = c["family"] if c["kind"] == "proton" else ""
            c.update(builds=found, skipped=[], recent=None)
            self._settle(c)
            return [b["version"] for b in reversed(managed)]

    def component_rollback(self, ident):
        with self._lock:
            c = self._component(ident)
            self._settle(c)
            managed = [b for b in c["builds"] if b["managed"]]
            found = [b for b in c["builds"] if not b["managed"]]
            if not managed:
                raise UniverseError("NotFound", f"Universe holds no build of {ident}")
            newest = managed[0]
            if len(managed) < 2 and not found:
                raise UniverseError("Invalid", f"{c['name']} {newest['version']} is the only build of {ident}: nothing to roll back to")
            c["builds"].remove(newest)
            c["skipped"].append(newest["version"])
            if newest["in_use"]:
                (managed[1:] or found)[0]["in_use"] = True
            c["recent"] = None
            self._settle(c)
            return managed[1]["version"] if len(managed) > 1 else ""

    def component_use(self, ident, build):
        with self._lock:
            c = self._component(ident)
            if c["kind"] == "tool":
                raise UniverseError("Invalid", f"{ident} has no choice of build: the one on PATH runs, else Universe's")
            managed = [b for b in c["builds"] if b["managed"]]
            if build == "latest":
                pick = managed[0] if managed else None
            elif build == "system":
                pick = next((b for b in c["builds"] if not b["managed"]), None)
            else:
                pick = next((b for b in c["builds"] if build in (b["version"], b.get("name"))), None)
            if pick is None:
                raise UniverseError("NotFound", f"{ident} {build} is not installed")
            for b in c["builds"]:
                b["in_use"] = b is pick
            c["setting"] = "" if build == "system" else c["family"] if build == "latest" and c["kind"] == "proton" else build
            self._settle(c)

    def component_cancel(self, ident):
        self._component_cancel = ident
        return True

    def _extension_row(self, listed, mine):
        either = mine or listed
        api = (mine or {}).get("api", (listed or {}).get("api", 0))
        incompatible = "" if api == EXTENSION_API else f"written for extension api {api}: this Universe reads api {EXTENSION_API}"
        newer = listed and mine and mine["origin"] == "registry" and listed["version"] != mine["version"] and listed["api"] == EXTENSION_API
        update = listed["version"] if newer else ""
        enabled = any(e["id"] == either["id"] and e.get("enabled") for e in self._data.get(either["kind"] + "s", []))
        return {
            **{k: either[k] for k in ("id", "kind", "name", "description")},
            "version": (listed or mine)["version"],
            "homepage": (listed or {}).get("homepage", ""),
            "size": (listed or {}).get("size", 0),
            "listed": listed is not None,
            "installed": mine is not None,
            "installed_version": (mine or {}).get("version", ""),
            "origin": (mine or {}).get("origin", ""),
            "from": (mine or {}).get("from", ""),
            "update": update,
            "enabled": enabled,
            "incompatible": incompatible,
        }

    def extensions(self):
        with self._lock:
            rows = [self._extension_row(listed, self._extensions.get(listed["id"])) for listed in self._index]
            listed = {e["id"] for e in self._index}
            rows += [self._extension_row(None, mine) for ident, mine in self._extensions.items() if ident not in listed]
        rows.sort(key=lambda r: (r["kind"], r["name"].lower()))
        return {"index": {"url": EXTENSION_INDEX, "error": ""}, "extensions": copy.deepcopy(rows)}

    def _themes_dir(self):
        return Path(self.data_home()) / "extensions" / "theme"

    def _theme_of(self, folder):
        """A folder's theme.toml as an index entry, checked as the core checks it; None for a folder of another kind."""
        try:
            manifest = tomllib.loads((folder / "theme.toml").read_text())
        except OSError:
            return None
        except tomllib.TOMLDecodeError as e:
            raise UniverseError("Invalid", f"theme.toml: {e}") from None
        ident = manifest.get("id") or ""
        if not re.fullmatch(r"[a-z0-9][a-z0-9_-]*", str(ident)):
            raise UniverseError("Invalid", f"theme.toml: id {ident!r} is not lowercase letters, digits, - and _")
        if not (folder / THEME_ENTRY).is_file():
            raise UniverseError("Invalid", f"a theme starts at its {THEME_ENTRY}, which it lacks")
        if ident in BUILTIN_THEMES:
            raise UniverseError("Invalid", f"{ident} ships with Universe: a theme of that id cannot be installed")
        if manifest.get("api", 0) != EXTENSION_API:
            raise UniverseError("Invalid", f"{ident}: written for extension api {manifest.get('api', 0)}: this Universe reads api {EXTENSION_API}")
        fields = {k: manifest.get(k) or "" for k in ("name", "version", "description")}
        return {"id": ident, "kind": "theme", **fields, "name": fields["name"] or ident, "api": EXTENSION_API}

    def extension_install(self, what, accepted=False, progress=None):
        if not accepted:
            raise UniverseError("Invalid", f"{what} runs programs as you: its install waits to be accepted")
        listed = next((e for e in self._index if e["id"] == what), None)
        if listed is None and "/" not in what:
            raise UniverseError("NotFound", f"{what} is not in the extension index ({EXTENSION_INDEX})")
        if listed is not None and listed["api"] != EXTENSION_API:
            raise UniverseError("Invalid", f"{what}: written for extension api {listed['api']}: this Universe reads api {EXTENSION_API}")
        folder = Path(what.removeprefix("file://")).expanduser() if listed is None else None
        theme = self._theme_of(folder) if folder is not None else None
        for step in range(1, 5):
            if progress is not None:
                progress(step, 4, f"Downloading {(listed or theme or {}).get('name', what)}")
            time.sleep(0.05)
        if theme is not None:
            listed = theme
        elif listed is None:
            ident = what.rstrip("/").rpartition("/")[2]
            listed = {"id": ident, "kind": "module", "name": ident.replace("-", " ").title(), "version": "1.0.0", "description": "", "api": EXTENSION_API}
        origin = "unlisted" if "/" in what else "registry"
        with self._lock:
            self._place_extension(listed, origin, what if origin == "unlisted" else EXTENSION_INDEX, folder)
        return {"id": listed["id"], "kind": listed["kind"], "name": listed["name"], "version": listed["version"], "origin": origin}

    def _place_theme(self, listed, folder):
        dest = self._themes_dir() / listed["id"]
        shutil.rmtree(dest, ignore_errors=True)
        if folder is not None:
            shutil.copytree(folder, dest, symlinks=True)
            return
        dest.mkdir(parents=True)
        fields = "".join(f"{k} = {json.dumps(listed[k])}\n" for k in ("id", "name", "version", "description"))
        (dest / "theme.toml").write_text(f"api = {listed['api']}\n{fields}")
        (dest / THEME_ENTRY).write_text(STUB_THEME)

    def _place_extension(self, listed, origin, source, folder=None):
        kind = listed["kind"] + "s"
        mine = {**{k: listed[k] for k in ("id", "kind", "name", "version", "description", "api")}, "origin": origin, "from": source}
        self._extensions[listed["id"]] = mine
        if listed["kind"] == "theme":
            self._place_theme(listed, folder)
            return
        entries = self._data.setdefault(kind, [])
        was = next((e for e in entries if e["id"] == listed["id"]), None)
        entry = {"id": listed["id"], "name": listed["name"], "version": listed["version"], "description": listed["description"]}
        entry.update(enabled=bool(was and was.get("enabled")), available=True, missing=[], settings=[], origin=origin)
        if kind == "sources":
            entry.update(capabilities=[], games_dir="", library_at="", logged_in=False, user="")
            entry["login"] = {"kind": "code", "hint": "Open the link, sign in, then enter the code it shows.", "purpose": "install games"}
        entries[:] = [e for e in entries if e["id"] != listed["id"]] + [entry]

    def extension_update(self, ident="", progress=None):
        if ident and ident not in self._extensions:
            raise UniverseError("NotFound", f"{ident} is not an installed extension")
        out = []
        for row in self.extensions()["extensions"]:
            if (ident and row["id"] != ident) or not row["update"]:
                continue
            listed = next(e for e in self._index if e["id"] == row["id"])
            if progress is not None:
                progress(1, 1, f"Downloading {listed['name']}")
            with self._lock:
                self._place_extension(listed, "registry", EXTENSION_INDEX)
            out.append({"id": row["id"], "kind": row["kind"], "name": row["name"], "version": listed["version"]})
        return out

    def extension_remove(self, ident):
        with self._lock:
            mine = self._extensions.pop(ident, None)
            if mine is None:
                if ident in BUILTIN_THEMES:
                    raise UniverseError("Invalid", f"{ident} ships with Universe; it stays")
                shipped = any(e["id"] == ident for e in self._data.get("modules", []) + self._data.get("sources", []))
                if shipped:
                    raise UniverseError("Invalid", f"{ident} ships with Universe; turn it off instead")
                raise UniverseError("NotFound", f"{ident} is not an installed extension")
            if mine["kind"] == "theme":
                shutil.rmtree(self._themes_dir() / ident, ignore_errors=True)
                return
            kind = mine["kind"] + "s"
            self._data[kind] = [e for e in self._data.get(kind, []) if e["id"] != ident]

    def themes(self):
        out = []
        for manifest in sorted(self._themes_dir().glob("*/theme.toml")):
            try:
                m = tomllib.loads(manifest.read_text())
            except (OSError, tomllib.TOMLDecodeError):
                continue
            ident, folder = m.get("id") or "", manifest.parent
            if not ident:
                continue

            def inside(rel, folder=folder):
                path = folder / rel
                return str(path) if rel and not Path(rel).is_absolute() and ".." not in Path(rel).parts and path.is_file() else ""

            api = m.get("api", 0)
            fields = {k: m.get(k) or "" for k in ("name", "version", "description", "author", "license")}
            out.append(
                {
                    "id": ident,
                    **fields,
                    "name": fields["name"] or ident,
                    "origin": (self._extensions.get(ident) or {}).get("origin") or "unlisted",
                    "dir": str(folder),
                    "entry": inside(THEME_ENTRY),
                    "screenshot": inside(m.get("screenshot") or ""),
                    "incompatible": "" if api == EXTENSION_API else f"written for extension api {api}: this Universe reads api {EXTENSION_API}",
                }
            )
        return sorted(out, key=lambda t: t["id"])

    def discover(self):
        report = copy.deepcopy(self._data.get("discover") or {"launchers": [], "gog_dirs": []})
        known = {g["id"] for g in self._data["games"]}
        for launcher in report["launchers"]:
            if launcher["id"] == "lutris":
                pending = [i for i in self._data.get("lutris", {}).get("imported", []) if i not in known]
                launcher["games"], launcher["titles"] = len(pending), [i.replace("-", " ").title() for i in pending]
            elif launcher["id"] == "roms":
                pending = self.import_roms(False)["imported"]
                launcher["games"], launcher["titles"] = len(pending), [f["title"] for f in pending][:6]
        return report

    def settings(self):
        computed = {"config_file": str(self._root / "config" / "config.toml"), "data_home": self.data_home(), "os": "other"}
        return {**copy.deepcopy(self._config), **computed, "set": copy.deepcopy(self._set)}

    # The core's own forms and writes, built from the views this fake answers: no form logic of its own.
    def _form_inputs(self, kind, ident):
        return {
            "set": self._set,
            "protons": list(self._config.get("protons") or []),
            "game": self.get(ident) if kind == "game" else None,
            "modules": self.modules(),
            "sources": self.sources(),
            "runners": self.runners(),
            "gpu": self.gpu(),
            "under_steam": self.under_steam(),
        }

    def form(self, kind, ident, screen=None):
        import universe_core

        return universe_core._form_fields(kind, ident, self._form_inputs(kind, ident), screen)

    def set_field(self, kind, ident, key, value):
        import universe_core

        self._apply(universe_core._set_writes(kind, ident, key, value))

    def promote_field(self, kind, ident, key):
        import universe_core

        self._apply(universe_core._all_games_writes(kind, ident, key, None, self._form_inputs(kind, ident)))

    def set_field_all(self, kind, ident, key, value):
        import universe_core

        self._apply(universe_core._all_games_writes(kind, ident, key, value, self._form_inputs(kind, ident)))

    def _apply(self, writes):
        setters = {
            "config": lambda w: self.set_setting(w["key"], w["value"]),
            "runner": lambda w: self.set_runner_setting(w["runner"], w["key"], w["value"]),
            "game": lambda w: self.set(w["game"], w["key"], w["value"]),
            "module": lambda w: self.set_module_setting(w["module"], "", w["key"], w["value"]),
            "source": lambda w: self.set_source_setting(w["source"], w["key"], w["value"]),
            "enable_module": lambda w: self.enable_module(w["module"], w["on"]),
            "enable_source": lambda w: self.enable_source(w["source"], w["on"]),
        }
        for write in writes:
            setters[write["to"]](write)

    def set_setting(self, key, value):
        for root in (self._config, self._set):
            self._write_setting(root, key, value)

    @staticmethod
    def _write_setting(root, key, value):
        node = root
        parts = key.split(".")
        for part in parts[:-1]:
            if not isinstance(node.get(part), dict):
                node[part] = {}
            node = node[part]
        if value == "":
            node.pop(parts[-1], None)
        elif (len(parts) == 3 and parts[0] == "launch" and parts[1] in ("env", "dll_overrides")) or key in RATE_KEYS:
            node[parts[-1]] = value
        elif value in ("true", "false"):
            node[parts[-1]] = value == "true"
        else:
            try:
                node[parts[-1]] = int(value)
            except ValueError:
                node[parts[-1]] = value

    def screen_mode(self, screen):
        return dict(self._data.get("screen") or {"screen": screen or "DP-1", "width": 2560, "height": 1440, "refresh": 144, "vrr": True})

    def gpu(self):
        return copy.deepcopy(self._data.get("gpu", GPU))

    def launch_keys(self, scope, screen):
        if scope not in ("game", "global", "both"):
            raise UniverseError("Invalid", f"scope must be game, global or both, not '{scope}'")
        mode = dict(screen or {})
        w, h, hz = int(mode.get("width") or 0), int(mode.get("height") or 0), int(mode.get("refresh") or 0)
        rates = [str(r) for r in (sorted({hz, *[r for r in REFRESH_RATES if r < hz]}, reverse=True) if hz else REFRESH_RATES)]
        resolutions = ["auto", "1920x1080", "1280x720"]
        if w and h:
            resolutions = ["auto"]
            for height in [h, *RESOLUTION_HEIGHTS]:
                wh = f"{round(w * height / h / 2) * 2}x{height}"
                if height <= h and wh not in resolutions:
                    resolutions.append(wh)
        choices = {"resolution": resolutions, "refresh": ["auto", *rates], "fps": ["auto", "none", *rates]}
        out = []
        for spec in self._launch_keys:
            if scope != "both" and spec["scope"] not in ("both", scope):
                continue
            if self.under_steam() and spec["key"] in ("mangohud", "fps_limit", "pause_on_home"):
                continue
            row = copy.deepcopy(spec)
            row["choices"] = choices.get(spec["type"], row["choices"])
            out.append(row)
        return out

    def _controller(self):
        return self._data.setdefault("controller", {"families": [], "macros": [], "presets": []})

    def _controller_family(self, ident):
        for family in self._controller().get("families", []):
            if family["id"] == ident:
                return family
        raise UniverseError("NotFound", f"no controller family '{ident}'")

    def controller_state(self):
        state = {k: v for k, v in copy.deepcopy(self._controller()).items() if k != "devices"}
        config = self._config["controller"]
        return {**state, **{k: config[k] for k in ("enabled", "hold_ms", "home_summons", "volume_step")}}

    def controller_pads(self):
        pads = []
        for pad in self._controller().get("devices", []):
            family = self._controller_family(pad["family"])
            slots = {}
            for slot in family.get("slots", []):
                codes = slot.get("codes") or []
                slots[slot["id"]] = {"code": codes[0] if codes else None, "bound": bool(codes)}
            pads.append(
                {"id": pad["id"], "name": family["name"], "family": family["id"], "family_name": family["name"], "bus": pad.get("bus", "usb"), "slots": slots}
            )
        return pads

    def set_controller_macro(self, macro):
        state = self._controller()
        presets = {p["id"]: p for p in state.get("presets", [])}
        family, button = str(macro.get("family") or ""), str(macro.get("button") or "")
        trigger, action = str(macro.get("trigger") or ""), str(macro.get("action") or "")
        if trigger not in ("press", "hold"):
            raise UniverseError("Invalid", f"trigger must be press or hold, not '{trigger}'")
        if action not in presets:
            raise UniverseError("Invalid", f"unknown action '{action}'")
        if presets[action].get("hold_only") and trigger != "hold":
            raise UniverseError("Invalid", f"{action} fires on a hold only")
        if family != "*" and button not in {s["id"] for s in self._controller_family(family)["slots"]}:
            raise UniverseError("NotFound", f"{family} has no button '{button}'")
        entry = {
            "family": family,
            "button": button,
            "trigger": trigger,
            "action": action,
            "keys": str(macro.get("keys") or ""),
            "command": str(macro.get("command") or ""),
        }
        state["macros"] = [m for m in state.get("macros", []) if (m["family"], m["button"], m["trigger"]) != (family, button, trigger)]
        state["macros"].append(entry)

    def remove_controller_macro(self, family, button, trigger):
        state = self._controller()
        state["macros"] = [
            m for m in state.get("macros", []) if not (m["family"] == family and m["button"] == button and (not trigger or m["trigger"] == trigger))
        ]

    def set_controller_button(self, family, slot, codes):
        for entry in self._controller_family(family)["slots"]:
            if entry["id"] == slot:
                entry["codes"] = [str(c) for c in codes or []]
                return
        raise UniverseError("NotFound", f"{family} has no slot '{slot}'")
