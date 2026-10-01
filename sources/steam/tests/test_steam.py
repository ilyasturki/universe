import contextlib
import importlib.machinery
import importlib.util
import json
import os
import shlex
import signal
import stat
import struct
import subprocess
import sys
import threading
import time
import urllib.request
from pathlib import Path

import pytest

MODULE_DIR = Path(__file__).resolve().parents[1]
PROGRAM = MODULE_DIR / "bin" / "source"
STEAMID = "76561199038489031"
ACCOUNT = "1078223303"

# The client as the source meets it: `steam -silent` writes its pid and signs in; a steam:// URL reaches the running one.
STEAM_SHIM = r"""#!SHIM_PYTHON
import json, os, sys, time
from pathlib import Path

args = sys.argv[1:]
with open(os.environ["SHIM_LOG"], "a") as f:
    f.write(json.dumps({"prog": "steam", "args": args}) + "\n")
root = Path(os.environ["SHIM_ROOT"])
mode = os.environ.get("SHIM_MODE", "ok")
if args == ["-silent"]:
    import ctypes
    ctypes.CDLL(None).prctl(15, b"steam", 0, 0, 0)
    pid_file = Path(os.environ["HOME"]) / ".steam" / "steam.pid"
    pid_file.parent.mkdir(parents=True, exist_ok=True)
    pid_file.write_text(str(os.getpid()))
    time.sleep(0.2)
    with open(root / "logs" / "connection_log.txt", "a") as f:
        f.write("[2026-09-27 18:56:44] [Logged On, 4, 7] [U:1:1078223303] RecvMsgClientLogOnResponse() : processing complete\n")
    time.sleep(20)
elif args and args[0].startswith("steam://install/") and mode != "ignore":
    appid = args[0].rsplit("/", 1)[1]
    lib = Path(os.environ["SHIM_LIBRARY"])
    manifest = lib / "steamapps" / f"appmanifest_{appid}.acf"
    folder = lib / "steamapps" / "common" / "Portal"
    for flags, downloaded, staged in ((1026, 0, 0), (1049858, 500, 0), (1049858, 1000, 1000)):
        manifest.write_text(os.environ["SHIM_MANIFEST"].format(appid=appid, flags=flags, downloaded=downloaded, staged=staged))
        time.sleep(0.3)
    folder.mkdir(parents=True, exist_ok=True)
    (folder / "portal.exe").write_bytes(b"MZ")
    manifest.write_text(os.environ["SHIM_MANIFEST"].format(appid=appid, flags=4, downloaded=1000, staged=2000))
elif args and args[0].startswith("steam://uninstall/") and mode != "ignore":
    appid = args[0].rsplit("/", 1)[1]
    time.sleep(0.3)
    for lib in (Path(os.environ["SHIM_LIBRARY"]), root):
        (lib / "steamapps" / f"appmanifest_{appid}.acf").unlink(missing_ok=True)
"""

SYSTEMD_RUN_SHIM = r"""#!SHIM_PYTHON
import json, os, subprocess, sys

args = sys.argv[1:]
with open(os.environ["SHIM_LOG"], "a") as f:
    f.write(json.dumps({"prog": "systemd-run", "args": args}) + "\n")
if os.environ.get("SHIM_SYSTEMD") == "fail":
    print("Failed to connect to bus: No medium found", file=sys.stderr)
    sys.exit(1)
split = args.index("--")
env = dict(os.environ)
for i, arg in enumerate(args[:split]):
    if arg == "-E":
        key, value = args[i + 1].split("=", 1)
        env[key] = value
    elif arg == "-p" and args[i + 1].startswith("UnsetEnvironment="):
        env.pop(args[i + 1].split("=", 1)[1], None)
subprocess.Popen(args[split + 1 :], env=env, start_new_session=True, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
"""

SYSTEMCTL_SHIM = r"""#!SHIM_PYTHON
import json, os, sys

with open(os.environ["SHIM_LOG"], "a") as f:
    f.write(json.dumps({"prog": "systemctl", "args": sys.argv[1:]}) + "\n")
sys.exit(3)
"""

PORTAL_MANIFEST = """"AppState"
{{
	"appid"		"{appid}"
	"name"		"Portal"
	"StateFlags"		"{flags}"
	"installdir"		"Portal"
	"buildid"		"7"
	"LastOwner"		"76561199038489031"
	"SizeOnDisk"		"4096"
	"BytesToDownload"		"1000"
	"BytesDownloaded"		"{downloaded}"
	"BytesToStage"		"2000"
	"BytesStaged"		"{staged}"
}}
"""

APPS = {
    480: {
        "common": {"name": "Spacewar", "type": "Game", "oslist": "windows,macos,linux"},
        "config": {"installdir": "Spacewar", "launch": {"0": {"executable": "SteamWorksExample.exe", "type": "none"}}},
        "depots": {
            "229006": {"config": {"oslist": "windows"}, "depotfromapp": 228980},
            "481": {"manifests": {"public": {"gid": "31835", "size": 1906055, "download": 797632}}},
        },
    },
    48700: {
        "common": {"name": "Mount & Blade: Warband", "type": "game", "oslist": "windows,macos,linux", "steam_release_date": "1270051200"},
        "config": {
            "installdir": "MountBlade Warband",
            "launch": {
                "0": {"executable": "mb_warband.exe", "config": {"oslist": "windows"}},
                "2": {"executable": "mb_warband_linux", "config": {"oslist": "linux"}},
                "3": {"executable": "mbw_config.sh", "config": {"oslist": "linux"}},
            },
        },
        "depots": {
            "48701": {"manifests": {"public": {"size": 1000, "download": 600}}},
            "48703": {"config": {"oslist": "linux"}, "manifests": {"public": {"size": 30, "download": 7}}},
            "48704": {"config": {"oslist": "windows"}, "manifests": {"public": {"size": 35, "download": 17}}},
            "48705": {"config": {"oslist": "windows", "language": "german"}, "manifests": {"public": {"size": 9, "download": 9}}},
        },
    },
    582160: {
        "common": {"name": "Assassin's Creed Origins", "type": "Game", "oslist": "windows"},
        "config": {
            "installdir": "Assassins Creed Origins",
            "launch": {
                "0": {
                    "executable": "ACOrigins.exe",
                    "arguments": '-uplay_steam_mode "C:\\a b"',
                    "type": "default",
                    "config": {"oslist": "windows", "osarch": "64"},
                },
                "1": {"executable": "ACOrigins.exe", "type": "server"},
            },
        },
    },
    1493710: {"common": {"name": "Proton - Experimental", "type": "Tool"}, "config": {"installdir": "Proton - Experimental"}},
    3658110: {"common": {"name": "Proton 10.0", "type": "Tool"}, "config": {"installdir": "Proton 10.0"}},
    891390: {
        "common": {"name": "SteamPlay 2.0 Manifests", "type": "Config"},
        "extended": {"compat_tools": {"proton_10": {"appid": 3658110, "aliases": "proton-10.0-4pin"}}, "app_mappings": {}},
    },
}
CONFIG_VDF = """"InstallConfigStore"
{{
	"Software"
	{{
		"Valve"
		{{
			"Steam"
			{{
				"CompatToolMapping"
				{{
					"480"
					{{
						"name"		"proton_10"
						"priority"		"250"
					}}
					"0"
					{{
						"name"		"{default}"
						"priority"		"75"
					}}
				}}
			}}
		}}
	}}
}}
"""


def kv_bytes(table, strings=None):
    out = bytearray()
    for key, value in table.items():
        if strings is None:
            name = key.encode() + b"\0"
        else:
            if key not in strings:
                strings.append(key)
            name = struct.pack("<I", strings.index(key))
        if isinstance(value, dict):
            out += b"\x00" + name + kv_bytes(value, strings)
        elif isinstance(value, int):
            out += b"\x02" + name + struct.pack("<i", value)
        else:
            out += b"\x01" + name + str(value).encode() + b"\0"
    return bytes(out) + b"\x08"


def appinfo_bytes(apps, version=29):
    strings = [] if version == 29 else None
    entries = bytearray()
    for appid, data in apps.items():
        body = struct.pack("<IIQ", 2, 0, 0) + b"\0" * 20 + struct.pack("<I", 0) + b"\0" * 20 + kv_bytes({"appinfo": {"appid": appid, **data}}, strings)
        entries += struct.pack("<II", appid, len(body)) + body
    entries += struct.pack("<I", 0)
    if strings is None:
        return struct.pack("<II", 0x07564428, 1) + bytes(entries)
    table = struct.pack("<I", len(strings)) + b"".join(s.encode() + b"\0" for s in strings)
    return struct.pack("<IIq", 0x07564429, 1, 16 + len(entries)) + bytes(entries) + table


def manifest_text(appid, name, installdir, flags=4, depots=(), **extra):
    fields = {"appid": appid, "name": name, "StateFlags": flags, "installdir": installdir, "LastOwner": STEAMID, **extra}
    lines = ['"AppState"', "{", *(f'\t"{k}"\t\t"{v}"' for k, v in fields.items())]
    lines += ['\t"InstalledDepots"', "\t{", *(f'\t\t"{d}"\n\t\t{{\n\t\t\t"manifest"\t\t"1"\n\t\t}}' for d in depots), "\t}", "}"]
    return "\n".join(lines) + "\n"


def install(lib, appid, name, installdir, files=(), **fields):
    (lib / "steamapps").mkdir(parents=True, exist_ok=True)
    (lib / "steamapps" / f"appmanifest_{appid}.acf").write_text(manifest_text(appid, name, installdir, **fields))
    folder = lib / "steamapps" / "common" / installdir
    folder.mkdir(parents=True, exist_ok=True)
    for file in files:
        (folder / file).write_bytes(b"MZ")
    return folder


def shim(folder, name, text):
    path = folder / name
    path.write_text(text.replace("SHIM_PYTHON", sys.executable, 1))
    path.chmod(path.stat().st_mode | stat.S_IEXEC)


@pytest.fixture
def src():
    loader = importlib.machinery.SourceFileLoader("steam_source", str(PROGRAM))
    spec = importlib.util.spec_from_loader("steam_source", loader)
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    module.POLL_S = 0.05
    module.PROGRESS_INTERVAL_S = 0
    return module


@pytest.fixture(autouse=True)
def offline(monkeypatch):
    def refuse(*args, **kwargs):
        raise OSError("no network in tests")

    monkeypatch.setattr(urllib.request, "urlopen", refuse)


@pytest.fixture
def env(tmp_path, monkeypatch):
    home, root, lib = tmp_path / "home", tmp_path / "Steam", tmp_path / "Library"
    for folder in (home, root / "steamapps", root / "config", root / "logs", root / "appcache" / "stats", root / "legacycompat", lib / "steamapps"):
        folder.mkdir(parents=True)
    (root / "logs" / "connection_log.txt").write_text("")
    for name in ("steamclient.dll", "steamclient64.dll", "GameOverlayRenderer64.dll", "SteamService.exe", "Steam.dll"):
        (root / "legacycompat" / name).write_bytes(b"MZ")
    (root / "steamapps" / "libraryfolders.vdf").write_text(
        f'"libraryfolders"\n{{\n\t"0"\n\t{{\n\t\t"path"\t\t"{root}"\n\t}}\n\t"1"\n\t{{\n\t\t"path"\t\t"{lib}"\n\t}}\n}}\n'
    )
    (root / "config" / "loginusers.vdf").write_text(
        f'"users"\n{{\n\t"76561198000000001"\n\t{{\n\t\t"PersonaName"\t\t"Old"\n\t\t"Timestamp"\t\t"1"\n\t}}\n'
        f'\t"{STEAMID}"\n\t{{\n\t\t"AccountName"\t\t"yasso"\n\t\t"PersonaName"\t\t"Yasso"\n\t\t"Timestamp"\t\t"1790529866"\n\t}}\n}}\n'
    )
    (root / "appcache" / "appinfo.vdf").write_bytes(appinfo_bytes(APPS))
    install(lib, 480, "Spacewar", "Spacewar", files=("SteamworksExample.exe",), depots=(481,), buildid=3538192, SizeOnDisk=1906055)
    install(root, 48700, "Mount & Blade: Warband", "MountBlade Warband", files=("mb_warband_linux", "mbw_config.sh"), depots=(48701, 48703))
    install(root, 1493710, "Proton - Experimental", "Proton - Experimental", files=("proton",))
    install(root, 582160, "Assassin's Creed Origins", "Assassins Creed Origins", flags=1026, BytesToDownload=1000, BytesDownloaded=250)
    install(lib, 3658110, "Proton 10.0", "Proton 10.0", files=("proton",))
    custom = root / "compatibilitytools.d" / "GE-Proton10-1"
    custom.mkdir(parents=True)
    (custom / "proton").write_text("")
    (custom / "compatibilitytool.vdf").write_text(
        '"compatibilitytools"\n{\n\t"compat_tools"\n\t{\n\t\t"GE-Proton10-1"\n\t\t{\n\t\t\t"install_path"\t\t"."\n\t\t}\n\t}\n}\n'
    )
    (root / "config" / "config.vdf").write_text(CONFIG_VDF.format(default="GE-Proton10-1"))
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    shim(bin_dir, "steam", STEAM_SHIM)
    shim(bin_dir, "systemd-run", SYSTEMD_RUN_SHIM)
    shim(bin_dir, "systemctl", SYSTEMCTL_SHIM)
    data = tmp_path / "data"
    monkeypatch.setenv("HOME", str(home))
    monkeypatch.setenv("PATH", f"{bin_dir}{os.pathsep}{os.environ['PATH']}")
    monkeypatch.setenv("SHIM_LOG", str(tmp_path / "shim.log"))
    monkeypatch.setenv("SHIM_ROOT", str(root))
    monkeypatch.setenv("SHIM_LIBRARY", str(lib))
    monkeypatch.setenv("SHIM_MANIFEST", PORTAL_MANIFEST)
    monkeypatch.setenv("SOURCE_DATA_DIR", str(data))
    settings = {"steam_root": str(root), "confirm_timeout_s": 5, "install_timeout_s": 20, "achievements": True}
    monkeypatch.setenv("SOURCE_SETTINGS_JSON", json.dumps(settings))
    yield {"tmp": tmp_path, "home": home, "root": root, "lib": lib, "data": data, "log": tmp_path / "shim.log"}
    with contextlib.suppress(OSError, ValueError):
        os.kill(int((home / ".steam" / "steam.pid").read_text()), signal.SIGTERM)


@pytest.fixture
def client(env):
    """A running client: a live process named steam."""
    proc = subprocess.Popen([sys.executable, "-c", "import ctypes, time; ctypes.CDLL(None).prctl(15, b'steam', 0, 0, 0); time.sleep(30)"])
    deadline = time.monotonic() + 5
    while Path(f"/proc/{proc.pid}/comm").read_text().strip() != "steam" and time.monotonic() < deadline:
        time.sleep(0.01)
    (env["home"] / ".steam").mkdir(exist_ok=True)
    (env["home"] / ".steam" / "steam.pid").write_text(str(proc.pid))
    yield proc
    proc.kill()
    proc.wait()


def calls(env, prog=None):
    lines = env["log"].read_text().splitlines() if env["log"].exists() else []
    return [entry["args"] for entry in map(json.loads, lines) if prog in (None, entry["prog"])]


def run(src, capsys, *argv):
    code = src.main(list(argv))
    out, err = capsys.readouterr()
    return code, [json.loads(line) for line in out.splitlines()], err


def games(events):
    return {e["id"]: e for e in events if e["event"] == "game"}


def save_key(env, steamid=STEAMID):
    env["data"].mkdir(exist_ok=True)
    (env["data"] / "key.json").write_text(json.dumps({"key": "A" * 32, "steamid": steamid}))


def test_login_takes_a_key_for_the_account_steam_remembers(src, env, capsys, monkeypatch):
    code, events, _ = run(src, capsys, "login")
    assert code == 0 and events == [{"event": "login_url", "url": src.KEY_URL}, {"event": "done"}]
    code, _, err = run(src, capsys, "login", "not-a-key")
    assert code == 1 and "32 hexadecimal" in err
    asked = []
    monkeypatch.setattr(src, "fetch", lambda url: asked.append(url) or {"response": {"game_count": 1, "games": [{"appid": 480, "name": "Spacewar"}]}})
    code, events, _ = run(src, capsys, "login", " 0123456789abcdef0123456789ABCDEF ")
    assert code == 0 and events == [{"event": "logged_in", "user": "Yasso"}, {"event": "done"}]
    assert f"steamid={STEAMID}" in asked[0], "the most recent sign-in, not the oldest"
    key = env["data"] / "key.json"
    assert json.loads(key.read_text()) == {"key": "0123456789abcdef0123456789ABCDEF", "steamid": STEAMID}
    assert stat.S_IMODE(key.stat().st_mode) == 0o600
    code, events, _ = run(src, capsys, "status")
    assert events == [{"event": "logged_in", "user": "Yasso"}, {"event": "done"}]


def test_a_refused_key_is_not_kept_and_another_accounts_key_is_not_used(src, env, capsys, monkeypatch):
    def refuse(url):
        raise src.HttpError("GET https://api.steampowered.com/...: HTTP 403", 403)

    monkeypatch.setattr(src, "fetch", refuse)
    code, _, err = run(src, capsys, "login", "A" * 32)
    assert code == 1 and "refused the Web API key" in err and "A" * 32 not in err
    assert not (env["data"] / "key.json").exists()
    save_key(env, steamid="76561198000000001")
    code, events, _ = run(src, capsys, "status")
    assert events == [{"event": "done"}], "Steam now remembers another account"
    code, _, err = run(src, capsys, "library")
    assert code == 1 and "not Yasso's" in err and "run login" in err


def test_scan_reports_the_games_of_every_library_and_leaves_the_tools_out(src, env, capsys):
    code, events, _ = run(src, capsys, "scan")
    assert code == 0
    assert [e["title"] for e in events[:-1]] == ["Assassin's Creed Origins", "Mount & Blade: Warband", "Spacewar"]
    found = games(events)
    assert found["480"] == {
        "event": "game",
        "id": "480",
        "title": "Spacewar",
        "owned": True,
        "installed": True,
        "dir": str(env["lib"] / "steamapps" / "common" / "Spacewar"),
        "exe": "SteamworksExample.exe",
        "build": "3538192",
        "dlcs": [],
        "release_year": None,
        "image": src.IMAGE_URL.format(id=480),
        "steam_appid": 480,
        "disk_size": 1906055,
        "store": "steam",
        "umu_id": "umu-480",
        "prefix": str(env["lib"] / "steamapps" / "compatdata" / "480"),
        "proton": str(env["lib"] / "steamapps" / "common" / "Proton 10.0"),
    }
    warband = found["48700"]
    assert (warband["runner"], warband["exe"], warband["release_year"]) == ("linux", "mb_warband_linux", 2010), "its Linux depot is the one installed"
    assert "prefix" not in warband, "a native build has no prefix"
    partial = found["582160"]
    assert (partial["installed"], partial["partial_bytes"], partial["download_size"]) == (False, 250, 1000)
    assert partial["partial_dir"] == str(env["root"] / "steamapps" / "common" / "Assassins Creed Origins")


def test_a_windows_depot_picks_the_windows_program_and_steams_prefix(src, env, capsys, monkeypatch):
    install(env["root"], 48700, "Mount & Blade: Warband", "MountBlade Warband", files=("mb_warband.exe",), depots=(48701, 48704))
    _, events, _ = run(src, capsys, "scan")
    warband = games(events)["48700"]
    assert warband["exe"] == "mb_warband.exe" and "runner" not in warband
    assert warband["prefix"] == str(env["root"] / "steamapps" / "compatdata" / "48700"), "the compatdata of the game's own library"
    assert warband["proton"] == str(env["root"] / "compatibilitytools.d" / "GE-Proton10-1"), "no pick for the game: the one for all other titles"
    apps = {**APPS, 891390: {**APPS[891390], "extended": {**APPS[891390]["extended"], "app_mappings": {"48700": {"tool": "proton-10.0-4pin"}}}}}
    (env["root"] / "appcache" / "appinfo.vdf").write_bytes(appinfo_bytes(apps))
    _, events, _ = run(src, capsys, "scan")
    assert games(events)["48700"]["proton"] == str(env["lib"] / "steamapps" / "common" / "Proton 10.0"), "Valve's pick, named by an alias"
    (env["root"] / "appcache" / "appinfo.vdf").write_bytes(appinfo_bytes(APPS))
    (env["root"] / "config" / "config.vdf").write_text(CONFIG_VDF.format(default="GE-Proton-gone"))
    src.compat_mapping.cache_clear()
    _, events, _ = run(src, capsys, "scan")
    assert "proton" not in games(events)["48700"] and "prefix" in games(events)["48700"], "a tool that is not there: Universe's Proton"
    monkeypatch.setenv("SOURCE_SETTINGS_JSON", json.dumps({"steam_root": str(env["root"]), "shared_prefix": False}))
    _, events, _ = run(src, capsys, "scan")
    assert {"prefix", "proton"}.isdisjoint(games(events)["48700"]), "off, the game gets a prefix of Universe's own"


def test_library_lists_the_owned_games_with_the_installed_ones(src, env, capsys, monkeypatch):
    code, _, err = run(src, capsys, "library")
    assert code == 1 and "no Steam Web API key" in err
    save_key(env)
    owned = [{"appid": 620, "name": "Portal 2"}, {"appid": 480, "name": "Spacewar"}]
    monkeypatch.setattr(src, "fetch", lambda url: {"response": {"game_count": 2, "games": owned}})
    code, events, _ = run(src, capsys, "library")
    assert code == 0 and [e["id"] for e in events[:-1]] == ["620", "480"]
    portal, spacewar = events[0], events[1]
    assert (portal["installed"], portal["owned"], portal["image"], portal["steam_appid"]) == (False, True, src.IMAGE_URL.format(id=620), 620)
    assert (spacewar["installed"], spacewar["exe"]) == (True, "SteamworksExample.exe")
    monkeypatch.setattr(src, "fetch", lambda url: {"response": {}})
    code, events, err = run(src, capsys, "library")
    assert code == 0 and events == [{"event": "done"}] and "private" in err


def test_search_asks_the_store_and_falls_back_on_the_listed_library(src, env, capsys, monkeypatch):
    (env["data"]).mkdir(exist_ok=True)
    (env["data"] / "library.json").write_text(json.dumps([{"id": "620", "title": "Portal 2"}, {"id": "400", "title": "Portal"}]))
    items = [{"type": "app", "id": 620, "name": "Portal 2"}, {"type": "app", "id": 1, "name": "Portal Knights"}, {"type": "sub", "id": 9, "name": "Bundle"}]
    monkeypatch.setattr(src, "fetch", lambda url: {"total": 3, "items": items})
    code, events, _ = run(src, capsys, "search", "portal")
    assert code == 0 and [(e["id"], e["owned"]) for e in events[:-1]] == [("620", True), ("1", False)]

    def down(url):
        raise src.SourceError("GET https://store.steampowered.com/api/storesearch/: timed out")

    monkeypatch.setattr(src, "fetch", down)
    code, events, err = run(src, capsys, "search", "portal")
    assert code == 0 and [e["id"] for e in events[:-1]] == ["620", "400"] and "searching the listed library" in err


def test_info_sums_the_depots_of_the_installed_platform(src, env, capsys):
    _, events, _ = run(src, capsys, "info", "48700")
    assert events[0]["data"]["platform"] == "linux"
    assert (events[0]["download_size"], events[0]["disk_size"]) == (607, 1030), "the shared depot and the Linux one"
    _, events, _ = run(src, capsys, "info", "480")
    assert (events[0]["download_size"], events[0]["disk_size"]) == (797632, 1906055), "a redistributable from another app is not counted"


def test_install_starts_steam_opens_its_window_and_follows_the_download(src, env, capsys):
    code, events, _ = run(src, capsys, "install", "400")
    assert code == 0
    assert events[0] == {"event": "window", "class": "steam", "title": "Install"}, "before the URL: the dialog maps right after"
    assert events[1] == {"event": "progress", "done": 0, "total": 0, "message": "confirm the install in Steam's window"}
    progress = [(e["done"], e["total"]) for e in events if e["event"] == "progress"][1:]
    assert (500, 1000) in progress and (1000, 2000) in progress, "the download, then the files written"
    game = events[-2]
    assert (game["id"], game["installed"], game["exe"], game["title"]) == ("400", True, None, "Portal"), "Portal has no appinfo here: no program known"
    assert calls(env, "systemctl") == [["--user", "is-active", "--quiet", "universe-steam"]]
    started, url = calls(env, "systemd-run")
    assert "--unit=universe-steam" in started and started[-1] == "-silent"
    assert url[-1] == "steam://install/400" and not any(a.startswith("--unit") for a in url)


def test_an_install_nobody_confirms_gives_up_and_an_installed_game_is_there_at_once(src, env, capsys, monkeypatch, client):
    monkeypatch.setenv("SHIM_MODE", "ignore")
    monkeypatch.setenv("SOURCE_SETTINGS_JSON", json.dumps({"steam_root": str(env["root"]), "confirm_timeout_s": 1, "install_timeout_s": 20}))
    code, events, err = run(src, capsys, "install", "400")
    assert code == 1 and "Steam started no install of 400" in err
    assert calls(env, "systemd-run")[0][-1] == "steam://install/400", "the client runs already: no second one"
    code, events, _ = run(src, capsys, "install", "480")
    assert code == 0 and [e["event"] for e in events] == ["game", "done"]
    assert len(calls(env, "systemd-run")) == 1


def test_uninstall_goes_through_steams_window_and_waits_for_the_manifest_to_go(src, env, capsys, monkeypatch, client):
    code, events, _ = run(src, capsys, "uninstall", "480")
    assert code == 0 and events == [
        {"event": "window", "class": "steam", "title": ""},
        {"event": "progress", "done": 0, "total": 0, "message": "confirm the uninstall in Steam's window"},
        {"event": "done"},
    ]
    assert not (env["lib"] / "steamapps" / "appmanifest_480.acf").exists()
    assert calls(env, "systemd-run")[-1][-1] == "steam://uninstall/480"
    code, events, _ = run(src, capsys, "uninstall", "480")
    assert code == 0 and events == [{"event": "done"}] and len(calls(env, "systemd-run")) == 1, "nothing left in Steam: no window"
    monkeypatch.setenv("SHIM_MODE", "ignore")
    monkeypatch.setenv("SOURCE_SETTINGS_JSON", json.dumps({"steam_root": str(env["root"]), "confirm_timeout_s": 1}))
    code, _, err = run(src, capsys, "uninstall", "48700")
    assert code == 1 and "did not uninstall Mount & Blade: Warband within 1 s" in err


def test_updates_are_listed_and_one_is_waited_for(src, env, capsys, client):
    lib = env["lib"]
    install(lib, 480, "Spacewar", "Spacewar", files=("SteamworksExample.exe",), flags=6, buildid=1, TargetBuildID=2)
    code, events, _ = run(src, capsys, "update")
    assert events == [
        {"event": "update", "id": "480", "title": "Spacewar", "local_build": "1", "remote_build": "2", "version": "2", "date": ""},
        {"event": "done"},
    ]

    def steam_updates():
        time.sleep(0.3)
        install(lib, 480, "Spacewar", "Spacewar", flags=1030, buildid=1, TargetBuildID=2, BytesToDownload=10, BytesDownloaded=5)
        time.sleep(0.3)
        install(lib, 480, "Spacewar", "Spacewar", flags=4, buildid=2, TargetBuildID=2)

    threading.Thread(target=steam_updates, daemon=True).start()
    code, events, _ = run(src, capsys, "update", "480")
    assert code == 0 and events[-2]["build"] == "2"
    assert {"event": "progress", "done": 5, "total": 10, "message": "50.0%"} in events
    assert calls(env, "systemd-run") == [], "no URL asks for an update"


def test_achievements_come_from_steams_stats_cache(src, env, capsys, monkeypatch):
    stats = env["root"] / "appcache" / "stats"
    code, events, _ = run(src, capsys, "achievements", "480")
    assert code == 0 and events == [{"event": "done"}], "no schema yet: nothing to list"
    bits = {
        "0": {
            "name": "WIN",
            "display": {
                "name": {"english": "Winner", "french": "Gagnant", "token": "T"},
                "desc": {"english": "Win."},
                "hidden": "0",
                "icon": "a.jpg",
                "icon_gray": "b.jpg",
            },
        },
        "1": {"name": "SECRET", "display": {"name": "Secret", "desc": "", "hidden": "1"}},
        "31": {"name": "LAST", "display": {"name": "Last", "desc": "Top bit."}},
    }
    schema = {"480": {"gamename": "Spacewar", "stats": {"0": {"type": "4", "id": "0", "bits": bits}, "5": {"type": "1", "name": "NumGames"}}}}
    (stats / "UserGameStatsSchema_480.bin").write_bytes(kv_bytes(schema))
    progress = {"cache": {"crc": 0, "0": {"data": -2147483647, "AchievementTimes": {"0": 1659290613}}, "5": {"data": 3}}}
    (stats / f"UserGameStats_{ACCOUNT}_480.bin").write_bytes(kv_bytes(progress))
    os.utime(stats / f"UserGameStats_{ACCOUNT}_480.bin", (1700000000, 1700000000))
    (env["home"] / ".steam").mkdir(exist_ok=True)
    (env["home"] / ".steam" / "registry.vdf").write_text(
        '"Registry"\n{\n"HKCU"\n{\n"Software"\n{\n"Valve"\n{\n"Steam"\n{\n"language"\t"french"\n}\n}\n}\n}\n}\n'
    )
    monkeypatch.setattr(
        src, "fetch", lambda url: {"achievementpercentages": {"achievements": [{"name": "WIN", "percent": "51.5"}, {"name": "LAST", "percent": 2}]}}
    )
    code, events, _ = run(src, capsys, "achievements", "480")
    assert code == 0
    win, secret, last = events[:3]
    assert win == {
        "event": "achievement",
        "key": "WIN",
        "name": "Gagnant",
        "description": "Win.",
        "unlocked_at": "2022-07-31T18:03:33+00:00",
        "hidden": False,
        "icon": src.ICON_URL.format(id="480", icon="a.jpg"),
        "icon_locked": src.ICON_URL.format(id="480", icon="b.jpg"),
        "rarity": 51.5,
    }
    assert (secret["unlocked_at"], secret["hidden"], secret["icon"], secret["rarity"]) == ("", True, "", None)
    assert (last["unlocked_at"], last["rarity"]) == ("2023-11-14T22:13:20+00:00", 2.0), "unlocked without a time: the file's"


def hook_env(env, monkeypatch, appid, kind, exe, prefix=None):
    env_file = env["tmp"] / "env-file"
    env_file.write_text("")
    monkeypatch.setenv("UNIVERSE_ENV_FILE", str(env_file))
    monkeypatch.setenv("SOURCE_GAME_ID", appid)
    monkeypatch.setenv("GAME_EXE", str(exe))
    monkeypatch.setenv("UNIVERSE_GAME_JSON", json.dumps({"effective": {"runner_kind": kind, "prefix": str(prefix or "")}}))
    return env_file


def env_lines(env_file):
    return dict(line.split("=", 1) for line in env_file.read_text().splitlines())


def pe_with(section):
    header = bytearray(b"MZ" + b"\0" * 62)
    struct.pack_into("<I", header, 0x3C, 64)
    coff = b"PE\0\0" + struct.pack("<HHIIIHH", 0x14C, 2, 0, 0, 0, 0, 0)
    sections = b"".join(name.ljust(8, b"\0") + b"\0" * 32 for name in (b".text", section))
    return bytes(header) + coff + sections


def test_pre_launch_links_steams_client_into_the_prefix_and_hands_the_arguments(src, env, capsys, monkeypatch, client):
    folder = install(env["root"], 582160, "Assassin's Creed Origins", "Assassins Creed Origins", files=("ACOrigins.exe",))
    prefix = env["tmp"] / "prefix"
    prefix.mkdir()
    (prefix / "pfx").symlink_to(".")
    env_file = hook_env(env, monkeypatch, "582160", "proton", folder / "ACOrigins.exe", prefix)
    assert src.main(["pre-launch"]) == 0
    assert shlex.split(env_lines(env_file)["UNIVERSE_GAME_ARGS"]) == ["-uplay_steam_mode", "C:\\a b"], "Windows quoting, backslashes kept"
    steam_dir = prefix / "drive_c" / "Program Files (x86)" / "Steam"
    assert os.readlink(steam_dir / "steamclient.dll") == str(env["root"] / "legacycompat" / "steamclient.dll")
    assert os.readlink(steam_dir / "steam.exe") == str(env["root"] / "legacycompat" / "SteamService.exe")
    assert calls(env, "systemd-run") == [], "the client runs already"

    compat = env["tmp"] / "compatdata"
    kept = compat / "pfx" / "drive_c" / "Program Files (x86)" / "Steam" / "steamclient.dll"
    kept.parent.mkdir(parents=True)
    kept.write_bytes(b"copied by Proton")
    hook_env(env, monkeypatch, "582160", "proton", folder / "ACOrigins.exe", compat)
    assert src.main(["pre-launch"]) == 0
    assert kept.read_bytes() == b"copied by Proton" and (kept.parent / "Steam.dll").is_symlink(), "Steam's layout: the prefix is pfx/"


def test_pre_launch_names_the_app_to_a_native_game_and_warns_of_steamstub(src, env, capsys, monkeypatch, client):
    folder = env["root"] / "steamapps" / "common" / "MountBlade Warband"
    env_file = hook_env(env, monkeypatch, "48700", "linux", folder / "mb_warband_linux")
    assert src.main(["pre-launch"]) == 0
    assert env_lines(env_file) == {"SteamAppId": "48700", "SteamGameId": "48700"}
    assert not (folder / "drive_c").exists()
    wrapped = env["lib"] / "steamapps" / "common" / "Spacewar" / "SteamworksExample.exe"
    wrapped.write_bytes(pe_with(b".bind"))
    hook_env(env, monkeypatch, "480", "proton", wrapped, env["tmp"] / "pfx480")
    assert src.main(["pre-launch"]) == 0
    assert "wrapped in SteamStub" in capsys.readouterr().err
    wrapped.write_bytes(pe_with(b".rdata"))
    src.main(["pre-launch"])
    assert "SteamStub" not in capsys.readouterr().err


def test_pre_launch_starts_steam_and_cancels_the_launch_when_it_cannot(src, env, capsys, monkeypatch):
    folder = env["lib"] / "steamapps" / "common" / "Spacewar"
    hook_env(env, monkeypatch, "480", "proton", folder / "SteamworksExample.exe", env["tmp"] / "pfx480")
    monkeypatch.setenv("DISPLAY", ":2")
    monkeypatch.delenv("WAYLAND_DISPLAY", raising=False)
    other = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)"])
    (env["home"] / ".steam").mkdir(exist_ok=True)
    (env["home"] / ".steam" / "steam.pid").write_text(str(other.pid))
    try:
        assert src.client_pid() is None, "the pid of a client killed long ago, another program's now"
        assert src.main(["pre-launch"]) == 0
    finally:
        other.kill()
        other.wait()
    assert src.client_pid() is not None and "starting Steam" in capsys.readouterr().err
    (started,) = calls(env, "systemd-run")
    assert "DISPLAY=:2" in started and "UnsetEnvironment=WAYLAND_DISPLAY" in started, "inside gamescope: its X display, not the desktop's Wayland one"
    os.kill(src.client_pid(), signal.SIGTERM)
    time.sleep(0.2)
    monkeypatch.setenv("SHIM_SYSTEMD", "fail")
    assert src.main(["pre-launch"]) == 1, "the game would only show Steam's Fatal Error"
    assert "systemd-run failed (exit 1): Failed to connect to bus" in capsys.readouterr().err


def test_appinfo_reads_the_inline_keys_and_the_string_table(src, env):
    older = env["tmp"] / "appinfo28.vdf"
    older.write_bytes(appinfo_bytes(APPS, version=28))
    for path in (older, env["root"] / "appcache" / "appinfo.vdf"):
        info = src.AppInfo(path)
        assert sorted(info.offsets) == sorted(str(appid) for appid in APPS)
        assert info.get("48700")["config"]["launch"]["2"]["executable"] == "mb_warband_linux"
        assert info.get("999") == {}
