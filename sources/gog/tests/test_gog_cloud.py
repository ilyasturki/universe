import json
import os
import time

import pytest

SAVES = r"<?APPLICATION_DATA_LOCAL_LOW?>\\Dinosaur Polo Club\\Mini Metro"
USER_REG = r"""WINE REGISTRY Version 2

[Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Shell Folders] 1766866615
#time=1dc776dbeeb8d28
"AppData"="C:\\users\\steamuser\\AppData\\Roaming"
"Local AppData"="C:\\users\\steamuser\\AppData\\Local"
"Personal"="C:\\users\\steamuser\\Documents"

[Software\\Wine] 1766866615
"Version"="win10"
"""


@pytest.fixture
def cloud(src, tmp_path, monkeypatch, shims):
    data, game, pfx = tmp_path / "data", tmp_path / "game", tmp_path / "prefix" / "pfx"
    data.mkdir()
    game.mkdir()
    (game / "goggame-1434554947.info").write_text(json.dumps({"gameId": "1434554947", "rootGameId": "1434554947", "clientId": "5010"}))
    (pfx / "drive_c" / "users" / "yasso" / "AppData" / "LocalLow").mkdir(parents=True)
    (pfx / "user.reg").write_text(USER_REG)
    (data / "cloud-locations.json").write_text(json.dumps({"5010": [{"name": "saves", "location": SAVES}]}))
    log = tmp_path / "shim.log"
    settings = {
        "games_dir": str(tmp_path / "games"),
        "scan_dirs": "",
        "auth_path": str(tmp_path / "auth.json"),
        "install_timeout_s": 30,
        "platform": "windows",
        "with_dlcs": True,
        "achievements": False,
        "cloud_saves": True,
    }
    game_json = {"effective": {"runner_kind": "proton", "prefix": str(pfx.parent)}}
    for key, value in {
        "SHIM_LOG": log,
        "SHIM_CLOUD": tmp_path / "cloud",
        "SOURCE_DATA_DIR": data,
        "SOURCE_SETTINGS_JSON": json.dumps(settings),
        "UNIVERSE_GAME_JSON": json.dumps(game_json),
        "UNIVERSE_BIN": shims / "universe",
        "GAME_DIR": game,
        "GAME_ID": "mini-metro",
        "SOURCE_GAME_ID": "1434554947",
    }.items():
        monkeypatch.setenv(key, str(value))
    monkeypatch.setattr(src, "reachable", lambda host: True)
    local = pfx / "drive_c" / "users" / "steamuser" / "AppData" / "LocalLow" / "Dinosaur Polo Club" / "Mini Metro"
    return {"data": data, "log": log, "local": local, "cloud": tmp_path / "cloud" / "saves"}


def put(folder, name, text, age_s=0):
    path = folder / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)
    stamp = time.time() - age_s
    os.utime(path, (stamp, stamp))
    return path


def ran(cloud):
    lines = [json.loads(line) for line in cloud["log"].read_text().splitlines()] if cloud["log"].exists() else []
    return ["backup" if "universe" in e else e["args"][-1] for e in lines if "universe" in e or "save-sync" in e["args"]]


def state(src):
    return src.cloud_entry("1434554947")


def test_a_first_sync_backs_up_before_gogdl_and_flags_saves_on_both_sides_without_storing_its_timestamp(src, cloud):
    put(cloud["local"], "profile.json", "local")
    put(cloud["cloud"], "profile.json", "cloud")
    assert src.main(["pre-launch"]) == 0, "the game launches anyway"
    assert ran(cloud) == ["backup", "--skip-upload"]
    assert state(src)["state"] == "conflict"
    assert state(src)["ts"] == {}, "gogdl's fresh timestamp would hide the conflict from every later sync"
    assert (cloud["local"] / "profile.json").read_text() == "local"

    assert src.main(["pre-launch"]) == 0
    assert ran(cloud) == ["backup", "--skip-upload", "--skip-upload"], "one backup before the first sync"
    assert state(src)["state"] == "conflict", "the flag stays until the player picks a side"


@pytest.mark.parametrize(("pick", "flag", "kept"), [("keep-cloud", "--force-download", "cloud"), ("keep-local", "--force-upload", "local")])
def test_a_conflict_ends_on_the_side_the_player_keeps(src, cloud, run, pick, flag, kept):
    put(cloud["local"], "profile.json", "local")
    put(cloud["cloud"], "profile.json", "cloud")
    src.main(["pre-launch"])
    code, events, _err = run("cloud-saves", "1434554947", pick)
    assert code == 0
    assert events[0]["event"] == "cloud" and events[0]["state"] == "synced" and events[0]["enabled"] is True
    assert ran(cloud)[-1] == flag
    if pick == "keep-cloud":
        assert ran(cloud)[-2] == "backup", "the saves the cloud replaces are backed up first"
    assert (cloud["local"] / "profile.json").read_text() == kept
    assert (cloud["cloud"] / "profile.json").read_text() == kept
    assert float(state(src)["ts"]["saves"]) > 0


def test_a_session_brings_newer_cloud_saves_down_before_it_and_its_own_up_after(src, cloud):
    src.update_cloud("1434554947", ts={"saves": str(time.time() - 100)})
    put(cloud["local"], "profile.json", "old", age_s=200)
    put(cloud["local"], "stale.log", "gone in the cloud", age_s=200)
    put(cloud["cloud"], "profile.json", "played elsewhere")
    assert src.main(["pre-launch"]) == 0
    assert (cloud["local"] / "profile.json").read_text() == "played elsewhere"
    assert not (cloud["local"] / "stale.log").exists(), "gogdl's download leaves what the cloud has"
    assert state(src)["state"] == "synced"
    assert state(src)["locations"] == [{"name": "saves", "path": str(cloud["local"])}]

    time.sleep(0.01)
    put(cloud["local"], "profile.json", "played here")
    assert src.main(["post-process"]) == 0
    assert (cloud["cloud"] / "profile.json").read_text() == "played here"
    assert ran(cloud) == ["--skip-upload", "--skip-download"], "no backup once synced: the session's own covers it"


def test_the_upload_after_a_session_leaves_gogdl_alone_with_no_saves_here(src, cloud):
    src.update_cloud("1434554947", ts={"saves": "1"})
    put(cloud["cloud"], "profile.json", "cloud")
    assert src.main(["post-process"]) == 0
    assert ran(cloud) == [], "gogdl downloads into an empty folder whatever --skip-download says"


@pytest.mark.slow
def test_a_download_out_of_time_is_killed_and_the_game_launches_on_its_saves_whole(src, cloud, monkeypatch):
    monkeypatch.setattr(src, "PRE_LAUNCH_BUDGET_S", 1.0)
    monkeypatch.setattr(src, "SYNC_MIN_S", 0.5)
    monkeypatch.setenv("SHIM_SYNC", "hang")
    src.update_cloud("1434554947", ts={"saves": "1"})
    put(cloud["local"], "profile.json", "local")
    started = time.monotonic()
    assert src.main(["pre-launch"]) == 0
    assert time.monotonic() - started < 15
    assert state(src)["state"] == "error"
    assert "did not come down in time" in state(src)["message"]
    assert (cloud["local"] / "profile.json").read_text() == "local"
    assert not (cloud["data"] / "staging" / "1434554947" / "saves").exists()


def test_offline_the_launch_goes_on_without_gogdl(src, cloud, monkeypatch):
    monkeypatch.setattr(src, "reachable", lambda host: False)
    assert src.main(["pre-launch"]) == 0
    assert ran(cloud) == []
    assert state(src)["state"] == "offline"


def test_with_the_setting_off_no_session_syncs(src, cloud, settings):
    settings(cloud_saves=False)
    put(cloud["local"], "profile.json", "local")
    assert src.main(["pre-launch"]) == 0
    assert src.main(["post-process"]) == 0
    assert ran(cloud) == []


def test_a_failed_backup_keeps_the_first_sync_from_running(src, cloud, monkeypatch):
    monkeypatch.setenv("SHIM_BACKUP", "fail")
    put(cloud["local"], "profile.json", "local")
    assert src.main(["pre-launch"]) == 0
    assert ran(cloud) == ["backup"]
    assert state(src)["state"] == "error" and "ludusavi does not know" in state(src)["message"]
    assert "ts" not in state(src), "the next launch backs up and tries again"


def test_status_answers_offline_from_what_the_last_sync_kept(src, cloud, run, monkeypatch):
    monkeypatch.delenv("UNIVERSE_GAME_JSON")
    code, events, _err = run("cloud-saves", "1434554947")
    assert code == 0
    assert events == [{"event": "cloud", "enabled": True, "state": "", "message": "", "at": "", "locations": []}, {"event": "done"}]


def test_keeping_the_saves_here_needs_some(src, cloud, run):
    code, events, err = run("cloud-saves", "1434554947", "keep-local")
    assert code == 1 and events == []
    assert "no saves on this device" in err
    assert state(src)["state"] == "error"


def test_the_prefix_names_the_user_folder(src, tmp_path):
    root = tmp_path / "pfx"
    (root / "drive_c").mkdir(parents=True)
    (root / "user.reg").write_text(USER_REG.replace("steamuser", "yasso"))
    folders = src.shell_folders(root, "proton")
    assert folders["DOCUMENTS"] == root / "drive_c" / "users" / "yasso" / "Documents"
    assert folders["APPLICATION_DATA_LOCAL_LOW"] == root / "drive_c" / "users" / "yasso" / "AppData" / "LocalLow"
    assert src.resolve_location(SAVES, folders) == root / "drive_c" / "users" / "yasso" / "AppData" / "LocalLow" / "Dinosaur Polo Club" / "Mini Metro"
    assert src.shell_folders(tmp_path / "bare", "proton")["SAVED_GAMES"] == tmp_path / "bare" / "drive_c" / "users" / "steamuser" / "Saved Games"
    assert src.resolve_location("<?NOWHERE?>/x", folders) is None
