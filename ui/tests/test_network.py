import os

from universe_ui.screens.network import FAKE, Network, read_link

WIRELESS_HEAD = "Inter-| sta-|   Quality        |\n face | tus | link level noise |\n"


def link(root, name, state="up", wlan=False, device=True):
    d = root / name
    d.mkdir(parents=True)
    (d / "operstate").write_text(state + "\n")
    (d / "uevent").write_text(("DEVTYPE=wlan\n" if wlan else "") + f"INTERFACE={name}\n")
    if device:
        (d / "device").write_text("")


def test_wifi_reads_its_arcs_and_a_cable_wins(tmp_path):
    root = tmp_path / "net"
    wireless = tmp_path / "wireless"
    link(root, "lo", state="unknown", device=False)
    link(root, "tailscale0", device=False)
    link(root, "wlan0", wlan=True)
    wireless.write_text(WIRELESS_HEAD + " wlan0: 0000   20.  -80.  -256  0 0 0 0 0 0\n")
    assert read_link(str(root), str(wireless)) == {"kind": "wifi", "bars": 1}
    wireless.write_text(WIRELESS_HEAD + " wlan0: 0000   59.  -51.  -256  0 0 0 0 0 0\n")
    assert read_link(str(root), str(wireless)) == {"kind": "wifi", "bars": 3}
    link(root, "eth0")
    assert read_link(str(root), str(wireless)) == {"kind": "wired", "bars": 0}


def test_offline_when_no_physical_link_is_up(tmp_path):
    root = tmp_path / "net"
    link(root, "lo", state="unknown", device=False)
    link(root, "wlan0", state="down", wlan=True)
    assert read_link(str(root), str(tmp_path / "missing")) == {"kind": "", "bars": 0}


def test_the_fixture_is_on_wifi(app):
    network = Network(os.path.join(FAKE, "class"), os.path.join(FAKE, "wireless"))
    assert (network.kind, network.bars) == ("wifi", 2)
