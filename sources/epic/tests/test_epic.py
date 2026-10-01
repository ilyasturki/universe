import importlib.machinery
import importlib.util
import io
import json
import os
import stat
import subprocess
import sys
import tarfile
import urllib.request
from pathlib import Path

import pytest

MODULE_DIR = Path(__file__).resolve().parents[1]
SOURCE = MODULE_DIR / "bin" / "source"

# legendary as the source drives it: its state is installed.json and user.json under LEGENDARY_CONFIG_PATH.
SHIM = r"""#!SHIM_PYTHON
import json, os, sys, time
from pathlib import Path

args = sys.argv[1:]
config = Path(os.environ["LEGENDARY_CONFIG_PATH"])
config.mkdir(parents=True, exist_ok=True)
with open(os.environ["SHIM_LOG"], "a") as f:
    f.write(json.dumps({"args": args, "config": str(config)}) + "\n")
yes = args[0] == "-y"
if yes:
    args = args[1:]
verb, rest = args[0], args[1:]
mode = os.environ.get("SHIM_MODE", "ok")
user = config / "user.json"
installed_path = config / "installed.json"
installed = json.loads(installed_path.read_text()) if installed_path.exists() else {}

def err(message):
    print(f"[cli] ERROR: {message}", file=sys.stderr, flush=True)

def save():
    installed_path.write_text(json.dumps(installed))

if mode == "signedout" and verb in ("list", "achievements"):
    print("[cli] INFO: Logging in...", file=sys.stderr)
    print("Traceback (most recent call last):", file=sys.stderr)
    print('  File "core.py", line 218, in _login', file=sys.stderr)
    print("ValueError: No saved credentials", file=sys.stderr)
    sys.exit(1)
if verb == "status":
    print(json.dumps({"account": json.loads(user.read_text())["displayName"] if user.exists() else "<not logged in>", "games_available": 2}))
elif verb == "auth":
    if "--delete" in rest:
        user.unlink(missing_ok=True)
    elif user.exists():
        print("[cli] INFO: Stored credentials are still valid", file=sys.stderr)
    elif rest[rest.index("--code") + 1] == "good":
        user.write_text(json.dumps({"displayName": "Yasso"}))
    else:
        err("Login attempt failed, please see log for details.")
elif verb == "list" and "--json" in rest:
    print(json.dumps(json.loads(os.environ["SHIM_LIBRARY"])))
elif verb == "list-installed":
    if "--csv" in rest:
        print("App name,App title,Installed version,Available version,Update available,Install size,Install path,Platform")
        for app, g in installed.items():
            latest = os.environ.get("SHIM_LATEST", g["version"])
            print(f"{app},{g['title']},{g['version']},{latest},{latest != g['version']},{g['install_size']},{g['install_path']},Windows")
    else:
        print(json.dumps(list(installed.values())))
elif verb == "info":
    print(json.dumps({"game": {"app_name": rest[0]}, "install": None, "manifest": {"download_size": 2097152, "disk_size": 4194304}}))
elif verb in ("install", "update"):
    app = rest[0]
    if mode == "fail":
        err("Failed to acquire installed data lock")
        sys.exit(0)
    if verb == "install":
        folder = Path(rest[rest.index("--base-path") + 1]) / os.environ.get("SHIM_FOLDER", app)
    else:
        folder = Path(installed[app]["install_path"])
    folder.mkdir(parents=True, exist_ok=True)
    print("[cli] INFO: Install size: 4.00 MiB", file=sys.stderr, flush=True)
    print("[cli] INFO: Download size: 2.00 MiB (Compression savings: 50.0%)", file=sys.stderr, flush=True)
    if mode == "partial":
        (config / "tmp").mkdir(exist_ok=True)
        (config / "tmp" / f"{app}.resume").write_text("x")
        (folder / "part.bin").write_bytes(b"x" * 300)
        print("[DLManager] INFO: = Progress: 25.00% (1/4), Running for 00:00:01, ETA: 00:00:03", file=sys.stderr, flush=True)
        time.sleep(60)
        sys.exit(0)
    for percent, n in (("50.00", 2), ("100.00", 4)):
        print(f"[DLManager] INFO: = Progress: {percent}% ({n}/4), Running for 00:00:01, ETA: 00:00:01", file=sys.stderr, flush=True)
    (folder / "Game.exe").write_bytes(b"MZ")
    (config / "tmp" / f"{app}.resume").unlink(missing_ok=True)
    installed[app] = {"app_name": app, "title": "Hades", "version": os.environ.get("SHIM_LATEST", "1.0"), "install_path": str(folder),
                      "executable": "x64\\Hades.exe", "install_size": 4194304, "is_dlc": False, "platform": "Windows"}
    save()
elif verb == "uninstall":
    if rest[0] in installed:
        import shutil
        shutil.rmtree(installed.pop(rest[0])["install_path"], ignore_errors=True)
        save()
elif verb == "import":
    app, path = rest[0], rest[1]
    if not user.exists():
        err("Log in failed!")
        sys.exit(0)
    installed[app] = {"app_name": app, "title": "Imported", "version": "7", "install_path": path, "executable": "Game.exe",
                      "install_size": 10, "is_dlc": False, "platform": "Windows"}
    save()
elif verb == "launch":
    if mode == "offline" and "--offline" not in rest:
        err("Login failed, cannot continue!")
        sys.exit(1)
    token = "" if "--offline" in rest else "ex-code"
    print(json.dumps({"game_parameters": ["-SaveToUserDir"], "game_executable": "x64/Hades.exe", "game_directory": "/g/Hades",
                      "egl_parameters": ["-AUTH_LOGIN=unused", f"-AUTH_PASSWORD={token}", "-AUTH_TYPE=exchangecode", "-epicapp=" + rest[0],
                                         "-epicusername=Yas so", f"-epicovt={config}/tmp/ns.ovt"],
                      "launch_command": [], "working_directory": "/g/Hades/x64", "user_parameters": [], "environment": {}}))
elif verb == "achievements":
    if mode == "none":
        print("[cli] INFO: No achievements found", file=sys.stderr)
    else:
        print(json.dumps({"completed": [{"name": "a", "display_name": "First", "description": "d", "unlocked": True, "hidden": False,
                                         "unlock_date": "2024-05-01 20:11:04+00:00", "icon_link": "https://x/a.png", "rarity": {"percent": 3.5}}],
                          "in_progress": [], "uninitiated": [{"name": "b", "display_name": "Second", "description": "", "unlocked": False,
                                                              "hidden": False, "unlock_date": None, "icon_link": "https://x/b-locked.png", "rarity": 12}],
                          "hidden": [{"name": "c", "display_name": "Hidden", "description": "", "unlocked": False, "hidden": True,
                                      "unlock_date": None, "icon_link": "", "rarity": None}]}))
else:
    err(f"shim: unexpected {args}")
    sys.exit(3)
"""

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
def src():
    loader = importlib.machinery.SourceFileLoader("epic_source", str(SOURCE))
    spec = importlib.util.spec_from_loader("epic_source", loader)
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
    shim = shim_dir / "legendary"
    shim.write_text(SHIM.replace("SHIM_PYTHON", sys.executable, 1))
    shim.chmod(shim.stat().st_mode | stat.S_IEXEC)
    data = tmp_path / "data"
    data.mkdir()
    (data / "umu.json").write_text(json.dumps({"Min": "umu-1145360"}))
    monkeypatch.setenv("PATH", f"{shim_dir}{os.pathsep}{os.environ['PATH']}")
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


def run(src, capsys, *argv):
    code = src.main(list(argv))
    out, err = capsys.readouterr()
    return code, [json.loads(line) for line in out.splitlines()], err


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
    folder.mkdir(parents=True, exist_ok=True)
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


def test_login_url_then_a_code_signs_in(src, env, capsys):
    code, events, _ = run(src, capsys, "login")
    assert code == 0 and events == [{"event": "login_url", "url": src.LOGIN_URL}, {"event": "done"}]
    assert "responseType%3Dcode" in src.LOGIN_URL
    code, events, _ = run(src, capsys, "login", '{"redirectUrl":"x","authorizationCode":"good","sid":null}')
    assert code == 0 and events == [{"event": "logged_in", "user": "Yasso"}, {"event": "done"}], "the page's whole JSON is taken too"
    assert ["auth", "--code", "good"] in calls(env)
    code, events, _ = run(src, capsys, "status")
    assert events == [{"event": "logged_in", "user": "Yasso"}, {"event": "done"}]
    assert calls(env)[-1] == ["status", "--offline", "--json"], "the probe stays off the network"


def test_a_bad_code_is_refused_and_a_new_sign_in_replaces_the_old(src, env, capsys):
    code, events, err = run(src, capsys, "login", "bad")
    assert code == 1 and events == [] and "rejected" in err
    code, events, _ = run(src, capsys, "status")
    assert events == [{"event": "done"}]
    logged_in(env)
    run(src, capsys, "login", '"good"')
    assert calls(env)[-3:] == [["auth", "--delete"], ["auth", "--code", "good"], ["status", "--offline", "--json"]], "legendary would keep the old session"


def test_library_lists_what_installs_and_leaves_the_rest_out(src, env, capsys):
    install_state(env, {"Min": hades(env), "MinDlc": {**hades(env), "app_name": "MinDlc", "is_dlc": True}})
    code, events, err = run(src, capsys, "library")
    assert code == 0
    assert [e["id"] for e in events[:-1]] == ["Min", "Cat"], "an EA title and a Mac one cannot be installed here"
    assert "Battlefield: installs through Origin" in err and "Mac Only: has no Windows build" in err
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


def test_signed_out_says_so_without_the_traceback(src, env, capsys, monkeypatch):
    monkeypatch.setenv("SHIM_MODE", "signedout")
    code, events, err = run(src, capsys, "library")
    assert code == 1 and events == []
    assert "legendary list failed (exit 1): not signed in to Epic (run login)" in err
    assert "Traceback" not in err and "Logging in" in err


def test_search_filters_the_owned_library(src, env, capsys):
    code, _, err = run(src, capsys, "search", "had")
    assert code == 1 and "never listed" in err
    seed_cache(env, [{"id": "Min", "title": "Hades", "image": "i"}, {"id": "Cat", "title": "Catalyst"}])
    code, events, _ = run(src, capsys, "search", "HAD")
    assert code == 0 and [e["id"] for e in events[:-1]] == ["Min"]
    assert events[0]["owned"] is True and events[0]["image"] == "i"


def test_info_reports_the_manifest_sizes(src, env, capsys):
    code, events, _ = run(src, capsys, "info", "Min")
    assert code == 0
    assert (events[0]["download_size"], events[0]["disk_size"]) == (2097152, 4194304)
    assert calls(env) == [["info", "Min", "--json", "--platform", "Windows"]]


def test_install_streams_bytes_and_lands_the_game(src, env, capsys, monkeypatch):
    seed_cache(env, [{"id": "Min", "title": "Hades", "folder": "Hades", "owned_dlcs": []}])
    monkeypatch.setenv("SHIM_FOLDER", "Hades")
    code, events, _ = run(src, capsys, "install", "Min")
    assert code == 0
    assert [e["event"] for e in events] == ["progress", "progress", "game", "done"]
    assert events[0] == {"event": "progress", "done": 1048576, "total": 2097152, "message": "50.00%"}, "chunks become bytes of the announced download"
    assert events[1]["done"] == 2097152
    game = events[2]
    assert (game["id"], game["dir"], game["exe"], game["build"], game["umu_id"]) == ("Min", str(env["games"] / "Hades"), "x64/Hades.exe", "1.0", "umu-1145360")
    install = next(c for c in calls(env) if "install" in c)
    assert install == ["-y", "install", "Min", "--base-path", str(env["games"]), "--platform", "Windows", "--skip-sdl", "--with-dlcs"]
    assert json.loads((env["data"] / "partials.json").read_text()) == {}, "a finished install leaves no partial"


def test_an_install_legendary_gives_up_on_is_an_error(src, env, capsys, monkeypatch):
    monkeypatch.setenv("SHIM_MODE", "fail")
    code, events, err = run(src, capsys, "install", "Min")
    assert code == 1 and [e["event"] for e in events] == []
    assert "not installed: Failed to acquire installed data lock" in err, "legendary exits 0 here; the listing tells"


def test_a_stopped_install_is_a_partial_the_next_install_resumes(src, env, capsys, monkeypatch):
    seed_cache(env, [{"id": "Min", "title": "Hades", "folder": "Hades"}])
    monkeypatch.setenv("SHIM_MODE", "partial")
    monkeypatch.setenv("SHIM_FOLDER", "Hades")
    proc = subprocess.Popen([sys.executable, str(SOURCE), "install", "Min"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=dict(os.environ))
    assert json.loads(proc.stdout.readline())["event"] == "progress"
    proc.send_signal(15)
    out, _ = proc.communicate(timeout=15)
    assert proc.returncode == 143 and "game" not in out
    monkeypatch.setenv("SHIM_MODE", "ok")
    code, events, _ = run(src, capsys, "scan")
    assert code == 0
    partial = events[0]
    assert (partial["id"], partial["installed"], partial["partial_dir"]) == ("Min", False, str(env["games"] / "Hades"))
    assert partial["partial_bytes"] >= 300 and (partial["download_size"], partial["disk_size"]) == (2097152, 4194304)
    (env["config"] / "tmp" / "Min.resume").unlink()
    code, events, _ = run(src, capsys, "scan")
    assert events == [{"event": "done"}], "without legendary's resume file there is nothing to resume"


def test_uninstall_goes_through_legendary_so_it_forgets_the_game(src, env, capsys):
    install_state(env, {"Min": hades(env)})
    code, events, _ = run(src, capsys, "uninstall", "Min")
    assert code == 0 and events == [{"event": "done"}]
    assert ["-y", "uninstall", "Min"] in calls(env)
    assert not (env["games"] / "Hades").exists() and json.loads((env["config"] / "installed.json").read_text()) == {}
    asked = len(calls(env))
    code, events, _ = run(src, capsys, "uninstall", "Min")
    assert code == 0 and events == [{"event": "done"}] and all(c[:2] != ["-y", "uninstall"] for c in calls(env)[asked:]), "nothing left to uninstall"


def test_updates_list_the_base_games_and_one_updates(src, env, capsys, monkeypatch):
    seed_cache(env, [{"id": "Min", "title": "Hades"}])
    install_state(env, {"Min": hades(env), "MinDlc": {**hades(env), "app_name": "MinDlc", "is_dlc": True}})
    monkeypatch.setenv("SHIM_LATEST", "2.0")
    code, events, _ = run(src, capsys, "update")
    assert code == 0
    assert events == [
        {"event": "update", "id": "Min", "title": "Hades", "local_build": "1.0", "remote_build": "2.0", "version": "2.0", "date": ""},
        {"event": "done"},
    ], "a DLC's update rides on its game's"
    code, events, _ = run(src, capsys, "update", "Min")
    assert code == 0 and events[-2]["build"] == "2.0"
    assert ["-y", "update", "Min", "--skip-sdl", "--with-dlcs"] in calls(env)
    code, _, err = run(src, capsys, "update", "Nope")
    assert code == 1 and "not installed" in err


def test_scan_adopts_heroics_installs_once_signed_in(src, env, capsys):
    heroic = env["tmp"] / "heroic"
    heroic.mkdir()
    game_dir = env["tmp"] / "elsewhere" / "Fall Guys"
    game_dir.mkdir(parents=True)
    (heroic / "installed.json").write_text(
        json.dumps({"Fall": {"app_name": "Fall", "title": "Fall Guys", "install_path": str(game_dir), "platform": "Windows", "is_dlc": False}})
    )
    code, events, err = run(src, capsys, "scan")
    assert code == 0 and events == [{"event": "done"}] and "Fall Guys not adopted: Log in failed!" in err
    run(src, capsys, "scan")
    assert sum("import" in c for c in calls(env)) == 1, "a failed import waits before the next try"
    (env["data"] / "adopted.json").write_text("{}")
    logged_in(env)
    code, events, err = run(src, capsys, "scan")
    assert [(e["id"], e["dir"], e["owned"]) for e in events[:-1]] == [("Fall", str(game_dir), None)]
    assert "adopted Fall Guys" in err
    assert ["-y", "import", "Fall", str(game_dir), "--with-dlcs", "--platform", "Windows"] in calls(env)


def test_achievements_come_out_of_every_group(src, env, capsys, monkeypatch):
    code, events, _ = run(src, capsys, "achievements", "Min")
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
    code, events, _ = run(src, capsys, "achievements", "Min")
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
    args = env_lines(env_file)["UNIVERSE_GAME_ARGS"]
    import shlex

    words = shlex.split(args)
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
    assert "launching offline" in capsys.readouterr().err


def test_pre_launch_points_proton_at_the_anticheat_runtime(src, env, capsys, monkeypatch):
    game_dir = env["tmp"] / "game"
    (game_dir / "EasyAntiCheat").mkdir(parents=True)
    tarball = io.BytesIO()
    with tarfile.open(fileobj=tarball, mode="w:xz") as tar:
        info = tarfile.TarInfo("eac_runtime/v2/lib64/easyanticheat_x64.so")
        info.size = 2
        tar.addfile(info, io.BytesIO(b"so"))
    fetched = []

    def fetch(url):
        fetched.append(url)
        return [{"name": "eac_runtime", "url": "https://x/eac.tar.xz"}, {"name": "battleye_runtime", "url": "https://x/be.tar.xz"}]

    monkeypatch.setattr(src, "fetch", fetch)
    monkeypatch.setattr(src, "fetch_bytes", lambda url: tarball.getvalue())
    env_file = hook_env(env, monkeypatch, game_dir=game_dir)
    assert src.main(["pre-launch"]) == 0
    runtime = env["data"] / "runtimes" / "eac_runtime" / "eac_runtime"
    assert env_lines(env_file)["PROTON_EAC_RUNTIME"] == str(runtime)
    assert (runtime / "v2" / "lib64" / "easyanticheat_x64.so").read_bytes() == b"so"
    assert "PROTON_BATTLEYE_RUNTIME" not in env_lines(env_file)
    src.main(["pre-launch"])
    assert fetched == [src.RUNTIMES_URL], "fetched once, kept"


def test_the_update_listing_reads_legendarys_csv(src, env, capsys, monkeypatch):
    install_state(env, {"Min": hades(env)})
    monkeypatch.setenv("SHIM_LATEST", "2.0")
    rows = src.pending_updates(src.load_settings())
    assert [(r["App name"], r["Available version"]) for r in rows] == [("Min", "2.0")]
