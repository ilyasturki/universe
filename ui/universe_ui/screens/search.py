"""Settings search: an index of every settings row — the pages, the runners', the modules' and sources', the controller's, each game's — and a
ranked, fuzzy match over it as the query is typed. A result carries `target`, what the page opens: {page, id, key, module}."""

import logging
import re
from collections.abc import Callable

from PySide6.QtCore import QObject, Signal, Slot

from ..models import file_url
from ..qt import Property
from .launch import build_launch
from .runners import build_runner
from .settings import ADVANCED_KEY, ModuleApi, SourceApi, build_game, build_page

# Words a setting is known by that its label and description do not carry; keyed by the row's key without `launch.`.
SYNONYMS = {
    "gamescope": ["compositor", "window", "fullscreen"],
    "gamescope_resolution": ["res", "1080p", "1440p", "4k", "720p", "render scale", "downscale"],
    "gamescope_refresh": ["hz", "hertz", "60hz", "120hz", "144hz"],
    "gamescope_adaptive_sync": ["vrr", "freesync", "gsync", "g-sync", "variable refresh", "tearing"],
    "mangohud": ["hud", "overlay", "fps counter", "frametime", "performance"],
    "fps_limit": ["fps", "frame rate", "framerate", "cap", "limiter", "vsync"],
    "pause_on_home": ["freeze", "suspend", "home button", "online"],
    "gamescope_scaler": ["integer scaling", "aspect ratio", "stretch", "fit"],
    "gamescope_filter": ["fsr", "nis", "nearest", "pixel art", "upscale", "sharpen", "linear"],
    "gamescope_sharpness": ["sharpen", "fsr", "nis"],
    "gamescope_args": ["flags", "command line", "extra arguments"],
    "env": ["environment", "variables", "export", "env vars"],
    "debug_log": ["logs", "proton log", "wine log", "dxvk", "crash", "debugging", "troubleshoot"],
    "wrapper": ["gamemode", "gamemoderun", "taskset", "prime-run", "mangohud"],
    "args": ["arguments", "parameters", "command line", "flags"],
    "working_dir": ["cwd", "folder", "directory"],
    "pre_command": ["hook", "script", "before launch"],
    "post_command": ["hook", "script", "after exit"],
    "proton": ["compatibility", "wine", "ge", "cachyos", "em"],
    "wayland": ["xwayland", "x11", "display server", "latency"],
    "hdr": ["high dynamic range", "10 bit"],
    "prefix": ["wineprefix", "pfx", "bottle", "drive c"],
    "arch": ["64 bit", "32 bit", "win32", "win64"],
    "umu_id": ["gameid", "protonfixes", "umu"],
    "store": ["protonfixes", "umu", "gog", "egs"],
    "dll_overrides": ["winedlloverrides", "dll", "native", "builtin"],
    "esync": ["sync", "eventfd", "synchronisation", "synchronization"],
    "fsync": ["sync", "futex", "synchronisation", "synchronization"],
    "ntsync": ["sync", "nt", "synchronisation", "synchronization", "kernel"],
    "dlss_upgrade": ["nvidia", "dlss", "rtx", "upscaler"],
    "fsr4_upgrade": ["amd", "radeon", "fsr", "rdna", "upscaler"],
    "xess_upgrade": ["intel", "arc", "xess", "upscaler"],
    "optiscaler": ["fsr", "xess", "dlss", "upscaler", "mod"],
    "gamescope_bin": ["binary", "executable", "path"],
    "umu_run": ["umu", "binary", "executable", "path"],
    "hide_cursor": ["mouse", "pointer", "cursor"],
    "favorite": ["favourite", "star", "pinned"],
    "hidden": ["hide", "invisible", "unlisted"],
    "tags": ["labels", "groups", "collections"],
    "sgdb_id": ["steamgriddb", "artwork", "grid", "cover"],
    "rawg_id": ["rawg", "metadata", "description"],
    "games_root": ["install folder", "library folder", "games directory"],
    "prefixes_root": ["wineprefix", "pfx", "bottles"],
    "recordings_root": ["videos", "captures", "clips"],
    "journal_root": ["notes", "markdown", "diary"],
    "overrides": ["artwork", "picks", "custom art"],
    "sgdb": ["steamgriddb", "api key", "token", "artwork"],
    "sgdb_file": ["steamgriddb", "api key", "token"],
    "rawg": ["api key", "token", "metadata"],
    "rawg_file": ["api key", "token"],
    "profile": ["gnome", "shell", "integration", "desktop environment"],
    "cursor_extension": ["gnome", "shell extension", "cursor"],
    "hold_ms": ["long press", "hold", "duration", "milliseconds"],
    "volume_step": ["volume", "loudness", "sound", "percent"],
    "codec": ["av1", "hevc", "h264", "h265", "encoder"],
    "quality": ["bitrate", "size", "compression"],
    "container": ["mkv", "mp4", "format"],
    "audio": ["sound", "microphone", "tracks"],
    "audio_codec": ["opus", "aac", "sound"],
    "audio_bitrate": ["kbps", "sound quality"],
    "min_duration_s": ["short sessions", "discard", "minimum length"],
    "window_wait_s": ["timeout", "delay"],
    "source": ["window", "screen", "capture", "picker"],
    "cursor": ["mouse", "pointer"],
    "provider": ["openai", "codex", "claude", "model", "ai"],
    "model": ["gpt", "claude", "llm", "ai"],
    "language": ["locale", "french", "english"],
    "markdown_export": ["notes", "obsidian", "export"],
    "platform": ["windows", "linux", "depot"],
    "with_dlcs": ["dlc", "expansions", "addons"],
    "games_dir": ["install folder", "directory"],
    "scan_dirs": ["scan", "folders", "detect"],
    "theme": ["look", "skin", "appearance", "dark", "switch", "reprise"],
    "enabled": ["on", "off", "enable", "disable", "toggle"],
    "test": ["buttons", "sticks", "triggers", "pad", "input"],
    "device": ["pad", "gamepad", "joypad"],
    "exe": ["program", "binary", "executable", "path", "rom"],
}

# The sidebar sections, by id.
SECTION_SYNONYMS = {
    "launch": ["display", "resolution", "overlay", "general", "global"],
    "runners": ["proton", "wine", "emulators", "dolphin", "ryujinx", "rpcs3", "pcsx2"],
    "controller": ["gamepad", "pad", "macros", "buttons", "paddles", "dualsense", "xbox"],
    "controllers": ["gamepad", "pad", "macros", "buttons", "paddles", "dualsense", "xbox"],
    "sources": ["gog", "store", "shop", "login", "sign in"],
    "install": ["download", "store", "shop", "library", "owned"],
    "updates": ["upgrade", "patch", "pending"],
    "modules": ["capture", "recording", "journal", "hooks", "extensions"],
    "artwork": ["art", "covers", "boxart", "logo", "banner", "steamgriddb"],
    "themes": ["look", "skin", "appearance", "reprise", "switch"],
    "doctor": ["health", "checks", "prerequisites", "missing", "diagnose"],
    "about": ["version", "build", "info"],
    "quit": ["exit", "close", "leave", "power off"],
    "search": [],
}

SKIPPED_KEYS = {"", ADVANCED_KEY, "game", "add_file", "link", "code", "logged_in", "test", "device"}
log = logging.getLogger("universe.search")


def normal(text):
    return re.sub(r"[^a-z0-9]+", " ", str(text or "").casefold()).strip()


def _edit_distance(a, b, limit):
    if abs(len(a) - len(b)) > limit:
        return limit + 1
    previous = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        current = [i]
        for j, cb in enumerate(b, 1):
            current.append(min(previous[j] + 1, current[j - 1] + 1, previous[j - 1] + (ca != cb)))
        if min(current) > limit:
            return limit + 1
        previous = current
    return previous[-1]


def _typo_budget(token):
    return 2 if len(token) >= 8 else 1 if len(token) >= 5 else 0


def _subsequence(token, word):
    # The token's letters in order inside one word, from its first letter: "wrk" finds "working".
    if not word.startswith(token[0]):
        return False
    it = iter(word)
    return all(c in it for c in token)


def word_score(token, words):
    """How well a token matches a list of words: exact, prefix, a typo away, or in order across the letters."""
    best = 0
    budget = _typo_budget(token)
    for word in words:
        if word == token:
            return 100
        if word.startswith(token):
            best = max(best, 90)
        elif token in word:
            best = max(best, 75)
        elif budget and _edit_distance(token, word, budget) <= budget:
            best = max(best, 60)
    if best == 0 and len(token) >= 3 and any(_subsequence(token, w) for w in words):
        best = 35
    return best


class Entry:
    __slots__ = (
        "advanced",
        "detail",
        "detail_n",
        "display",
        "games",
        "icon",
        "image",
        "key",
        "keywords",
        "kind",
        "label",
        "module",
        "order",
        "path",
        "path_n",
        "target",
        "value_n",
        "words",
    )
    made = 0

    def __init__(
        self, label, path, target, kind="setting", detail="", display="", key="", module="", advanced=False, games=None, image="", icon="", synonyms=()
    ):
        self.label, self.path, self.target, self.kind = str(label), list(path), dict(target), kind
        self.detail, self.display, self.key, self.module, self.advanced = str(detail or ""), str(display or ""), key, module, bool(advanced)
        self.games, self.image, self.icon = games if games is not None else [], image, icon
        self.words = normal(label).split()
        short = key.split(".")[-1]
        self.keywords = [normal(s) for s in list(synonyms) + SYNONYMS.get(short, []) + ([short.replace("_", " ")] if short else [])]
        self.detail_n = normal(detail)
        self.value_n = normal(display)
        self.path_n = normal(" ".join(path))
        Entry.made += 1
        self.order = Entry.made

    def score(self, tokens):
        total = 0
        for token in tokens:
            best = word_score(token, self.words)
            for keyword in self.keywords:
                if keyword == token or keyword.startswith(token + " ") or (" " + token) in (" " + keyword):
                    best = max(best, 70)
                    break
            if best < 50 and self.value_n and (token in self.value_n.split() or any(w.startswith(token) for w in self.value_n.split())):
                best = max(best, 50)
            if best < 40 and self.detail_n and (" " + token) in (" " + self.detail_n):
                best = max(best, 30)
            if best < 25 and self.path_n and (" " + token) in (" " + self.path_n):
                best = max(best, 20)
            if best == 0:
                return 0
            total += best
        return total

    def row(self):
        path = " › ".join(self.path)
        row = {
            "label": self.label,
            "display": self.display,
            "type": "action",
            "action": "Open",
            "path": path,
            "tag": "ADVANCED" if self.advanced else "",
            "detail": self.detail,
            "advanced": self.advanced,
            "target": self.target,
            "kind": self.kind,
            "key": self.key,
            "module": self.module,
            "choices": [],
            "value": "",
            "inherited": False,
            "image": self.image,
            "icon": self.icon,
        }
        return row


class SettingsSearch(QObject):
    queryChanged = Signal()
    resultsChanged = Signal()
    readyChanged = Signal()
    sectionsChanged = Signal()

    def __init__(self, client, screen_mode: Callable[[], dict] = dict, themes: Callable[[], list] = list, controller=None, parent=None):
        super().__init__(parent)
        self._client = client
        self._screen_mode = screen_mode
        self._themes = themes
        self._controller = controller
        self._sections = []
        self._entries = []
        self._games = []
        self._query = ""
        self._results = []
        self._expanded = set()
        self._ready = False
        self._loading = False
        self._stale = False

    # --- the index ---

    @Slot()
    def load(self):
        if self._loading:
            self._stale = True
            return
        self._loading = True
        # The pad's rows are read here, on the UI thread; the index is built off it.
        controller_rows = [dict(r) for r in self._controller.rows] if self._controller is not None else []
        self._client.runAsync(lambda: self._build(controller_rows), self._built)

    def _built(self, built):
        self._loading = False
        self._entries, self._games = built
        self._ready = True
        self.readyChanged.emit()
        self._search()
        if self._stale:
            self._stale = False
            self.load()

    def _build(self, controller_rows):
        entries = []
        try:
            self._index_sections(entries)
            self._index_launch(entries)
            self._index_runners(entries)
            self._index_pages(entries)
            self._index_controller(entries, controller_rows)
            self._index_themes(entries)
            games = self._index_games(entries)
        except Exception:  # a half-built index is still an index; the log line says what broke
            log.exception("settings index")
            games = []
        return entries, games

    def _index_sections(self, entries):
        for section in self._sections:
            ident = str(section.get("id") or "")
            if ident in ("search", ""):
                continue
            entries.append(
                Entry(
                    section.get("label") or ident,
                    ["Settings"],
                    {"page": "section", "id": ident, "key": "", "module": ""},
                    kind="section",
                    icon=str(section.get("icon") or ""),
                    synonyms=SECTION_SYNONYMS.get(ident, []),
                )
            )

    def _index_launch(self, entries):
        rows, groups, _ = build_launch(self._client, self._screen_mode)
        self._index_rows(entries, rows, groups, ["Launch"], {"page": "launch", "id": "", "module": ""})

    def _index_runners(self, entries):
        for runner in self._client.runners():
            info, rows, groups = build_runner(self._client, runner["id"], self._screen_mode)
            if not info:
                continue
            entries.append(
                Entry(
                    info["name"],
                    ["Runners"],
                    {"page": "runner", "id": runner["id"], "key": "", "module": ""},
                    detail=info.get("meta", ""),
                    display="" if not info.get("warning") else info["warning"],
                    image=info.get("icon", ""),
                    synonyms=["runner", "emulator"] if runner.get("kind") == "emulator" else ["runner"],
                )
            )
            self._index_rows(entries, rows, groups, ["Runners", info["name"]], {"page": "runner", "id": runner["id"], "module": ""})

    def _index_pages(self, entries):
        for api in (_ModuleIndex(self._client), _SourceIndex(self._client)):
            listing = api._entries()
            for entry in listing:
                info, rows, groups = build_page(api, entry["id"], listing)
                if not info:
                    continue
                page = "source" if api.source else "module"
                entries.append(
                    Entry(
                        info["name"],
                        [api.kind],
                        {"page": page, "id": entry["id"], "key": "enabled", "module": entry["id"]},
                        detail=info.get("description", ""),
                        display="On" if info["enabled"] else "Off",
                        synonyms=["source", "store"] if api.source else ["module", "hooks"],
                    )
                )
                self._index_rows(entries, rows, groups, [api.kind, info["name"]], {"page": page, "id": entry["id"], "module": entry["id"]}, skip_control=True)

    def _index_controller(self, entries, rows):
        for row in rows:
            if row.get("type") == "info":
                continue
            key = str(row.get("key") or "")
            if key in SKIPPED_KEYS and "slot" not in row:
                continue
            entries.append(
                Entry(
                    row["label"],
                    ["Controller"],
                    {"page": "controller", "id": "", "key": key, "module": ""},
                    detail=row.get("detail", ""),
                    display=row.get("display", ""),
                    key=key,
                    advanced=bool(row.get("advanced")),
                    synonyms=["macro", "button", "paddle"] if "slot" in row else (),
                )
            )

    def _index_themes(self, entries):
        themes = list(self._themes() or [])
        current = next((t["name"] for t in themes if t.get("current")), "")
        entries.append(
            Entry(
                "Theme",
                ["Themes"],
                {"page": "themes", "id": "", "key": "theme", "module": ""},
                display=current,
                key="theme",
                detail="The look of the launcher, switched live.",
            )
        )
        for theme in themes:
            entries.append(
                Entry(
                    theme["name"],
                    ["Themes"],
                    {"page": "themes", "id": theme["id"], "key": "theme", "module": ""},
                    detail=theme.get("detail", ""),
                    display="Current" if theme.get("current") else "",
                    synonyms=["theme", "look", "skin"],
                )
            )

    def _index_rows(self, entries, rows, groups, path, target, skip_control=False):
        for group in groups:
            crumbs = path + ([group["title"]] if group.get("title") not in ("", None, "Settings") and group["title"] not in path[-1:] else [])
            for i in group["rows"]:
                row = rows[i]
                key = str(row.get("key") or "")
                if key in SKIPPED_KEYS or row.get("map") or row.get("type") in ("info", "static") or (skip_control and key == "enabled"):
                    continue
                entries.append(
                    Entry(
                        row["label"],
                        crumbs,
                        {**target, "key": key, "module": str(row.get("module") or target.get("module") or "")},
                        detail=row.get("detail", ""),
                        display=row.get("display", ""),
                        key=key,
                        module=str(row.get("module") or ""),
                        advanced=bool(row.get("advanced")),
                    )
                )

    def _index_games(self, entries):
        """One entry per game-scope key across the library — `games` lists each game with its value — and one per game."""
        keyed = {}
        games = []
        for game in self._client.list():
            if game.get("removed_at") or game.get("hidden"):
                continue
            game_id = str(game.get("id") or "")
            try:
                rows, groups, title = build_game(self._client, game_id, self._screen_mode)
            except Exception:  # one broken game does not empty the index
                log.exception("settings index: %s", game_id)
                continue
            media = game.get("media") or {}
            art = file_url(next((p for p in (media.get("square"), media.get("box_front")) if p), "")).toString()
            games.append({"id": game_id, "title": title, "image": art})
            entries.append(
                Entry(
                    title,
                    ["Games"],
                    {"page": "game", "id": game_id, "key": "", "module": ""},
                    kind="game",
                    image=art,
                    detail="Game settings",
                    synonyms=["game", "settings"],
                )
            )
            for group in groups:
                for i in group["rows"]:
                    row = rows[i]
                    key = str(row.get("key") or "")
                    if key in SKIPPED_KEYS or row.get("map") or row.get("entry") or row.get("type") in ("info", "static"):
                        continue
                    ident = (str(row.get("module") or ""), key)
                    entry = keyed.get(ident)
                    if entry is None:
                        entry = keyed[ident] = Entry(
                            row["label"],
                            ["Games", group["title"]],
                            {"page": "game", "id": "", "key": key, "module": ident[0]},
                            kind="gamekey",
                            detail=row.get("detail", ""),
                            key=key,
                            module=ident[0],
                            advanced=bool(row.get("advanced")),
                            games=[],
                        )
                    entry.games.append(
                        {
                            "id": game_id,
                            "title": title,
                            "image": art,
                            "display": str(row.get("display") or ""),
                            "own": not row.get("inherited", False),
                            "origin": str(row.get("origin") or ""),
                            "value": normal(row.get("display")),
                        }
                    )
        # A game overrides a key only where a global value exists to override: the launch page's and the runners' keys.
        global_keys = {(e.module, e.key) for e in entries if e.kind == "setting" and e.key}
        for (module, key), entry in keyed.items():
            if (module, key) not in global_keys:
                for game in entry.games:
                    game["own"] = None
        entries.extend(keyed.values())
        return games

    # --- the query ---

    def _set_query(self, value):
        value = str(value)
        if value == self._query:
            return
        self._query = value
        self._expanded = set()
        self.queryChanged.emit()
        self._search()

    def _set_sections(self, sections):
        self._sections = [dict(s) for s in (sections or [])]
        self.sectionsChanged.emit()

    @Slot(int)
    def expand(self, index):
        row = self._results[index] if 0 <= index < len(self._results) else None
        if not row or row.get("kind") != "gamekey":
            return
        ident = (row["module"], row["key"])
        if ident in self._expanded:
            self._expanded.discard(ident)
        else:
            self._expanded.add(ident)
        self._search()

    @Slot(int, result="QVariant")
    def row(self, index):
        return dict(self._results[index]) if 0 <= index < len(self._results) else {}

    def _search(self):
        tokens = normal(self._query).split()
        if not tokens:
            self._results = []
            self.resultsChanged.emit()
            return
        settings = [e for e in self._entries if e.kind != "game"]
        # A word no setting knows but a game's title does names the game: the rest of the query is what to find in it.
        filters, wanted = [], []
        for token in tokens:
            if any(e.score([token]) for e in settings):
                wanted.append(token)
            elif any(word_score(token, normal(g["title"]).split()) for g in self._games):
                filters.append(token)
            else:
                self._results = []
                self.resultsChanged.emit()
                return
        picked = [g for g in self._games if all(word_score(t, normal(g["title"]).split()) for t in filters)] if filters else []
        scored = []
        for entry in self._entries:
            if entry.kind == "game":
                if filters and entry.target["id"] in {g["id"] for g in picked} and not wanted:
                    scored.append((100 + max(word_score(t, entry.words) for t in filters), entry))
                elif not filters:
                    s = entry.score(tokens)
                    if s:
                        scored.append((s, entry))
                continue
            # A game named in the query narrows the results to it.
            if filters and entry.kind != "gamekey":
                continue
            if not wanted:
                scored.append((1, entry))
                continue
            s = entry.score(wanted)
            if s:
                scored.append((s + (10 if entry.kind == "section" else 5 if entry.kind != "gamekey" else 0), entry))
        scored.sort(key=lambda pair: (-pair[0], pair[1].kind == "gamekey", pair[1].order))
        results = []
        for _, entry in scored[:80]:
            if entry.kind == "gamekey":
                results.extend(self._game_rows(entry, picked if filters else None))
            else:
                results.append(entry.row())
        self._results = results
        self.resultsChanged.emit()

    def _game_rows(self, entry, picked):
        games = entry.games
        if picked is not None:
            ids = {g["id"] for g in picked}
            games = [g for g in games if g["id"] in ids]
            if not games:
                return []
        rows = []
        if picked is None:
            own = sum(1 for g in games if g["own"] is True)
            head = entry.row()
            expanded = (entry.module, entry.key) in self._expanded
            head.update(
                display=f"in {len(games)} game{'' if len(games) == 1 else 's'}" + (f" · {own} override{'' if own == 1 else 's'} it" if own else ""),
                action="Collapse" if expanded else "Expand",
                expanded=expanded,
                count=len(games),
            )
            rows.append(head)
            if not expanded:
                return rows
        for game in games:
            row = entry.row()
            # Under its head the row is the game; filtered to a game it is the setting.
            if picked is None:
                row.update(label=game["title"], path="", image=game["image"], detail="")
            else:
                row.update(label=entry.label, path=game["title"])
            row.update(
                display=game["display"], inherited=game["own"] is False, origin=game["origin"], kind="gamerow", target={**entry.target, "id": game["id"]}
            )
            rows.append(row)
        return rows

    query = Property(str, lambda self: self._query, _set_query, notify=queryChanged)
    sections = Property(list, lambda self: list(self._sections), _set_sections, notify=sectionsChanged)
    results = Property(list, lambda self: [dict(r) for r in self._results], notify=resultsChanged)
    count = Property(int, lambda self: len(self._results), notify=resultsChanged)
    ready = Property(bool, lambda self: self._ready, notify=readyChanged)
    indexed = Property(int, lambda self: len(self._entries), notify=readyChanged)


class _ModuleIndex(ModuleApi):
    def __init__(self, client):
        self._client = client


class _SourceIndex(SourceApi):
    def __init__(self, client):
        self._client = client
