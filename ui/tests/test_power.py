from universe_ui.screens.power import Power, read_sources


def supply(root, name, **files):
    d = root / name
    d.mkdir(parents=True)
    for key, value in files.items():
        (d / key).write_text(value + "\n")
    return d


def test_batteries_come_from_sysfs_and_pads_map_to_their_event_nodes(tmp_path):
    root = tmp_path / "power_supply"
    supply(root, "AC", type="Mains", online="1")
    supply(root, "BAT1", type="Battery", capacity="12", status="Charging", scope="System")
    supply(root, "hid-battery", type="Battery", capacity_level="Normal", status="Discharging", scope="Device")
    supply(root, "gone", type="Battery", present="0", capacity="50")
    supply(root, "mystery", type="Battery", status="Unknown", capacity_level="Unknown")
    pad = tmp_path / "devices" / "hid0"
    (pad / "power_supply").mkdir(parents=True)
    (root / "hid-battery").rename(pad / "power_supply" / "hid-battery")
    (root / "hid-battery").symlink_to(pad / "power_supply" / "hid-battery")
    inputs = tmp_path / "input"
    (inputs / "event7" / "device").mkdir(parents=True)
    (inputs / "event7" / "device" / "device").symlink_to(pad)
    (inputs / "mouse0").mkdir()

    sources = read_sources(root)
    assert [(s["name"], s["kind"], s["percent"], s["charging"], s["inputs"]) for s in sources] == [
        ("BAT1", "system", 12, True, []),
        ("hid-battery", "pad", 50, False, ["event7"]),
    ]


def test_power_polls_and_notifies(app, tmp_path):
    root = tmp_path / "power_supply"
    bat = supply(root, "BAT0", type="Battery", capacity="80", status="Discharging")
    power = Power(str(root))
    assert power.count == 1 and power.sources[0]["percent"] == 80
    changes = []
    power.sourcesChanged.connect(lambda: changes.append(power.sources))
    power.refresh()
    assert changes == []
    (bat / "capacity").write_text("79\n")
    power.refresh()
    assert len(changes) == 1 and changes[0][0]["percent"] == 79
    assert power.forInput("event1") is None


def test_a_reported_charge_joins_the_sources_until_forgotten(app, tmp_path):
    root = tmp_path / "power_supply"
    supply(root, "BAT0", type="Battery", capacity="80", status="Discharging")
    listed = supply(root, "ps-controller-battery", type="Battery", scope="Device", capacity="60", status="Charging")
    (listed / "inputs").write_text("event30\n")
    power = Power(str(root))
    power.report("event31", "8BitDo Pro 3", {"percent": 80, "charging": False})
    assert [(s["name"], s["kind"], s["percent"], s["inputs"]) for s in power.sources] == [
        ("BAT0", "system", 80, []),
        ("ps-controller-battery", "pad", 60, ["event30"]),
        ("8BitDo Pro 3", "pad", 80, ["event31"]),
    ]
    assert power.forInput("event31")["charging"] is False
    power.report("event30", "Wireless Controller", {"percent": 10, "charging": False})
    assert power.forInput("event30")["percent"] == 60, "the kernel's reading wins over the watcher's"
    power.refresh()
    assert power.count == 3, "a poll keeps what was reported"
    power.report("event31", "8BitDo Pro 3", None)
    assert power.count == 2
