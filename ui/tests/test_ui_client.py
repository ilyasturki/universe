import pytest

from conftest import wait_for
from universe_ui.universe_client import ERROR_PREFIX, SERVICE, UniverseClient, UniverseError, _json


def test_json_tolerance():
    assert _json("", []) == []
    assert _json(None, {}) == {}
    assert _json("not json", {"d": 1}) == {"d": 1}
    assert _json('{"a": [1]}', None) == {"a": [1]}
    assert _json(42, None) == 42


def test_error_and_reply_unpacking(app):
    from PySide6.QtDBus import QDBusMessage

    with pytest.raises(UniverseError) as info:
        UniverseClient._unpack(QDBusMessage.createError(ERROR_PREFIX + "NotFound", "no such game"))
    assert info.value.kind == "NotFound"
    assert info.value.message == "no such game"

    with pytest.raises(UniverseError) as info:
        UniverseClient._unpack(QDBusMessage.createError("org.freedesktop.DBus.Error.ServiceUnknown", ""))
    assert info.value.kind == "org.freedesktop.DBus.Error.ServiceUnknown"

    with pytest.raises(UniverseError) as info:
        UniverseClient._unpack(QDBusMessage.createError("org.freedesktop.DBus.Error.Failed",
                                                        ERROR_PREFIX + "Busy: busy: a game is running"))
    assert (info.value.kind, info.value.message) == ("Busy", "busy: a game is running")

    call = QDBusMessage.createMethodCall("a.b", "/a", "a.b.I", "M")
    assert UniverseClient._unpack(call.createReply('{"x": 1}')) == '{"x": 1}'
    assert UniverseClient._unpack(call.createReply()) is None


def test_bus_subscriptions_bind(app):
    from PySide6.QtDBus import QDBusConnection

    bus = QDBusConnection.sessionBus()
    if not bus.isConnected() or not bus.interface().isServiceRegistered(SERVICE):
        pytest.skip("universed is not on the session bus (a method call would activate it)")
    client = UniverseClient()
    assert client.connected and all(client.connected.values()), client.connected


def test_list_resolves_defaults(fake):
    rows = fake.list()
    assert [r["id"] for r in rows][:2] == ["the-technomancer", "mini-metro"]
    control = fake.game("control")
    assert control["launch"]["proton"] == "proton-ge"
    assert control["modules"]["capture"]["enabled"] is True
    assert control["stats"]["play_count"] == 8
    assert fake.game("mirrors-edge")["stats"]["last_played"] is None


def test_errors_are_signalled_not_raised(fake):
    seen = []
    fake.error.connect(lambda kind, message: seen.append((kind, message)))
    assert fake.game("nope") == {}
    assert seen and seen[0][0] == "NotFound"
    assert fake.setSetting("capture", "", "codec", "mpeg2") is False
    assert seen[-1][0] == "Invalid"
    assert fake.setSetting("capture", "", "codec", "hevc") is True
    assert fake.getSettings("capture", "")["codec"] == "hevc"


def test_settings_merges_game_scope(fake):
    settings = fake.settings("the-technomancer")
    assert set(settings) == {"capture", "journal"}
    assert settings["journal"]["language"] == "fr"
    assert fake.settings("control")["capture"]["cursor"] is False
    assert fake.set("control", "tags", "a, b") is True
    assert fake.game("control")["tags"] == ["a", "b"]


def test_launch_runs_a_session(fake):
    started, launched, ended = [], [], []
    fake.sessionStarted.connect(lambda sid, ident: started.append(ident))
    fake.launched.connect(lambda sid, ident: launched.append(ident))
    assert fake.currentSession is None
    fake.launch("control", "DP-1")
    assert launched == ["control"]
    assert fake.currentSession["id"] == "control"
    assert fake.currentSession["screen"] == "DP-1"

    busy = []
    fake.launchFailed.connect(lambda ident, message: busy.append(message))
    fake.launch("mini-metro", "DP-1")
    assert busy and "running" in busy[0]

    args = wait_for(fake.sessionEnded, 6000)
    assert args is not None and args[1] == "control" and args[2] >= 1
    assert started == ["control"]
    assert fake.currentSession is None
    assert fake.game("control")["stats"]["play_count"] == 9


def test_install_job_reports_progress(fake):
    steps = []
    fake.progress.connect(lambda job, done, total, message: steps.append((done, total)))
    job = fake.install("gog", "1207658930")
    assert job.startswith("job-")
    args = wait_for(fake.jobFinished, 10000)
    assert args is not None and args[0] == job and args[1] is True
    assert steps and steps[-1] == (20, 20)
    assert next(g for g in fake.sourceLibrary("gog") if g["id"] == "1207658930")["installed"] is True
