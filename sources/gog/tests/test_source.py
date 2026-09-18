import importlib.machinery
import importlib.util
import json
import os
import stat
import sys
import time
from pathlib import Path

import pytest

MODULE_DIR = Path(__file__).resolve().parents[1]
SOURCE = MODULE_DIR / "bin" / "source"

SHIM = r'''#!SHIM_PYTHON
import json, os, sys, time
from pathlib import Path

args = sys.argv[1:]
with open(os.environ["SHIM_LOG"], "a") as f:
    f.write(json.dumps({"args": args, "config": os.environ.get("GOGDL_CONFIG_PATH")}) + "\n")
while args and args[0].startswith("--"):
    args = args[2:]
verb = args[0]
if verb == "auth":
    ok = ("--code" not in args or args[args.index("--code") + 1] == "good") and os.environ.get("SHIM_MODE") != "noauth"
    print(json.dumps({"access_token": "tok", "refresh_token": "r", "expires_in": 3600, "loginTime": 1} if ok else {"error": True}))
elif verb == "info":
    print(json.dumps({"buildId": "B2", "folder_name": "Mini Metro",
                      "size": {"*": {"download_size": 700, "disk_size": 1000}, "en-US": {"download_size": 100, "disk_size": 200},
                               "fr-FR": {"download_size": 50, "disk_size": 60}},
                      "dlcs": [{"id": "77", "size": {"*": {"download_size": 10, "disk_size": 20}}}]}))
elif verb in ("download", "update"):
    mode = os.environ.get("SHIM_MODE", "ok")
    if mode == "critical":
        print("[TASK_EXEC] CRITICAL: Task writer failed", file=sys.stderr, flush=True)
        time.sleep(60)
        sys.exit(0)
    if mode == "hang":
        time.sleep(60)
        sys.exit(0)
    if mode == "partial":
        folder = Path(args[args.index("--path") + 1]) / "Mini Metro"
        folder.mkdir(parents=True, exist_ok=True)
        (folder / "part.bin").write_bytes(b"x" * 300)
        (folder / ".gogdl-resume").write_text("h:game:part.bin\n")
        print("[PROGRESS] INFO: = Progress: 30.00 300/1000, Running for: 00:00:01, ETA: 00:00:01", file=sys.stderr, flush=True)
        time.sleep(60)
        sys.exit(0)
    for pct, done in (("0.00", 0), ("50.00", 500), ("100.00", 1000)):
        print(f"[PROGRESS] INFO: = Progress: {pct} {done}/1000, Running for: 00:00:01, ETA: 00:00:01",
              file=sys.stderr, flush=True)
    print("(0.001, 'MB') 1000")
    game_id = args[1]
    folder = Path(args[args.index("--path") + 1])
    if verb == "download":
        folder = folder / "Mini Metro"
    folder.mkdir(parents=True, exist_ok=True)
    (folder / f"goggame-{game_id}.info").write_text(json.dumps({
        "gameId": game_id, "rootGameId": game_id, "name": "Mini Metro",
        "buildId": os.environ.get("SHIM_BUILD", "B2"),
        "playTasks": [{"isPrimary": True, "type": "FileTask", "path": "Mini Metro.exe"}]}))
    (folder / ".gogdl-resume").unlink(missing_ok=True)
    sys.exit(int(os.environ.get("SHIM_EXIT", "0")))
'''

LIBRARY_PAGE = {"totalPages": 1, "products": [
    {"id": 1434554947, "title": "Mini Metro", "image": "//images-2.gog-statics.com/abc",
     "releaseDate": {"date": "2015-11-06 00:00:00.000000"}},
    {"id": 1136126792, "title": "Absolute Drift", "image": None, "releaseDate": None},
]}

CATALOG_PAGE = {"products": [
    {"id": "1434554947", "title": "Mini Metro", "releaseDate": "2015.11.06", "coverVertical": "https://x/mm.jpg"},
    {"id": "999", "title": "Metro Exodus", "releaseDate": "2019.02.15", "coverVertical": "https://x/me.jpg"},
]}

BUILDS = {"items": [
    {"build_id": "B1", "branch": None, "version_name": "1.0", "date_published": "2024-01-01T00:00:00+0000"},
    {"build_id": "B3", "branch": "beta", "version_name": "1.2b", "date_published": "2025-06-01T00:00:00+0000"},
    {"build_id": "B2", "branch": None, "version_name": "1.1", "date_published": "2025-03-01T00:00:00+0000"},
]}


def write_info(folder, game_id, root_id, name, build="B1", exe="Game.exe"):
    folder.mkdir(parents=True, exist_ok=True)
    (folder / f"goggame-{game_id}.info").write_text(json.dumps({
        "gameId": game_id, "rootGameId": root_id, "name": name, "buildId": build,
        "playTasks": [{"isPrimary": True, "type": "FileTask", "path": exe},
                      {"type": "URLTask", "link": "http://gog.com"}]}))


@pytest.fixture
def src():
    loader = importlib.machinery.SourceFileLoader("gog_source", str(SOURCE))
    spec = importlib.util.spec_from_loader("gog_source", loader)
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module


@pytest.fixture
def env(tmp_path, monkeypatch):
    shim_dir = tmp_path / "bin"
    shim_dir.mkdir()
    shim = shim_dir / "gogdl"
    shim.write_text(SHIM.replace("SHIM_PYTHON", sys.executable, 1))
    shim.chmod(shim.stat().st_mode | stat.S_IEXEC)
    log = tmp_path / "gogdl.log"
    games = tmp_path / "games"
    scan = tmp_path / "scan"
    write_info(scan / "Mini Metro", "1434554947", "1434554947", "Mini Metro", exe="Mini Metro.exe")
    monkeypatch.setenv("PATH", f"{shim_dir}{os.pathsep}{os.environ['PATH']}")
    monkeypatch.setenv("SHIM_LOG", str(log))
    monkeypatch.setenv("SOURCE_DATA_DIR", str(tmp_path / "data"))
    monkeypatch.setenv("SOURCE_SETTINGS_JSON", json.dumps({
        "games_dir": str(games), "scan_dirs": f"{scan},{tmp_path / 'missing'}",
        "auth_path": str(tmp_path / "auth" / "auth.json"), "install_timeout_s": 30, "platform": "windows", "with_dlcs": True}))
    return {"tmp": tmp_path, "log": log, "games": games, "scan": scan, "data": tmp_path / "data"}


def calls(env):
    if not env["log"].exists():
        return []
    return [json.loads(line) for line in env["log"].read_text().splitlines()]


def run(src, capsys, *argv):
    code = src.main(list(argv))
    out, err = capsys.readouterr()
    events = [json.loads(line) for line in out.splitlines()]
    return code, events, err


def fake_fetch(monkeypatch, src, responses):
    seen = []

    def fetch(url, token=None):
        seen.append((url, token))
        for prefix, payload in responses.items():
            if url.startswith(prefix):
                if isinstance(payload, Exception):
                    raise payload
                return payload
        raise src.SourceError(f"unexpected {url}")

    monkeypatch.setattr(src, "fetch", fetch)
    return seen


def seed_cache(src, env, products=None):
    env["data"].mkdir(exist_ok=True)
    (env["data"] / "library.json").write_text(json.dumps(LIBRARY_PAGE["products"] if products is None else products))


def test_login_url(src, env, capsys):
    code, events, _ = run(src, capsys, "login")
    assert code == 0
    assert events == [{"event": "login_url", "url": src.AUTH_URL}, {"event": "done"}]
    assert calls(env) == []


def test_login_code(src, env, capsys, monkeypatch):
    seen = fake_fetch(monkeypatch, src, {src.USER_URL: {"username": "Yasso"}})
    code, events, _ = run(src, capsys, "login", "good")
    assert code == 0
    assert events == [{"event": "logged_in", "user": "Yasso"}, {"event": "done"}]
    assert seen == [(src.USER_URL, "tok")]
    call = calls(env)[0]
    assert call["args"] == ["--auth-config-path", str(env["tmp"] / "auth" / "auth.json"), "auth", "--code", "good"]
    assert call["config"] == str(env["data"] / "gogdl")
    assert (env["tmp"] / "auth" / "auth.json").read_text() == "{}"


def test_status_logged_in(src, env, capsys, monkeypatch):
    fake_fetch(monkeypatch, src, {src.USER_URL: {"username": "Yasso"}})
    code, events, _ = run(src, capsys, "status")
    assert code == 0
    assert events == [{"event": "logged_in", "user": "Yasso"}, {"event": "done"}]


def test_status_logged_out(src, env, capsys, monkeypatch):
    monkeypatch.setenv("SHIM_MODE", "noauth")
    code, events, _ = run(src, capsys, "status")
    assert code == 0 and events == [{"event": "done"}]


def test_login_bad_code(src, env, capsys, monkeypatch):
    fake_fetch(monkeypatch, src, {})
    code, events, err = run(src, capsys, "login", "bad")
    assert code == 1 and events == []
    assert "rejected" in err


def test_library(src, env, capsys, monkeypatch):
    seen = fake_fetch(monkeypatch, src, {"https://embed.gog.com/account/getFilteredProducts": LIBRARY_PAGE})
    code, events, _ = run(src, capsys, "library")
    assert code == 0
    assert seen == [("https://embed.gog.com/account/getFilteredProducts?mediaType=1&page=1", "tok")]
    assert [e["event"] for e in events] == ["game", "game", "done"]
    mini, drift = events[0], events[1]
    info_size = (env["scan"] / "Mini Metro" / "goggame-1434554947.info").stat().st_size
    assert mini == {"event": "game", "id": "1434554947", "title": "Mini Metro", "owned": True, "installed": True,
                    "dir": str(env["scan"] / "Mini Metro"), "exe": "Mini Metro.exe", "build": "B1", "dlcs": [],
                    "release_year": 2015, "image": "https://images-2.gog-statics.com/abc.jpg", "disk_size": info_size}
    assert drift["installed"] is False and drift["dir"] is None and drift["release_year"] is None
    assert "disk_size" not in drift
    assert not (env["data"] / "library.json").exists(), "the core owns the library cache"


def test_library_pages(src, env, capsys, monkeypatch):
    pages = {1: {"totalPages": 2, "products": [{"id": 1, "title": "A"}]},
             2: {"totalPages": 2, "products": [{"id": 2, "title": "B"}]}}
    monkeypatch.setattr(src, "fetch", lambda url, token=None: pages[int(url[-1])])
    code, events, _ = run(src, capsys, "library")
    assert code == 0 and [e["id"] for e in events[:-1]] == ["1", "2"]


def test_library_offline_fails_even_with_cache(src, env, capsys, monkeypatch):
    seed_cache(src, env)
    fake_fetch(monkeypatch, src, {"https://embed.gog.com/account/getFilteredProducts": src.SourceError("offline")})
    code, events, err = run(src, capsys, "library")
    assert code == 1 and events == [] and "offline" in err, "the core keeps its cache; a silent fallback would hide new purchases"


def test_library_marks_partial_downloads(src, env, capsys, monkeypatch):
    fake_fetch(monkeypatch, src, {"https://embed.gog.com/account/getFilteredProducts": LIBRARY_PAGE})
    folder = env["games"] / "Absolute Drift"
    folder.mkdir(parents=True)
    (folder / "part.bin").write_bytes(b"x" * 300)
    (folder / ".gogdl-resume").write_text("h:game:part.bin\n")
    write_info(folder, "1136126792", "1136126792", "Absolute Drift")
    env["data"].mkdir(exist_ok=True)
    (env["data"] / "partials.json").write_text(json.dumps({"1136126792": {"dir": str(folder), "download_size": 800, "disk_size": 1000}}))
    code, events, err = run(src, capsys, "library")
    assert code == 0
    drift = events[1]
    assert drift["installed"] is False, "a folder still holding .gogdl-resume is a download, not an install"
    assert drift["partial_dir"] == str(folder) and drift["partial_bytes"] > 300
    assert drift["download_size"] == 800 and drift["disk_size"] == 1000
    assert "not finished" in err


def test_a_stopped_update_stays_an_install(src, env, capsys, monkeypatch):
    fake_fetch(monkeypatch, src, {"https://embed.gog.com/account/getFilteredProducts": LIBRARY_PAGE})
    (env["scan"] / "Mini Metro" / ".gogdl-resume").write_text("h:game:part.bin\n")
    code, events, _ = run(src, capsys, "library")
    assert code == 0
    mini = events[0]
    assert mini["installed"] is True and mini["build"] == "B1" and "partial_dir" not in mini, "the old build is still there; update resumes"


def test_search(src, env, capsys, monkeypatch):
    seed_cache(src, env)
    seen = fake_fetch(monkeypatch, src, {src.CATALOG_URL: CATALOG_PAGE})
    code, events, _ = run(src, capsys, "search", "mini", "metro")
    assert code == 0
    assert seen == [(f"{src.CATALOG_URL}?query=like%3Amini%20metro&productType=in%3Agame&limit=20", None)]
    assert [e["event"] for e in events] == ["game", "game", "done"]
    assert events[0]["owned"] is True and events[0]["installed"] is True and events[0]["release_year"] == 2015
    assert events[0]["image"] == "https://x/mm.jpg"
    assert events[1]["owned"] is False and events[1]["installed"] is False


def test_search_without_cache(src, env, capsys, monkeypatch):
    fake_fetch(monkeypatch, src, {src.CATALOG_URL: CATALOG_PAGE})
    code, events, _ = run(src, capsys, "search", "metro")
    assert code == 0 and events[0]["owned"] is None


def test_info(src, env, capsys):
    code, events, _ = run(src, capsys, "info", "1434554947")
    assert code == 0
    assert events[0]["event"] == "info" and events[0]["data"]["folder_name"] == "Mini Metro"
    assert (events[0]["download_size"], events[0]["disk_size"]) == (700 + 100 + 10, 1000 + 200 + 20), "shared + en-US + owned dlcs"
    assert events[1] == {"event": "done"}
    assert calls(env)[0]["args"][2:] == ["info", "1434554947", "--platform", "windows", "--with-dlcs"]


def test_info_skip_dlcs(src, env, capsys, monkeypatch):
    settings = json.loads(os.environ["SOURCE_SETTINGS_JSON"])
    settings.update({"with_dlcs": False, "platform": "linux"})
    monkeypatch.setenv("SOURCE_SETTINGS_JSON", json.dumps(settings))
    code, events, _ = run(src, capsys, "info", "1")
    assert code == 0
    assert calls(env)[0]["args"][-3:] == ["--platform", "linux", "--skip-dlcs"]
    assert (events[0]["download_size"], events[0]["disk_size"]) == (800, 1200)


def test_size_total_falls_back_to_the_first_language(src):
    assert src.size_total({"*": {"download_size": 1, "disk_size": 2}, "de-DE": {"download_size": 10, "disk_size": 20}}) == {"download_size": 11, "disk_size": 22}
    assert src.size_total({}) is None and src.size_total(None) is None


def test_install(src, env, capsys, monkeypatch):
    seed_cache(src, env)
    code, events, _ = run(src, capsys, "install", "1434554947")
    assert code == 0
    assert [e["event"] for e in events] == ["progress", "progress", "game", "done"]
    assert events[0] == {"event": "progress", "done": 0, "total": 1000, "message": "0.00%"}
    assert events[1] == {"event": "progress", "done": 1000, "total": 1000, "message": "100.00%"}
    assert events[2] == {"event": "game", "id": "1434554947", "title": "Mini Metro", "owned": True,
                         "installed": True, "dir": str(env["games"] / "Mini Metro"), "exe": "Mini Metro.exe",
                         "build": "B2", "dlcs": [], "release_year": 2015,
                         "image": "https://images-2.gog-statics.com/abc.jpg",
                         "disk_size": (env["games"] / "Mini Metro" / "goggame-1434554947.info").stat().st_size}
    info, call = calls(env)
    assert info["args"][2] == "info", "the folder and the sizes come from info before the download"
    assert call["args"][2:] == ["download", "1434554947", "--platform", "windows", "--with-dlcs",
                                "--path", str(env["games"])]
    assert call["config"] == str(env["data"] / "gogdl")
    assert json.loads((env["data"] / "partials.json").read_text()) == {}, "a finished install leaves no partial"


def test_install_stopped_by_sigterm_keeps_a_resumable_partial(src, env, capsys, monkeypatch):
    monkeypatch.setenv("SHIM_MODE", "partial")
    import subprocess
    proc = subprocess.Popen([sys.executable, str(SOURCE), "install", "1434554947"], stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, text=True, env=dict(os.environ))
    line = proc.stdout.readline()
    assert json.loads(line)["event"] == "progress"
    proc.send_signal(15)
    out, err = proc.communicate(timeout=10)
    assert proc.returncode == 143 and "game" not in out
    folder = env["games"] / "Mini Metro"
    assert (folder / ".gogdl-resume").exists() and (folder / "part.bin").exists(), "the files stay for the resume"
    assert json.loads((env["data"] / "partials.json").read_text()) == {
        "1434554947": {"dir": str(folder), "title": "Mini Metro", "download_size": 810, "disk_size": 1220}}
    assert src.partial_of("1434554947")["bytes"] > 300
    assert src.scan_installs(src.load_settings(), [str(env["games"])]) == [], "not an install yet"
    import shutil
    shutil.rmtree(env["scan"] / "Mini Metro")
    code, events, _ = run(src, capsys, "scan")
    stopped = next(e for e in events if e.get("id") == "1434554947" and not e["installed"])
    assert stopped["partial_dir"] == str(folder) and stopped["partial_bytes"] > 300 and stopped["title"] == "Mini Metro"
    assert stopped["disk_size"] == 1220 and stopped["download_size"] == 810
    monkeypatch.delenv("SHIM_MODE")
    code, events, _ = run(src, capsys, "install", "1434554947")
    assert code == 0 and events[-2]["installed"] is True, "installing again resumes over the folder"
    assert not (folder / ".gogdl-resume").exists() and src.partial_of("1434554947") is None


def test_install_critical_kills(src, env, capsys, monkeypatch):
    monkeypatch.setenv("SHIM_MODE", "critical")
    started = time.monotonic()
    code, events, err = run(src, capsys, "install", "1434554947")
    assert time.monotonic() - started < 5
    assert code == 1 and events == []
    assert "CRITICAL" in err and "killed" in err


def test_install_timeout_kills(src, env, capsys, monkeypatch):
    monkeypatch.setenv("SHIM_MODE", "hang")
    settings = json.loads(os.environ["SOURCE_SETTINGS_JSON"])
    settings["install_timeout_s"] = 1
    monkeypatch.setenv("SOURCE_SETTINGS_JSON", json.dumps(settings))
    started = time.monotonic()
    code, events, err = run(src, capsys, "install", "1434554947")
    assert time.monotonic() - started < 5
    assert code == 1 and events == [] and "exceeded 1s" in err


def test_install_nonzero_exit(src, env, capsys, monkeypatch):
    monkeypatch.setenv("SHIM_EXIT", "2")
    code, events, err = run(src, capsys, "install", "1434554947")
    assert code == 1 and [e["event"] for e in events] == ["progress", "progress"]
    assert "exit 2" in err


def test_update_list(src, env, capsys, monkeypatch):
    seed_cache(src, env)
    write_info(env["scan"] / "Current", "2", "2", "Current", build="B2")
    write_info(env["scan"] / "Unowned", "3", "3", "Unowned", build="B0")
    write_info(env["scan"] / "Unknown", "4", "4", "Unknown", build=None)
    seed_cache(src, env, LIBRARY_PAGE["products"] + [{"id": 2, "title": "Current"}, {"id": 4, "title": "Unknown"}])
    fake_fetch(monkeypatch, src, {"https://content-system.gog.com/products/": BUILDS})
    code, events, err = run(src, capsys, "update")
    assert code == 0
    assert events == [
        {"event": "update", "id": "1434554947", "title": "Mini Metro", "local_build": "B1", "remote_build": "B2",
         "version": "1.1", "date": "2025-03-01"},
        {"event": "update", "id": "4", "title": "Unknown", "local_build": None, "remote_build": "B2",
         "version": "1.1", "date": "2025-03-01"},
        {"event": "done"}]
    assert "Unowned" in err and "not in the library" in err
    assert calls(env) == []


def test_update_list_no_answer(src, env, capsys, monkeypatch):
    fake_fetch(monkeypatch, src, {"https://content-system.gog.com/products/": src.SourceError("boom")})
    code, events, err = run(src, capsys, "update")
    assert code == 0 and events == [{"event": "done"}] and "boom" in err


def test_update_id_same_build_skips_gogdl(src, env, capsys, monkeypatch):
    write_info(env["scan"] / "Mini Metro", "1434554947", "1434554947", "Mini Metro", build="B2")
    fake_fetch(monkeypatch, src, {"https://content-system.gog.com/products/": BUILDS})
    code, events, err = run(src, capsys, "update", "1434554947")
    assert code == 0 and events == [{"event": "done"}]
    assert "already current" in err
    assert calls(env) == []


def test_update_id(src, env, capsys, monkeypatch):
    seed_cache(src, env)
    seen = fake_fetch(monkeypatch, src, {"https://content-system.gog.com/products/": BUILDS})
    code, events, _ = run(src, capsys, "update", "1434554947")
    assert code == 0
    assert seen == [("https://content-system.gog.com/products/1434554947/os/windows/builds?generation=2", None)]
    assert [e["event"] for e in events] == ["progress", "progress", "game", "done"]
    assert events[2]["build"] == "B2" and events[2]["dir"] == str(env["scan"] / "Mini Metro")
    assert calls(env)[0]["args"][2:] == ["update", "1434554947", "--platform", "windows", "--with-dlcs",
                                         "--path", str(env["scan"] / "Mini Metro")]


def test_update_id_build_mismatch_after(src, env, capsys, monkeypatch):
    monkeypatch.setenv("SHIM_BUILD", "B1")
    fake_fetch(monkeypatch, src, {"https://content-system.gog.com/products/": BUILDS})
    code, events, err = run(src, capsys, "update", "1434554947")
    assert code == 1 and [e["event"] for e in events] == ["progress", "progress"]
    assert "expected B2" in err


def test_update_id_not_installed(src, env, capsys, monkeypatch):
    fake_fetch(monkeypatch, src, {})
    code, events, err = run(src, capsys, "update", "42")
    assert code == 1 and events == [] and "not installed" in err


def test_update_id_unowned_refused(src, env, capsys, monkeypatch):
    seed_cache(src, env, [{"id": 5, "title": "Other"}])
    fake_fetch(monkeypatch, src, {})
    code, events, err = run(src, capsys, "update", "1434554947")
    assert code == 1 and events == [] and "not in the GOG library" in err
    assert calls(env) == []


def test_scan(src, env, capsys):
    folder = env["scan"] / "Dead Cells"
    write_info(folder, "1237807960", "1237807960", "Dead Cells", build="B5", exe="deadcells.exe")
    write_info(folder, "1114691340", "1237807960", "Dead Cells - The Bad Seed", build="B5", exe="")
    write_info(env["scan"] / "prefix" / "drive_c" / "GOG Games" / "Deep", "7", "7", "Deep", build="B7")
    write_info(env["scan"] / "OnlyDlc", "8", "9", "Orphan DLC")
    code, events, err = run(src, capsys, "scan")
    assert code == 0
    assert [e["event"] for e in events] == ["game", "game", "game", "done"]
    dead = events[0]
    assert dead == {"event": "game", "id": "1237807960", "title": "Dead Cells", "owned": None, "installed": True,
                    "dir": str(folder), "exe": "deadcells.exe", "build": "B5", "dlcs": ["1114691340"],
                    "release_year": None, "image": None, "disk_size": sum(p.stat().st_size for p in folder.iterdir())}
    assert [e["id"] for e in events[:-1]] == ["1237807960", "1434554947", "7"]
    assert "OnlyDlc" in err
    assert calls(env) == []


def test_scan_crosses_cache(src, env, capsys):
    seed_cache(src, env)
    write_info(env["scan"] / "Other", "3", "3", "Other")
    code, events, _ = run(src, capsys, "scan")
    assert code == 0
    by_id = {e["id"]: e for e in events[:-1]}
    assert by_id["1434554947"]["owned"] is True and by_id["1434554947"]["release_year"] == 2015
    assert by_id["3"]["owned"] is False


def test_usage(src, capsys):
    assert src.main([]) == 2
    assert src.main(["bogus"]) == 2
