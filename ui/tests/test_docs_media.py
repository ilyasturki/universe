import importlib.machinery
import importlib.util
import json
import os
import shutil
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


def test_publish_leaves_the_media_branch_one_commit_that_fetch_lands_in_the_site(tmp_path):
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
    shutil.rmtree(tmp_path / "site/src/assets/media")
    shutil.rmtree(tmp_path / "site/public/media")
    code, out = docs_media(tmp_path, "fetch", "throwaway")
    assert code == 0, out
    manifest = json.loads((tmp_path / "site/src/assets/media/manifest.json").read_text())
    assert manifest["version"] == "0.0.13", "fetch lands the last published media where the site reads them"
    assert (tmp_path / "site/src/assets/media/library.avif").is_file() and (tmp_path / "site/public/media/browse.av1.mp4").is_file()
    assert subprocess.run(["git", "-C", str(tmp_path), "rev-parse", "main"], capture_output=True, text=True, check=True).stdout == main


def test_bump_holds_a_release_to_its_recorded_media_and_lets_its_pages_ride_along(tmp_path):
    git = ["git", "-C", str(tmp_path), "-c", "user.name=t", "-c", "user.email=t@t"]
    subprocess.run(["git", "init", "-q", str(tmp_path)], check=True)
    (tmp_path / "tools").mkdir()
    for name in ("bump", "changelog", "docs-media"):
        shutil.copy2(REPO / "tools" / name, tmp_path / "tools" / name)
    (tmp_path / "Cargo.toml").write_text('[workspace.package]\nversion = "0.0.11"\n')
    (tmp_path / "CHANGELOG.md").write_text(
        "# Changelog\n\n## [0.0.12] - 2026-10-10\n\n### Fixed\n\n- A fix.\n\n[0.0.12]: https://github.com/ilyasturki/universe/compare/v0.0.11...v0.0.12\n"
    )
    page = tmp_path / "site/src/content/docs/docs/using/home.mdx"
    page.parent.mkdir(parents=True)
    page.write_text('<Shot name="library" />\n')
    (tmp_path / ".gitignore").write_text("/site/src/assets/media/\n/site/public/media/\n")
    site(tmp_path)
    subprocess.run([*git, "add", "-A"], check=True)
    subprocess.run([*git, "commit", "-q", "-m", "main"], check=True)

    def bump(**env):
        done = subprocess.run(
            ["bash", str(tmp_path / "tools/bump"), "--dry-run", "0.0.12"], capture_output=True, text=True, check=False, env={**os.environ, **env}
        )
        return done.returncode, done.stdout + done.stderr

    page.write_text('<Shot name="library" />\nA page the release fixed.\n')
    (tmp_path / "site/media/takes.toml").write_text(TAKES.replace("E Wait:5", "E Wait:6"))
    code, out = bump()
    assert code == 1 and "not recorded at 0.0.12" in out, out
    assert bump(BUMP_SKIP_MEDIA="1")[0] == 0, "a hotfix releases on the last release's media"
    recorded(tmp_path, "0.0.11")
    assert bump()[0] == 1, "media recorded for the last release"
    recorded(tmp_path, "0.0.12")
    code, out = bump()
    assert code == 0 and "dry run" in out, "the docs page and takes.toml ride in chore(release)"
    (tmp_path / "Cargo.toml").write_text('[workspace.package]\nversion = "0.0.11"\nedition = "2024"\n')
    code, out = bump()
    assert code == 1 and "not clean" in out
    assert subprocess.run([*git, "tag"], capture_output=True, text=True, check=True).stdout == "", "a dry run tags nothing"
