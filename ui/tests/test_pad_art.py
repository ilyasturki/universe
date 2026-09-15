import pytest
from PySide6.QtCore import QUrl
from PySide6.QtQml import QQmlComponent, QQmlEngine, QQmlExpression
from PySide6.QtQuick import QQuickItem  # noqa: F401  (down-casts created objects, for childItems)

from conftest import pump
from universe_ui import host

FAMILIES = ["dualsense-edge", "dualsense", "dualshock4", "xbox-elite", "xbox", "switch-pro", "8bitdo-pro-3", "generic"]


@pytest.fixture
def engine(api):
    engine = QQmlEngine()
    engine.rootContext().setContextProperty("api", api)
    engine.made = []
    yield engine
    # The items go before the api they bind to, or their bindings complain on the way out.
    for item in engine.made:
        item.deleteLater()
    pump(20)
    engine.deleteLater()
    pump(20)


def create(engine, name, **props):
    component = QQmlComponent(engine, QUrl.fromLocalFile(str(host.QML_DIR / "ui" / name)), engine)
    assert component.isReady(), component.errorString()
    item = component.createWithInitialProperties(props)
    assert item is not None, component.errorString()
    engine.made.append(item)
    return item


def call(engine, item, expression):
    result = QQmlExpression(engine.rootContext(), item, expression).evaluate()
    return result[0] if isinstance(result, tuple) else result


def descendant(item, prop):
    for child in item.childItems():
        if child.property(prop) is not None:
            return child
        found = descendant(child, prop)
        if found is not None:
            return found
    return None


def slots_of(fake, family):
    return [s["id"] for f in fake.controllerState()["families"] if f["id"] == family for s in f["slots"]]


@pytest.mark.parametrize("family", FAMILIES)
def test_pad_art_has_a_button_per_slot(engine, fake, family):
    art = create(engine, "PadArt.qml", family=family, width=900, height=600)
    pump(50)
    drawn = {c.property("slot") for c in art.childItems() if c.property("slot") is not None}
    expected = set(slots_of(fake, family))
    missing = expected - drawn
    assert not missing, f"{family} draws no {sorted(missing)}"
    assert drawn <= expected | {"mute"}, f"{family} draws slots the core does not list: {sorted(drawn - expected)}"
    art.setProperty("pressed", {"south": True})
    art.setProperty("axes", {"lx": -0.5, "rt": 0.7})
    pump(50)


def test_every_slot_has_a_glyph(engine, fake):
    for family in FAMILIES:
        for slot in slots_of(fake, family) + ["dpad"]:
            glyph = create(engine, "PadGlyph.qml", family=family, slot=slot)
            spec = glyph.property("spec").toVariant()
            assert spec["shape"], (family, slot)
            assert spec["text"] or spec["symbol"] or spec["shape"] == "dpad", (family, slot)


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
    pump(50)
    pad = descendant(art, "geo")
    before = pad.height()
    call(engine, art, "axis('rt', 0.4)")
    assert art.property("lastSlot") == "rt" and abs(art.property("lastPull") - 0.4) < 1e-6
    call(engine, art, "press('south', true)")
    assert art.property("lastSlot") == "south" and art.property("lastPull") == 0
    pump(50)
    assert pad.height() == before, "the caption's room is reserved: a press does not resize the pad"
    call(engine, art, "clear()")
    assert art.property("lastSlot") == ""


def test_cards_land_past_the_search_row_and_leave_left(engine, fake):
    rows = [
        {"type": "search", "label": "Search GOG", "display": ""},
        {"type": "action", "label": "A game", "display": "Installed"},
        {"type": "action", "label": "Another", "display": "Owned"},
    ]
    groups = [{"title": "Installed", "rows": [0, 1]}, {"title": "Owned", "rows": [2]}]
    cards = create(engine, "SettingsCards.qml", rows=rows, groups=groups, columns=1, width=1200, height=800)
    pump(50)
    call(engine, cards, "reset()")
    assert cards.property("index") == 1, "the search field is reached by going up, not landed on"
    layout = cards.property("layout").toVariant()
    assert {c["col"] for c in layout["cards"]} == {0}, "one column: every card in it"
    left = []
    cards.escapedLeft.connect(lambda: left.append(True))
    call(engine, cards, "cross(-1)")
    assert left == [True]
    call(engine, cards, "step(-1)")
    assert cards.property("index") == 0
