import io
import json
import os
import shutil
import sqlite3
import urllib.request

import pytest

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
def env(tmp_path, monkeypatch, shims):
    state = tmp_path / "state.json"
    keys = [{"id": 900 + int(g), "gameId": int(g), "ownerId": 7} for g in ("11", "12", "13", "14")]
    state.write_text(json.dumps({"games": GAMES, "uploads": UPLOADS, "keys": keys, "caves": {}, "locations": {}, "queued": {}, "updates": []}))
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


def calls(env):
    lines = [json.loads(line) for line in env["log"].read_text().splitlines()] if env["log"].exists() else []
    return [line for line in lines if "method" in line]


def state(env):
    return json.loads(env["state"].read_text())


def patch_state(env, **changes):
    data = state(env)
    data.update(changes)
    env["state"].write_text(json.dumps(data))


def signed_in(run):
    code, _, _ = run("login", "good-key")
    assert code == 0


def test_an_api_key_signs_in_and_status_reads_it_back(env, run):
    code, events, _ = run("status")
    assert events == [{"event": "done"}], "no profile yet"
    code, events, _ = run("login", "bad")
    assert code == 1 and events == []
    code, events, _ = run("login", " good-key ")
    assert events == [{"event": "logged_in", "user": "yasso"}, {"event": "done"}]
    code, events, _ = run("status")
    assert events == [{"event": "logged_in", "user": "yasso"}, {"event": "done"}]
    assert "--dbpath" in json.loads(env["log"].read_text().splitlines()[0])["daemon"], "butler keeps its database in the source's data folder"


def test_library_pages_the_owned_keys_and_leaves_out_what_is_not_played(env, run):
    code, _, _ = run("library")
    assert code == 1, "not signed in"
    signed_in(run)
    code, events, _ = run("library")
    assert code == 0
    assert [e["id"] for e in events[:-1]] == ["11", "12", "14"], "the soundtrack is not a game"
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


def test_install_picks_the_linux_build_and_lands_it_native(env, run):
    signed_in(run)
    code, events, _ = run("install", "11")
    assert code == 0
    assert [(e["done"], e["total"]) for e in events[:2]] == [(1000, 2000), (2000, 2000)]
    game = events[2]
    folder = env["games"] / "celeste classic"
    assert (game["id"], game["dir"], game["exe"], game["runner"], game["store"], game["build"]) == ("11", str(folder), "game.x86_64", "linux", "itchio", "5")
    queued = next(c for c in calls(env) if c["method"] == "Install.Queue")["params"]
    assert queued["upload"]["id"] == 103, "not the demo, not the .deb, the Linux build over the Windows one"
    assert queued["installLocationId"].startswith("universe-") and state(env)["locations"][queued["installLocationId"]] == str(env["games"])
    assert json.loads((env["data"] / "partials.json").read_text()) == {}


def test_a_windows_build_runs_through_proton_without_its_uninstaller(env, run):
    signed_in(run)
    code, events, _ = run("install", "14")
    game = events[-2]
    assert code == 0 and game["exe"] == "Game.exe" and "runner" not in game, "Proton, the default runner"
    code, events, _ = run("install", "12")
    assert code == 1 and events == [], "a browser game has nothing to download"


def test_a_stopped_install_resumes_over_its_staging_folder(env, run, interrupt, monkeypatch):
    signed_in(run)
    monkeypatch.setenv("SHIM_MODE", "partial")
    interrupt("install", "11")
    monkeypatch.setenv("SHIM_MODE", "ok")
    code, events, _ = run("scan")
    partial = events[0]
    assert (partial["id"], partial["installed"], partial["partial_dir"]) == ("11", False, str(env["games"] / "celeste classic"))
    assert partial["partial_bytes"] >= 300 and partial["download_size"] == 2000
    code, events, _ = run("install", "11")
    assert code == 0 and events[-2]["installed"] is True
    performs = [c["params"] for c in calls(env) if c["method"] == "Install.Perform"]
    assert performs[-1]["id"] == "resume-11" and performs[-1]["stagingFolder"] == performs[0]["stagingFolder"], "the same staging folder, no new queue"
    assert sum(c["method"] == "Install.Queue" for c in calls(env)) == 1


def test_an_install_made_gone_outside_butler_is_forgotten_before_reinstalling(env, run):
    signed_in(run)
    run("install", "14")
    shutil.rmtree(env["games"] / "windows only")
    code, events, _ = run("scan")
    assert events == [{"event": "done"}]
    code, events, _ = run("install", "14")
    assert code == 0 and events[-2]["installed"] is True
    assert [c["method"] for c in calls(env)].count("Uninstall.Perform") == 1


def test_uninstall_goes_through_butler_so_it_forgets_the_install(env, run):
    signed_in(run)
    run("install", "14")
    code, events, _ = run("uninstall", "14")
    assert code == 0 and events == [{"event": "done"}]
    assert not (env["games"] / "windows only").exists() and state(env)["caves"] == {}
    code, events, _ = run("uninstall", "14")
    assert code == 0 and [c["method"] for c in calls(env)].count("Uninstall.Perform") == 1, "nothing left to uninstall"


def test_updates_list_the_direct_ones_and_one_updates(env, run):
    signed_in(run)
    run("install", "11")
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
    code, events, _ = run("update")
    assert events == [
        {"event": "update", "id": "11", "title": "Celeste Classic", "local_build": "5", "remote_build": "6", "version": "1.1", "date": "2026-09-20"},
        {"event": "done"},
    ], "a guess among other uploads is not an update"
    code, events, _ = run("update", "11")
    assert code == 0 and events[-2]["id"] == "11"
    queued = [c["params"] for c in calls(env) if c["method"] == "Install.Queue"][-1]
    assert (queued["caveId"], queued["reason"], queued["build"]["id"]) == ("cave-11", "update", 6)


def test_scan_adopts_the_itch_apps_installs_where_they_lie(env, run):
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
    code, events, _ = run("scan")
    assert code == 0
    game = events[0]
    assert (game["id"], game["dir"], game["exe"], game["owned"]) == ("11", str(apps / "celeste"), "celeste.sh", None), (
        "a lone script is the launcher; a refused folder leaves the others adopted"
    )
    assert os.access(apps / "celeste" / "celeste.sh", os.X_OK), "the launch target is made executable"
    code, events, _ = run("scan")
    assert [e["id"] for e in events[:-1]] == ["11"], "adopted once"


def test_search_asks_itch_and_falls_back_to_the_library(env, run, monkeypatch):
    signed_in(run)
    (env["data"] / "library.json").write_text(json.dumps([{"id": "11", "title": "Celeste Classic"}, {"id": "14", "title": "Windows Only"}]))
    seen = []

    def answer(request, timeout=None):
        seen.append((request.full_url, request.get_header("Authorization")))
        return io.BytesIO(json.dumps({"games": [GAMES["14"], GAMES["13"], {"id": 55, "title": "Celestial", "classification": "game"}]}).encode())

    monkeypatch.setattr(urllib.request, "urlopen", answer)
    code, events, _ = run("search", "cel", "es")
    assert code == 0 and seen == [("https://api.itch.io/search/games?query=cel%20es", "good-key")]
    assert [(e["id"], e["owned"]) for e in events[:-1]] == [("14", True), ("55", False)]

    def refuse(*a, **k):
        raise OSError("offline")

    monkeypatch.setattr(urllib.request, "urlopen", refuse)
    code, events, _ = run("search", "celeste")
    assert code == 0 and [e["id"] for e in events[:-1]] == ["11"]


def test_info_reports_the_build_size_and_the_disk(env, run):
    code, events, _ = run("info", "11")
    assert code == 0
    assert (events[0]["download_size"], events[0]["disk_size"], events[0]["data"]["upload"]["id"]) == (2000, 9000, 103)
    code, _, _ = run("info", "celeste")
    assert code == 1, "an itch.io id is a number"


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
