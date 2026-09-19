import copy
import json
import os
import re
import shutil
import subprocess
import tempfile
import threading
import time
from datetime import datetime, timezone
from pathlib import Path

from .errors import UniverseError

FIXTURE = Path(__file__).parent / "fixtures" / "library.json"
LAUNCH_KEYS = Path(__file__).parent / "fixtures" / "launch_keys.json"
GPU = {"vendor": "amd", "name": "AMD Radeon RX 7900 GRE", "rdna": 3, "label": "AMD Radeon RX 7900 GRE · RDNA 3",
       "fits": {"dlss_upgrade": False, "fsr4_upgrade": True, "xess_upgrade": True, "optiscaler": True}}
REFRESH_RATES = [240, 165, 144, 120, 100, 90, 75, 60, 50, 48, 40, 30]
RESOLUTION_HEIGHTS = [2160, 1800, 1440, 1080, 720]
STEP_S = 0.15
SESSION_S = 2.0
WINDOW_S = 0.4
FRAME_S = 0.0
JOURNAL_S = 8.0
CLIP_S = 20

SLOTS = ("box_front", "square", "banner", "background", "logo")


def _now():
    return datetime.now(timezone.utc).astimezone().replace(microsecond=0).isoformat()


def _epoch(value):
    if not value:
        return 0
    try:
        return datetime.fromisoformat(str(value)).timestamp()
    except ValueError:
        return 0


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


class FakeCore:
    def __init__(self, fixture=FIXTURE, root=None, fake_launch=False):
        with open(fixture) as f:
            self._data = json.load(f)
        self._config = dict(self._data.get("config") or {})
        with open(LAUNCH_KEYS) as f:
            self._launch_keys = json.load(f)
        self._config["launch"] = {**{k["key"]: k["default"] for k in self._launch_keys if k["scope"] != "game"}, **(self._config.get("launch") or {})}
        self._tmp = tempfile.TemporaryDirectory(prefix="universe-fake-") if root is None else None
        self._root = Path(root if root is not None else self._tmp.name)
        self._cache = os.path.join(os.environ.get("XDG_CACHE_HOME") or os.path.expanduser("~/.cache"), "universe", "fake-art")
        self._fake_launch = fake_launch
        self._lock = threading.Lock()
        self._timers = []
        self._process = None
        self._session = None
        self._session_started = None
        self._closed = False
        self._installing, self._cancel = "", ""
        self.library_calls = []
        self.last_splash = ""
        self.game_shown = False
        self.frozen = False
        self.hud_shown = False
        self.frames = 0
        self.level, self.muted = 62, False
        self._config.setdefault("paths", {})["overrides"] = str(self._root / "overrides")
        self._lay_out()

    def data_home(self):
        return str(self._root / "data")

    def state_home(self):
        return str(self._root / "state")

    def version(self):
        return "0.0.0-fake"

    def _game_dir(self, ident):
        return self._root / "data" / "games" / ident

    def _lay_out(self):
        from .fixtures.art import paint_library

        (self._root / "state").mkdir(parents=True, exist_ok=True)
        (self._root / "overrides").mkdir(parents=True, exist_ok=True)
        paint_library(self._data["games"], self._cache)
        for game in self._data["games"]:
            self._write_game(game)
            media = game.setdefault("media", {})
            for slot in SLOTS:
                if media.get(slot):
                    media[slot] = _place(media[slot], str(self._game_dir(game["id"]) / "media" / f"{slot}.png"))
            media["screenshots"] = [_place(p, str(self._game_dir(game["id"]) / "media" / f"screenshot{n + 1}.png")) for n, p in enumerate(media.get("screenshots") or [])]
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
                out.append({"game": i, "title": self._game(i)["title"], "path": str(p), "taken_at": taken, "session": session})
        out.sort(key=lambda r: os.path.basename(r["path"]), reverse=True)
        return out

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
        (d / "game.toml.tmp").write_text(f'schema = 1\nid = "{game["id"]}"\ntitle = {json.dumps(game.get("title", game["id"]))}\n# {_now()} {json.dumps(game.get("launch") or {})}\n')
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
        out.setdefault("stats", {"hours": 0, "play_count": 0, "last_played": None})
        out.setdefault("removed", False)
        out["media"] = self._effective_media(game)
        out.pop("overrides", None)
        launch = out.setdefault("launch", {})
        effective = out["effective"] = {}
        for spec in (k for k in self._launch_keys if k["scope"] == "both"):
            own = launch.get(spec["key"])
            effective[spec["key"]] = own if own not in (None, "") else self._config["launch"].get(spec["key"], spec["default"])
        effective["gamescope_args"] = launch.get("gamescope_args") or ""
        effective["hide_cursor"] = out.setdefault("desktop", {}).get("hide_cursor", self._config.get("desktop", {}).get("hide_cursor", True))
        runner = self._runner_of(launch)
        spec = self._runner(runner) or {"id": runner, "name": runner, "kind": "", "platforms": [], "path": "", "options": []}
        options = {o["key"]: o.get("value", o.get("default")) for o in spec.get("options") or []}
        options.update(launch.get("options") or {})
        effective.update({
            "runner": spec["id"], "runner_name": spec.get("name", runner), "runner_kind": spec.get("kind", ""),
            "runner_path": launch.get("runner_exe") or spec.get("path") or "",
            "platform": out.get("platform") or (spec.get("platforms") or [""])[0],
            "options": options, "inputplumber": bool(options.get("inputplumber")),
        })
        out.setdefault("platform", effective["platform"])
        modules = out.setdefault("modules", {})
        for module in self._data.get("modules", []):
            merged = modules.setdefault(module["id"], {})
            for setting in module.get("settings", []):
                if setting.get("scope") == "game":
                    merged.setdefault(setting["key"], setting.get("default"))
        return out

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
            parts = ["modules", "capture"] + parts[1:]
        for part in parts[:-1]:
            node = node.setdefault(part, {})
        leaf = parts[-1]
        if value == "":
            node.pop(leaf, None)
        elif value in ("true", "false"):
            node[leaf] = value == "true"
        elif leaf in ("tags",):
            node[leaf] = [v.strip() for v in value.split(",") if v.strip()]
        else:
            node[leaf] = value
        self._write_game(game)

    def remove(self, ident, purge):
        game = self._game(ident)
        game["removed"] = True
        if purge:
            shutil.rmtree(self._game_dir(ident), ignore_errors=True)
        else:
            self._write_game(game)

    def uninstall(self, ident):
        self._game(ident)
        entries = [e for entries in self._data.get("source_library", {}).values() for e in entries if e.get("game_id") == ident]
        if not any(e.get("installed") for e in entries):
            raise UniverseError("Invalid", f"{ident} has no install folder")
        for entry in entries:
            entry["installed"] = False
            entry["dir"] = None

    def reload(self):
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
                game = {"id": ident, "title": ident.replace("-", " ").title(), "source": "lutris", "favorite": False, "hidden": False,
                        "platform": "windows", "launch": {"runner": "proton", "exe": f"/games/{ident}/{ident}.exe"},
                        "stats": {"hours": report.get("hours_imported", {}).get(ident, 0)}, "metadata": {}, "media": {"screenshots": []}}
                self._data["games"].append(game)
                self._write_game(game)
        return {"backend_promoted": [], "options_promoted": [], "runners": [], "skipped": [], "updated": [], "media_imported": [],
                "env_diffs": [], **copy.deepcopy(report), "applied": bool(apply)}

    def add_game(self, spec):
        runner = self._runner_of({"runner": spec.get("runner", "")})
        runner_spec = self._runner(runner)
        if runner_spec is None:
            raise UniverseError("Invalid", f"unknown runner '{spec.get('runner')}'")
        path = str(spec.get("exe") or "")
        if not path:
            raise UniverseError("Invalid", "a game file is needed")
        title = str(spec.get("title") or "").strip() or os.path.splitext(os.path.basename(path))[0]
        ident = re.sub(r"[^a-z0-9]+", "-", title.casefold()).strip("-")
        if any(g["id"] == ident for g in self._data["games"]):
            raise UniverseError("Invalid", f"{ident} is already in the library")
        game = {
            "id": ident, "title": title, "source": "manual", "favorite": False, "hidden": False,
            "platform": spec.get("platform") or (runner_spec.get("platforms") or [""])[0],
            "launch": {"runner": runner, "exe": path}, "metadata": {}, "media": {"screenshots": []},
        }
        self._data["games"].append(game)
        self._write_game(game)
        return ident

    def runners(self):
        return copy.deepcopy(self._data.get("runners", []))

    def set_runner_setting(self, runner, key, value):
        spec = self._runner(self._runner_of({"runner": runner}))
        if spec is None:
            raise UniverseError("NotFound", f"runner {runner}")
        if key == "exe":
            spec["exe"] = value
            spec["path"] = value or spec.get("detected", "")
            spec["source"] = "config" if value else ("path" if spec.get("detected") else "")
            spec["available"] = bool(spec["path"])
            return
        if key == "args":
            spec["args"] = value
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
        game = self._game(ident)
        with self._lock:
            running = self.current()
            if running:
                raise UniverseError("Busy", f"{running['title']} is running")
            session_id = time.strftime("%Y%m%d-%H%M%S")
            current = {
                "session_id": session_id, "id": ident, "title": game["title"],
                "unit": f"universe-game-{ident}-{session_id}.scope", "screen": screen, "started_at": _now(),
            }
            self._session = current
            self._session_started = time.monotonic()
            self.hud_shown = bool(self._resolved(game)["effective"].get("mangohud"))
            self._marker().write_text(json.dumps({**current, "hook_env": [], "undo": []}))
        if self._fake_launch and shutil.which("sleep"):
            self._process = subprocess.Popen(["sleep", str(int(SESSION_S))])
            process = self._process

            def wait():
                code = process.wait()
                if self._process is process:
                    self._end_session(code)

            threading.Thread(target=wait, daemon=True, name="fake-session").start()
        else:
            self._later(SESSION_S, lambda: self._end_session(0))
        return session_id

    # As `session-end`: the session line first, the marker last.
    def _end_session(self, exit_code):
        with self._lock:
            current, self._session, self._process = self._session, None, None
            if not current or self._closed:
                return
            self.game_shown = self.frozen = self.hud_shown = False
            duration = max(1, int(round(time.monotonic() - self._session_started)))
            game = self._game(current["id"])
            stats = game.setdefault("stats", {"hours": 0, "play_count": 0, "last_played": None})
            stats["hours"] = float(stats.get("hours") or 0) + duration / 3600
            stats["play_count"] = int(stats.get("play_count") or 0) + 1
            stats["last_played"] = _now()
            self._data.setdefault("sessions", {}).setdefault(current["id"], []).insert(0, {
                "session": current["session_id"], "game": current["id"],
                "started_at": current["started_at"], "ended_at": _now(), "duration_s": duration,
                "source": "daemon", "unit": current["unit"], "screen": current["screen"],
                "exit": exit_code, "recording": None,
            })
            self._write_sessions(current["id"])
            self._pend_journal(current["id"], current["session_id"])
            try:
                self._marker().unlink()
            except OSError:
                pass

    def stop(self, session_id):
        if self._process is not None:
            self._process.kill()
        elif self._session:
            self._end_session(-15)
        else:
            raise UniverseError("NotFound", "no session running")

    def adopt_scope(self):
        return ""

    def _window(self):
        return {"id": 1, "pid": os.getpid(), "focused": True}

    def session_window(self):
        if not self.current():
            raise UniverseError("NotFound", "no session running")
        return self._window()

    def wait_session_window(self, session_id, timeout_ms):
        time.sleep(min(WINDOW_S, timeout_ms / 1000))
        current = self.current()
        if not current or current["session_id"] != session_id:
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

    def freeze(self, on):
        if not self.current():
            raise UniverseError("NotFound", "no session running")
        self.frozen = bool(on)

    def nested(self):
        return bool(os.environ.get("GAMESCOPE_WAYLAND_DISPLAY"))

    def nest_game_shown(self):
        return bool(self.current()) and self.game_shown

    def nest_overlay(self, window, input, opacity):
        pass

    def nest_frame(self):
        self.frames += 1
        time.sleep(FRAME_S)
        return os.path.join(self._cache, "screenshot.png")

    def host_gamescope(self, screen):
        gamescope = shutil.which("gamescope")
        return [gamescope, "-f", "--force-composition", "--mangoapp"] if gamescope else None

    def set_fps_limit(self):
        if not self.current():
            raise UniverseError("NotFound", "no session running")
        return "Shift_L+F4"

    def set_mangohud(self, on=None):
        current = self.current()
        if not current:
            raise UniverseError("NotFound", "no session running")
        if on is None:
            on = not self.get(current["id"])["effective"]["mangohud"]
        self.set(current["id"], "launch.mangohud", "true" if on else "false")
        self.hud_shown = on
        return on

    def nest_filter(self, filter, sharpness=None):
        pass

    def volume(self, change, value=0):
        step = int((self._config.get("controller") or {}).get("volume_step") or 2)
        if change == "up":
            self.level = min(100, self.level + step)
        elif change == "down":
            self.level = max(0, self.level - step)
        elif change == "mute":
            self.muted = not self.muted
        elif change == "set":
            self.level = max(0, min(100, int(value)))
        elif change != "get":
            raise UniverseError("Invalid", f"volume: up, down, mute, set or get, not '{change}'")
        return {"percent": self.level, "muted": self.muted, "output": "Fake speakers"}

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

    def _session_row(self, ident, line):
        row = copy.deepcopy(line)
        row.pop("recording_duration_s", None)
        row["title"] = self._game(ident)["title"]
        row["recording"] = None
        if line.get("recording"):
            path = line["recording"] if os.path.isfile(line["recording"]) else self._fake_clip(ident, line["session"])
            exists = os.path.isfile(path)
            size = self._data.get("recordings", {}).get(ident, {}).get(line["session"]) or (os.path.getsize(path) if exists else 0)
            duration = CLIP_S if path.startswith(self._cache) and exists else line.get("recording_duration_s") or 0
            row["recording"] = {"path": path, "size": size, "exists": exists, "duration_s": duration}
        entry = next((e for e in self._data.get("journal", {}).get(ident, []) if e.get("session") == line.get("session")), None)
        row["journal"] = None if entry is None else {"state": entry.get("state") or "written", "title": entry.get("title") or "", "written_at": entry.get("written_at") or ""}
        return row

    def _tick(self, progress, message, steps):
        for done in range(1, steps + 1):
            if self._closed:
                raise UniverseError("Io", "the core is closed")
            time.sleep(STEP_S)
            if progress:
                progress(done, steps, f"{message} ({done}/{steps})")

    def sources(self):
        return copy.deepcopy(self._data.get("sources", []))

    def _source(self, ident):
        for source in self._data.get("sources", []):
            if source["id"] == ident:
                return source
        if any(m["id"] == ident for m in self._data.get("modules", [])):
            raise UniverseError("Invalid", f"{ident} is a module, not a source")
        raise UniverseError("NotFound", f"no source '{ident}'")

    def enable_source(self, ident, enabled):
        self._source(ident)["enabled"] = bool(enabled)

    def source_settings(self, ident):
        source = self._source(ident)
        merged = {s["key"]: s.get("default") for s in source.get("settings", [])}
        merged.update(self._config.get("sources", {}).get(ident, {}))
        return merged

    def source_setting_choices(self, ident, key):
        for setting in self._source(ident).get("settings", []):
            if setting["key"] == key:
                return list(setting.get("dynamic_choices") or setting.get("choices") or [])
        raise UniverseError("Invalid", f"{ident} has no setting '{key}'")

    def set_source_setting(self, ident, key, value):
        schema = {s["key"]: s for s in self._source(ident).get("settings", [])}
        self._config.setdefault("sources", {}).setdefault(ident, {})[key] = _coerce(schema, ident, key, value)

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
        return f"Installing {game_id}: done"

    def update(self, source, game_id, progress=None):
        self._tick(progress, "Updating" + (f" {game_id}" if game_id else " everything"), 12)
        before = len(self._data.get("updates", []))
        self._data["updates"] = [u for u in self._data.get("updates", []) if game_id and u["id"] != game_id]
        return before - len(self._data["updates"])

    def updates(self):
        return copy.deepcopy(self._data.get("updates", []))

    def scan(self, source, progress=None):
        self._tick(progress, "Scanning", 4)
        return 0

    def media_refresh(self, ident, force, progress=None):
        games = [self._game(ident)] if ident else [g for g in self._data["games"] if not g.get("removed")]
        self._tick(progress, "Refreshing media", 5)
        for game in games:
            media = game.get("media") or {}
            for path in (media.get(slot) for slot in SLOTS):
                if path and os.path.exists(path):
                    os.replace(path, path + ".part")
                    os.replace(path + ".part", path)
        return (0, len(games))

    def _effective_media(self, game):
        media = dict(game.get("media") or {})
        media.update({k: v for k, v in (game.get("overrides") or {}).items() if v})
        return media

    def _sgdb_hits(self, game):
        title, base = game.get("title", game["id"]), 5000 + len(game["id"])
        return [{"provider": "sgdb", "id": base, "name": title, "year": 2016, "verified": True},
                {"provider": "sgdb", "id": base + 1, "name": f"{title} Remastered", "year": 2021, "verified": False},
                {"provider": "sgdb", "id": base + 2, "name": f"{title} II", "year": 2019, "verified": True}]

    def _sgdb_entry(self, game):
        hits = self._sgdb_hits(game)
        pinned = int((game.get("metadata") or {}).get("sgdb_id") or 0) or hits[0]["id"]
        return next((h for h in hits if h["id"] == pinned), {**hits[0], "id": pinned})

    def _status_of(self, game):
        media = game.get("media") or {}
        overrides = game.get("overrides") or {}
        slots = []
        for slot in SLOTS:
            default, over = media.get(slot) or "", overrides.get(slot) or ""
            kind = "picked" if over else "default" if default else "missing"
            slots.append({"slot": slot, "path": over or default, "default": default, "override": over,
                          "origin": "picked" if over else "sgdb" if default else "", "default_origin": "sgdb" if default else "", "kind": kind})
        entry = self._sgdb_entry(game)
        return {"id": game["id"], "title": game.get("title", game["id"]), "sgdb_id": entry["id"], "sgdb_name": entry["name"], "sgdb_year": entry["year"], "slots": slots}

    def media_status(self, ident):
        games = [self._game(ident)] if ident else [g for g in self._data["games"] if not g.get("removed")]
        return [self._status_of(g) for g in games]

    def media_set_slot(self, ident, slot, path):
        game = self._game(ident)
        placed = _place(path, str(self._root / "overrides" / ident / f"{slot}.png"))
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
            try:
                os.remove(gone)
            except OSError:
                pass
        return bool(gone)

    def media_candidates(self, ident, slot, page=0):
        from .fixtures.art import paint_candidates

        game = self._game(ident)
        items = [] if int(page) > 0 else paint_candidates(self._cache, ident, slot, game.get("title", ident))
        return {"items": items, "page": int(page), "more": False, "entry": {**self._sgdb_entry(game), "current": True}}

    def media_search(self, ident, query):
        game = self._game(ident)
        current = self._sgdb_entry(game)["id"]
        return [{**h, "current": h["id"] == current} for h in self._sgdb_hits(game) if query.casefold() in h["name"].casefold()]

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
                [ffmpeg, "-loglevel", "error", "-y", "-f", "lavfi", "-i", f"testsrc=size=640x360:rate=30:duration={CLIP_S}",
                 "-f", "lavfi", "-i", f"sine=frequency=440:duration={CLIP_S}", "-c:v", "libx264", "-preset", "ultrafast",
                 "-pix_fmt", "yuv420p", "-c:a", "aac", "-shortest", out],
                capture_output=True, timeout=30,
            )
        return out

    def journal(self, ident):
        entries = self._data.get("journal", {}).get(ident, [])
        shots = self._game(ident).get("media", {}).get("screenshots") or []
        sessions = {s.get("session"): s for s in self._data.get("sessions", {}).get(ident, [])}
        out = []
        for e in entries:
            line = sessions.get(e.get("session")) or {}
            out.append({"state": "written", "started_at": line.get("started_at") or "", "ended_at": line.get("ended_at") or "",
                        "duration_s": line.get("duration_s") or 0, **e, "images": e.get("images") or shots})
        return out

    def pending_journals(self):
        return [{"game": game, "title": self._game(game)["title"], "session": e.get("session"), "started_at": e.get("started_at")}
                for game, entries in self._data.get("journal", {}).items()
                for e in entries if e.get("state") == "pending"]

    def _pend_journal(self, ident, session_id):
        entries = self._data.setdefault("journal", {}).setdefault(ident, [])
        entry = {"session": session_id, "game": ident, "state": "pending", "started_at": _now(),
                 "written_at": "", "lang": "en", "title": "", "provider": "fake", "paragraphs": [], "next_up": "", "images": []}
        entries.insert(0, entry)
        self._write_entry(ident, entry)

        def write():
            with self._lock:
                if self._closed or entry not in entries:
                    return
                entry.update(state="written", written_at=_now(), title="A short session",
                             paragraphs=["A quick look around, nothing decided yet."])
                self._write_entry(ident, entry)

        self._later(JOURNAL_S, write)

    def remove_journal_entry(self, ident, session_id):
        entries = self._data.get("journal", {}).get(ident, [])
        kept = [e for e in entries if e.get("session") != session_id]
        if len(kept) == len(entries):
            raise UniverseError("NotFound", f"journal entry {session_id}")
        self._data["journal"][ident] = kept
        for old in (self._game_dir(ident) / "journal").glob(f"{session_id}*.json"):
            old.unlink()

    def render_journal(self, ident):
        return os.path.join(self._cache, f"{ident}.md")

    def add_entry(self, session_id, entry):
        entry = dict(entry)
        entry.setdefault("session", session_id)
        ident = entry.get("game") or self._game_of_session(session_id)
        entry["game"] = ident
        self._data.setdefault("journal", {}).setdefault(ident, []).insert(0, entry)
        self._write_entry(ident, entry)

    def modules(self):
        return copy.deepcopy(self._data.get("modules", []))

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
        value = _coerce(schema, module_id, key, value)
        if game_id:
            game = self._game(game_id)
            game.setdefault("modules", {}).setdefault(module_id, {})[key] = value
            self._write_game(game)
        else:
            self._config.setdefault("modules", {}).setdefault(module_id, {})[key] = value

    def doctor(self):
        return copy.deepcopy(self._data.get("doctor", []))

    def settings(self):
        return copy.deepcopy(self._config)

    def set_setting(self, key, value):
        node = self._config
        parts = key.split(".")
        for part in parts[:-1]:
            node = node.setdefault(part, {})
        if value == "":
            node.pop(parts[-1], None)
        elif value in ("true", "false"):
            node[parts[-1]] = value == "true"
        else:
            try:
                node[parts[-1]] = int(value)
            except ValueError:
                node[parts[-1]] = value

    def screen_mode(self, screen):
        return dict(self._data.get("screen") or {"screen": screen or "DP-1", "width": 2560, "height": 1440, "refresh": 144})

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
        return copy.deepcopy(self._controller())

    def controller_pads(self):
        pads = []
        for pad in self._controller().get("devices", []):
            family = self._controller_family(pad["family"])
            slots = {}
            for slot in family.get("slots", []):
                codes = slot.get("codes") or []
                slots[slot["id"]] = {"code": codes[0] if codes else None, "bound": bool(codes)}
            pads.append({"id": pad["id"], "name": family["name"], "family": family["id"], "family_name": family["name"],
                         "bus": pad.get("bus", "usb"), "slots": slots})
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
        entry = {"family": family, "button": button, "trigger": trigger, "action": action,
                 "keys": str(macro.get("keys") or ""), "command": str(macro.get("command") or "")}
        state["macros"] = [m for m in state.get("macros", []) if (m["family"], m["button"], m["trigger"]) != (family, button, trigger)]
        state["macros"].append(entry)

    def remove_controller_macro(self, family, button, trigger):
        state = self._controller()
        state["macros"] = [m for m in state.get("macros", [])
                           if not (m["family"] == family and m["button"] == button and (not trigger or m["trigger"] == trigger))]

    def set_controller_button(self, family, slot, codes):
        for entry in self._controller_family(family)["slots"]:
            if entry["id"] == slot:
                entry["codes"] = [str(c) for c in codes or []]
                return
        raise UniverseError("NotFound", f"{family} has no slot '{slot}'")
