import json
from pathlib import Path

import pytest
from looks import invoke, read
from PySide6.QtCore import QCoreApplication, QEvent, QObject, Qt, QUrl
from PySide6.QtQml import QQmlComponent, QQmlEngine
from uitest import record, until

from universe_ui import gamepad, host

FAMILIES = ["steam-deck", "dualsense-edge", "dualsense", "dualshock4", "xbox-elite", "xbox", "switch2-pro", "switch-pro", "8bitdo-pro-3", "generic"]
RECORDED = {p.stem: json.loads(p.read_text()) for p in sorted((Path(__file__).resolve().parents[2] / "tests" / "pads").glob("*.json"))}
LAUNCHER = {
    **dict.fromkeys(("A", "Cross"), Qt.Key.Key_Return),
    **dict.fromkeys(("B", "Circle"), Qt.Key.Key_Escape),
    **dict.fromkeys(("X", "Square"), Qt.Key.Key_I),
    **dict.fromkeys(("Y", "Triangle"), Qt.Key.Key_F),
}


@pytest.fixture
def engine(api):
    engine = QQmlEngine()
    engine.rootContext().setContextProperty("api", api)
    engine.made = []
    yield engine
    # The items go before the api they bind to, or their bindings complain on the way out.
    for item in engine.made:
        item.deleteLater()
    QCoreApplication.sendPostedEvents(None, QEvent.Type.DeferredDelete)
    engine.deleteLater()
    QCoreApplication.sendPostedEvents(None, QEvent.Type.DeferredDelete)


def create(engine, name, **props):
    component = QQmlComponent(engine, QUrl.fromLocalFile(str(host.QML_DIR / "ui" / name)), engine)
    assert component.isReady(), component.errorString()
    item = component.createWithInitialProperties(props)
    assert item is not None, component.errorString()
    engine.made.append(item)
    return item


def slots_of(fake, family):
    return [s["id"] for f in fake.controllerState()["families"] if f["id"] == family for s in f["slots"]]


@pytest.fixture(scope="module")
def labels(tmp_path_factory):
    """Each family's slot labels, as the real core sends them."""
    universe_core = pytest.importorskip("universe_core")
    root = tmp_path_factory.mktemp("core")
    with pytest.MonkeyPatch.context() as env:
        for kind in ("data", "config", "state", "cache"):
            env.setenv(f"UNIVERSE_{kind.upper()}_HOME", str(root / kind))
        state = universe_core.Core().controller_state()
    return {f["id"]: {s["id"]: s["label"] for s in f["slots"]} for f in state["families"]}


@pytest.mark.parametrize("family", FAMILIES)
def test_pad_art_has_a_button_per_slot(engine, fake, family):
    art = create(engine, "PadArt.qml", family=family, width=900, height=600)
    drawn = {b["slot"] for b in read(art, "geo")["buttons"]}
    expected = set(slots_of(fake, family))
    missing = expected - drawn
    assert not missing, f"{family} draws no {sorted(missing)}"
    assert drawn <= expected | {"mute"}, f"{family} draws slots the core does not list: {sorted(drawn - expected)}"
    art.setProperty("pressed", {"south": True})
    art.setProperty("axes", {"lx": -0.5, "rt": 0.7})


def test_every_slot_has_a_glyph(engine, fake):
    for family in FAMILIES:
        for slot in [*slots_of(fake, family), "dpad"]:
            glyph = create(engine, "PadGlyph.qml", family=family, slot=slot)
            spec = glyph.property("spec").toVariant()
            assert spec["shape"], (family, slot)
            assert spec["text"] or spec["symbol"] or spec["shape"] == "dpad", (family, slot)
    marks = {f: create(engine, "PadGlyph.qml", family=f, slot="select").property("spec").toVariant()["symbol"] for f in ("dualshock4", "dualsense")}
    assert marks == {"dualshock4": "share", "dualsense": "create"}, "a DualShock 4 has Share where a DualSense has Create"


# The watcher's line for a recorded pad, each control standing for its own element: the launcher takes the button labelled A (Cross on
# a PlayStation pad) wherever it sits, reads the sticks off their axes and a trigger by its pull when it has one; its glyphs say the same.
@pytest.mark.parametrize("name", RECORDED)
def test_a_recorded_pad_drives_and_draws_the_launcher_by_its_labels(engine, labels, name):
    pad = RECORDED[name]
    face = labels[pad["family"]]
    fields = dict(f.split(":", 1) for f in gamepad.mapping_fields(pad["buttons"], pad["axes"], face).split(","))
    slot_of = {element: slot for slot, element in pad["buttons"].items()}
    for sdl, button in {"a": gamepad.BTN_A, "b": gamepad.BTN_B, "x": gamepad.BTN_X, "y": gamepad.BTN_Y}.items():
        label = face[slot_of[fields[sdl]]]
        assert gamepad.Mapper().button(button, True) == [(LAUNCHER[label], True, False)], f"{label} on SDL's {sdl}"
    assert [fields[f"{side}{axis}"] for side in ("left", "right") for axis in "xy"] == [pad["axes"][r] for r in ("lx", "ly", "rx", "ry")]
    assert [fields[f"{side}trigger"] for side in ("left", "right")] == [pad["axes"].get(t) or pad["buttons"][t] for t in ("lt", "rt")]
    extras = {element for slot, element in pad["buttons"].items() if slot not in (*gamepad.FACE_POSITIONS, *gamepad.SDL_SLOTS)}
    assert not extras & set(fields.values()), "the launcher reads no extra"
    for slot in gamepad.FACE_POSITIONS:
        spec = create(engine, "PadGlyph.qml", family=pad["family"], slot=slot).property("spec").toVariant()
        assert (spec["text"] or spec["symbol"]).lower() == face[slot].lower(), slot


def test_hint_glyphs_follow_the_pad(engine, fake):
    glyph = create(engine, "ButtonGlyph.qml", glyph="A")
    assert glyph.property("family") == "xbox", "no pad seen: Xbox letters"
    from universe_ui.screens.controller import FakeWatcher

    fake_api = engine.rootContext().contextProperty("api")
    fake_api.screens.controller.start(FakeWatcher("dualsense-edge"))
    assert glyph.property("family") == "dualsense-edge"
    pair = create(engine, "ButtonGlyph.qml", glyph="Start Select")
    assert pair.property("names").toVariant() == ["Start", "Select"] and pair.property("chord") is False
    chord = create(engine, "ButtonGlyph.qml", glyph="Start+Select")
    assert chord.property("names").toVariant() == ["Start", "Select"] and chord.property("chord") is True


def test_live_view_names_a_pulled_trigger(engine, fake):
    art = create(engine, "ControllerArt.qml", family="dualsense-edge", connected=True, width=1200, height=800)
    pad = art.findChild(QObject, "padArt")
    before = until(pad.height)
    invoke(art, "axis", "rt", 0.4)
    assert art.property("lastSlot") == "", "a trigger logs once it passes half"
    invoke(art, "axis", "rt", 0.6)
    assert art.property("lastSlot") == "rt" and abs(art.property("lastPull") - 0.6) < 1e-6
    invoke(art, "axis", "rt", 0.9)
    invoke(art, "press", "south", True)
    invoke(art, "press", "south", False)
    for name, value in [("ly", -0.4), ("lx", 0.7), ("lx", 0), ("ly", 0), ("ly", 0.6), ("lx", -0.6)]:
        invoke(art, "axis", name, value)
    entries = read(art, "entries")
    assert [e["slot"] for e in entries] == ["ls", "ls", "south", "rt"], "newest first; a release and a held axis log nothing new"
    assert entries[0]["label"].endswith("down-left") and entries[1]["label"].endswith("up-right"), "an entry follows its push's peak"
    assert entries[3]["label"].endswith("90 %")
    assert entries[3]["dt"] is None and all(e["dt"] is not None for e in entries[:3])
    assert art.property("lastSlot") == "ls" and art.property("lastPull") == 0
    assert pad.height() == before, "the history column has its own room: a press does not resize the pad"
    invoke(art, "clear")
    assert art.property("lastSlot") == "" and read(art, "entries") == []


def test_cards_land_on_the_first_row_and_leave_left(engine, fake):
    rows = [
        {"type": "action", "label": "A game", "display": "Installed"},
        {"type": "action", "label": "Another", "display": "Owned"},
    ]
    groups = [{"title": "Installed", "rows": [0]}, {"title": "Owned", "rows": [1]}]
    cards = create(engine, "SettingsCards.qml", rows=rows, groups=groups, columns=1, width=1200, height=800)
    invoke(cards, "reset")
    assert cards.property("index") == 0
    assert {c["col"] for c in read(cards, "layout")["cards"]} == {0}, "one column: every card in it"
    left = record(cards.escapedLeft)
    invoke(cards, "cross", -1)
    assert left == [()]
    invoke(cards, "step", 1)
    assert cards.property("index") == 1
