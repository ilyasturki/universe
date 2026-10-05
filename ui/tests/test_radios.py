import pytest
from uitest import record, until

from universe_ui.fake_radios import KEYBOARD_PASSKEY, PASSWORD


def by_ssid(screen):
    return {n["ssid"]: n for n in screen.networks}


def test_the_networks_come_from_the_first_read_and_drive_the_status(api):
    wifi = api.screens.network
    until(lambda: wifi.networks)
    home = by_ssid(wifi)["Home"]
    assert (home["active"], home["saved"], home["secured"], home["bars"]) == (True, True, True, 3)
    assert (by_ssid(wifi)["Campus"]["joinable"], by_ssid(wifi)["Café Lumière"]["secured"]) == (False, False)
    assert (wifi.available, wifi.link, wifi.ssid, wifi.connectivity) == (True, "wifi", "Home", "full")
    assert (api.network.kind, api.network.bars) == ("wifi", 3), "NetworkManager's link replaces the sysfs one"
    assert api.system.network and api.system.bluetooth


def test_a_new_network_joins_with_its_password_and_a_wrong_one_fails_as_such(api, fake):
    wifi = api.screens.network
    until(lambda: wifi.networks)
    assert wifi.needsPassword("Atelier") and not wifi.needsPassword("Home") and not wifi.needsPassword("Café Lumière")
    failed, joined = record(wifi.failed), record(wifi.joined)
    assert wifi.join("Atelier", "nope")
    assert wifi.connecting == "Atelier"
    until(lambda: failed)
    assert failed == [("Atelier", "password", "the password was not accepted")]
    assert (wifi.connecting, wifi.error["reason"]) == ("", "password")
    wifi.join("Atelier", PASSWORD)
    assert wifi.error == {}, "a new try clears the last failure"
    until(lambda: joined)
    assert joined == [("Atelier", "full")]
    until(lambda: wifi.ssid == "Atelier")
    assert by_ssid(wifi)["Atelier"]["saved"]
    assert {"cmd": "connect", "ssid": "Atelier", "password": PASSWORD} in fake.core.wifi.commands


def test_the_card_scans_while_a_page_is_open_and_forgets_on_demand(api, fake):
    wifi = api.screens.network
    commands = fake.core.wifi.commands
    wifi.open()
    wifi.open()
    wifi.close()
    until(lambda: commands)
    assert commands == [{"cmd": "scan", "on": True}], "the second page open shares the scan"
    wifi.close()
    until(lambda: len(commands) == 2)
    assert commands[-1] == {"cmd": "scan", "on": False}
    wifi.forget("Home")
    until(lambda: wifi.link == "")
    assert not by_ssid(wifi)["Home"]["saved"]
    assert (api.network.kind, api.network.bars) == ("", 0)


def test_wifi_switches_off_and_on(api):
    wifi = api.screens.network
    until(lambda: wifi.networks)
    wifi.setEnabled(False)
    until(lambda: not wifi.enabled)
    assert wifi.networks == [] and wifi.link == ""
    wifi.setEnabled(True)
    until(lambda: wifi.enabled)


@pytest.mark.parametrize(("mode", "network"), [("none", False), ("offline", True)])
def test_no_networkmanager_hides_the_pages_and_offline_has_no_link(monkeypatch, mode, network, app, xdg, fixture_art, tmp_path):
    from universe_ui.api import Api
    from universe_ui.fake_core import FIXTURE, FakeCore
    from universe_ui.screens.network import FAKE as FAKE_NET
    from universe_ui.screens.power import FAKE
    from universe_ui.universe_client import CoreClient

    monkeypatch.setenv("UNIVERSE_FAKE_NETWORK", mode)
    client = CoreClient(FakeCore(FIXTURE, tmp_path / "core"))
    api = Api(client, memory_path=str(tmp_path / "memory.json"), power_root=FAKE, net_root=FAKE_NET)
    try:
        until(lambda: api.system.bluetooth)
        assert (api.system.network, api.screens.network.available) == (network, network)
        assert api.screens.network.link == ""
    finally:
        api.shutdown()
        client.shutdown()


def test_steams_game_mode_keeps_both_pages(monkeypatch, app, xdg, fixture_art, tmp_path):
    from universe_ui.api import Api
    from universe_ui.fake_core import FIXTURE, FakeCore
    from universe_ui.screens.power import FAKE
    from universe_ui.universe_client import CoreClient

    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    monkeypatch.setenv("UNIVERSE_FAKE_STEAM", "1")
    client = CoreClient(FakeCore(FIXTURE, tmp_path / "core"))
    api = Api(client, memory_path=str(tmp_path / "memory.json"), power_root=FAKE)
    try:
        until(lambda: api.screens.network.available and api.screens.bluetooth.available)
        assert (api.system.network, api.system.bluetooth) == (False, False)
        api.startRadios()
        assert api.screens.network._stream is None and api.screens.bluetooth._stream is None
    finally:
        api.shutdown()
        client.shutdown()


def addresses(devices):
    return [d["address"] for d in devices]


def test_a_search_finds_devices_and_a_pad_pairs_from_it(api, fake):
    bt = api.screens.bluetooth
    until(lambda: bt.paired)
    assert [d["kind"] for d in bt.paired] == ["pad", "audio"] and bt.found == []
    bt.open()
    until(lambda: bt.discovering and len(bt.found) == 3)
    pad = next(d for d in bt.found if d["kind"] == "pad")
    done = record(bt.done)
    assert bt.pair(pad["address"])
    assert bt.pairing == pad["address"]
    until(lambda: done)
    assert done == [("pair", pad["address"])]
    assert pad["address"] in addresses(bt.paired) and bt.pairing == ""
    bt.close()
    until(lambda: not bt.discovering)
    assert bt.found == [], "found devices go with the search"


def test_a_keyboards_passkey_is_asked_and_its_answer_pairs_it(api, fake):
    bt = api.screens.bluetooth
    bt.open()
    until(lambda: bt.found)
    keyboard = next(d for d in bt.found if d["kind"] == "keyboard")
    bt.pair(keyboard["address"])
    until(lambda: bt.request)
    assert (bt.request["kind"], bt.request["code"], bt.request["address"]) == ("confirm", KEYBOARD_PASSKEY, keyboard["address"])
    bt.answer(True)
    assert bt.request is None
    until(lambda: keyboard["address"] in addresses(bt.paired))


def test_a_refused_passkey_fails_the_pairing(api, fake):
    bt = api.screens.bluetooth
    bt.open()
    until(lambda: bt.found)
    keyboard = next(d for d in bt.found if d["kind"] == "keyboard")
    failed = record(bt.failed)
    bt.pair(keyboard["address"])
    until(lambda: bt.request)
    bt.answer(False)
    until(lambda: failed)
    assert failed[0][:3] == ("pair", keyboard["address"], "rejected")


# A Sony pad plugged in by cable: BlueZ asks the session's default agent; the user's yes pairs it.
def test_a_cable_pairing_asks_over_the_launcher(api, fake, universe_session):
    bt = api.screens.bluetooth
    api.startRadios()
    if not universe_session:
        assert bt._stream is None, "outside the session the desktop's agent answers cable pairings"
        return
    until(lambda: fake.core.bt.watches)
    pad = "98:B6:E9:12:34:56"
    fake.core.bt.ask(pad, "authorize")
    until(lambda: bt.request)
    assert (bt.request["kind"], bt.request["deviceKind"]) == ("authorize", "pad")
    bt.answer(True)
    until(lambda: pad in addresses(bt.paired))


def test_a_question_while_a_game_holds_the_screen_is_refused_with_a_notice(api, fake, monkeypatch):
    monkeypatch.setattr(api.home, "_shown", "game")
    bt = api.screens.bluetooth
    bt.start()
    until(lambda: fake.core.bt.watches)
    notices = record(fake.notice)
    asked = fake.core.bt.ask("98:B6:E9:12:34:56", "authorize")
    until(lambda: {"cmd": "answer", "id": asked, "yes": False} in fake.core.bt.commands)
    assert bt.request is None
    assert notices == [("Plug Pro Controller in again from HOME",)]


def test_bluez_withdrawing_its_question_closes_it(api, fake):
    bt = api.screens.bluetooth
    bt.start()
    until(lambda: fake.core.bt.watches)
    fake.core.bt.ask("AC:80:0A:6B:21:BC", "confirm", "123456")
    until(lambda: bt.request)
    fake.core.bt.cancel_ask()
    until(lambda: bt.request is None)


def test_connect_disconnect_forget_and_power(api, fake):
    bt = api.screens.bluetooth
    headset = "AC:80:0A:6B:21:BC"
    until(lambda: bt.paired)
    bt.connectDevice(headset)
    until(lambda: next(d for d in bt.paired if d["address"] == headset)["connected"])
    bt.disconnectDevice(headset)
    until(lambda: not next(d for d in bt.paired if d["address"] == headset)["connected"])
    bt.forget(headset)
    until(lambda: headset not in addresses(bt.paired))
    bt.setPowered(False)
    until(lambda: not bt.powered)


def test_a_stream_that_ends_starts_again(api, fake, monkeypatch):
    from universe_ui.screens import stream

    monkeypatch.setattr(stream, "RESTART_MS", 10)
    wifi = api.screens.network
    wifi._delay = 10
    wifi.start()
    first = wifi._stream
    wifi._received({"event": "off", "code": 1})
    assert wifi._stream is None
    until(lambda: wifi._stream is not None and wifi._stream is not first)
