"""The pads draw offscreen: every family's art builds a button per slot the core lists, and every
slot has a glyph, with no QML errors."""

import pytest
from PySide6.QtCore import QUrl
from PySide6.QtQml import QQmlComponent, QQmlEngine
from PySide6.QtQuick import QQuickItem  # noqa: F401  (down-casts created objects, for childItems)

from conftest import pump
from universe_ui import host

FAMILIES = ["dualsense-edge", "dualsense", "dualshock4", "xbox-elite", "xbox", "switch-pro", "8bitdo-pro-3", "generic"]


@pytest.fixture
def engine(api):
    engine = QQmlEngine()
    for p in host.qml_import_paths() if host.qt_paths_unset() else []:
        engine.addImportPath(p)
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
    assert pair.property("names").toVariant() == ["Start", "Select"]
