import pytest
from looks import REVEAL, SUBMIT, activate, content_rows, invoke, read
from PySide6.QtCore import QObject, Qt
from uitest import until

from universe_ui.fake_radios import PASSWORD

PAD = "98:B6:E9:12:34:56"
KEYBOARD = "F0:3E:90:AB:CD:EF"
HEADSET = "AC:80:0A:6B:21:BC"


def shows(page, **want):
    return any(all(r.get(k) == v for k, v in want.items()) for r in content_rows(page))


def section(look, ident, **want):
    page = look.settings(ident)
    until(lambda: shows(page, **want), f"no {want} in the {ident} section")
    return page


def sheet_open(look, secret):
    sheet = look.find("textSheet")
    until(lambda: sheet.property("open") is True and sheet.property("secret") is secret)
    return sheet


def type_in(look, sheet, text):
    sheet.setProperty("text", text)
    look.press(SUBMIT[look.name])


def dialog_open(look):
    dialog = look.dialog()
    until(lambda: dialog.property("open") is True)
    return dialog


def test_the_sections_follow_what_the_machine_has(look, api):
    page = look.settings("about")
    until(lambda: {"network", "bluetooth"} <= {s["id"] for s in read(page, "sections")})
    api.system.setRadios(False, False)
    until(lambda: not {"network", "bluetooth"} & {s["id"] for s in read(page, "sections")}, "no NetworkManager, no adapter, or under Steam: no page")


def test_a_secured_network_joins_with_a_masked_password(look, api, fake):
    page = section(look, "network", key="network", ssid="Atelier")
    assert shows(page, key="wifi")
    until(lambda: {"cmd": "scan", "on": True} in fake.core.wifi.commands, "the section scans while it is open")
    activate(page, content_rows(page), key="network", ssid="Atelier")
    sheet = sheet_open(look, True)
    sheet.setProperty("text", PASSWORD)
    assert read(sheet, "masked") == "•" * len(PASSWORD), "dots on the TV"
    look.press(REVEAL[look.name])
    until(lambda: read(sheet, "masked") == PASSWORD)
    look.press(SUBMIT[look.name])
    until(lambda: api.screens.network.ssid == "Atelier")
    assert {"cmd": "connect", "ssid": "Atelier", "password": PASSWORD} in fake.core.wifi.commands


def test_a_refused_password_asks_for_it_again(look, api, fake):
    page = section(look, "network", key="network", ssid="Atelier")
    activate(page, content_rows(page), key="network", ssid="Atelier")
    type_in(look, sheet_open(look, True), "nope")
    dialog_open(look)
    look.press(Qt.Key.Key_Return)
    sheet = sheet_open(look, True)
    assert read(sheet, "text") == "", "Try Again starts on an empty field"


def test_a_saved_network_joins_as_saved_and_the_joined_one_forgets_after_asking(look, api, fake):
    wifi = api.screens.network
    page = section(look, "network", key="network", ssid="Home", active=True)
    activate(page, content_rows(page), key="network", ssid="Home")
    dialog_open(look)
    look.press(Qt.Key.Key_Return)
    until(lambda: {"cmd": "forget", "ssid": "Home"} in fake.core.wifi.commands, "Forget is the answer under the cursor")
    until(lambda: wifi.link == "")
    next(n for n in fake.core.wifi.networks if n["ssid"] == "Atelier")["saved"] = True
    fake.core.wifi._changed()
    until(lambda: shows(page, key="network", ssid="Atelier", saved=True))
    activate(page, content_rows(page), key="network", ssid="Atelier")
    until(lambda: wifi.ssid == "Atelier", "a saved network joins without a password")
    assert {"cmd": "connect", "ssid": "Atelier"} in fake.core.wifi.commands


def test_a_network_forgotten_then_refused_asks_for_its_password_again(look, api, fake):
    page = section(look, "network", key="network", ssid="Home", active=True)
    activate(page, content_rows(page), key="network", ssid="Home")
    dialog_open(look)
    look.press(Qt.Key.Key_Return)
    until(lambda: shows(page, key="network", ssid="Home", saved=False))
    activate(page, content_rows(page), key="network", ssid="Home")
    type_in(look, sheet_open(look, True), "nope")
    dialog_open(look)
    look.press(Qt.Key.Key_Return)
    sheet_open(look, True)


def test_a_network_universe_cannot_set_up_is_refused(look, api, fake):
    page = section(look, "network", key="network", ssid="Campus")
    activate(page, content_rows(page), key="network", ssid="Campus")
    assert not any(c.get("cmd") == "connect" for c in fake.core.wifi.commands)
    assert look.find("textSheet").property("open") is not True


def test_wifi_switches_off_from_its_row(look, api, fake):
    page = section(look, "network", key="wifi")
    activate(page, content_rows(page), key="wifi")
    until(lambda: {"cmd": "wifi", "on": False} in fake.core.wifi.commands)


def test_the_bluetooth_section_searches_while_open_and_pairs_a_found_pad(look, api, fake):
    page = section(look, "bluetooth", key="device", address=HEADSET, paired=True)
    assert shows(page, key="power")
    until(lambda: {"cmd": "scan", "on": True} in fake.core.bt.commands)
    until(lambda: shows(page, key="device", address=PAD, paired=False))
    activate(page, content_rows(page), key="device", address=PAD)
    until(lambda: PAD in [d["address"] for d in api.screens.bluetooth.paired])


def test_a_keyboard_pairs_through_its_passkey_question(look, api, fake):
    page = section(look, "bluetooth", key="device", address=KEYBOARD)
    activate(page, content_rows(page), key="device", address=KEYBOARD)
    dialog_open(look)
    look.press(Qt.Key.Key_Return)
    until(lambda: KEYBOARD in [d["address"] for d in api.screens.bluetooth.paired], "Pair is the answer under the cursor")


def test_a_paired_device_connects_and_disconnects(look, api, fake):
    bt = api.screens.bluetooth
    page = section(look, "bluetooth", key="device", address=HEADSET)
    activate(page, content_rows(page), key="device", address=HEADSET)
    dialog_open(look)
    look.press(Qt.Key.Key_Return)
    until(lambda: next(d for d in bt.paired if d["address"] == HEADSET)["connected"], "Connect is the answer under the cursor")


def test_a_cable_pairing_in_the_session_asks_over_any_screen(look, api, fake, monkeypatch):
    monkeypatch.setenv("UNIVERSE_FAKE_SESSION", "1")
    look.home()
    api.startRadios()
    until(lambda: fake.core.bt.watches)
    fake.core.bt.ask(PAD, "authorize")
    dialog_open(look)
    look.press(Qt.Key.Key_Return)
    until(lambda: PAD in [d["address"] for d in api.screens.bluetooth.paired])


def test_a_pairing_question_waits_for_the_password_being_typed(look, api, fake):
    page = section(look, "network", key="network", ssid="Atelier")
    api.screens.bluetooth.start()
    until(lambda: fake.core.bt.watches)
    activate(page, content_rows(page), key="network", ssid="Atelier")
    sheet = sheet_open(look, True)
    fake.core.bt.ask(PAD, "authorize")
    until(lambda: api.screens.bluetooth.request)
    assert sheet.property("open") is True and sheet.property("secret") is True, "the password keeps its sheet"
    type_in(look, sheet, PASSWORD)
    until(lambda: api.screens.network.ssid == "Atelier")
    dialog_open(look)
    look.press(Qt.Key.Key_Return)
    until(lambda: PAD in [d["address"] for d in api.screens.bluetooth.paired], "the question comes once the sheet is gone")


def test_a_code_to_type_on_the_device_shows_until_cancelled(look, api, fake):
    look.home()
    api.screens.bluetooth.start()
    until(lambda: fake.core.bt.watches)
    fake.core.bt.ask(KEYBOARD, "display", "123456")
    dialog = dialog_open(look)
    look.press(Qt.Key.Key_Escape)
    until(lambda: {"cmd": "cancel", "address": KEYBOARD} in fake.core.bt.commands)
    until(lambda: dialog.property("open") is False)


def test_a_pin_is_typed_on_the_sheet(look, api, fake):
    look.home()
    api.screens.bluetooth.start()
    until(lambda: fake.core.bt.watches)
    asked = fake.core.bt.ask(HEADSET, "pin")
    type_in(look, sheet_open(look, False), "0000")
    until(lambda: {"cmd": "answer", "id": asked, "value": "0000"} in fake.core.bt.commands)


def test_a_question_bluez_withdraws_closes_its_dialog(look, api, fake):
    look.home()
    api.screens.bluetooth.start()
    until(lambda: fake.core.bt.watches)
    fake.core.bt.ask(HEADSET, "confirm", "111111")
    dialog = dialog_open(look)
    fake.core.bt.cancel_ask()
    until(lambda: dialog.property("open") is False)


@pytest.mark.parametrize("look", ["switch2", "ps5"], indirect=True)
def test_the_controllers_page_searches_for_pads(look, api, fake):
    page = look.open("pages/ControllersPage.qml")
    until(lambda: page.findChild(QObject, "pairController"), "no Pair entry on the page")
    invoke(page, "pairController")
    settings = look.page("settingsPage")
    until(lambda: read(settings, "sectionId") == "bluetooth" and shows(settings, key="device", address=PAD))
    kinds = {r["kind"] for r in content_rows(settings) if r.get("key") == "device"}
    assert kinds == {"pad"}, "the search shows pads alone"
