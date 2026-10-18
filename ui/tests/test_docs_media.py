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


def recorded(root, version, release=()):
    module = tool()
    takes, errors = module.load_takes(module.Site(root))
    assert not errors
    entries = {}
    if release:
        folder = root / "target/docs-media/release"
        folder.mkdir(parents=True, exist_ok=True)
        chosen = {t.name: t for t in takes if t.name in release}
        for take in chosen.values():
            for file in take.release_files:
                (folder / file).write_bytes(b"media")
        manifest = {"version": version, "takes": {n: {"hash": t.hash, "files": t.release_files} for n, t in chosen.items()}}
        (folder / "manifest.json").write_text(json.dumps(manifest))
    for take in (t for t in takes if not t.release):
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
gap = 250
alt = "x"

[[take]]
name = "faked-launch"
look = "reprise"
size = "1280x800"
kind = "gamescope-clip"
keys = "Expect:dead-cells Return Mark:a"
segments = ["a..b"]
gap = 0
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
    assert "take launches: gap is a clip's" in out, "the stills of a look share one take's gap"
    assert "take faked-launch: gap is the ms between keys" in out


def test_check_holds_pages_to_the_takes_and_strict_to_a_recording_of_them_at_that_version(tmp_path):
    site(tmp_path, page='<Shot name="library" />\n<Clip name="browse" caption="x" />\n')
    assert docs_media(tmp_path, "check")[0] == 0, "nothing recorded yet is no error"
    code, out = docs_media(tmp_path, "check", "--strict", "0.0.12")
    assert code == 1 and "nothing recorded yet" in out
    recorded(tmp_path, "0.0.12")
    assert docs_media(tmp_path, "check", "--strict", "0.0.12")[0] == 0
    site(tmp_path, TAKES.replace('kind = "clip"', 'kind = "clip"\ngap = 400'))
    assert docs_media(tmp_path, "check", "--strict", "0.0.12")[0] == 0, "the default gap spelled out is the same take"
    site(tmp_path, TAKES.replace('kind = "clip"', 'kind = "clip"\ngap = 250'))
    code, out = docs_media(tmp_path, "check", "--strict", "0.0.12")
    assert code == 1 and "take browse: changed since it was recorded" in out, "a faster take is filmed again"
    site(tmp_path)
    code, out = docs_media(tmp_path, "check", "--strict", "0.0.13")
    assert code == 1 and "take library: recorded at 0.0.12, not 0.0.13" in out
    site(tmp_path, TAKES.replace("E Wait:5", "E Wait:6"), page='<Shot name="library" />\n<Shot name="browse" />\n<Clip name="ghost" />\n')
    code, out = docs_media(tmp_path, "check", "--strict", "0.0.12")
    assert code == 1
    assert "take library: changed since it was recorded" in out
    assert '<Shot name="browse"> names a clip' in out
    assert '<Clip name="ghost"> is no take' in out


RELEASE_TAKE = """
[[take]]
name = "reconnect"
look = "reprise"
size = "1920x1080"
kind = "clip"
keys = "Mark:a Guide Wait:3 Mark:b"
segments = ["a..b"]
alt = "Reconnecting a pad from the dock"
release = "0.0.12"
"""


def changelog(version, *images):
    lines = "".join(f"- An entry.\n  ![x](https://github.com/ilyasturki/universe/releases/download/v{v}/{f})\n" for v, f in images)
    return f"# Changelog\n\n## [{version}] - 2026-10-10\n\n### Added\n\n- A fix.\n{lines}\n[{version}]: https://github.com/ilyasturki/universe/compare/v0.0.11...v{version}\n"


def test_the_changelog_s_images_are_takes_of_their_release_held_to_their_assets(tmp_path):
    site(
        tmp_path,
        TAKES + RELEASE_TAKE + RELEASE_TAKE.replace('"reconnect"', '"older"').replace('"0.0.12"', '"0.0.11"'),
        page='<Shot name="library" />\n<Clip name="browse" />\n',
    )
    (tmp_path / "CHANGELOG.md").write_text(changelog("0.0.12", ("0.0.12", "reconnect.avif"), ("0.0.12", "library.webp"), ("0.0.12", "older.webp")))
    recorded(tmp_path, "0.0.12")
    code, out = docs_media(tmp_path, "check", "--strict", "0.0.12")
    assert code == 1
    assert "image reconnect: its release assets are not recorded at 0.0.12" in out
    assert "image older: no take older (its release is 0.0.11)" in out, "an older release's take is no longer recorded"
    assert "no page shows it" not in out, "a release take is shown by the notes, not a page"

    (tmp_path / "CHANGELOG.md").write_text(changelog("0.0.12", ("0.0.12", "reconnect.avif"), ("0.0.12", "library.webp")))
    recorded(tmp_path, "0.0.12", release=("reconnect", "library"))
    code, out = docs_media(tmp_path, "check", "--strict", "0.0.12")
    assert code == 0, out
    (tmp_path / "target/docs-media/release/reconnect.mp4").unlink()
    code, out = docs_media(tmp_path, "check", "--strict", "0.0.12")
    assert code == 1 and "no target/docs-media/release/reconnect.mp4" in out, "a clip plays its MP4 in the apps"
    (tmp_path / "CHANGELOG.md").write_text(changelog("0.0.12", ("0.0.12", "reconnect.webp")))
    code, out = docs_media(tmp_path, "check", "--strict", "0.0.12")
    assert code == 1 and "a still names the clip reconnect" in out

    notes = tmp_path / "notes.md"
    notes.write_text(changelog("0.0.12", ("0.0.12", "reconnect.avif"), ("0.0.12", "library.webp")))
    cl = lambda *a: subprocess.run([sys.executable, str(REPO / "tools/changelog"), "--file", str(notes), *a], capture_output=True, text=True, check=False)
    assert cl("media", "0.0.12").stdout.split() == ["reconnect.avif", "reconnect.mp4", "library.webp"]
    assert "![" not in cl("metainfo", "0.0.12").stdout, "AppStream holds no image"
    assert "releases/download/v0.0.12/reconnect.avif" in cl("notes", "0.0.12").stdout, "the GitHub release shows them"
    notes.write_text(changelog("0.0.12", ("0.0.11", "older.webp")))
    done = cl("check", "0.0.12")
    assert done.returncode == 1 and "v0.0.11/older.webp: an image is" in done.stderr, "an image copied from the last release's entry"


def test_publish_leaves_the_media_branch_one_commit_that_fetch_lands_in_the_site(tmp_path):
    git = ["git", "-c", "user.name=t", "-c", "user.email=t@t", "-c", "init.defaultBranch=main"]
    subprocess.run([*git, "init", "-q", str(tmp_path)], check=True)
    subprocess.run([*git, "-C", str(tmp_path), "commit", "-q", "--allow-empty", "-m", "main"], check=True)
    main = subprocess.run(["git", "-C", str(tmp_path), "rev-parse", "main"], capture_output=True, text=True, check=True).stdout
    site(tmp_path, page='<Shot name="library" />\n')
    env = {"GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t", "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@t"}
    (tmp_path / "CHANGELOG.md").write_text(changelog("0.0.13", ("0.0.13", "library.webp")))
    for version in ("0.0.12", "0.0.13"):
        recorded(tmp_path, version, release=("library",) if version == "0.0.13" else ())
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
    listed = subprocess.run(["git", "-C", str(tmp_path), "ls-tree", "-r", "--name-only", "throwaway", "release"], capture_output=True, text=True, check=True)
    assert listed.stdout.split() == ["release/library.webp", "release/manifest.json"], "the release workflow uploads release/ from it"
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
    subprocess.run(["git", "-C", str(tmp_path), "checkout", "-q", "--", "Cargo.toml"], check=True)
    (tmp_path / "CHANGELOG.md").write_text(changelog("0.0.12", ("0.0.12", "library.webp")))
    code, out = bump()
    assert code == 1 and "release's images are not recorded" in out, out
    code, out = bump(BUMP_SKIP_MEDIA="1")
    assert code == 1 and "has images" in out, "a hotfix ships no images it never recorded"
    recorded(tmp_path, "0.0.12", release=("library",))
    assert bump()[0] == 0
