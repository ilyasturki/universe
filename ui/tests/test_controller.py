from pathlib import Path

import pytest

from conftest import pump, rows_by_key, wait_for
from universe_ui.screens.controller import FakeWatcher, Watcher


# The pad's rows: the Timing card and the Advanced row sit behind them on every state.
def pad_rows(screen):
    return [r for r in screen.rows if not r["advanced"] and r["key"] != "advanced"]


@pytest.fixture
def started(api):
    screen = api.screens.controller
    screen.restart_ms = 0
    watcher = FakeWatcher("dualsense-edge")
    screen.start(watcher)
    return screen, watcher


def test_rows_follow_the_watcher_and_the_macros(api, fake):
    screen = api.screens.controller
    assert not screen.connected and screen.family == "xbox"
    assert [r["type"] for r in pad_rows(screen)] == ["info"] and len(screen.groups) == 2, "the pad card, the Advanced row"
    timing = {r["key"]: r for r in screen.rows if r["advanced"]}
    assert timing["controller.hold_ms"]["value"] == "600" and timing["controller.volume_step"]["value"] == "2" and timing["controller.hold_ms"]["type"] == "int"
    assert screen.rows[-1]["key"] == "advanced" and screen.groups[-1]["rows"] == [len(screen.rows) - 1]
    assert not screen.showAdvanced and [g["title"] for g in screen.groups] == ["Controller", ""]
    screen.showAdvanced = True
    assert [g["title"] for g in screen.groups] == ["Controller", "", "Timing"] and screen.groups[-1]["advanced"] is True
    assert screen.setValue(screen.reveal("controller.hold_ms", ""), "800") is True
    assert fake.config()["controller"]["hold_ms"] == 800 and {r["key"]: r for r in screen.rows}["controller.hold_ms"]["value"] == "800"
    screen.showAdvanced = False

    watcher = FakeWatcher("dualsense-edge")
    assert screen.start(watcher) is True
    assert screen.connected and screen.family == "dualsense-edge" and screen.status == "ready"
    assert api.memory.get("controllerFamily") == "dualsense-edge"
    rows = rows_by_key(screen)
    assert "device" not in rows, "one pad needs no picker row"
    assert rows["paddle_left"]["display"] == "Press · Volume down"
    assert rows["fn_left"]["display"] == "Press · Screenshot"
    assert rows["fn_left"]["press"]["label"] == "Screenshot" and rows["fn_left"]["hold"] is None, "the chip prints the macro's label"
    assert rows["south"]["display"] == "—" and rows["south"]["label"] == "Cross"
    assert rows["paddle_left"]["extra"] is True and rows["paddle_left"]["bound"] is True
    assert rows["paddle_left"]["label"] == "Left back button (LB)" and rows["paddle_left"]["family"] == "dualsense-edge"
    assert [r["key"] for r in screen.rows][:4] == ["test", "walk", "fn_left", "fn_right"], (
        "the live view, the walk, then the extras: they are what the page is for"
    )
    assert rows["test"]["type"] == "action" and rows["test"]["action"] == "Start" and "slot" not in rows["test"]
    assert rows["walk"]["label"] == "Set up the buttons" and "slot" not in rows["walk"]
    assert pad_rows(screen)[-1]["key"] == "dpad_right"
    group = screen.groups[0]
    assert group["title"] == "DualSense Edge" and group["meta"] == "Bluetooth · 85% · 4 extra buttons"
    assert group["rows"] == list(range(len(pad_rows(screen))))

    presses = []
    screen.buttonPressed.connect(lambda ident, slot, pressed: presses.append((ident, slot, pressed)))
    watcher.emit({"event": "button", "id": "event30", "slot": "paddle_left", "code": "BTN_TRIGGER_HAPPY3", "pressed": True})
    watcher.emit({"event": "button", "id": "event30", "slot": "paddle_left", "code": "BTN_TRIGGER_HAPPY3", "pressed": False})
    assert presses == [("event30", "paddle_left", True), ("event30", "paddle_left", False)]
    unknown = []
    screen.unknownPressed.connect(lambda ident, code: unknown.append(code))
    watcher.emit({"event": "unknown", "id": "event30", "code": "BTN_TRIGGER_HAPPY9"})
    assert unknown == ["BTN_TRIGGER_HAPPY9"]
    axes = []
    screen.axisMoved.connect(lambda ident, axis, value: axes.append((ident, axis, value)))
    watcher.axis("lx", -0.5)
    watcher.emit({"event": "axis", "id": "event30", "axis": "wheel", "value": 1})
    assert axes == [("event30", "lx", -0.5)], "only the six named axes reach the page"


def test_the_watchers_reading_reaches_the_sdl_mapper(started):
    screen, watcher = started
    mappings = []
    screen.mapping.connect(lambda vendor, product, fields: mappings.append((vendor, product, fields)))
    line = watcher.device("event31", family="8bitdo-pro-3")
    line.update(
        vendor=0x2DC8, product=0x6009, sdl={"south": "b1", "east": "b0", "lt": "b8"}, sdl_axes={"lx": "a0", "lt": "a5"}, axes={"lx": "ABS_X", "lt": "ABS_BRAKE"}
    )
    watcher.emit(line)
    assert mappings == [(0x2DC8, 0x6009, "a:b0,b:b1,lefttrigger:a5,leftx:a0")], "the A of a Nintendo-style pad is SDL's A, a trigger its pull"
    assert screen.devices[1]["axes"] == {"lx": "ABS_X", "lt": "ABS_BRAKE"}
    watcher.emit(watcher.device("event32", family="xbox"))
    assert len(mappings) == 1, "a line without SDL numbers (a fake, an old core) maps nothing"


def test_a_pads_own_charge_reading_shows_as_its_battery(started, api):
    screen, watcher = started
    line = watcher.device("event31", family="8bitdo-pro-3")
    line["battery"] = {"percent": 80, "charging": False}
    watcher.emit(line)
    assert api.power.forInput("event31") == {"name": "8BitDo Pro 3", "kind": "pad", "percent": 80, "charging": False, "inputs": ["event31"]}
    screen.setCurrent("event31")
    assert "80%" in screen.groups[0]["meta"]
    watcher.emit({"event": "battery", "id": "event31", "percent": 79, "charging": True})
    assert screen.devices[1]["battery"] == {"percent": 79, "charging": True}
    assert "79%, charging" in screen.groups[0]["meta"]
    watcher.emit({"event": "gone", "id": "event31"})
    assert api.power.forInput("event31") is None


def test_the_walk_learns_each_button_then_the_sticks(api, fake):
    screen = api.screens.controller
    offers, messages = [], []
    screen.walkOffered.connect(lambda family, name: offers.append((family, name)))
    screen.message.connect(messages.append)
    watcher = FakeWatcher("8bitdo-pro-3")
    screen.start(watcher)
    assert offers == [("8bitdo-pro-3", "8BitDo Pro 3")], "a family never set up is offered the walk once it connects"
    watcher.emit(watcher.device())
    assert len(offers) == 1, "once"
    screen.declineWalk("8bitdo-pro-3")
    assert api.memory.get("controllerWalks") == {"8bitdo-pro-3": "declined"}

    line = watcher.device()
    line["axes"] = {"lx": "ABS_X", "ly": "ABS_Y", "rx": "ABS_Z", "ry": "ABS_RZ", "lt": "ABS_BRAKE", "rt": "ABS_GAS"}
    watcher.emit(line)
    assert screen.startWalk() is True
    assert screen.walking and screen.learning == "south"
    step = screen.walkStep
    assert (step["slot"], step["label"], step["prompt"], step["index"], step["seconds"]) == ("south", "B", "Press B", 1, 8)
    assert step["count"] == 17 + 5 + 4, "the standard slots, the Pro 3's extras, the four throws"
    assert watcher.commands[-1] == {"cmd": "learn", "id": "event30", "slot": "south"}
    learned = []
    screen.learned.connect(lambda family, slot, code: learned.append(slot))
    watcher.emit({"event": "learned", "family": "8bitdo-pro-3", "slot": "south", "code": "BTN_EAST", "from": None})
    assert screen.walkStep["slot"] == "east" and watcher.commands[-1]["slot"] == "east" and learned == [], "a step answered moves on quietly"
    watcher.emit({"event": "learned", "family": "8bitdo-pro-3", "slot": "north", "code": "BTN_NORTH", "from": None})
    assert screen.walkStep["slot"] == "east", "a stale answer is not this step's"
    watcher.emit({"event": "learned", "family": "8bitdo-pro-3", "slot": "east", "code": "BTN_EAST", "from": "south"})
    assert messages[-1] == "That button was B: it is A now" and screen.walkStep["slot"] == "west"
    screen.skipStep()
    assert watcher.commands[-2] == {"cmd": "cancel"} and screen.walkStep["slot"] == "north"
    for _ in range(5):
        screen._walk_tick()
    assert screen.walkStep["seconds"] == 3 and screen.walkStep["slot"] == "north"
    for _ in range(3):
        screen._walk_tick()
    assert screen.walkStep["slot"] == "lb", "eight seconds unanswered skip the step"
    assert screen.walkStep["prompt"] == "Press L1" and screen.walkStep["index"] == 5
    watcher.emit({"event": "learn_timeout"})
    assert screen.walkStep["slot"] == "rb" and messages[-1] != "No button pressed: learning stopped"
    while screen.walkStep["slot"]:
        watcher.emit({"event": "learned", "family": "8bitdo-pro-3", "slot": screen.walkStep["slot"], "code": "BTN_X", "from": None})
    assert (screen.walkStep["axis"], screen.walkStep["prompt"], screen.learning) == ("lx", "Push the left stick right", "ls")
    assert watcher.commands[-1] == {"cmd": "learn", "id": "event30", "axis": "lx"}
    watcher.emit({"event": "learned", "family": "8bitdo-pro-3", "axis": "lx", "code": "ABS_X-"})
    assert screen.devices[0]["axes"]["lx"] == "ABS_X-" and screen.walkStep["axis"] == "ly"
    for axis in ("ly", "rx", "ry"):
        watcher.emit({"event": "learned", "family": "8bitdo-pro-3", "axis": axis, "code": "ABS_Y"})
    assert not screen.walking and screen.learning == "" and screen.walkStep == {}
    assert messages[-1] == "8BitDo Pro 3: 22 set up, 4 skipped: B, Y, X, L1"
    assert api.memory.get("controllerWalks") == {"8bitdo-pro-3": "done"}

    assert screen.startWalk() is True
    screen.cancelWalk()
    assert not screen.walking and watcher.commands[-1] == {"cmd": "cancel"} and messages[-1] == "Setup stopped"
    assert screen.startWalk() is True
    watcher.emit({"event": "gone", "id": "event30"})
    assert not screen.walking and messages[-1] == "Controller gone: setup stopped"


def test_bind_unbind_and_learn(started, fake):
    screen, watcher = started

    assert screen.bind("paddle_left", "hold", "stop", "", "") is True
    assert rows_by_key(screen)["paddle_left"]["display"] == "Press · Volume down / Hold · Stop the game"
    assert rows_by_key(screen)["paddle_left"]["hold"]["label"] == "Stop the game"
    assert watcher.commands[-1] == {"cmd": "reload"}, "the watcher rereads the config after a write"
    macros = {(m["button"], m["trigger"]): m for m in fake.controllerState()["macros"]}
    assert macros[("paddle_left", "hold")]["action"] == "stop" and macros[("paddle_left", "hold")]["family"] == "dualsense-edge"

    messages = []
    screen.message.connect(messages.append)
    assert screen.bind("south", "press", "stop", "", "") is False, "stop is hold-only"
    assert messages == ["Stop the game only fires on a hold"]
    assert screen.bind("south", "press", "nope", "", "") is False
    assert screen.bind("south", "tap", "screenshot", "", "") is False

    assert screen.unbind("paddle_left", "press") is True
    assert rows_by_key(screen)["paddle_left"]["display"] == "Hold · Stop the game"
    assert screen.unbind("paddle_left", "") is True
    assert rows_by_key(screen)["paddle_left"]["display"] == "—"

    assert screen.bind("fn_right", "press", "keys", "Ctrl+F1", "") is True
    assert rows_by_key(screen)["fn_right"]["display"] == "Press · Ctrl+F1"
    assert screen.bind("fn_right", "hold", "command", "", "notify-send hi") is True
    assert rows_by_key(screen)["fn_right"]["display"] == "Press · Ctrl+F1 / Hold · Command"

    device = watcher.device()
    device["slots"]["paddle_right"] = {"code": None, "bound": False}
    watcher.emit(device)
    row = rows_by_key(screen)["paddle_right"]
    assert row["display"] == "Unbound" and row["bound"] is False
    assert screen.unboundSlots == ["paddle_right"]

    assert screen.learn("paddle_right") is True
    assert screen.learning == "paddle_right"
    assert watcher.commands[-1] == {"cmd": "learn", "id": "event30", "slot": "paddle_right"}
    learned = []
    screen.learned.connect(lambda family, slot, code: learned.append((family, slot, code)))
    watcher.emit({"event": "learned", "family": "dualsense-edge", "slot": "paddle_right", "code": "BTN_TRIGGER_HAPPY9", "from": "paddle_left"})
    assert learned == [("dualsense-edge", "paddle_right", "BTN_TRIGGER_HAPPY9")]
    assert screen.learning == ""
    rows = rows_by_key(screen)
    assert rows["paddle_right"]["bound"] is True and rows["paddle_right"]["code"] == "BTN_TRIGGER_HAPPY9"
    assert rows["paddle_left"]["bound"] is False, "the previous owner of the code lost it"

    screen.learn("fn_left")
    screen.cancelLearn()
    assert screen.learning == "" and watcher.commands[-1] == {"cmd": "cancel"}
    screen.suspend()
    assert watcher.commands[-1] == {"cmd": "suspend", "dock": False}
    screen.learn("fn_left")
    screen.resume()
    assert watcher.commands[-2:] == [{"cmd": "cancel"}, {"cmd": "resume"}], "leaving the section drops a pending learn"


def test_two_pads_and_hotplug(started, fake):
    screen, watcher = started
    watcher.emit(watcher.device("event40", "xbox-elite", "usb"))
    rows = rows_by_key(screen)
    assert screen.current == "event30" and screen.family == "dualsense-edge"
    assert rows["device"]["type"] == "enum" and rows["device"]["choices"] == ["DualSense Edge", "Xbox Elite Series 2"]
    assert rows["device"]["value"] == "DualSense Edge" and screen.groups[0]["rows"][0] == 0
    assert [r["key"] for r in screen.rows][:2] == ["device", "test"]
    assert len(screen.groups) == 2, "one card and the Advanced row, the art takes the other column"

    assert screen.setValue(0, "Xbox Elite Series 2") is True
    assert screen.current == "event40" and screen.family == "xbox-elite"
    rows = rows_by_key(screen)
    assert rows["paddle_p1"]["display"] == "Press · Toggle MangoHud" and rows["south"]["label"] == "A"
    assert screen.groups[0]["meta"] == "USB · 4 extra buttons"
    assert screen.setValue(0, "Nope") is False

    watcher.emit({"event": "gone", "id": "event40"})
    assert screen.current == "event30" and screen.family == "dualsense-edge"
    assert "device" not in rows_by_key(screen)
    watcher.emit({"event": "gone", "id": "event30"})
    assert not screen.connected and screen.family == "dualsense-edge", "the art keeps the last pad"
    assert [r["type"] for r in pad_rows(screen)] == ["info"]
    assert screen.bind("south", "press", "screenshot", "", "") is False


def test_learn_timeout_and_error_clear_learning(started, fake):
    screen, watcher = started
    messages = []
    screen.message.connect(messages.append)
    assert screen.learn("paddle_left") is True and screen.learning == "paddle_left"
    watcher.emit({"event": "learn_timeout"})
    assert screen.learning == "" and messages == ["No button pressed: learning stopped"]
    assert screen.learn("paddle_left") is True
    watcher.emit({"event": "error", "message": "cannot learn paddle_left on event30"})
    assert screen.learning == "" and messages[-1] == "cannot learn paddle_left on event30"
    watcher.emit({"event": "learn_timeout"})
    assert len(messages) == 2, "a timeout with nothing to stop says nothing"


def test_the_hud_event_gets_a_toast_the_macros_do_not(started, fake):
    screen, watcher = started
    notices = []
    screen.macroNotice.connect(notices.append)
    fire = lambda action: watcher.emit({"event": "macro", "id": "event30", "slot": "fn_right", "trigger": "press", "action": action})
    fire("mangohud")
    fire("volume_up")
    fire("screenshot")
    assert notices == [], "the fire says nothing: the HUD event after it does"
    watcher.emit({"event": "hud", "shown": None, "title": ""})
    assert notices == ["MangoHud: no game running"]
    watcher.emit({"event": "hud", "shown": True, "title": "Control"})
    watcher.emit({"event": "hud", "shown": False, "title": "Control"})
    assert notices[1:] == ["MangoHud shown · Control", "MangoHud hidden · Control"]


def test_waiting_lists_the_cores_pads_passively(api, fake):
    screen = api.screens.controller
    watcher = FakeWatcher("none")
    screen.start(watcher)
    assert not screen.connected and not screen.passive
    watcher.emit({"event": "waiting"})
    assert screen.status == "waiting" and screen.passive and screen.connected
    assert screen.current == "event30" and screen.family == "dualsense-edge"
    first = screen.rows[0]
    assert first["type"] == "info" and first["label"] == "Macros are running in the game session"
    assert "device" not in rows_by_key(screen) and "test" not in rows_by_key(screen), "no live view without the pads"
    assert screen.setTesting(True) is False and not screen.testing
    assert rows_by_key(screen)["paddle_left"]["display"] == "Press · Volume down"
    messages = []
    screen.message.connect(messages.append)
    assert screen.learn("paddle_left") is False and screen.learning == ""
    assert messages == ["Live presses and learning resume when it ends"]
    assert screen.bind("paddle_left", "hold", "stop", "", "") is True, "binding writes the config the holder rereads"
    stop = next(p["label"] for p in screen.presets if p["id"] == "stop")
    assert rows_by_key(screen)["paddle_left"]["display"] == "Press · Volume down / Hold · " + stop
    assert watcher.commands[-1] == {"cmd": "reload"}
    watcher.emit({"event": "ready"})
    assert screen.status == "ready" and not screen.passive and not screen.connected
    assert [r["type"] for r in pad_rows(screen)] == ["info"]
    watcher.emit(watcher.device("event31", "dualsense-edge"))
    assert screen.connected and screen.rows[0]["type"] != "info"
    assert screen.learn("paddle_left") is True


def test_watcher_restarts_after_it_dies(started, fake):
    screen, watcher = started
    assert screen.learn("paddle_left") is True
    watcher.exit(3)
    assert screen.status == "off" and not screen.connected and screen.learning == ""
    assert not watcher.started
    first = screen.rows[0]
    assert first["type"] == "info" and first["label"] == "Controller macros stopped"
    assert screen.learn("paddle_left") is False
    pump(50)
    assert watcher.started and screen.status == "ready" and screen.connected
    screen.shutdown()
    assert not watcher.started
    watcher.exit(1)
    assert screen.status == "off" and [r["type"] for r in pad_rows(screen)] == ["info"]
    pump(50)
    assert not watcher.started, "no restart after shutdown"


def test_reconnect_returns_to_the_shown_pad(started, fake):
    screen, watcher = started
    watcher.emit(watcher.device("event40", "xbox-elite", "usb"))
    assert screen.current == "event30"
    watcher.emit({"event": "gone", "id": "event30"})
    assert screen.current == "event40"
    watcher.emit(watcher.device("event31", "dualsense-edge"))
    assert screen.current == "event31" and screen.family == "dualsense-edge"
    assert screen.setValue(0, "Xbox Elite Series 2") is True and screen.current == "event40"
    watcher.emit({"event": "gone", "id": "event31"})
    watcher.emit(watcher.device("event32", "dualsense-edge"))
    assert screen.current == "event40", "a pad picked by hand keeps the page"


def test_testing_streams_axes_and_ends_with_the_pad(api, fake):
    screen = api.screens.controller
    assert screen.setTesting(True) is False, "nothing to test before a pad"
    watcher = FakeWatcher("dualsense-edge")
    screen.start(watcher)
    changes = []
    screen.testingChanged.connect(lambda: changes.append(screen.testing))
    assert screen.setTesting(True) is True and screen.testing
    assert api.pad.muted, "the pad's presses stop being keys while it is on show"
    assert watcher.commands[-1] == {"cmd": "axes", "on": True}
    assert screen.setTesting(True) is True and changes == [True], "already on"
    assert screen.setTesting(False) is True and not screen.testing and not api.pad.muted
    assert watcher.commands[-1] == {"cmd": "axes", "on": False}
    assert screen.setTesting(False) is False

    screen.setTesting(True)
    screen.resume()
    assert not screen.testing and not api.pad.muted, "leaving the section ends it"
    screen.setTesting(True)
    watcher.emit(watcher.device("event40", "xbox-elite", "usb"))
    screen.setCurrent("event40")
    assert not screen.testing, "another pad shown ends it"
    screen.setTesting(True)
    watcher.emit({"event": "gone", "id": "event40"})
    assert screen.testing, "the other pad leaving does not"
    watcher.emit({"event": "gone", "id": "event30"})
    assert not screen.testing and not api.pad.muted, "the last pad gone ends it"
    watcher.emit(watcher.device())
    screen.setTesting(True)
    watcher.emit({"event": "waiting"})
    assert not screen.testing and not api.pad.muted
    watcher.emit({"event": "ready"})
    watcher.emit(watcher.device())
    screen.setTesting(True)
    watcher.exit(1)
    assert not screen.testing and not api.pad.muted and screen.status == "off"


def test_a_watcher_restart_ends_testing(started, fake):
    screen, watcher = started
    assert screen.setTesting(True) is True
    watcher.exit(3)
    assert not screen.testing
    sent = len(watcher.commands)
    pump(50)
    assert watcher.started and not screen.testing
    assert {"cmd": "axes", "on": True} not in watcher.commands[sent:], "a fresh watcher streams nothing until asked"


def test_watcher_process_round_trip(api, monkeypatch):
    monkeypatch.setenv("UNIVERSE_BIN", str(Path(__file__).parent / "fake-universe"))
    screen = api.screens.controller
    watcher = Watcher()
    echoed = []
    watcher.received.connect(lambda line: echoed.append(line) if line.get("event") == "echo" else None)
    assert screen.start(watcher) is True
    wait_for(screen.devicesChanged, 5000)
    assert screen.connected and screen.family == "xbox" and screen.status == "ready"
    assert rows_by_key(screen)["share"]["display"] == "Unbound"
    screen.suspend()
    screen.learn("share")
    wait_for(watcher.received, 5000)
    wait_for(watcher.received, 5000)
    assert [e["command"] for e in echoed] == [{"cmd": "suspend", "dock": False}, {"cmd": "learn", "id": "event9", "slot": "share"}]
    screen.shutdown()
    assert watcher._process is None


def test_watcher_without_a_binary(api, monkeypatch):
    monkeypatch.setenv("UNIVERSE_BIN", "")
    monkeypatch.setenv("PATH", "/nonexistent")
    assert api.screens.controller.start(Watcher()) is False
    assert not api.screens.controller.connected
