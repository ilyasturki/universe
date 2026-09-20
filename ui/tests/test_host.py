import os
import sys

import pytest

from universe_ui import host


@pytest.fixture
def execv(monkeypatch):
    calls = []
    monkeypatch.setattr(os, "execv", lambda program, argv: calls.append((program, argv)))
    monkeypatch.delenv("LD_LIBRARY_PATH", raising=False)
    return calls


class Gamescope:
    def hostGamescope(self, screen):
        return ["/bin/gamescope", "-f"]


def test_re_execs_the_launcher_itself(monkeypatch, execv, tmp_path):
    launcher = tmp_path / "universe-ui"
    launcher.write_text("#!/bin/sh\n")
    launcher.chmod(0o755)
    monkeypatch.setattr(sys, "argv", [str(launcher), "--theme", "switch2"])

    host.exec_in_gamescope(Gamescope(), ["--theme", "switch2"])
    assert execv == [("/bin/gamescope", ["/bin/gamescope", "-f", "--", str(launcher), "--theme", "switch2"])], (
        "a wrapper binary is not a script the interpreter can read"
    )


def test_falls_back_to_the_interpreter_when_argv0_is_not_executable(monkeypatch, execv, tmp_path):
    script = tmp_path / "host.py"
    script.write_text("")
    monkeypatch.setattr(sys, "argv", [str(script)])

    host.exec_in_gamescope(Gamescope(), [])
    assert execv[0][1] == ["/bin/gamescope", "-f", "--", sys.executable, str(script)]


def test_the_library_path_is_put_back_past_the_wrapper(monkeypatch, execv, tmp_path):
    launcher = tmp_path / "universe-ui"
    launcher.write_text("#!/bin/sh\n")
    launcher.chmod(0o755)
    monkeypatch.setattr(sys, "argv", [str(launcher)])
    monkeypatch.setenv("LD_LIBRARY_PATH", "/nix/store/pipewire/lib")
    monkeypatch.setattr(host.shutil, "which", lambda name: {"env": "/usr/bin/env"}.get(name, name))

    host.exec_in_gamescope(Gamescope(), [])
    assert execv[0][1] == ["/bin/gamescope", "-f", "--", "/usr/bin/env", "LD_LIBRARY_PATH=/nix/store/pipewire/lib", str(launcher)], (
        "the CAP_SYS_NICE wrapper's loader drops it; the launcher inside gamescope needs it for pipewire"
    )


def test_without_gamescope_it_stays_on_the_desktop(monkeypatch, execv):
    class NoGamescope(Gamescope):
        def hostGamescope(self, screen):
            return None

    host.exec_in_gamescope(NoGamescope(), [])
    assert execv == []
