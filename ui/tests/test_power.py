from universe_ui.screens.power import Power, number_pads, read_player, read_sources


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
    power.pad("event31", "8BitDo Pro 3", "8bitdo-pro-3", {"percent": 80, "charging": False})
    assert [(s["name"], s["kind"], s["percent"], s["inputs"]) for s in power.sources] == [
        ("BAT0", "system", 80, []),
        ("ps-controller-battery", "pad", 60, ["event30"]),
        ("8BitDo Pro 3", "pad", 80, ["event31"]),
    ]
    assert power.forInput("event31")["charging"] is False
    power.pad("event30", "Wireless Controller", "dualsense", {"percent": 10, "charging": False})
    assert power.forInput("event30")["percent"] == 60, "the kernel's reading wins over the watcher's"
    assert power.forInput("event30")["family"] == "dualsense", "the kernel's supply takes the watcher's family"
    power.refresh()
    assert power.count == 3, "a poll keeps what was reported"
    power.report("event31", None)
    assert power.count == 2
    power.report("event31", {"percent": 70, "charging": True})
    assert power.forInput("event31")["percent"] == 70
    power.forget("event31")
    power.report("event31", {"percent": 70, "charging": True})
    assert power.forInput("event31") is None, "a charge for a pad that left is dropped"


def test_pads_are_numbered_once_two_show(app, tmp_path):
    power = Power(str(tmp_path / "power_supply"))
    power.pad("event30", "DualSense", "dualsense", {"percent": 50, "charging": False})
    assert power.forInput("event30")["player"] == 0, "one pad needs no number"
    power.pad("event31", "Xbox", "xbox")
    assert power.forInput("event30")["player"] == 0, "a pad with no reading is not drawn"
    power.report("event31", {"percent": 40, "charging": False})
    power.pad("event32", "Xbox", "xbox", {"percent": 30, "charging": False})
    assert [(s["family"], s["player"]) for s in power.sources] == [("dualsense", 1), ("xbox", 2), ("xbox", 3)]
    power.forget("event30")
    assert [s["player"] for s in power.sources] == [1, 2]
    power.pad("event30", "DualSense", "dualsense", {"percent": 50, "charging": False})
    assert [s["player"] for s in power.sources] == [1, 2, 3], "a pad back joins at the end"


def test_the_kernels_numbers_win_and_the_rest_fill_around_them():
    assert number_pads(["a", "b", "c"], {"b": 1}) == {"b": 1, "a": 2, "c": 3}
    assert number_pads(["a", "b", "c"], {"a": 3}) == {"a": 3, "b": 1, "c": 2}
    assert number_pads(["a", "b"], {"a": 1, "b": 1}) == {"a": 1, "b": 1}, "two drivers can light the same number"


def led(root, name, owner, brightness):
    d = root / "leds" / name
    d.mkdir(parents=True)
    (d / "device").symlink_to(owner)
    (d / "brightness").write_text(f"{brightness}\n")


def event(root, name, device):
    (root / "input" / name / "device").mkdir(parents=True)
    (root / "input" / name / "device" / "device").symlink_to(device)


def test_the_kernels_player_leds_give_a_pads_number(tmp_path):
    hid = tmp_path / "devices" / "0005:054C:0CE6.0001"
    hid.mkdir(parents=True)
    event(tmp_path, "event20", hid)
    for i, on in enumerate([0, 1, 0, 1, 0], 1):
        led(tmp_path, f"input40:white:player-{i}", hid, on)
    assert read_player(tmp_path, "event20") == 2, "a DualSense's second player lights LEDs 2 and 4"

    switch = tmp_path / "devices" / "0005:057E:2009.0002"
    switch.mkdir(parents=True)
    event(tmp_path, "event21", switch)
    for i, on in enumerate([1, 0, 0, 1], 1):
        led(tmp_path, f"0005:057E:2009.0002:green:player-{i}", switch, on)
    assert read_player(tmp_path, "event21") == 5

    usb = tmp_path / "devices" / "usb1" / "1-2"
    (usb / "1-2:1.0").mkdir(parents=True)
    event(tmp_path, "event22", usb / "1-2:1.0")
    led(tmp_path, "xpad0", usb, 3)
    assert read_player(tmp_path, "event22") == 2, "xpad's command 3 flashes then lights the second quadrant"

    receiver = tmp_path / "devices" / "usb1" / "1-3"
    for slot in range(2):
        (receiver / f"1-3:1.{slot}").mkdir(parents=True)
        led(tmp_path, f"xpad{slot + 1}", receiver, slot + 2)
    event(tmp_path, "event23", receiver / "1-3:1.0")
    assert read_player(tmp_path, "event23") == 0, "a wireless receiver's LEDs cannot be told apart"

    plain = tmp_path / "devices" / "0003:2DC8:6009.0003"
    plain.mkdir(parents=True)
    event(tmp_path, "event24", plain)
    assert read_player(tmp_path, "event24") == 0
    assert read_player(tmp_path, "event99") == 0


def test_a_lit_led_numbers_the_pad_in_the_sources(app, tmp_path):
    hid = tmp_path / "devices" / "hid0"
    hid.mkdir(parents=True)
    event(tmp_path, "event31", hid)
    for i, on in enumerate([0, 0, 1, 0, 0], 1):
        led(tmp_path, f"input40:white:player-{i}", hid, on)
    power = Power(str(tmp_path / "power_supply"))
    power.pad("event30", "Xbox", "xbox", {"percent": 40, "charging": False})
    power.pad("event31", "DualSense", "dualsense", {"percent": 50, "charging": False})
    assert [(s["family"], s["player"]) for s in power.sources] == [("xbox", 2), ("dualsense", 1)]
