import os
from pathlib import Path

import dbus
import dbusmock
import pytest
from dbusmock import BusType, PrivateDBus, SpawnedMock
from uitest import until

CARD = Path(__file__).with_name("networkmanager_card.py")
DISCONNECTED, ACTIVATED = 30, 100
NO_CONNECTIVITY = 1
OPEN, WPA2_PSK = 0, 0x100


@pytest.fixture
def networkmanager(monkeypatch):
    monkeypatch.delenv("DBUS_SYSTEM_BUS_ADDRESS", raising=False)
    with PrivateDBus(BusType.SYSTEM) as bus:
        monkeypatch.setenv("DBUS_SYSTEM_BUS_ADDRESS", bus.address)
        with SpawnedMock.spawn_with_template("networkmanager", bustype=BusType.SYSTEM) as nm:
            mock = dbus.Interface(nm.obj, dbusmock.MOCK_IFACE)
            mock.AddTemplate(str(CARD), dbus.Dictionary({}, signature="sv"))
            mock.SetConnectivity(NO_CONNECTIVITY)
            yield mock


@pytest.fixture
def wifi(networkmanager):
    card = networkmanager.AddWiFiCard("wlan0", "wlan0", DISCONNECTED)
    networkmanager.AddAccessPoint(card, "home", "Home", "11:22:33:44:55:01", 2, 2437, 54000, 80, WPA2_PSK)
    networkmanager.AddAccessPoint(card, "atelier", "Atelier", "11:22:33:44:55:02", 2, 5180, 54000, 55, OPEN)
    return card


@pytest.fixture
def first_run(app, xdg, tmp_path, monkeypatch):
    import universe_core
    from universe_ui.api import Api
    from universe_ui.screens.network import FAKE as FAKE_NET
    from universe_ui.screens.power import FAKE
    from universe_ui.universe_client import CoreClient

    if not os.environ.get("UNIVERSE_BIN"):
        pytest.fail("UNIVERSE_BIN names the universe build whose network watch the setup runs (`just test` sets it)")
    for kind in ("data", "config", "state", "cache"):
        monkeypatch.setenv(f"UNIVERSE_{kind.upper()}_HOME", str(tmp_path / kind))
    for kind in ("modules", "sources"):
        (tmp_path / kind).mkdir()
        monkeypatch.setenv(f"UNIVERSE_{kind.upper()}_PATH", str(tmp_path / kind))
    catalogue = tmp_path / "catalogue.json"
    catalogue.write_text('{"schema": 1, "components": {}}')
    monkeypatch.setenv("UNIVERSE_CATALOGUE", catalogue.as_uri())
    monkeypatch.setenv("HOME", str(tmp_path / "home"))
    monkeypatch.setenv("XDG_RUNTIME_DIR", str(tmp_path / "run"))
    monkeypatch.setenv("DBUS_SESSION_BUS_ADDRESS", "unix:path=/dev/null/bus")
    client = CoreClient(universe_core.Core())
    api = Api(client, memory_path=str(tmp_path / "memory.json"), power_root=FAKE, net_root=FAKE_NET)

    def load():
        form = api.screens.onboarding
        form.load()
        until(lambda: not form.loading and form.steps)
        return form

    load.api = api
    yield load
    api.shutdown()


def rows(form):
    return {r["ssid"]: r for r in form.rows if r["key"] == "network"}


def test_offline_with_a_wifi_card_the_setup_joins_a_network_first(networkmanager, wifi, first_run):
    form = first_run()
    assert [s["id"] for s in form.steps][:2] == ["network", "found"]
    until(lambda: {"Home", "Atelier"} <= set(rows(form)), "the card's networks, read through the watch")
    assert (rows(form)["Home"]["secured"], rows(form)["Atelier"]["secured"]) == (True, False)
    assert first_run.api.screens.network.join("Atelier", "")
    until(lambda: rows(form)["Atelier"]["active"], "the step shows the network joined")
    assert first_run.api.screens.network.link == "wifi"
    form.next()
    assert form.stepId == "found"


@pytest.mark.parametrize("link", ["wifi", "wired"])
def test_online_the_setup_starts_at_the_machine(networkmanager, wifi, first_run, link):
    if link == "wired":
        networkmanager.AddEthernetDevice("eth0", "eth0", ACTIVATED)
    else:
        saved = networkmanager.AddWiFiConnection(wifi, "home", "Home", "wpa-psk")
        networkmanager.AddActiveConnection([wifi], saved, "/org/freedesktop/NetworkManager/AccessPoint/home", "home", 2)
    assert first_run().steps[0]["id"] == "found"
