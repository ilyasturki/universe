import importlib.machinery
import importlib.util
import json
import os
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
TOOL = REPO / "tools" / "docs-media"

TAKES = """
[[take]]
name = "library"
look = "reprise"
size = "1920x1080"
kind = "still"
keys = "E Wait:5 Shot:"
alt = "The library grid"
fake = true

[[take]]
name = "browse"
look = "reprise"
size = "1920x1080"
kind = "clip"
keys = "Mark:start Right Wait:3 Mark:end"
segments = ["start..end"]
alt = "Moving along the row"
"""


def docs_media(root, *args):
    done = subprocess.run([sys.executable, str(TOOL), "--root", str(root), *args], capture_output=True, text=True, check=False)
    return done.returncode, done.stdout + done.stderr


def tool():
    loader = importlib.machinery.SourceFileLoader("docs_media", str(TOOL))
    module = importlib.util.module_from_spec(importlib.util.spec_from_loader("docs_media", loader))
    sys.modules["docs_media"] = module
    loader.exec_module(module)
    return module


def site(root, takes=TAKES, page=""):
    (root / "site/media").mkdir(parents=True, exist_ok=True)
    (root / "site/media/takes.toml").write_text(takes)
    if page:
        (root / "site/src/pages").mkdir(parents=True, exist_ok=True)
        (root / "site/src/pages/index.mdx").write_text(page)


def recorded(root, version):
    module = tool()
    takes, errors = module.load_takes(module.Site(root))
    assert not errors
    entries = {}
    for take in takes:
        folder = root / ("site/src/assets/media" if take.kind == "still" else "site/public/media")
        folder.mkdir(parents=True, exist_ok=True)
        for file in take.files.values():
            (folder / file).write_bytes(b"media")
        w, h = take.encoded
        entries[take.name] = {"name": take.name, "kind": take.kind, "files": take.files, "w": w, "h": h, "alt": take.alt, "version": version, "hash": take.hash}
    (root / "site/src/assets/media/manifest.json").write_text(json.dumps({"version": version, "takes": entries}))


def test_the_repos_takes_are_well_formed_and_a_bad_take_says_why(tmp_path):
    code, out = docs_media(REPO, "check")
    assert code == 0, out
    bad = """
[[take]]
name = "launches"
look = "reprise"
size = "1920x1080"
kind = "still"
keys = "Right Return Shot:"
alt = "x"

[[take]]
name = "faked-launch"
look = "reprise"
size = "1280x800"
kind = "gamescope-clip"
keys = "Expect:dead-cells Return Mark:a"
segments = ["a..b"]
alt = "x"
fake = true
"""
    site(tmp_path, bad)
    code, out = docs_media(tmp_path, "check")
    assert code == 1
    assert "take launches: key 2 (Return)" in out, "a Return with no Expect: before it"
    assert "take launches: its Shot: comes right after a Wait:3" in out, "a still shot before the screen settles"
    assert "take faked-launch: a gamescope-clip launches a real game: never fake" in out
    assert "take faked-launch: segment 'a..b': no Mark:b in its keys" in out


def test_check_holds_pages_to_the_takes_and_strict_to_a_recording_of_them_at_that_version(tmp_path):
    site(tmp_path, page='<Shot name="library" />\n<Clip name="browse" caption="x" />\n')
    assert docs_media(tmp_path, "check")[0] == 0, "nothing recorded yet is no error"
    code, out = docs_media(tmp_path, "check", "--strict", "0.0.12")
    assert code == 1 and "nothing recorded yet" in out
    recorded(tmp_path, "0.0.12")
    assert docs_media(tmp_path, "check", "--strict", "0.0.12")[0] == 0
    code, out = docs_media(tmp_path, "check", "--strict", "0.0.13")
    assert code == 1 and "take library: recorded at 0.0.12, not 0.0.13" in out
    site(tmp_path, TAKES.replace("E Wait:5", "E Wait:6"), page='<Shot name="library" />\n<Shot name="browse" />\n<Clip name="ghost" />\n')
    code, out = docs_media(tmp_path, "check", "--strict", "0.0.12")
    assert code == 1
    assert "take library: changed since it was recorded" in out
    assert '<Shot name="browse"> names a clip' in out
    assert '<Clip name="ghost"> is no take' in out


def test_publish_leaves_the_media_branch_one_commit_of_the_latest_media(tmp_path):
    git = ["git", "-c", "user.name=t", "-c", "user.email=t@t", "-c", "init.defaultBranch=main"]
    subprocess.run([*git, "init", "-q", str(tmp_path)], check=True)
    subprocess.run([*git, "-C", str(tmp_path), "commit", "-q", "--allow-empty", "-m", "main"], check=True)
    main = subprocess.run(["git", "-C", str(tmp_path), "rev-parse", "main"], capture_output=True, text=True, check=True).stdout
    site(tmp_path, page='<Shot name="library" />\n')
    env = {"GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t", "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@t"}
    for version in ("0.0.12", "0.0.13"):
        recorded(tmp_path, version)
        done = subprocess.run(
            [sys.executable, str(TOOL), "--root", str(tmp_path), "publish", version, "--branch", "throwaway"],
            capture_output=True,
            text=True,
            check=False,
            env={**os.environ, **env},
        )
        assert done.returncode == 0, done.stdout + done.stderr
        count = subprocess.run(["git", "-C", str(tmp_path), "rev-list", "--count", "throwaway"], capture_output=True, text=True, check=True).stdout
        assert count.strip() == "1", f"publish {version} left {count.strip()} commits"
    files = subprocess.run(["git", "-C", str(tmp_path), "ls-tree", "-r", "--name-only", "throwaway"], capture_output=True, text=True, check=True).stdout
    assert {"src/assets/media/manifest.json", "src/assets/media/library.avif", "public/media/browse.av1.mp4"} <= set(files.split())
    shown = subprocess.run(["git", "-C", str(tmp_path), "show", "throwaway:src/assets/media/manifest.json"], capture_output=True, text=True, check=True).stdout
    assert json.loads(shown)["version"] == "0.0.13"
    assert subprocess.run(["git", "-C", str(tmp_path), "rev-parse", "main"], capture_output=True, text=True, check=True).stdout == main
