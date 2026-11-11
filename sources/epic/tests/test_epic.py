import io
import json
import os
import shlex
import shutil
import tarfile
import time

import pytest

LIBRARY = [
    {
        "app_name": "Min",
        "app_title": "Hades ",
        "asset_infos": {"Windows": {"build_version": "1.0"}},
        "metadata": {
            "keyImages": [{"type": "Thumbnail", "url": "https://x/thumb.jpg"}, {"type": "DieselGameBoxTall", "url": "https://x/tall.jpg"}],
            "customAttributes": {"FolderName": {"value": "Hades"}},
        },
        "dlcs": [{"app_name": "MinDlc"}],
    },
    {"app_name": "Cat", "app_title": "Catalyst", "asset_infos": {"Windows": {}}, "metadata": {}, "dlcs": []},
    {
        "app_name": "Ea",
        "app_title": "Battlefield",
        "asset_infos": {"Windows": {}},
        "metadata": {"customAttributes": {"ThirdPartyManagedApp": {"value": "Origin"}}},
    },
    {"app_name": "Mac", "app_title": "Mac Only", "asset_infos": {"Mac": {}}, "metadata": {}},
]


@pytest.fixture
def env(tmp_path, monkeypatch, shims):
    data = tmp_path / "data"
    data.mkdir()
    (data / "umu.json").write_text(json.dumps({"Min": "umu-1145360"}))
    monkeypatch.setenv("SHIM_LOG", str(tmp_path / "legendary.log"))
    monkeypatch.setenv("SHIM_LIBRARY", json.dumps(LIBRARY))
    monkeypatch.setenv("SOURCE_DATA_DIR", str(data))
    settings = {
        "games_dir": str(tmp_path / "games"),
        "config_path": str(tmp_path / "legendary"),
        "adopt_from": str(tmp_path / "heroic"),
        "install_timeout_s": 30,
        "with_dlcs": True,
        "achievements": True,
        "anticheat_runtimes": True,
    }
    monkeypatch.setenv("SOURCE_SETTINGS_JSON", json.dumps(settings))
    return {"tmp": tmp_path, "data": data, "config": tmp_path / "legendary", "games": tmp_path / "games", "log": tmp_path / "legendary.log"}


def calls(env):
    return [json.loads(line)["args"] for line in env["log"].read_text().splitlines()] if env["log"].exists() else []


def install_state(env, apps):
    env["config"].mkdir(exist_ok=True)
    (env["config"] / "installed.json").write_text(json.dumps(apps))


def logged_in(env):
    env["config"].mkdir(exist_ok=True)
    (env["config"] / "user.json").write_text(json.dumps({"displayName": "Yasso"}))


def seed_cache(env, events):
    (env["data"] / "library.json").write_text(json.dumps(events))


def hades(env, **extra):
    folder = env["games"] / "Hades"
    (folder / "x64").mkdir(parents=True, exist_ok=True)
    (folder / "x64" / "Hades.exe").write_bytes(b"MZ")
    return {
        "app_name": "Min",
        "title": "Hades",
        "version": "1.0",
        "install_path": str(folder),
        "executable": "x64\\Hades.exe",
        "install_size": 4194304,
        "is_dlc": False,
        "platform": "Windows",
        **extra,
    }


def test_a_code_signs_in_from_the_whole_page_and_status_asks_offline(src, env, run):
    assert "responseType%3Dcode" in src.LOGIN_URL
    code, events, _ = run("login", '{"redirectUrl":"x","authorizationCode":"good","sid":null}')
    assert code == 0 and events == [{"event": "logged_in", "user": "Yasso"}, {"event": "done"}], "the page's whole JSON is taken too"
    assert ["auth", "--code", "good"] in calls(env)
    code, events, _ = run("status")
    assert events == [{"event": "logged_in", "user": "Yasso"}, {"event": "done"}]
    assert calls(env)[-1] == ["status", "--offline", "--json"], "the probe stays off the network"


def test_a_bad_code_is_refused_and_a_new_sign_in_replaces_the_old(env, run):
    code, events, _ = run("login", "bad")
    assert code == 1 and events == []
    code, events, _ = run("status")
    assert events == [{"event": "done"}]
    logged_in(env)
    run("login", '"good"')
    assert calls(env)[-3:] == [["auth", "--delete"], ["auth", "--code", "good"], ["status", "--offline", "--json"]], "legendary would keep the old session"


def test_library_lists_what_installs_and_leaves_the_rest_out(env, run):
    install_state(env, {"Min": hades(env), "MinDlc": {**hades(env), "app_name": "MinDlc", "is_dlc": True}})
    code, events, _ = run("library")
    assert code == 0
    assert [e["id"] for e in events[:-1]] == ["Min", "Cat"], "an EA title and a Mac one cannot be installed here"
    assert events[0] == {
        "event": "game",
        "id": "Min",
        "title": "Hades",
        "owned": True,
        "installed": True,
        "dir": str(env["games"] / "Hades"),
        "exe": "x64/Hades.exe",
        "build": "1.0",
        "dlcs": ["MinDlc"],
        "release_year": None,
        "image": "https://x/tall.jpg",
        "folder": "Hades",
        "owned_dlcs": ["MinDlc"],
        "art": {"box_front": "https://x/tall.jpg"},
        "disk_size": 4194304,
        "store": "egs",
        "umu_id": "umu-1145360",
    }
    assert events[1]["installed"] is False and events[1]["image"] is None and events[1]["folder"] == "Cat"
    assert "art" not in events[1], "a title without key images hands no art"
    assert ["list", "--json"] in calls(env)


def test_signed_out_says_so_without_the_traceback(env, run, monkeypatch):
    monkeypatch.setenv("SHIM_MODE", "signedout")
    code, events, err = run("library")
    assert code == 1 and events == []
    assert "Traceback" not in err and "Logging in" in err, "legendary's own word, not a Python trace"


def test_search_filters_the_owned_library(env, run):
    code, _, _ = run("search", "had")
    assert code == 1, "never listed: nothing to search"
    seed_cache(env, [{"id": "Min", "title": "Hades", "image": "i"}, {"id": "Cat", "title": "Catalyst"}])
    code, events, _ = run("search", "HAD")
    assert code == 0 and [e["id"] for e in events[:-1]] == ["Min"]
    assert events[0]["owned"] is True and events[0]["image"] == "i"


def test_info_reports_the_manifest_sizes(env, run):
    code, events, _ = run("info", "Min")
    assert code == 0
    assert (events[0]["download_size"], events[0]["disk_size"]) == (2097152, 4194304)
    assert calls(env) == [["info", "Min", "--json", "--platform", "Windows"]]


@pytest.mark.parametrize("cached", [False, True], ids=["uncached-folder", "cached-folder"])
def test_install_streams_bytes_and_lands_the_game(env, run, monkeypatch, cached):
    if cached:
        seed_cache(env, [{"id": "Min", "title": "Hades", "folder": "Hades", "owned_dlcs": []}])
    monkeypatch.setenv("SHIM_FOLDER", "Hades")
    code, events, _ = run("install", "Min")
    assert code == 0
    assert [(e["done"], e["total"]) for e in events[:2]] == [(1048576, 2097152), (2097152, 2097152)], "chunks become bytes of the announced download"
    game = events[2]
    folder = env["games"] / ("Hades" if cached else "Min")
    assert (game["id"], game["dir"], game["exe"], game["build"], game["umu_id"]) == ("Min", str(folder), "x64/Hades.exe", "1.0", "umu-1145360")
    assert json.loads((env["data"] / "partials.json").read_text()) == {}, "a finished install leaves no partial"


@pytest.mark.parametrize("emptied", [False, True], ids=["folder-gone", "folder-emptied"])
def test_a_game_whose_program_vanished_installs_again(env, run, monkeypatch, emptied):
    seed_cache(env, [{"id": "Min", "title": "Hades", "folder": "Hades", "owned_dlcs": []}])
    install = hades(env)
    (env["games"] / "Hades" / "x64" / "Hades.exe").unlink()
    if not emptied:
        install["install_path"] = str(env["tmp"] / "unmounted" / "Hades")
    install_state(env, {"Min": install})
    code, events, _ = run("scan")
    assert events == [{"event": "done"}], "nothing to play is no install"
    monkeypatch.setenv("SHIM_FOLDER", "Hades")
    code, events, err = run("install", "Min")
    assert code == 0, err
    game = next(e for e in events if e["event"] == "game")
    assert (game["installed"], game["dir"]) == (True, str(env["games"] / "Hades"))
    assert (env["games"] / "Hades" / "x64" / "Hades.exe").is_file(), "downloaded again"
    assert ["-y", "uninstall", "Min", "--keep-files"] in calls(env), "legendary forgets the old folder first"


@pytest.mark.parametrize("scanned", [True, False], ids=["scan-then-uninstall", "uninstall"])
def test_a_game_a_failed_reinstall_forgot_comes_back_with_its_drive(env, run, monkeypatch, scanned):
    logged_in(env)
    seed_cache(env, [{"id": "Min", "title": "Hades", "folder": "Hades", "owned_dlcs": []}])
    old = env["tmp"] / "unmounted" / "Hades"
    install_state(env, {"Min": hades(env, install_path=str(old))})
    monkeypatch.setenv("SHIM_MODE", "fail")
    code, _, _ = run("install", "Min")
    assert code == 1
    (old / "x64").mkdir(parents=True)
    (old / "x64" / "Hades.exe").write_bytes(b"MZ")
    monkeypatch.setenv("SHIM_MODE", "ok")
    if scanned:
        code, events, _ = run("scan")
        assert [(e["id"], e["installed"], e["dir"]) for e in events if e["event"] == "game"] == [("Min", True, str(old))]
    code, events, _ = run("uninstall", "Min")
    assert code == 0 and not old.exists(), "legendary knows it again, so it removes it"


@pytest.mark.parametrize("resume", [False, True], ids=["no-resume-file", "paused"])
def test_a_returning_games_drive_restores_the_whole_copy_after_a_cut_install(env, run, monkeypatch, resume):
    logged_in(env)
    seed_cache(env, [{"id": "Min", "title": "Hades", "folder": "Hades", "owned_dlcs": []}])
    monkeypatch.setenv("SHIM_FOLDER", "Hades")
    code, _, err = run("install", "Min")
    assert code == 0, err
    folder = env["games"] / "Hades"
    program = folder / "x64" / "Hades.exe"
    program.write_bytes(b"MZ whole game")
    save = folder / "z-save.dat"
    save.write_bytes(b"player progress")
    away = env["tmp"] / "drive-away"
    env["games"].rename(away)
    env["games"].mkdir()
    monkeypatch.setenv("SHIM_FOLDER", "Hades")
    monkeypatch.setenv("SHIM_MODE", "cut-program" if resume else "cut-program-no-resume")
    code, _, _ = run("install", "Min")
    assert code == 1 and program.read_bytes() == b"MZ"
    code, events, err = run("scan")
    assert code == 0, err
    games = [e for e in events if e["event"] == "game"]
    if resume:
        assert [(e["id"], e["installed"], e.get("partial_dir")) for e in games] == [("Min", False, str(folder))]
    else:
        assert games == []
    assert not any(c[:2] == ["-y", "import"] for c in calls(env))
    env["games"].rename(env["tmp"] / "cut-download")
    away.rename(env["games"])
    code, events, err = run("scan")
    assert code == 0, err
    games = [e for e in events if e["event"] == "game"]
    assert [(e["id"], e["installed"], e["dir"], e.get("partial_dir")) for e in games] == [("Min", True, str(folder), None)]
    assert ["-y", "import", "Min", str(folder), "--with-dlcs", "--platform", "Windows"] in calls(env)
    assert program.read_bytes() == b"MZ whole game" and save.read_bytes() == b"player progress"


@pytest.mark.parametrize(
    ("after_uninstall", "folder_name"),
    [(False, "Hades"), (True, "Hades"), (False, "Hades ")],
    ids=["reinstall-cut", "reinstalled-uninstalled-install-cut", "trailing-space"],
)
@pytest.mark.parametrize("resume", [False, True], ids=["no-resume-file", "paused"])
def test_a_cut_download_is_never_restored_as_the_old_copy(env, run, monkeypatch, after_uninstall, folder_name, resume):
    logged_in(env)
    seed_cache(env, [{"id": "Min", "title": "Hades", "folder": folder_name, "owned_dlcs": []}])
    old = hades(env)
    folder = env["games"] / "Hades"
    (folder / "x64" / "Hades.exe").unlink()
    install_state(env, {"Min": old})
    monkeypatch.setenv("SHIM_FOLDER", "Hades")
    if after_uninstall:
        code, _, err = run("install", "Min")
        assert code == 0, err
        code, _, err = run("uninstall", "Min")
        assert code == 0, err
    monkeypatch.setenv("SHIM_MODE", "cut-program" if resume else "cut-program-no-resume")
    code, _, _ = run("install", "Min")
    assert code == 1 and (folder / "x64" / "Hades.exe").is_file()
    for restore_record in ("current", "old-forgotten", "unmarked-download"):
        if restore_record == "old-forgotten":
            (env["data"] / "forgotten.json").write_text(json.dumps({"Min": old}))
        elif restore_record == "unmarked-download":
            partials_path = env["data"] / "partials.json"
            partials = json.loads(partials_path.read_text())
            partials["Min"].pop("download_marker")
            partials["Min"]["games_dir_identity"] = [0, 0]
            partials_path.write_text(json.dumps(partials))
            (folder / ".universe-download").unlink()
        code, events, err = run("scan")
        assert code == 0, err
        assert not any(c[:2] == ["-y", "import"] for c in calls(env)), "the new program is not the old whole copy"
        games = [e for e in events if e["event"] == "game"]
        if resume:
            assert [(e["id"], e["installed"], e.get("partial_dir")) for e in games] == [("Min", False, str(folder))]
        else:
            assert games == []


@pytest.mark.parametrize("shift", ["setting", "device", "copied-back"])
@pytest.mark.parametrize("resume", [False, True], ids=["no-resume-file", "paused"])
def test_a_cut_reinstall_stays_cut_when_its_surroundings_change(env, run, monkeypatch, shift, resume):
    logged_in(env)
    seed_cache(env, [{"id": "Min", "title": "Hades", "folder": "Hades", "owned_dlcs": []}])
    install_state(env, {"Min": hades(env)})
    folder = env["games"] / "Hades"
    program = folder / "x64" / "Hades.exe"
    program.unlink()
    save = folder / "z-save.dat"
    save.write_bytes(b"player progress")
    monkeypatch.setenv("SHIM_FOLDER", "Hades")
    monkeypatch.setenv("SHIM_MODE", "cut-program" if resume else "cut-program-no-resume")
    code, _, _ = run("install", "Min")
    assert code == 1 and program.read_bytes() == b"MZ"
    if shift == "setting":
        settings = json.loads(os.environ["SOURCE_SETTINGS_JSON"])
        other = env["tmp"] / "other-games"
        other.mkdir()
        settings["games_dir"] = str(other)
        monkeypatch.setenv("SOURCE_SETTINGS_JSON", json.dumps(settings))
    elif shift == "device":
        partials_path = env["data"] / "partials.json"
        partials = json.loads(partials_path.read_text())
        stat = env["games"].stat()
        identity = partials["Min"].setdefault("games_dir_identity", [stat.st_dev, stat.st_ino])
        identity[0] += 1
        partials_path.write_text(json.dumps(partials))
    else:
        copied = env["tmp"] / "copied-games"
        shutil.copytree(env["games"], copied)
        env["games"].rename(env["tmp"] / "games-away")
        copied.rename(env["games"])
    code, events, err = run("scan")
    assert code == 0, err
    assert not any(c[:2] == ["-y", "import"] for c in calls(env)), "a cut download stays cut after the shift"
    games = [e for e in events if e["event"] == "game"]
    if resume:
        assert [(e["id"], e["installed"], e.get("partial_dir")) for e in games] == [("Min", False, str(folder))]
    else:
        assert games == []
    assert program.read_bytes() == b"MZ" and save.read_bytes() == b"player progress"


def test_an_install_legendary_gives_up_on_is_an_error(env, run, monkeypatch):
    monkeypatch.setenv("SHIM_MODE", "fail")
    code, events, err = run("install", "Min")
    assert code == 1 and events == []
    assert "Failed to acquire installed data lock" in err, "legendary exits 0 here; the listing tells"


def test_a_stopped_install_is_a_partial_the_next_install_resumes(env, run, interrupt, monkeypatch):
    seed_cache(env, [{"id": "Min", "title": "Hades", "folder": "Hades"}])
    monkeypatch.setenv("SHIM_MODE", "partial")
    monkeypatch.setenv("SHIM_FOLDER", "Hades")
    interrupt("install", "Min")
    monkeypatch.setenv("SHIM_MODE", "ok")
    code, events, _ = run("scan")
    assert code == 0
    partial = events[0]
    assert (partial["id"], partial["installed"], partial["partial_dir"]) == ("Min", False, str(env["games"] / "Hades"))
    assert partial["partial_bytes"] >= 300 and (partial["download_size"], partial["disk_size"]) == (2097152, 4194304)
    (env["config"] / "tmp" / "Min.resume").unlink()
    code, events, _ = run("scan")
    assert events == [{"event": "done"}], "without legendary's resume file there is nothing to resume"


def test_uninstall_goes_through_legendary_so_it_forgets_the_game(env, run):
    install_state(env, {"Min": hades(env)})
    code, events, _ = run("uninstall", "Min")
    assert code == 0 and events == [{"event": "done"}]
    assert ["-y", "uninstall", "Min"] in calls(env)
    assert not (env["games"] / "Hades").exists() and json.loads((env["config"] / "installed.json").read_text()) == {}
    asked = len(calls(env))
    code, events, _ = run("uninstall", "Min")
    assert code == 0 and events == [{"event": "done"}] and all(c[:2] != ["-y", "uninstall"] for c in calls(env)[asked:]), "nothing left to uninstall"


def test_updates_list_the_base_games_and_one_updates(env, run, monkeypatch):
    seed_cache(env, [{"id": "Min", "title": "Hades"}])
    install_state(env, {"Min": hades(env), "MinDlc": {**hades(env), "app_name": "MinDlc", "is_dlc": True}})
    monkeypatch.setenv("SHIM_LATEST", "2.0")
    code, events, _ = run("update")
    assert code == 0
    assert events == [
        {"event": "update", "id": "Min", "title": "Hades", "local_build": "1.0", "remote_build": "2.0", "version": "2.0", "date": ""},
        {"event": "done"},
    ], "a DLC's update rides on its game's"
    code, events, _ = run("update", "Min")
    assert code == 0 and events[-2]["build"] == "2.0"
    assert ["-y", "update", "Min", "--skip-sdl", "--with-dlcs"] in calls(env)
    code, events, _ = run("update", "Nope")
    assert code == 1 and events == [], "not installed"


def test_updates_fail_when_legendary_could_not_log_in_to_check(env, run, monkeypatch):
    install_state(env, {"Min": hades(env)})
    monkeypatch.setenv("SHIM_MODE", "offline")
    code, events, err = run("update")
    assert code == 1 and events == [], "legendary's list is its cache, exit 0"
    assert err.rstrip().splitlines()[-1].endswith("please try again later."), "the cause, not legendary's closing line"


def test_scan_adopts_heroics_installs_once_signed_in(env, run):
    heroic = env["tmp"] / "heroic"
    heroic.mkdir()
    game_dir = env["tmp"] / "elsewhere" / "Fall Guys"
    game_dir.mkdir(parents=True)
    (game_dir / "FallGuys.exe").write_bytes(b"MZ")
    (heroic / "installed.json").write_text(
        json.dumps({"Fall": {"app_name": "Fall", "title": "Fall Guys", "install_path": str(game_dir), "platform": "Windows", "is_dlc": False}})
    )
    code, events, _ = run("scan")
    assert code == 0 and events == [{"event": "done"}], "legendary refuses an import while signed out"
    run("scan")
    assert sum("import" in c for c in calls(env)) == 1, "a failed import waits before the next try"
    (env["data"] / "adopted.json").write_text("{}")
    logged_in(env)
    code, events, _ = run("scan")
    assert [(e["id"], e["dir"], e["owned"]) for e in events[:-1]] == [("Fall", str(game_dir), None)]
    assert ["-y", "import", "Fall", str(game_dir), "--with-dlcs", "--platform", "Windows"] in calls(env)


def test_achievements_come_out_of_every_group(env, run, monkeypatch):
    code, events, _ = run("achievements", "Min")
    assert code == 0
    first, second, hidden = events[:3]
    assert first == {
        "event": "achievement",
        "key": "a",
        "name": "First",
        "description": "d",
        "unlocked_at": "2024-05-01T20:11:04+00:00",
        "hidden": False,
        "icon": "https://x/a.png",
        "icon_locked": "",
        "rarity": 3.5,
    }
    assert (second["unlocked_at"], second["icon"], second["icon_locked"], second["rarity"]) == ("", "", "https://x/b-locked.png", 12.0)
    assert hidden["hidden"] is True and hidden["rarity"] is None
    monkeypatch.setenv("SHIM_MODE", "none")
    code, events, _ = run("achievements", "Min")
    assert code == 0 and events == [{"event": "done"}]


def hook_env(env, monkeypatch, kind="proton", game_dir=None):
    env_file = env["tmp"] / "env-file"
    env_file.write_text("")
    monkeypatch.setenv("UNIVERSE_ENV_FILE", str(env_file))
    monkeypatch.setenv("SOURCE_GAME_ID", "Min")
    monkeypatch.setenv("GAME_DIR", str(game_dir or env["tmp"] / "nogame"))
    monkeypatch.setenv("UNIVERSE_GAME_JSON", json.dumps({"effective": {"runner_kind": kind}}))
    return env_file


def env_lines(env_file):
    return dict(line.split("=", 1) for line in env_file.read_text().splitlines())


def test_pre_launch_hands_the_game_its_epic_arguments(src, env, capsys, monkeypatch):
    env_file = hook_env(env, monkeypatch)
    assert src.main(["pre-launch"]) == 0
    words = shlex.split(env_lines(env_file)["UNIVERSE_GAME_ARGS"])
    assert words[:3] == ["-SaveToUserDir", "-AUTH_LOGIN=unused", "-AUTH_PASSWORD=ex-code"]
    assert "-epicusername=Yas so" in words
    assert words[-1] == "-epicovt=Z:" + str(env["config"] / "tmp" / "ns.ovt").replace("/", "\\"), "a Windows game reads a Windows path"
    assert ["launch", "Min", "--json", "--skip-version-check", "--no-wine"] in calls(env)

    env_file = hook_env(env, monkeypatch, kind="linux")
    src.main(["pre-launch"])
    assert shlex.split(env_lines(env_file)["UNIVERSE_GAME_ARGS"])[-1].startswith("-epicovt=/"), "a native game keeps the Unix one"


def test_pre_launch_falls_back_to_offline(src, env, capsys, monkeypatch):
    monkeypatch.setenv("SHIM_MODE", "offline")
    env_file = hook_env(env, monkeypatch)
    assert src.main(["pre-launch"]) == 0
    assert "-AUTH_PASSWORD=" in env_lines(env_file)["UNIVERSE_GAME_ARGS"].split()
    assert calls(env)[-1][-1] == "--offline"


def test_pre_launch_points_proton_at_the_anticheat_runtime(web, src, env, capsys, monkeypatch):
    game_dir = env["tmp"] / "game"
    (game_dir / "EasyAntiCheat").mkdir(parents=True)
    tarball = io.BytesIO()
    with tarfile.open(fileobj=tarball, mode="w:xz") as tar:
        info = tarfile.TarInfo("eac_runtime/v2/lib64/easyanticheat_x64.so")
        info.size = 2
        tar.addfile(info, io.BytesIO(b"so"))
    web.answers[src.RUNTIMES_URL] = [{"name": "eac_runtime", "url": "https://x/eac.tar.xz"}, {"name": "battleye_runtime", "url": "https://x/be.tar.xz"}]
    web.answers["https://x/eac.tar.xz"] = tarball.getvalue()
    env_file = hook_env(env, monkeypatch, game_dir=game_dir)
    assert src.main(["pre-launch"]) == 0
    runtime = env["data"] / "runtimes" / "eac_runtime" / "eac_runtime"
    assert env_lines(env_file)["PROTON_EAC_RUNTIME"] == str(runtime)
    assert (runtime / "v2" / "lib64" / "easyanticheat_x64.so").read_bytes() == b"so"
    assert "PROTON_BATTLEYE_RUNTIME" not in env_lines(env_file)
    src.main(["pre-launch"])
    assert [url for url, _ in web.seen] == [src.RUNTIMES_URL, "https://x/eac.tar.xz"], "fetched once, kept"


def test_an_empty_umu_answer_is_asked_again_once_a_week_old(web, src, env):
    web.answers["https://umu.openwinecomponents.org"] = []
    week = src.UMU_MISS_TTL_S
    (env["data"] / "umu.json").write_text(
        json.dumps({"Min": "umu-1145360", "Old": {"missed_at": time.time() - week - 60}, "Bare": "", "New": {"missed_at": time.time() - week + 60}})
    )
    assert [src.umu_id(app) for app in ("Min", "Old", "Bare", "New")] == ["umu-1145360", None, None, None]
    asked = [url for url, _ in web.seen]
    assert asked == [src.UMU_URL.format(id="Old"), src.UMU_URL.format(id="Bare")], "an expired miss and an old empty answer are asked again"
    assert src.umu_id("Old") is None and len(web.seen) == 2, "the new miss is kept"
