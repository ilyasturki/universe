import importlib.machinery
import importlib.util
import io
import json
import os
import sqlite3
import stat
import subprocess
import sys
import urllib.request
from pathlib import Path

import pytest

MODULE_DIR = Path(__file__).resolve().parents[1]
SOURCE = MODULE_DIR / "bin" / "source"

# butler as the source drives it: `daemon` answers JSON-RPC lines, `configure` lists a folder's executables.
# Its state is a JSON file (games, uploads, keys, caves, locations); the profiles and the keys also land in the
# SQLite database the source reads directly.
SHIM = r"""#!SHIM_PYTHON
import json, os, shutil, sqlite3, sys, time
from pathlib import Path

args = sys.argv[1:]
state_path = Path(os.environ["SHIM_STATE"])
mode = os.environ.get("SHIM_MODE", "ok")

def load():
    return json.loads(state_path.read_text())

def save(state):
    state_path.write_text(json.dumps(state))

if args[0] == "configure":
    folder = Path(args[-1])
    wanted = args[args.index("--os-filter") + 1]
    candidates = []
    for path in sorted(folder.rglob("*")):
        if not path.is_file() or ".itch" in path.parts:
            continue
        name, depth = path.name, len(path.relative_to(folder).parts)
        flavor = "windows" if name.endswith(".exe") else "script" if name.endswith(".sh") else "linux" if path.suffix in ("", ".x86_64") else None
        if flavor is None or (flavor == "windows") != (wanted == "windows"):
            continue
        entry = {"path": str(path.relative_to(folder)), "depth": depth, "flavor": flavor, "arch": "amd64", "size": path.stat().st_size}
        if name.startswith("unins"):
            entry["windowsInfo"] = {"installerType": "inno"}
        candidates.append(entry)
    print(json.dumps({"type": "log", "level": "info", "message": "configured"}))
    print(json.dumps({"type": "result", "value": {"basePath": str(folder), "totalSize": 0, "candidates": candidates}}))
    sys.exit(0)

db = sqlite3.connect(args[args.index("--dbpath") + 1])
db.execute("CREATE TABLE IF NOT EXISTS profiles (id INTEGER PRIMARY KEY, api_key TEXT, last_connected TEXT, user_id INTEGER)")
db.execute("CREATE TABLE IF NOT EXISTS download_keys (id INTEGER PRIMARY KEY, game_id INTEGER)")
db.commit()
with open(os.environ["SHIM_LOG"], "a") as f:
    f.write(json.dumps({"daemon": args}) + "\n")
server_ids = iter(range(1000))

def send(message):
    print(json.dumps({"jsonrpc": "2.0", **message}), flush=True)

def notify(method, params):
    send({"method": method, "params": params})

def ask(method, params):
    ident = next(server_ids)
    send({"id": ident, "method": method, "params": params})
    reply = json.loads(sys.stdin.readline())
    assert reply["id"] == ident
    return reply.get("result")

def profiles():
    return [{"id": row[0], "lastConnected": row[1], "user": {"id": row[0], "username": "yasso"}} for row in db.execute("SELECT id, last_connected FROM profiles")]

def page(items, params):
    start = int(params.get("cursor") or 0)
    size = params.get("limit") or len(items)
    end = start + min(size, 2)
    return {"items": items[start:end], **({"nextCursor": str(end)} if end < len(items) else {})}

def cave_of(state, cave_id):
    cave = dict(state["caves"][cave_id])
    location = state["locations"][cave.pop("location")]
    cave["installInfo"] = {"installFolder": str(Path(location) / cave.pop("folder")), "installedSize": 4096, "installLocation": "x"}
    return cave

def handle(method, params):
    state = load()
    with open(os.environ["SHIM_LOG"], "a") as f:
        f.write(json.dumps({"method": method, "params": params}) + "\n")
    if method == "Profile.LoginWithAPIKey":
        if params["apiKey"] != "good-key":
            raise ValueError("itch.io API error (403): /profile: invalid key")
        db.execute("INSERT OR REPLACE INTO profiles VALUES (7, ?, '2026-09-27T12:00:00Z', 7)", (params["apiKey"],))
        db.commit()
        return {"profile": {"id": 7, "user": {"id": 7, "username": "yasso"}}}
    if method == "Profile.List":
        return {"profiles": profiles()}
    if method == "Fetch.ProfileOwnedKeys":
        if params.get("fresh"):
            for key in state["keys"]:
                db.execute("INSERT OR REPLACE INTO download_keys VALUES (?, ?)", (key["id"], key["gameId"]))
            db.commit()
        keys = [{**k, "game": state["games"][str(k["gameId"])]} for k in state["keys"]]
        return page(keys, params)
    if method == "Fetch.Caves":
        return page([cave_of(state, c) for c in state["caves"]], params)
    if method == "Fetch.Cave":
        return {"cave": cave_of(state, params["caveId"])} if params["caveId"] in state["caves"] else {}
    if method == "Fetch.Game":
        return {"game": state["games"].get(str(params["gameId"]))}
    if method == "Fetch.GameUploads":
        return {"uploads": state["uploads"].get(str(params["gameId"]), [])}
    if method == "Install.PlanUpload":
        return {"info": {"type": "archive", "diskUsage": {"finalDiskUsage": 9000}}}
    if method == "Install.Locations.Add":
        if "readonly" in params["path"]:
            raise ValueError("install location is not writable")
        state["locations"][params["id"]] = params["path"]
        save(state)
        return {"installLocation": {"id": params["id"], "path": params["path"]}}
    if method == "Install.Locations.List":
        return {"installLocations": [{"id": k, "path": v} for k, v in state["locations"].items()]}
    if method == "Install.Queue":
        cave_id = params.get("caveId")
        if not cave_id:
            if any(c["upload"]["id"] == params["upload"]["id"] for c in state["caves"].values()):
                raise ValueError("That upload is already installed!")
            cave_id = f"cave-{params['game']['id']}"
            location, folder = state["locations"][params["installLocationId"]], params["game"]["title"].lower()
        else:
            location, folder = state["locations"][state["caves"][cave_id]["location"]], state["caves"][cave_id]["folder"]
        ident = f"op-{params['game']['id']}"
        state["queued"][ident] = {"caveId": cave_id, "game": params["game"], "upload": params["upload"], "location": params.get("installLocationId") or state["caves"][cave_id]["location"], "folder": folder}
        save(state)
        return {"id": ident, "caveId": cave_id, "installFolder": str(Path(location) / folder), "stagingFolder": str(Path(location) / "downloads" / ident)}
    if method == "Install.Perform":
        staging = Path(params["stagingFolder"])
        queued = next(q for i, q in state["queued"].items() if staging.name == i)
        staging.mkdir(parents=True, exist_ok=True)
        (staging / "operate-context.json").write_text(json.dumps({"caveId": queued["caveId"]}))
        notify("TaskStarted", {"type": "install", "totalSize": 2000})
        if mode == "partial":
            (staging / "part.bin").write_bytes(b"x" * 300)
            notify("Progress", {"progress": 0.25, "eta": 3, "bps": 100})
            time.sleep(60)
        notify("Progress", {"progress": 0.5, "eta": 1, "bps": 100})
        notify("Progress", {"progress": 1.0, "eta": 0, "bps": 100})
        folder = Path(state["locations"][queued["location"]]) / queued["folder"]
        folder.mkdir(parents=True, exist_ok=True)
        linux = bool(queued["upload"].get("platforms", {}).get("linux"))
        (folder / ("game.x86_64" if linux else "Game.exe")).write_bytes(b"x" * 64)
        (folder / ("unins000.exe")).write_bytes(b"x" * 999)
        for f in staging.iterdir():
            f.unlink()
        staging.rmdir()
        state["caves"][queued["caveId"]] = {"id": queued["caveId"], "game": queued["game"], "upload": queued["upload"], "build": queued["upload"].get("build"), "location": queued["location"], "folder": queued["folder"]}
        save(state)
        return {"caveId": queued["caveId"], "events": []}
    if method == "CheckUpdate":
        updates = [u for u in state["updates"] if u["caveId"] in params.get("caveIds", [])]
        return {"updates": [{**u, "game": state["caves"][u["caveId"]]["game"]} for u in updates], "warnings": []}
    if method == "Uninstall.Perform":
        cave = state["caves"].pop(params["caveId"], None)
        if cave:
            shutil.rmtree(Path(state["locations"][cave["location"]]) / cave["folder"], ignore_errors=True)
        save(state)
        return {}
    if method == "Install.Locations.Scan":
        found = []
        for ident, location in state["locations"].items():
            for receipt in Path(location).glob("*/.itch/receipt.json"):
                data = json.loads(receipt.read_text())
                cave_id = f"cave-{data['game']['id']}"
                if cave_id not in state["caves"]:
                    found.append((cave_id, {"id": cave_id, "game": data["game"], "upload": data["upload"], "build": None, "location": ident, "folder": receipt.parent.parent.name}))
        if found and not (ask("Install.Locations.Scan.ConfirmImport", {"numItems": len(found)}) or {}).get("confirm"):
            return {"numFoundItems": len(found), "numImportedItems": 0}
        state["caves"].update(dict(found))
        save(state)
        return {"numFoundItems": len(found), "numImportedItems": len(found)}
    raise KeyError(method)

print(json.dumps({"level": "info", "message": "butlerd: listening", "type": "log"}), file=sys.stderr, flush=True)
for line in sys.stdin:
    request = json.loads(line)
    try:
        send({"id": request["id"], "result": handle(request["method"], request["params"])})
    except (ValueError, KeyError) as exc:
        send({"id": request["id"], "error": {"code": -32603, "message": str(exc)}})
"""

GAMES = {
    "11": {
        "id": 11,
        "title": "Celeste Classic",
        "classification": "game",
        "coverUrl": "https://img/c.gif",
        "stillCoverUrl": "https://img/c.png",
        "publishedAt": "2015-08-01T00:00:00Z",
    },
    "12": {"id": 12, "title": "Winter", "classification": "game", "coverUrl": "https://img/w.png"},
    "13": {"id": 13, "title": "Soundtrack", "classification": "soundtrack"},
    "14": {"id": 14, "title": "Windows Only", "classification": "game"},
}
UPLOADS = {
    "11": [
        {"id": 101, "filename": "celeste-demo.zip", "type": "default", "demo": True, "platforms": {"linux": "all"}, "size": 10},
        {"id": 102, "filename": "celeste-win.zip", "type": "default", "platforms": {"windows": "all"}, "size": 1500},
        {"id": 103, "filename": "celeste-linux.tar.gz", "type": "default", "platforms": {"linux": "amd64"}, "size": 2000, "build": {"id": 5, "version": 3}},
        {"id": 104, "filename": "celeste.deb", "type": "default", "platforms": {"linux": "amd64"}, "size": 1},
    ],
    "12": [{"id": 121, "filename": "winter.html", "type": "html", "platforms": {}}],
    "14": [{"id": 141, "filename": "wo.zip", "type": "default", "platforms": {"windows": "all"}, "size": 700}],
}


@pytest.fixture
def src():
    loader = importlib.machinery.SourceFileLoader("itch_source", str(SOURCE))
    spec = importlib.util.spec_from_loader("itch_source", loader)
    assert spec is not None
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module


@pytest.fixture(autouse=True)
def offline(monkeypatch):
    def refuse(*args, **kwargs):
        raise OSError("no network in tests")

    monkeypatch.setattr(urllib.request, "urlopen", refuse)


@pytest.fixture
def env(tmp_path, monkeypatch):
    shim_dir = tmp_path / "bin"
    shim_dir.mkdir()
    shim = shim_dir / "butler"
    shim.write_text(SHIM.replace("SHIM_PYTHON", sys.executable, 1))
    shim.chmod(shim.stat().st_mode | stat.S_IEXEC)
    state = tmp_path / "state.json"
    keys = [{"id": 900 + int(g), "gameId": int(g), "ownerId": 7} for g in ("11", "12", "13", "14")]
    state.write_text(json.dumps({"games": GAMES, "uploads": UPLOADS, "keys": keys, "caves": {}, "locations": {}, "queued": {}, "updates": []}))
    monkeypatch.setenv("PATH", f"{shim_dir}{os.pathsep}{os.environ['PATH']}")
    monkeypatch.setenv("SHIM_STATE", str(state))
    monkeypatch.setenv("SHIM_LOG", str(tmp_path / "butler.log"))
    monkeypatch.setenv("SOURCE_DATA_DIR", str(tmp_path / "data"))
    settings = {
        "games_dir": str(tmp_path / "games"),
        "platform": "linux",
        "db_path": "",
        "adopt_from": str(tmp_path / "itch-app" / "butler.db"),
        "install_timeout_s": 30,
    }
    monkeypatch.setenv("SOURCE_SETTINGS_JSON", json.dumps(settings))
    return {"tmp": tmp_path, "state": state, "data": tmp_path / "data", "games": tmp_path / "games", "log": tmp_path / "butler.log"}


def run(src, capsys, *argv):
    code = src.main(list(argv))
    out, err = capsys.readouterr()
    return code, [json.loads(line) for line in out.splitlines()], err


def calls(env):
    lines = [json.loads(line) for line in env["log"].read_text().splitlines()] if env["log"].exists() else []
    return [line for line in lines if "method" in line]


def state(env):
    return json.loads(env["state"].read_text())


def patch_state(env, **changes):
    data = state(env)
    data.update(changes)
    env["state"].write_text(json.dumps(data))


def signed_in(src, capsys):
    code, _, _ = run(src, capsys, "login", "good-key")
    assert code == 0


def test_login_takes_an_api_key_and_status_reads_it_back(src, env, capsys):
    code, events, _ = run(src, capsys, "login")
    assert code == 0 and events == [{"event": "login_url", "url": src.KEYS_URL}, {"event": "done"}]
    code, events, _ = run(src, capsys, "status")
    assert events == [{"event": "done"}], "no profile yet"
    code, events, err = run(src, capsys, "login", "bad")
    assert code == 1 and "invalid key" in err
    code, events, _ = run(src, capsys, "login", " good-key ")
    assert events == [{"event": "logged_in", "user": "yasso"}, {"event": "done"}]
    code, events, _ = run(src, capsys, "status")
    assert events == [{"event": "logged_in", "user": "yasso"}, {"event": "done"}]
    assert "--dbpath" in json.loads(env["log"].read_text().splitlines()[0])["daemon"], "butler keeps its database in the source's data folder"


def test_library_pages_the_owned_keys_and_leaves_out_what_is_not_played(src, env, capsys):
    code, _, err = run(src, capsys, "library")
    assert code == 1 and "not signed in" in err
    signed_in(src, capsys)
    code, events, err = run(src, capsys, "library")
    assert code == 0
    assert [e["id"] for e in events[:-1]] == ["11", "12", "14"], "the soundtrack is not a game"
    assert "Soundtrack: a soundtrack, left out" in err
    celeste = events[0]
    assert (celeste["title"], celeste["owned"], celeste["installed"], celeste["image"], celeste["release_year"]) == (
        "Celeste Classic",
        True,
        False,
        "https://img/c.png",
        2015,
    )
    owned = [c for c in calls(env) if c["method"] == "Fetch.ProfileOwnedKeys"]
    assert [c["params"].get("fresh") for c in owned] == [True, None], "fresh once, then the pages"
    assert owned[1]["params"]["cursor"] == "2"


def test_install_picks_the_linux_build_and_lands_it_native(src, env, capsys):
    signed_in(src, capsys)
    code, events, _ = run(src, capsys, "install", "11")
    assert code == 0
    assert [e["event"] for e in events] == ["progress", "progress", "game", "done"]
    assert events[0] == {"event": "progress", "done": 1000, "total": 2000, "message": "50.0%"}
    game = events[2]
    folder = env["games"] / "celeste classic"
    assert (game["id"], game["dir"], game["exe"], game["runner"], game["store"], game["build"]) == ("11", str(folder), "game.x86_64", "linux", "itchio", "5")
    queued = next(c for c in calls(env) if c["method"] == "Install.Queue")["params"]
    assert queued["upload"]["id"] == 103, "not the demo, not the .deb, the Linux build over the Windows one"
    assert queued["installLocationId"].startswith("universe-") and state(env)["locations"][queued["installLocationId"]] == str(env["games"])
    assert json.loads((env["data"] / "partials.json").read_text()) == {}


def test_a_windows_build_runs_through_proton_without_its_uninstaller(src, env, capsys):
    signed_in(src, capsys)
    code, events, _ = run(src, capsys, "install", "14")
    game = events[-2]
    assert code == 0 and game["exe"] == "Game.exe" and "runner" not in game, "Proton, the default runner"
    code, _, err = run(src, capsys, "install", "12")
    assert code == 1 and "no Linux or Windows build" in err, "a browser game has nothing to download"


def test_a_stopped_install_resumes_over_its_staging_folder(src, env, capsys, monkeypatch):
    signed_in(src, capsys)
    monkeypatch.setenv("SHIM_MODE", "partial")
    proc = subprocess.Popen([sys.executable, str(SOURCE), "install", "11"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=dict(os.environ))
    assert proc.stdout is not None
    assert json.loads(proc.stdout.readline())["event"] == "progress"
    proc.send_signal(15)
    out, _ = proc.communicate(timeout=15)
    assert proc.returncode == 143 and "game" not in out
    monkeypatch.setenv("SHIM_MODE", "ok")
    code, events, _ = run(src, capsys, "scan")
    partial = events[0]
    assert (partial["id"], partial["installed"], partial["partial_dir"]) == ("11", False, str(env["games"] / "celeste classic"))
    assert partial["partial_bytes"] >= 300 and partial["download_size"] == 2000
    code, events, _ = run(src, capsys, "install", "11")
    assert code == 0 and events[-2]["installed"] is True
    performs = [c["params"] for c in calls(env) if c["method"] == "Install.Perform"]
    assert performs[-1]["id"] == "resume-11" and performs[-1]["stagingFolder"] == performs[0]["stagingFolder"], "the same staging folder, no new queue"
    assert sum(c["method"] == "Install.Queue" for c in calls(env)) == 1


def test_an_install_made_gone_outside_butler_is_forgotten_before_reinstalling(src, env, capsys):
    signed_in(src, capsys)
    run(src, capsys, "install", "14")
    import shutil

    shutil.rmtree(env["games"] / "windows only")
    code, events, err = run(src, capsys, "scan")
    assert events == [{"event": "done"}] and "is gone, skipped" in err
    code, events, _ = run(src, capsys, "install", "14")
    assert code == 0 and events[-2]["installed"] is True
    assert [c["method"] for c in calls(env)].count("Uninstall.Perform") == 1


def test_uninstall_goes_through_butler_so_it_forgets_the_install(src, env, capsys):
    signed_in(src, capsys)
    run(src, capsys, "install", "14")
    code, events, _ = run(src, capsys, "uninstall", "14")
    assert code == 0 and events == [{"event": "done"}]
    assert not (env["games"] / "windows only").exists() and state(env)["caves"] == {}
    code, events, _ = run(src, capsys, "uninstall", "14")
    assert code == 0 and [c["method"] for c in calls(env)].count("Uninstall.Perform") == 1, "nothing left to uninstall"


def test_updates_list_the_direct_ones_and_one_updates(src, env, capsys):
    signed_in(src, capsys)
    run(src, capsys, "install", "11")
    newer = {"id": 103, "filename": "celeste-linux.tar.gz", "type": "default", "platforms": {"linux": "amd64"}, "size": 2100}
    patch_state(
        env,
        updates=[
            {
                "caveId": "cave-11",
                "direct": True,
                "choices": [{"upload": newer, "build": {"id": 6, "version": 4, "userVersion": "1.1", "updatedAt": "2026-09-20T10:00:00Z"}}],
            },
            {"caveId": "cave-11", "direct": False, "choices": [{"upload": {"id": 999}}]},
        ],
    )
    code, events, _ = run(src, capsys, "update")
    assert events == [
        {"event": "update", "id": "11", "title": "Celeste Classic", "local_build": "5", "remote_build": "6", "version": "1.1", "date": "2026-09-20"},
        {"event": "done"},
    ], "a guess among other uploads is not an update"
    code, events, _ = run(src, capsys, "update", "11")
    assert code == 0 and events[-2]["id"] == "11"
    queued = [c["params"] for c in calls(env) if c["method"] == "Install.Queue"][-1]
    assert (queued["caveId"], queued["reason"], queued["build"]["id"]) == ("cave-11", "update", 6)


def test_scan_adopts_the_itch_apps_installs_where_they_lie(src, env, capsys):
    app_dir = env["tmp"] / "itch-app"
    app_dir.mkdir()
    apps = env["tmp"] / "apps"
    receipt = apps / "celeste" / ".itch" / "receipt.json"
    receipt.parent.mkdir(parents=True)
    receipt.write_text(json.dumps({"game": GAMES["11"], "upload": UPLOADS["11"][2]}))
    (apps / "celeste" / "celeste.sh").write_text("#!/bin/sh\n")
    (env["tmp"] / "readonly").mkdir()
    with sqlite3.connect(app_dir / "butler.db") as con:
        con.execute("CREATE TABLE install_locations (id TEXT PRIMARY KEY, path TEXT)")
        con.execute("INSERT INTO install_locations VALUES ('r', ?), ('a', ?), ('b', '/nonexistent')", (str(env["tmp"] / "readonly"), str(apps)))
    code, events, err = run(src, capsys, "scan")
    assert code == 0 and "adopted 1 of the itch app's installs" in err
    assert "readonly: Install.Locations.Add: install location is not writable" in err, "one refused folder leaves the others adopted"
    game = events[0]
    assert (game["id"], game["dir"], game["exe"], game["owned"]) == ("11", str(apps / "celeste"), "celeste.sh", None), "a lone script is the launcher"
    assert os.access(apps / "celeste" / "celeste.sh", os.X_OK), "the launch target is made executable"
    code, events, err = run(src, capsys, "scan")
    assert [e["id"] for e in events[:-1]] == ["11"] and "adopted" not in err, "once"


def test_search_asks_itch_and_falls_back_to_the_library(src, env, capsys, monkeypatch):
    signed_in(src, capsys)
    (env["data"] / "library.json").write_text(json.dumps([{"id": "11", "title": "Celeste Classic"}, {"id": "14", "title": "Windows Only"}]))
    seen = []

    def answer(request, timeout=None):
        seen.append((request.full_url, request.get_header("Authorization")))
        return io.BytesIO(json.dumps({"games": [GAMES["14"], GAMES["13"], {"id": 55, "title": "Celestial", "classification": "game"}]}).encode())

    monkeypatch.setattr(urllib.request, "urlopen", answer)
    code, events, _ = run(src, capsys, "search", "cel", "es")
    assert code == 0 and seen == [("https://api.itch.io/search/games?query=cel%20es", "good-key")]
    assert [(e["id"], e["owned"]) for e in events[:-1]] == [("14", True), ("55", False)]

    def refuse(*a, **k):
        raise OSError("offline")

    monkeypatch.setattr(urllib.request, "urlopen", refuse)
    code, events, err = run(src, capsys, "search", "celeste")
    assert code == 0 and [e["id"] for e in events[:-1]] == ["11"] and "searching the library instead" in err


def test_info_reports_the_build_size_and_the_disk(src, env, capsys):
    code, events, _ = run(src, capsys, "info", "11")
    assert code == 0
    assert (events[0]["download_size"], events[0]["disk_size"], events[0]["data"]["upload"]["id"]) == (2000, 9000, 103)
    code, _, err = run(src, capsys, "info", "celeste")
    assert code == 1 and "usage" in err


def test_the_launch_target_follows_the_itch_apps_pick(src, env, tmp_path):
    game = tmp_path / "g"
    (game / "bin").mkdir(parents=True)
    (game / "bin" / "Game.exe").write_bytes(b"x" * 10)
    (game / "Big.exe").write_bytes(b"x" * 500)
    (game / "Small.exe").write_bytes(b"x" * 5)
    (game / "unins000.exe").write_bytes(b"x" * 9999)
    assert src.launch_target(str(game), "windows") == "Big.exe", "the shallowest, the biggest, no uninstaller"
    (game / ".itch.toml").write_text('[[actions]]\nname = "play"\npath = "bin/Game.exe"\n')
    assert src.launch_target(str(game), "windows") == "bin/Game.exe", "the manifest's action wins"
    assert src.launch_target(str(tmp_path / "missing"), "linux") is None
