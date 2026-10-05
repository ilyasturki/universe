import json
import os
import time
from pathlib import Path

import pytest

USER_REG = r"""WINE REGISTRY Version 2

[Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Shell Folders] 1766866615
"AppData"="C:\\users\\steamuser\\AppData\\Roaming"
"Local AppData"="C:\\users\\steamuser\\AppData\\Local"
"Personal"="C:\\users\\steamuser\\Documents"
"""


@pytest.fixture
def cloud(src, tmp_path, monkeypatch, shims):
    data, config, game, pfx = tmp_path / "data", tmp_path / "legendary", tmp_path / "games" / "Hades", tmp_path / "prefix"
    for folder in (data, config / "metadata", game, pfx / "drive_c" / "users" / "yasso"):
        folder.mkdir(parents=True)
    (pfx / "user.reg").write_text(USER_REG)
    (config / "user.json").write_text(json.dumps({"displayName": "Yasso", "account_id": "acc42"}))
    installed = {"Min": {"app_name": "Min", "title": "Hades", "version": "1.0", "install_path": str(game), "save_path": None, "is_dlc": False}}
    (config / "installed.json").write_text(json.dumps(installed))
    meta = {"app_name": "Min", "metadata": {"customAttributes": {"CloudSaveFolder": {"value": "{AppData}/Hades/{EpicId}"}}}}
    (config / "metadata" / "Min.json").write_text(json.dumps(meta))
    settings = {
        "games_dir": str(tmp_path / "games"),
        "config_path": str(config),
        "adopt_from": "",
        "install_timeout_s": 30,
        "with_dlcs": True,
        "achievements": False,
        "anticheat_runtimes": False,
        "cloud_saves": True,
    }
    log = tmp_path / "shim.log"
    for key, value in {
        "SHIM_LOG": log,
        "SHIM_CLOUD": tmp_path / "cloud",
        "SOURCE_DATA_DIR": data,
        "SOURCE_SETTINGS_JSON": json.dumps(settings),
        "UNIVERSE_GAME_JSON": json.dumps({"effective": {"runner_kind": "proton", "prefix": str(pfx)}}),
        "UNIVERSE_BIN": shims / "universe",
        "GAME_DIR": game,
        "GAME_ID": "hades",
        "SOURCE_GAME_ID": "Min",
    }.items():
        monkeypatch.setenv(key, str(value))
    monkeypatch.setattr(src, "reachable", lambda host: True)
    local = pfx / "drive_c" / "users" / "steamuser" / "AppData" / "Local" / "Hades" / "acc42"
    return {"data": data, "config": config, "log": log, "local": local, "cloud": tmp_path / "cloud"}


def put(folder, name, text, age_s=0):
    path = folder / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)
    stamp = time.time() - age_s
    os.utime(path, (stamp, stamp))


def syncs(cloud):
    lines = [json.loads(line) for line in cloud["log"].read_text().splitlines()] if cloud["log"].exists() else []
    out = []
    for e in lines:
        if "universe" in e:
            out.append("backup")
        elif "sync-saves" in e["args"]:
            args = e["args"][e["args"].index("sync-saves") + 1 :]
            assert args[0] == "Min", "without an app name legendary syncs every installed game"
            out.append(" ".join(a for a in args[1:] if a.startswith("--") and a != "--save-path"))
    return out


def state(src):
    return src.cloud_entry("Min")


COMPARE = "--skip-upload --skip-download"


def test_a_first_sync_backs_up_and_flags_saves_on_both_sides_where_legendary_would_keep_the_newer(src, cloud):
    put(cloud["local"], "slot1.sav", "local", age_s=600)
    put(cloud["cloud"], "slot1.sav", "cloud")
    assert src.main(["pre-launch"]) == 0
    assert syncs(cloud) == [COMPARE, "backup"]
    assert state(src)["state"] == "conflict"
    assert "base" not in state(src)
    assert (cloud["local"] / "slot1.sav").read_text() == "local"


@pytest.mark.parametrize(("pick", "flag", "kept"), [("keep-cloud", "--force-download", "cloud"), ("keep-local", "--force-upload", "local")])
def test_a_conflict_ends_on_the_side_the_player_keeps(src, cloud, run, pick, flag, kept):
    put(cloud["local"], "slot1.sav", "local", age_s=600)
    put(cloud["cloud"], "slot1.sav", "cloud")
    src.main(["pre-launch"])
    code, events, _err = run("cloud-saves", "Min", pick)
    assert code == 0 and events[0]["state"] == "synced"
    assert syncs(cloud)[-1] == flag
    if pick == "keep-cloud":
        assert syncs(cloud)[-2:] == ["backup", flag]
    assert (cloud["local"] / "slot1.sav").read_text() == kept
    assert (cloud["cloud"] / "slot1.sav").read_text() == kept


def save_path(cloud):
    return json.loads((cloud["config"] / "installed.json").read_text())["Min"]["save_path"]


def test_a_session_brings_newer_cloud_saves_down_before_it_and_its_own_up_after(src, cloud):
    put(cloud["local"], "slot1.sav", "old", age_s=900)
    put(cloud["local"], "stale.sav", "gone in the cloud", age_s=900)
    src.update_cloud("Min", base={"cloud": src.utc_date(time.time() - 900), "local": time.time() - 900})
    put(cloud["cloud"], "slot1.sav", "played elsewhere")
    assert src.main(["pre-launch"]) == 0
    assert syncs(cloud) == [COMPARE, "--skip-upload"]
    assert (cloud["local"] / "slot1.sav").read_text() == "played elsewhere"
    assert not (cloud["local"] / "stale.sav").exists()
    assert state(src)["state"] == "synced"
    assert state(src)["locations"] == [{"name": "saves", "path": str(cloud["local"])}]
    assert save_path(cloud) == str(cloud["local"]), "legendary's own record names the game's folder, not the copy it downloaded into"

    put(cloud["local"], "slot1.sav", "played here", age_s=-120)
    assert src.main(["post-process"]) == 0
    assert syncs(cloud)[2:] == [COMPARE, "--skip-download"]
    assert (cloud["cloud"] / "slot1.sav").read_text() == "played here"
    assert state(src)["state"] == "synced"


def test_saves_changed_here_while_the_cloud_moved_on_are_flagged_after_a_session(src, cloud):
    put(cloud["local"], "slot1.sav", "base", age_s=900)
    src.update_cloud("Min", base={"cloud": src.utc_date(time.time() - 900), "local": time.time() - 900})
    put(cloud["cloud"], "slot1.sav", "played elsewhere", age_s=300)
    put(cloud["local"], "slot1.sav", "played here")
    assert src.main(["post-process"]) == 0
    assert syncs(cloud) == [COMPARE]
    assert state(src)["state"] == "conflict"
    assert (cloud["cloud"] / "slot1.sav").read_text() == "played elsewhere"


@pytest.mark.parametrize("case", ["no folder", "legendary"])
def test_a_game_epic_keeps_no_saves_of_is_unsupported(src, cloud, monkeypatch, case):
    if case == "no folder":
        (cloud["config"] / "metadata" / "Min.json").write_text(json.dumps({"metadata": {"customAttributes": {}}}))
    else:
        monkeypatch.setenv("SHIM_MODE", "cloudless")
    assert src.main(["pre-launch"]) == 0
    assert syncs(cloud) == ([] if case == "no folder" else [COMPARE])
    assert state(src)["state"] == "unsupported"


def test_offline_the_launch_goes_on_without_legendary(src, cloud, monkeypatch):
    monkeypatch.setattr(src, "reachable", lambda host: False)
    assert src.main(["pre-launch"]) == 0
    assert syncs(cloud) == []
    assert state(src)["state"] == "offline"


@pytest.mark.slow
def test_a_download_killed_at_the_deadline_puts_legendarys_save_path_back(src, cloud, monkeypatch):
    monkeypatch.setattr(src, "PRE_LAUNCH_BUDGET_S", 1.5)
    monkeypatch.setattr(src, "SYNC_MIN_S", 0.5)
    monkeypatch.setenv("SHIM_MODE", "hangdown")
    put(cloud["local"], "slot1.sav", "old", age_s=900)
    src.update_cloud("Min", base={"cloud": src.utc_date(time.time() - 900), "local": time.time() - 900})
    put(cloud["cloud"], "slot1.sav", "played elsewhere")
    assert src.main(["pre-launch"]) == 0
    assert syncs(cloud) == [COMPARE, "--skip-upload"]
    assert state(src)["state"] == "error"
    assert save_path(cloud) == str(cloud["local"])
    assert (cloud["local"] / "slot1.sav").read_text() == "old"


def test_a_save_path_legendary_holds_locked_fails_the_sync_rather_than_stay_on_the_copy(src, cloud, monkeypatch):
    import fcntl

    monkeypatch.setattr(src, "INSTALLED_LOCK_WAIT_S", 0.2)
    with open(cloud["config"] / "installed.json.lock", "a") as held:
        fcntl.flock(held, fcntl.LOCK_EX)
        assert src.restore_save_path(json.loads(os.environ["SOURCE_SETTINGS_JSON"]), "Min", cloud["local"]) is False
    assert src.restore_save_path(json.loads(os.environ["SOURCE_SETTINGS_JSON"]), "Min", cloud["local"]) is True
    assert save_path(cloud) == str(cloud["local"])


@pytest.mark.slow
def test_a_sync_out_of_time_is_killed_and_the_game_launches_on_its_saves(src, cloud, monkeypatch):
    monkeypatch.setattr(src, "PRE_LAUNCH_BUDGET_S", 1.0)
    monkeypatch.setattr(src, "SYNC_MIN_S", 0.5)
    monkeypatch.setenv("SHIM_MODE", "hang")
    put(cloud["local"], "slot1.sav", "local")
    assert src.main(["pre-launch"]) == 0
    assert state(src)["state"] == "error" and "did not come down in time" in state(src)["message"]
    assert (cloud["local"] / "slot1.sav").read_text() == "local"


def test_status_answers_offline_from_what_the_last_sync_kept(src, cloud, run, settings):
    settings(cloud_saves=False)
    code, events, _err = run("cloud-saves", "Min")
    assert code == 0
    assert events == [{"event": "cloud", "enabled": False, "state": "", "message": "", "at": "", "locations": []}, {"event": "done"}]
    assert syncs(cloud) == []


def test_the_save_folder_resolves_in_the_prefix_whatever_its_case(src, tmp_path):
    root = tmp_path / "pfx"
    saves = root / "drive_c" / "users" / "yasso" / "AppData" / "Local" / "HADES"
    saves.mkdir(parents=True)
    (root / "user.reg").write_text(USER_REG.replace("steamuser", "yasso"))
    settings = {"config_path": str(tmp_path / "none")}
    assert src.resolve_save_path(settings, "{AppData}\\Hades\\Saves", root, {"runner": "proton", "dir": "/g"}) == saves / "Saves"
    assert src.resolve_save_path(settings, "{InstallDir}/Saves", root, {"runner": "proton", "dir": "/g"}) == Path("/g/Saves")
    assert src.resolve_save_path(settings, "{Unknown}/x", root, {"runner": "proton", "dir": "/g"}) is None
