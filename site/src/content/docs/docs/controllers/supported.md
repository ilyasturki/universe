---
title: Supported controllers
description: The controller models Universe knows, how it tells them apart, the glyphs it draws for each, and what your system needs to read them.
sidebar:
  order: 1
---

Universe works with any pad Linux reads as a gamepad. Nine models, which the app calls families, get their own button names, glyphs and extra buttons. Every other pad is a plain **Controller** with the standard buttons.

The menus read your pad through SDL. HOME, macros, button setup and the button test read the pad's `/dev/input` node, without taking it from the game.

## Controller families

| Family | Id | Pads | Extra buttons |
|---|---|---|---|
| Steam Deck | `steam-deck` | The Deck's own controls (`28de:1205`), and Steam Input's pad on a Deck | L4 `grip_l4`, L5 `grip_l5`, R4 `grip_r4`, R5 `grip_r5`, Quick access `quick`, Left trackpad `pad_left`, Right trackpad `pad_right` |
| DualSense Edge | `dualsense-edge` | `054c:0df2` | Left Fn `fn_left`, Right Fn `fn_right`, Left back button (LB) `paddle_left`, Right back button (RB) `paddle_right` |
| DualSense | `dualsense` | `054c:0ce6` | none |
| DualShock 4 | `dualshock4` | `054c:05c4`, `054c:09cc`, `054c:0ba0` | none |
| Xbox Elite Series 2 | `xbox-elite` | `045e:0b00`, `045e:0b05`, `045e:0b22`, and any Xbox pad that reports paddles | P1 (right upper) `paddle_p1`, P2 (right lower) `paddle_p2`, P3 (left upper) `paddle_p3`, P4 (left lower) `paddle_p4` |
| Xbox | `xbox` | Every other Microsoft pad (vendor `045e`), and pads named after the Xbox | Share `share` |
| Switch 2 Pro | `switch2-pro` | `057e:2069` | GL `gl`, GR `gr`, C `c`, Capture `capture` |
| Switch Pro | `switch-pro` | `057e:2009`, `057e:2017` | Capture `capture` |
| 8BitDo Pro 3 | `8bitdo-pro-3` | `2dc8:6009` | L4 `paddle_l4`, R4 `paddle_r4`, PL `paddle_pl`, PR `paddle_pr`, Star `star` |
| Controller | `generic` | Any other pad | none |

The [`universe controller`](/docs/reference/cli/controller/) commands take these ids. Every family also has 17 standard buttons: the face buttons `south`, `east`, `north` and `west`, by position, then `lb`, `rb`, `lt`, `rt`, `select`, `start`, `guide` (HOME), `ls` and `rs` (the stick clicks), and `dpad_up`, `dpad_down`, `dpad_left` and `dpad_right`. Each family labels them as its pad prints them.

Some extra buttons depend on the driver:

- The DualSense Edge's four extra buttons reach Linux from kernel 7.2.
- The Elite's paddles come as grip buttons under xpadneo or xone, and under other codes with older drivers. Universe takes whichever the pad reports.
- The 8BitDo Pro 3's Star has reached Linux in no mode seen so far, so its row stays **Unbound**.

A button that sends no code on the pad's current connection reads **Unbound** in Settings › **Controller**. If the pad does have it, [learn it](/docs/controllers/button-setup/#learn-one-button).

## How a pad gets its family

Universe tries, in order:

1. the pad's USB vendor and product, from the table above;
2. a name that fits the model, from the model's vendor ("Xbox" in the name of a Microsoft pad);
3. the vendor alone, so any Microsoft pad is an Xbox pad;
4. the name alone, whatever the vendor;
5. the generic Controller.

The 8BitDo Pro 3 is the only 8BitDo with a family of its own. In X-input mode it gets the Xbox family, and in Switch mode it reads as a Switch Pro.

Learned buttons, macros and the **Nintendo games** choice belong to the family, so every pad of that model follows them.

## Button glyphs

The hint bar names buttons the Xbox way and draws them the way your pad prints them:

| Family | Face buttons | Shoulders | Select, Start, HOME | Stick clicks |
|---|---|---|---|---|
| DualSense, DualSense Edge | ✕ ○ △ □ | L1 R1 L2 R2 | Create, Options, PS | L3 R3 |
| DualShock 4 | ✕ ○ △ □ | L1 R1 L2 R2 | Share, Options, PS | L3 R3 |
| Xbox, Xbox Elite Series 2, Controller | A B X Y | LB RB LT RT | View, Menu, Xbox | LS RS |
| Switch Pro, Switch 2 Pro | B A X Y | L R ZL ZR | Minus, Plus, Home | LS RS |
| 8BitDo Pro 3 | B A X Y | L1 R1 L2 R2 | Minus, Plus, Home | L3 R3 |
| Steam Deck | A B X Y | L1 R1 L2 R2 | View, Menu, Steam | L3 R3 |

On a Nintendo-style pad (Switch Pro, Switch 2 Pro, 8BitDo Pro 3), a hint that names a letter lands on the button printed with it, and the menus go by the print too, so the A on the right confirms and the B at the bottom goes back.

Until a pad is seen, the hints follow the last family Universe saw, else the Xbox one (the Steam Deck's on a Deck). The first-run setup's **Buttons and glyphs** row picks the family shown until a pad is plugged in. The PS5 look draws DualSense glyphs until it sees a pad. Under a keyboard or a mouse, the hints show key caps instead.

## Steam Deck and Steam Input

The Deck's controls are the Steam Deck family whether Universe reads them from the kernel's driver or, under Steam, from Steam Input's virtual pad (`28de:11ff`). Its back buttons L4, L5, R4 and R5 ship with no macros, ready for [yours](/docs/controllers/macros/).

Off a Deck, Universe hides Steam Input's virtual pad while the real pad it copies is connected.

With neither Steam nor InputPlumber running, the kernel starts the Deck's controls in mouse-and-keyboard mode. Hold **Menu** for about half a second to switch them to a gamepad.

[Steam's Game Mode](/docs/getting-started/steam-deck/#steams-game-mode) lists what changes under Steam, macros included.

## Switch 2 Pro Controller

Linux can't read the Switch 2 Pro yet. hid-nintendo's Switch 2 driver isn't merged, so the pad gets no input node. Until the driver lands, the menus don't move with it, and HOME, macros, button setup, the button test and its battery level don't work. Emulators still read it over USB, through SDL.

While one is plugged in by USB, Settings › **System check** shows a failing **Switch 2 Pro Controller** entry, and [`universe doctor`](/docs/reference/cli/doctor/) exits with status 1.

## System setup

Universe needs three things only root can set up:

- `/dev/uinput` open to your session, for **Key combo…** macros, which type through a virtual keyboard named "Universe controller macros";
- `/dev/uhid` open to your session, with the uhid module loaded at boot, for the [Virtual pads](/docs/controllers/emulator-controls/) module;
- the virtual pads' hidraw nodes open to your session, so emulators can read them.

A udev rule, `70-universe.rules`, and a module load, `universe.conf`, cover all three. Each install path puts them in place:

| Install | What it does |
|---|---|
| AUR | The packages install both under `/usr/lib`. `game-devices-udev` is an optional dependency. |
| Fedora (COPR) | The package installs both, and suggests `steam-devices`. |
| install.sh | Asks for your password to put them under `/etc/udev/rules.d` and `/etc/modules-load.d`, reloads udev and loads uhid. It skips this when a distribution package already installed the rule. Without sudo, it prints the commands to run as root, then a reboot. |
| NixOS | `programs.universe.controller.enable`, on by default, turns on uinput, adds the rule and `game-devices-udev-rules` to udev, and loads uhid at boot. It renumbers those rules so they open your pads to your session: added to `services.udev.packages` as shipped, they run too late to. |

A pad's hidraw nodes carry its gyro, its touchpad and some of its extra buttons. Emulators read them through SDL and need them open to your session. Universe's menus and macros work without them.

- `game-devices-udev` opens Sony and Nintendo pads, so a DualShock 4, a DualSense or a Switch Pro needs nothing from Steam. On NixOS, `programs.universe.controller.enable` brings it.
- It opens no hidraw node for an Xbox Series pad on Bluetooth or an 8BitDo Pro 3 in D-input mode. `steam-devices` opens the Xbox pad's (`hardware.steam-hardware.enable` on NixOS). Without a rule that opens it, SDL reads the pad from its `/dev/input` node instead: the emulator still gets it, and an 8BitDo loses its gyro and rumble.

## System check

With `controller.enabled` on in `config.toml` (the default), [Settings › System check](/docs/settings/system-check/) lists these entries under **Controller**, or under **Needs attention** when they fail. `universe doctor` prints the same.

- **Key macros (uinput)** fails when `/dev/uinput` isn't writable. Its fix is game-devices-udev on Arch or steam-devices on Fedora and Debian, then a reboot; on NixOS, `hardware.uinput.enable` and the `uinput` group; elsewhere, a udev rule that gives your session `/dev/uinput`.
- **Controllers** lists the pads Universe reads, with their family and connection. It never fails.
- **Controller buttons** fails when a pad's button sends no code on this connection, and names it. Learn it in Settings › Controller. A button the pad never sends, like the 8BitDo Pro 3's Star, fails here too.
- **Controller extras (hidraw)** appears when a connected pad has hidraw nodes, and fails when one isn't readable. The fix is game-devices-udev on Arch or steam-devices on Fedora and Debian, then replugging the pad. On NixOS, where Sony and Nintendo pads are already open, it is `hardware.steam-hardware.enable` for the others.
- **Switch 2 Pro Controller** appears only while one is on USB, and always fails.

No entry checks `/dev/uhid` or SDL3. When Virtual pads can't use them, the game gets your physical pads and the module logs why.

## Record your pad

`universe controller dump` prints a pad as Universe reads it: its family, connection and kernel driver, the codes its `/dev/input` node reports, its HID report descriptor, and SDL3's view of it, the one emulators get. Pass the pad's id from `universe controller ls`:

```sh
universe controller ls
universe controller dump <id> --walk --json > my-pad.json
```

`--walk` asks you to press each button, move each stick and pull each trigger, and records what the pad sent for each. The project's pad tests replay recordings like this one, one per model and connection ([Controllers by model](/docs/developers/api/#controllers-by-model) lists them), so report a pad Universe reads wrong with its file attached. The [`universe controller`](/docs/reference/cli/controller/) reference has every option.

## Battery levels

In the Universe look, the batteries sit next to the clock: the machine's own when it has one, and one per pad whose charge Universe can read. The pad Settings › Controller shows is green, one at 15% or less and not charging is red, and a charging one carries a bolt. With more than one pad, each also gets a number, the player its own lights show or else the lowest one free. Emulators don't follow it ([Emulator controls](/docs/controllers/emulator-controls/#player-order) sets their player order).

The charge comes from the kernel when the pad's driver reports one. Otherwise Universe reads it from the pad's own reports on an 8BitDo, or from BlueZ for a Bluetooth pad such as an Xbox pad on Bluetooth LE, which reports no charging state. Over a game, the dock's **Controllers** button shows the charge of an 8BitDo or a Bluetooth pad whose battery BlueZ reads.

The PS5 look draws the same badges. The Switch 2 look puts the machine's battery by its clock and each pad's charge on its Controllers page.

## Keyboard, mouse and touch

Every look takes a keyboard, and the Universe look also a mouse and a touchscreen. [Getting around](/docs/using/getting-around/) has the key for each button, and what a click, a tap or a drag does.
