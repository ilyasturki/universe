import json
import os
import shutil
import subprocess
import time
from pathlib import Path

import pytest

ROOT = Path(__file__).parent.parent


def cargo_build():
    run = subprocess.run(["cargo", "build", "--message-format=json"], cwd=ROOT, check=True, capture_output=True, text=True)
    messages = map(json.loads, run.stdout.splitlines())
    return [m["target"]["name"] for m in messages if m["reason"] == "compiler-artifact" and not m["fresh"]]


@pytest.mark.slow
def test_a_second_cargo_build_compiles_nothing():
    cargo_build()
    assert cargo_build() == []


def test_the_core_build_script_watches_real_paths_and_leaves_the_index_alone(tmp_path):
    repo = tmp_path / "repo"
    manifest = repo / "crates/universe"
    manifest.mkdir(parents=True)
    tracked = repo / "tracked"
    tracked.write_text("same bytes")

    def git(*args):
        subprocess.run(["git", "-C", repo, "-c", "user.name=t", "-c", "user.email=t@t", *args], check=True, capture_output=True)

    git("init", "-q")
    git("add", ".")
    git("commit", "-qm", "first")
    git("pack-refs", "--all")
    os.utime(tracked, (1, 1))
    index = repo / ".git/index"
    before = index.stat().st_mtime_ns

    script = tmp_path / "build-script"
    subprocess.run(
        ["rustc", "--edition=2021", ROOT / "crates/universe/build.rs", "-o", script], env={**os.environ, "CARGO_MANIFEST_DIR": str(manifest)}, check=True
    )
    env = {k: v for k, v in os.environ.items() if k != "UNIVERSE_GIT_REV"} | {"CARGO_PKG_VERSION": "0.0.0"}
    run = subprocess.run([script], env=env, check=True, capture_output=True, text=True)

    assert "cargo:rustc-env=UNIVERSE_BUILD=0.0.0 (" in run.stdout, "the script read the rev, and the tree as clean"
    assert "-dirty" not in run.stdout
    assert index.stat().st_mtime_ns == before
    watched = [manifest / line.removeprefix("cargo:rerun-if-changed=") for line in run.stdout.splitlines() if line.startswith("cargo:rerun-if-changed=")]
    assert index.resolve() in [path.resolve() for path in watched if path.exists()]
    assert [path for path in watched if not path.exists()] == [], "cargo counts a missing path as changed on every build"


def test_just_develop_installs_again_for_a_source_file_only(tmp_path):
    source = tmp_path / "crates/universe-py/src/lib.rs"
    tests = tmp_path / "crates/universe-py/tests"
    tests.mkdir(parents=True)
    source.parent.mkdir()
    (tmp_path / "ui").mkdir()
    for name in (source, "Cargo.toml", "Cargo.lock", "flake.lock", "ui/pyproject.toml"):
        (tmp_path / name).touch()
    shutil.copy(ROOT / "justfile", tmp_path)

    venv = tmp_path / ".venv"
    (venv / "bin").mkdir(parents=True)
    (venv / "pyvenv.cfg").write_text(f"home = {Path(shutil.which('python3')).parent}\n")
    shims = tmp_path / "shims"
    shims.mkdir()
    for path, body in ((shims / "nix", 'shift 3\nexec "$@"'), (shims / "maturin", 'echo >> "$VIRTUAL_ENV/installs"'), (venv / "bin/pip", "")):
        path.write_text(f"#!/bin/sh\n{body}\n")
        path.chmod(0o755)

    def installs_after_develop():
        env = {**os.environ, "PATH": f"{shims}:{os.environ['PATH']}"}
        subprocess.run(["just", "--justfile", tmp_path / "justfile", "develop"], env=env, check=True, capture_output=True)
        return len((venv / "installs").read_text().splitlines())

    later = time.time() + 10
    assert installs_after_develop() == 1
    os.utime(tests, (later, later))
    assert installs_after_develop() == 1, "pytest's __pycache__ bumps the directory, no source changed"
    os.utime(source, (later, later))
    assert installs_after_develop() == 2
