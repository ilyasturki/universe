import contextlib
import importlib.machinery
import importlib.util
import json
import os
import sqlite3
import stat
import sys
import time
import urllib.request
import zlib
from pathlib import Path

import pytest

MODULE_DIR = Path(__file__).resolve().parents[1]
SOURCE = MODULE_DIR / "bin" / "source"

SHIM = r"""#!SHIM_PYTHON
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
    token = f"tok-{args[args.index('--client-id') + 1]}" if "--client-id" in args else "tok"
    print(json.dumps({"access_token": token, "refresh_token": "r", "user_id": "4242", "expires_in": 3600, "loginTime": 1} if ok else {"error": True}))
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
"""

LIBRARY_PAGE = {
    "totalPages": 1,
    "products": [
        {"id": 1434554947, "title": "Mini Metro", "image": "//images-2.gog-statics.com/abc", "releaseDate": {"date": "2015-11-06 00:00:00.000000"}},
        {"id": 1136126792, "title": "Absolute Drift", "image": None, "releaseDate": None},
    ],
}

CATALOG_PAGE = {
    "products": [
        {"id": "1434554947", "title": "Mini Metro", "releaseDate": "2015.11.06", "coverVertical": "https://x/mm.jpg"},
        {"id": "999", "title": "Metro Exodus", "releaseDate": "2019.02.15", "coverVertical": "https://x/me.jpg"},
    ]
}

BUILDS = {
    "items": [
        {"build_id": "B1", "branch": None, "version_name": "1.0", "date_published": "2024-01-01T00:00:00+0000"},
        {"build_id": "B3", "branch": "beta", "version_name": "1.2b", "date_published": "2025-06-01T00:00:00+0000"},
        {"build_id": "B2", "branch": None, "version_name": "1.1", "date_published": "2025-03-01T00:00:00+0000"},
    ]
}


def write_info(folder, game_id, root_id, name, build="B1", exe="Game.exe"):
    folder.mkdir(parents=True, exist_ok=True)
    (folder / f"goggame-{game_id}.info").write_text(
        json.dumps(
            {
                "gameId": game_id,
                "rootGameId": root_id,
                "name": name,
                "buildId": build,
                "playTasks": [{"isPrimary": True, "type": "FileTask", "path": exe}, {"type": "URLTask", "link": "http://gog.com"}],
            }
        )
    )


@pytest.fixture
def src():
    loader = importlib.machinery.SourceFileLoader("gog_source", str(SOURCE))
    spec = importlib.util.spec_from_loader("gog_source", loader)
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
    (tmp_path / "data").mkdir()
    (tmp_path / "data" / "umu.json").write_text(json.dumps({"1434554947": "umu-287980"}))
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
    monkeypatch.setenv(
        "SOURCE_SETTINGS_JSON",
        json.dumps(
            {
                "games_dir": str(games),
                "scan_dirs": f"{scan},{tmp_path / 'missing'}",
                "auth_path": str(tmp_path / "auth" / "auth.json"),
                "install_timeout_s": 30,
                "platform": "windows",
                "with_dlcs": True,
            }
        ),
    )
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

    def fetch(url, token=None, headers=None):
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


def test_login_takes_the_whole_address_it_lands_on(src, env, capsys, monkeypatch):
    fake_fetch(monkeypatch, src, {src.USER_URL: {"username": "Yasso"}})
    code, events, _ = run(src, capsys, "login", " https://embed.gog.com/on_login_success?origin=client&code=good ")
    assert code == 0 and events[0] == {"event": "logged_in", "user": "Yasso"}
    assert calls(env)[0]["args"][-2:] == ["--code", "good"]


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
    assert mini == {
        "event": "game",
        "id": "1434554947",
        "title": "Mini Metro",
        "owned": True,
        "installed": True,
        "dir": str(env["scan"] / "Mini Metro"),
        "exe": "Mini Metro.exe",
        "build": "B1",
        "dlcs": [],
        "release_year": 2015,
        "image": "https://images-2.gog-statics.com/abc.jpg",
        "disk_size": info_size,
        "store": "gog",
        "umu_id": "umu-287980",
    }
    assert drift["installed"] is False and drift["dir"] is None and drift["release_year"] is None
    assert "disk_size" not in drift
    assert not (env["data"] / "library.json").exists(), "the core owns the library cache"


def test_library_pages(src, env, capsys, monkeypatch):
    pages = {1: {"totalPages": 2, "products": [{"id": 1, "title": "A"}]}, 2: {"totalPages": 2, "products": [{"id": 2, "title": "B"}]}}
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
    assert src.size_total({"*": {"download_size": 1, "disk_size": 2}, "de-DE": {"download_size": 10, "disk_size": 20}}) == {
        "download_size": 11,
        "disk_size": 22,
    }
    assert src.size_total({}) is None and src.size_total(None) is None


def test_install(src, env, capsys, monkeypatch):
    seed_cache(src, env)
    code, events, _ = run(src, capsys, "install", "1434554947")
    assert code == 0
    assert [e["event"] for e in events] == ["progress", "progress", "game", "done"]
    assert events[0] == {"event": "progress", "done": 0, "total": 1000, "message": "0.00%"}
    assert events[1] == {"event": "progress", "done": 1000, "total": 1000, "message": "100.00%"}
    assert events[2] == {
        "event": "game",
        "id": "1434554947",
        "title": "Mini Metro",
        "owned": True,
        "installed": True,
        "dir": str(env["games"] / "Mini Metro"),
        "exe": "Mini Metro.exe",
        "build": "B2",
        "dlcs": [],
        "release_year": 2015,
        "image": "https://images-2.gog-statics.com/abc.jpg",
        "disk_size": (env["games"] / "Mini Metro" / "goggame-1434554947.info").stat().st_size,
        "store": "gog",
        "umu_id": "umu-287980",
    }
    info, call = calls(env)
    assert info["args"][2] == "info", "the folder and the sizes come from info before the download"
    assert call["args"][2:] == ["download", "1434554947", "--platform", "windows", "--with-dlcs", "--path", str(env["games"])]
    assert call["config"] == str(env["data"] / "gogdl")
    assert json.loads((env["data"] / "partials.json").read_text()) == {}, "a finished install leaves no partial"


def test_install_stopped_by_sigterm_keeps_a_resumable_partial(src, env, capsys, monkeypatch):
    monkeypatch.setenv("SHIM_MODE", "partial")
    import subprocess

    proc = subprocess.Popen(
        [sys.executable, str(SOURCE), "install", "1434554947"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=dict(os.environ)
    )
    line = proc.stdout.readline()
    assert json.loads(line)["event"] == "progress"
    proc.send_signal(15)
    out, _err = proc.communicate(timeout=10)
    assert proc.returncode == 143 and "game" not in out
    folder = env["games"] / "Mini Metro"
    assert (folder / ".gogdl-resume").exists() and (folder / "part.bin").exists(), "the files stay for the resume"
    assert json.loads((env["data"] / "partials.json").read_text()) == {
        "1434554947": {"dir": str(folder), "title": "Mini Metro", "download_size": 810, "disk_size": 1220}
    }
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
        {"event": "update", "id": "1434554947", "title": "Mini Metro", "local_build": "B1", "remote_build": "B2", "version": "1.1", "date": "2025-03-01"},
        {"event": "update", "id": "4", "title": "Unknown", "local_build": None, "remote_build": "B2", "version": "1.1", "date": "2025-03-01"},
        {"event": "done"},
    ]
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
    assert calls(env)[0]["args"][2:] == ["update", "1434554947", "--platform", "windows", "--with-dlcs", "--path", str(env["scan"] / "Mini Metro")]


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
    assert dead == {
        "event": "game",
        "id": "1237807960",
        "title": "Dead Cells",
        "owned": None,
        "installed": True,
        "dir": str(folder),
        "exe": "deadcells.exe",
        "build": "B5",
        "dlcs": ["1114691340"],
        "release_year": None,
        "image": None,
        "disk_size": sum(p.stat().st_size for p in folder.iterdir()),
        "store": "gog",
        "umu_id": None,
    }
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


MANIFEST = {"clientId": "5010", "clientSecret": "s3cret", "baseProductId": "1434554947"}


def achievement(key, unlocked=None, visible=True):
    return {
        "achievement_key": key,
        "name": key.title(),
        "description": f"Do {key}",
        "image_url_unlocked": f"https://img/{key}_gac_60.jpg",
        "image_url_locked": f"https://img/{key}-locked.png",
        "visible": visible,
        "date_unlocked": unlocked,
        "rarity": 12.5,
    }


def fake_manifest(monkeypatch, src, manifest=MANIFEST):
    monkeypatch.setattr(src, "fetch_bytes", lambda url: zlib.compress(json.dumps(manifest).encode()))


def test_achievements_page_through_the_games_client(src, env, capsys, monkeypatch):
    fake_manifest(monkeypatch, src)
    base = src.ACHIEVEMENTS_URL.format(client="5010", user="4242")
    seen = fake_fetch(
        monkeypatch,
        src,
        {
            f"{base}?page_token=p2": {"total_count": 3, "page_token": "", "items": [achievement("hidden", visible=False)]},
            base: {"total_count": 3, "page_token": "p2", "items": [achievement("first", "2024-05-01T20:11:04+0000"), achievement("second")]},
            "https://content-system.gog.com/": {"items": [{"link": "https://cdn/manifest"}]},
        },
    )
    code, events, _ = run(src, capsys, "achievements", "1434554947")
    assert code == 0
    assert [e.get("key") for e in events] == ["first", "second", "hidden", None]
    assert events[0] == {
        "event": "achievement",
        "key": "first",
        "name": "First",
        "description": "Do first",
        "unlocked_at": "2024-05-01T20:11:04+0000",
        "hidden": False,
        "icon": "https://img/first.jpg",
        "icon_locked": "https://img/first-locked.png",
        "rarity": 12.5,
    }
    assert events[1]["unlocked_at"] == "" and events[2]["hidden"] is True
    assert [t for u, t in seen if u.startswith(base)] == ["tok-5010", "tok-5010"], "the game's own client token, not Galaxy's"
    auth = [c["args"] for c in calls(env) if "auth" in c["args"]]
    assert auth[-1][-4:] == ["--client-id", "5010", "--client-secret", "s3cret"]

    seen.clear()
    run(src, capsys, "achievements", "1434554947")
    assert not any(u.startswith("https://content-system") for u, _ in seen), "the client id is kept"


def test_achievements_of_a_game_without_a_client(src, env, capsys, monkeypatch):
    fake_manifest(monkeypatch, src, {"baseProductId": "1"})
    fake_fetch(monkeypatch, src, {"https://content-system.gog.com/": {"items": [{"link": "https://cdn/manifest"}]}})
    code, events, err = run(src, capsys, "achievements", "1")
    assert code == 1 and events == []
    assert "no Galaxy client" in err


def test_has_galaxy_looks_a_few_levels_down(src, tmp_path):
    assert not src.has_galaxy("")
    assert not src.has_galaxy(str(tmp_path))
    deep = tmp_path / "Game_Data" / "Plugins" / "x86_64"
    deep.mkdir(parents=True)
    (deep / "Galaxy64.dll").write_bytes(b"")
    assert src.has_galaxy(str(tmp_path))


@pytest.fixture
def hooks(src, env, monkeypatch, tmp_path):
    game = tmp_path / "game"
    game.mkdir()
    (game / "Galaxy64.dll").write_bytes(b"")
    ran = []
    real_run = src.subprocess.run

    def fake_run(cmd, **kwargs):
        if cmd[0] == "gogdl":
            return real_run(cmd, **kwargs)
        ran.append(cmd)
        return src.subprocess.CompletedProcess(cmd, 0, "", "")

    monkeypatch.setattr(src.subprocess, "run", fake_run)
    monkeypatch.setattr(src.shutil, "which", lambda name: "/nix/bin/comet" if name == "comet" else None)
    monkeypatch.setattr(src, "port_open", lambda port: True)
    monkeypatch.setenv("GAME_DIR", str(game))
    monkeypatch.setenv("GAME_ID", "mini-metro")
    monkeypatch.setenv("SOURCE_GAME_ID", "1434554947")
    monkeypatch.setenv("SESSION_ID", "20260924-200000")
    monkeypatch.setenv("SESSION_STARTED_AT", "2026-09-24T20:00:00+02:00")
    monkeypatch.setenv("XDG_DATA_HOME", str(tmp_path / "xdg"))
    monkeypatch.setenv("UNIVERSE_BIN", "/nix/bin/universe")
    auth = tmp_path / "auth" / "auth.json"
    auth.parent.mkdir(parents=True, exist_ok=True)
    auth.write_text(json.dumps({src.GALAXY_CLIENT_ID: {"access_token": "tok", "refresh_token": "r", "user_id": "4242"}}))
    return {"ran": ran, "game": game, "auth": auth, **env}


def test_pre_launch_starts_one_comet_on_gogdls_tokens(src, hooks, capsys):
    src.remember_user(username="Yasso")
    assert src.main(["pre-launch"]) == 0
    stop, start = hooks["ran"]
    assert stop == ["systemctl", "--user", "--quiet", "stop", src.COMET_UNIT]
    assert start[0] == "systemd-run" and f"--unit={src.COMET_UNIT}" in start
    assert start[-5:] == ["/nix/bin/comet", "--from-heroic", "--username", "Yasso", "-q"]
    assert not any("tok" in a for a in start), "no token on a command line"
    config = Path(next(a for a in start if a.startswith("--setenv=XDG_CONFIG_PATH=")).split("=", 2)[2])
    assert os.readlink(config / "heroic" / "gog_store" / "auth.json") == str(hooks["tmp"] / "auth" / "auth.json")
    assert f"--setenv=XDG_DATA_HOME={hooks['tmp'] / 'xdg'}" in start
    session = json.loads((hooks["data"] / "comet-session.json").read_text())
    assert session == {"session": "20260924-200000", "user_id": "4242", "data_home": str(hooks["tmp"] / "xdg")}


@pytest.mark.parametrize("case", ["off", "no-comet", "no-galaxy", "logged-out"])
def test_pre_launch_leaves_comet_alone(src, hooks, monkeypatch, capsys, case):
    if case == "off":
        settings = json.loads(os.environ["SOURCE_SETTINGS_JSON"])
        monkeypatch.setenv("SOURCE_SETTINGS_JSON", json.dumps({**settings, "achievements": False}))
    elif case == "no-comet":
        monkeypatch.setattr(src.shutil, "which", lambda name: None)
    elif case == "no-galaxy":
        (hooks["game"] / "Galaxy64.dll").unlink()
    else:
        hooks["auth"].write_text("{}")
    assert src.main(["pre-launch"]) == 0
    assert hooks["ran"] == []
    assert not (hooks["data"] / "comet-session.json").exists()


def test_pre_launch_goes_on_when_comet_fails(src, hooks, monkeypatch, capsys):
    def boom(*args):
        raise src.subprocess.CalledProcessError(1, ["systemd-run"])

    monkeypatch.setattr(src, "start_comet", boom)
    assert src.main(["pre-launch"]) == 0
    assert "comet did not start" in capsys.readouterr().err
    assert not (hooks["data"] / "comet-session.json").exists()


@pytest.fixture
def prefix(src, hooks, monkeypatch, tmp_path):
    pfx = tmp_path / "pfx"
    (pfx / "drive_c" / "windows").mkdir(parents=True)
    source_dir = tmp_path / "source"
    source_dir.mkdir()
    (source_dir / "GalaxyCommunication.exe").write_bytes(b"MZ stub")
    monkeypatch.setenv("SOURCE_DIR", str(source_dir))
    game = {
        "effective": {"runner_kind": "proton", "runner_path": "/nix/bin/umu-run", "proton_path": "/nix/proton-ge", "prefix": str(pfx)},
        "launch": {"umu_id": "", "arch": ""},
    }
    monkeypatch.setenv("UNIVERSE_GAME_JSON", json.dumps(game))
    return pfx


def test_pre_launch_registers_galaxys_service_in_the_prefix(src, hooks, prefix, capsys):
    assert src.main(["pre-launch"]) == 0
    sc, reg, _stop, start = hooks["ran"]
    assert sc[:4] == ["systemd-run", "--user", "--wait", "--pipe"], "through the user manager: gamescope's capabilities stop at the hook"
    assert sc[sc.index("/nix/bin/umu-run") :] == ["/nix/bin/umu-run", "sc", "create", "GalaxyCommunication", f"binpath={src.GALAXY_SERVICE_EXE}"]
    assert reg[reg.index("/nix/bin/umu-run") :][:4] == ["/nix/bin/umu-run", "reg", "add", src.GALAXY_PATHS_KEY]
    env = dict(a.removeprefix("--setenv=").split("=", 1) for a in sc if a.startswith("--setenv="))
    assert (env["WINEPREFIX"], env["PROTONPATH"], env["GAMEID"], env["PROTON_VERB"]) == (str(prefix), "/nix/proton-ge", "umu-default", "run")
    assert env["PATH"] == os.environ["PATH"]
    assert (prefix / "drive_c/ProgramData/GOG.com/Galaxy/redists/GalaxyCommunication.exe").read_bytes() == b"MZ stub"
    assert start[0] == "systemd-run"


def test_pre_launch_leaves_a_registered_prefix_alone(src, hooks, prefix, capsys):
    (prefix / "system.reg").write_text(
        "[System\\\\ControlSet001\\\\Services\\\\GalaxyCommunication] 1790289668\n[Software\\\\Wow6432Node\\\\GOG.com\\\\GalaxyClient\\\\paths] 1790289688\n"
    )
    assert src.main(["pre-launch"]) == 0
    assert [cmd[0] for cmd in hooks["ran"]] == ["systemctl", "systemd-run"]


@pytest.mark.parametrize("case", ["unmade-prefix", "linux", "no-stub"])
def test_pre_launch_skips_the_service_yet_starts_comet(src, hooks, prefix, monkeypatch, capsys, case):
    if case == "unmade-prefix":
        (prefix / "drive_c" / "windows").rmdir()
    elif case == "linux":
        monkeypatch.setenv("UNIVERSE_GAME_JSON", json.dumps({"effective": {"runner_kind": "linux", "prefix": ""}}))
    else:
        (Path(os.environ["SOURCE_DIR"]) / "GalaxyCommunication.exe").unlink()
    assert src.main(["pre-launch"]) == 0
    assert [cmd[0] for cmd in hooks["ran"]] == ["systemctl", "systemd-run"]


def test_pre_launch_starts_comet_when_wine_hangs(src, hooks, prefix, monkeypatch, capsys):
    fake_run = src.subprocess.run

    def hang(cmd, **kwargs):
        if "/nix/bin/umu-run" in cmd:
            raise src.subprocess.TimeoutExpired(cmd, src.WINE_STEP_S)
        return fake_run(cmd, **kwargs)

    monkeypatch.setattr(src.subprocess, "run", hang)
    assert src.main(["pre-launch"]) == 0
    assert "sc in the prefix" in capsys.readouterr().err
    assert [cmd[0] for cmd in hooks["ran"]] == ["systemctl", "systemd-run"], "no reg add after a failed sc"


def test_pre_launch_logs_why_the_runner_refused(src, hooks, prefix, monkeypatch, capsys):
    fake_run = src.subprocess.run

    def refuse(cmd, **kwargs):
        if "/nix/bin/umu-run" in cmd:
            raise src.subprocess.CalledProcessError(1, cmd, b"", b"steamrt4 updates disabled\nFileNotFoundError: no such runtime\n")
        return fake_run(cmd, **kwargs)

    monkeypatch.setattr(src.subprocess, "run", refuse)
    assert src.main(["pre-launch"]) == 0
    assert "status 1.: steamrt4 updates disabled | FileNotFoundError: no such runtime" in capsys.readouterr().err


COMET_ROW = "INSERT INTO achievement (key, name, description, visible_while_locked, unlock_time, image_url_locked, image_url_unlocked, changed, rarity) VALUES (?, ?, '', 1, ?, '', '', 0, 1.5)"


def comet_db(path, rows):
    path.parent.mkdir(parents=True, exist_ok=True)
    with contextlib.closing(sqlite3.connect(path)) as con:
        if not rows or rows[0][0] != "later":
            con.execute(
                "CREATE TABLE achievement (id INTEGER PRIMARY KEY, key TEXT, name TEXT, description TEXT, visible_while_locked INTEGER, "
                "unlock_time TEXT, image_url_locked TEXT, image_url_unlocked TEXT, changed INTEGER, rarity REAL)"
            )
        con.executemany(COMET_ROW, [(key, key.title(), at) for key, at in rows])
        con.commit()


def test_post_launch_files_the_unlocks_of_this_session(src, hooks, monkeypatch, capsys):
    src.save_json(src.clients_path(), {"1434554947": {"client_id": "5010", "client_secret": "s"}})
    src.save_json(src.session_path(), {"session": "20260924-200000", "user_id": "4242", "data_home": str(hooks["tmp"] / "xdg")})
    db = hooks["tmp"] / "xdg" / "comet" / "gameplay" / "5010" / "4242" / "gameplay.db"
    comet_db(db, [("old", "2025-01-01T10:00:00+0000"), ("new", "2026-09-24T18:30:00+0000"), ("locked", None)])
    ticks = []

    def sleep(s):
        ticks.append(s)
        if len(ticks) > 1:
            raise SystemExit(0)
        comet_db(db, [("later", "2026-09-24T18:45:00"), ("epoch", "1970-01-01T00:00:00+00:00")])

    handlers = {}
    monkeypatch.setattr(src.time, "sleep", sleep)
    monkeypatch.setattr(src.signal, "signal", handlers.__setitem__)
    with pytest.raises(SystemExit):
        src.main(["post-launch"])
    with pytest.raises(SystemExit) as stopped:
        handlers[src.signal.SIGTERM](15, None)
    assert stopped.value.code == 0, "the session's end stopping the watcher is no failure"
    assert all(c[:3] == ["/nix/bin/universe", "achievement-unlocked", "mini-metro"] for c in hooks["ran"])
    reported = [json.loads(c[3]) for c in hooks["ran"]]
    assert [r["key"] for r in reported] == ["new", "later", "epoch"], "an unlock from before the session is no news; a naive time is UTC"
    assert reported[1]["unlocked_at"] == "2026-09-24T18:45:00"
    assert reported[2]["unlocked_at"] >= "2026-09-24", "a game's zero time is stamped now: the key only appeared this session"
    assert reported[0] == {
        "key": "new",
        "name": "New",
        "description": "",
        "unlocked_at": "2026-09-24T18:30:00+0000",
        "hidden": False,
        "icon": "",
        "icon_locked": "",
        "rarity": 1.5,
    }


def test_post_launch_without_this_sessions_comet_does_nothing(src, hooks, capsys):
    src.save_json(src.session_path(), {"session": "an-older-one", "user_id": "4242", "data_home": "/x"})
    assert src.main(["post-launch"]) == 0
    assert hooks["ran"] == []


def test_session_end_stops_its_own_comet_only(src, hooks, monkeypatch, capsys):
    src.save_json(src.session_path(), {"session": "20260924-200000", "user_id": "4242", "data_home": "/x"})
    assert src.main(["session-end"]) == 0
    assert hooks["ran"] == [["systemctl", "--user", "--quiet", "--no-block", "stop", src.COMET_UNIT]]
    hooks["ran"].clear()
    monkeypatch.setenv("SESSION_ID", "another")
    assert src.main(["session-end"]) == 0
    assert hooks["ran"] == []


def test_post_process_refreshes_the_cache(src, hooks, capsys):
    assert src.main(["post-process"]) == 0
    assert hooks["ran"] == [["/nix/bin/universe", "achievements", "mini-metro", "--refresh", "--json"]]


def test_umu_id_is_asked_once_and_a_failed_lookup_again(src, env, monkeypatch):
    seen = fake_fetch(monkeypatch, src, {"https://umu.openwinecomponents.org/umu_api.php?store=gog&codename=20920": [{"title": "W2", "umu_id": "umu-20920"}]})
    assert src.umu_id("20920") == "umu-20920"
    assert src.umu_id("20920") == "umu-20920" and len(seen) == 1, "kept"
    assert src.umu_id("404") is None and src.umu_id("404") is None and len(seen) == 3, "no answer is not kept: asked again"
    fake_fetch(monkeypatch, src, {"https://umu.openwinecomponents.org": []})
    assert src.umu_id("1") is None
    assert json.loads((env["data"] / "umu.json").read_text()) == {"1434554947": "umu-287980", "20920": "umu-20920", "1": ""}, "an empty answer is kept"
