import contextlib
import json
import os
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path

import pytest

# `UNIVERSE_SOURCE=<folder>[:<folder>…] pytest test_protocol.py` checks those sources; each drives offline from its tests/protocol.toml.
FOLDERS = [Path(f).resolve() for f in os.environ.get("UNIVERSE_SOURCE", "").split(os.pathsep) if f] or sorted(
    p.parent for p in Path(__file__).parent.glob("*/source.toml")
)

# docs/api.md, Source protocol: per verb and argument, the events it may print before `done`, and those it must.
FORMS = {
    "login": ({"login_url"}, {"login_url"}),
    "login <code>": ({"logged_in"}, {"logged_in"}),
    "status": ({"logged_in"}, set()),
    "library": ({"game"}, set()),
    "search <text>": ({"game"}, set()),
    "info <id>": ({"info"}, {"info"}),
    "install <id>": ({"progress", "game"}, {"game"}),
    "update": ({"update"}, set()),
    "update <id>": ({"progress", "game"}, set()),
    "scan": ({"game"}, set()),
    "achievements <id>": ({"achievement"}, set()),
    "uninstall <id>": ({"progress"}, set()),
}
CAPABILITIES = {"achievements", "uninstall"}

NULL = type(None)
REQUIRED = {
    "login_url": {"url": str},
    "logged_in": {"user": str},
    "game": {"id": str, "title": str, "owned": (bool, NULL), "installed": bool},
    "info": {"data": object},
    "progress": {"done": int, "total": int, "message": str},
    "update": {"id": str, "title": str, "local_build": (str, NULL), "remote_build": str, "version": str, "date": str},
    "achievement": {
        "key": str,
        "name": str,
        "description": str,
        "unlocked_at": str,
        "hidden": bool,
        "icon": str,
        "icon_locked": str,
        "rarity": (int, float, NULL),
    },
    "window": {"class": str, "title": str},
}
# Read by the core when present: a value of another type is dropped, or drops the whole line.
OPTIONAL = {
    "dir": (str, NULL),
    "exe": (str, NULL),
    "build": (str, NULL),
    "download_size": (int, NULL),
    "disk_size": (int, NULL),
    "partial_dir": str,
    "partial_bytes": int,
    "release_year": (int, NULL),
    "dlcs": list,
    "art": dict,
    "steam_appid": (int, str),
}

OFFLINE = """
import io, json, os, runpy, sys, urllib.request
answers = json.loads(os.environ.pop("PROTOCOL_HTTP"))
def urlopen(request, *args, **kwargs):
    url = getattr(request, "full_url", request)
    for prefix, body in answers.items():
        if url.startswith(prefix):
            return io.BytesIO(body.encode())
    raise OSError(f"offline: {url}")
urllib.request.urlopen = urlopen
sys.argv = sys.argv[1:]
runpy.run_path(sys.argv[0], run_name="__main__")
"""


def forms(folder):
    named = tomllib.loads((folder / "source.toml").read_text()).get("capabilities", [])
    return [form for form in FORMS if form.split()[0] not in CAPABILITIES] + [f"{verb} <id>" for verb in named]


CASES = [pytest.param(folder, form, id=f"{folder.name}-{form.replace(' <', '-').rstrip('>')}") for folder in FOLDERS for form in forms(folder)]


def expand(value, tmp):
    if isinstance(value, str):
        return value.replace("{tmp}", str(tmp))
    if isinstance(value, list):
        return [expand(v, tmp) for v in value]
    if isinstance(value, dict):
        return {k: expand(v, tmp) for k, v in value.items()}
    return value


class Source:
    def __init__(self, folder, tmp):
        self.folder, self.manifest = folder, tomllib.loads((folder / "source.toml").read_text())
        data_file = folder / "tests" / "protocol.toml"
        self.data = expand(tomllib.loads(data_file.read_text()) if data_file.exists() else {}, tmp)
        for path, text in self.data.get("files", {}).items():
            (tmp / path).parent.mkdir(parents=True, exist_ok=True)
            (tmp / path).write_text(text)
        settings = {s["key"]: s.get("default") for s in self.manifest.get("settings", [])}
        if settings.get("games_dir") == "":
            settings["games_dir"] = str(tmp / "games")
        shims = [str(data_file.parent / p) for p in self.data.get("path", [])]
        homes = {name: str(tmp / "home" / name.lower()) for name in ("XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME", "XDG_STATE_HOME")}
        proxies = dict.fromkeys(("http_proxy", "https_proxy", "all_proxy", "HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"), "http://127.0.0.1:1")
        answers = {prefix: body if isinstance(body, str) else json.dumps(body) for prefix, body in self.data.get("http", {}).items()}
        self.env = {
            **os.environ,
            **homes,
            **proxies,
            "no_proxy": "",
            "NO_PROXY": "",
            "HOME": str(tmp / "home"),
            "PATH": os.pathsep.join([*shims, os.environ["PATH"]]),
            "SOURCE_DIR": str(folder),
            "SOURCE_DATA_DIR": str(tmp / "data"),
            "SOURCE_SETTINGS_JSON": json.dumps({**settings, **self.data.get("settings", {})}),
            "UNIVERSE_BIN": shutil.which("false") or "false",
            "PROTOCOL_HTTP": json.dumps(answers),
            **self.data.get("env", {}),
        }
        for name in self.manifest.get("requires", {}).get("bins", []):
            assert shutil.which(name, path=self.env["PATH"]), f"{name} is not on PATH: name a folder of shims in tests/protocol.toml's path"
        exe = folder / self.manifest["exe"]
        self.command = [sys.executable, "-c", OFFLINE, str(exe)] if b"python" in exe.read_bytes().partition(b"\n")[0] else [str(exe)]

    def run(self, *argv):
        proc = subprocess.run([*self.command, *argv], cwd=self.folder, env=self.env, capture_output=True, text=True, timeout=30, check=False)
        return proc.returncode, proc.stdout, proc.stderr


def parsed(line):
    with contextlib.suppress(ValueError):
        return json.loads(line)


# The core reads a number as unsigned, and a bool as no number.
def fits(value, kinds):
    kinds = kinds if isinstance(kinds, tuple) else (kinds,)
    if isinstance(value, bool) and bool not in kinds:
        return object in kinds
    return isinstance(value, kinds) and not (isinstance(value, int) and value < 0)


def check_event(event):
    name = event["event"]
    for key, kinds in REQUIRED[name].items():
        assert key in event and fits(event[key], kinds), f"{name}.{key} is {event.get(key)!r}"
    for key, kinds in OPTIONAL.items() if name in ("game", "info") else ():
        assert key not in event or fits(event[key], kinds), f"{name}.{key} is {event[key]!r}"
    if name == "game":
        assert event["id"] and event["title"], "the core drops a game without an id or a title"
        assert not event["installed"] or event.get("dir"), "an installed game names its folder"
        assert not os.path.isabs(event.get("exe") or ""), "exe is relative to dir"
        assert "partial_dir" not in event or event["installed"] is False, "a stopped download is not installed"


@pytest.mark.parametrize(("folder", "form"), CASES)
def test_every_verb_keeps_the_source_protocol(folder, form, tmp_path):
    assert form in FORMS, f"{form.split()[0]} is not a verb of the protocol"
    source = Source(folder, tmp_path)
    argv = form.split()[:1]
    if "<" in form:
        assert form in source.data.get("args", {}), f"tests/protocol.toml's [args] names no {form}"
        argv += source.data["args"][form]
    code, out, err = source.run(*argv)
    assert code == 0, f"exit {code}: {err}"
    events = [parsed(line) for line in out.splitlines()]
    assert all(isinstance(e, dict) and "event" in e for e in events), f"not one JSON event per line: {out}"
    assert events and events[-1] == {"event": "done"}, "every verb ends with done"
    names = [e["event"] for e in events[:-1]]
    may, must = FORMS[form]
    assert set(names) <= may | {"window"}, f"{form} prints {names}"
    assert must <= set(names), f"{form} prints no {must - set(names)}"
    for event in events[:-1]:
        check_event(event)
