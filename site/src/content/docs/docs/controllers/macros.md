---
title: Macros and paddles
description: Bind volume, screenshots, MangoHud, key combos or commands to a pad's back paddles and spare buttons, on a press or on a hold.
sidebar:
  order: 3
---

A macro is an action Universe runs when you press or hold a button, such as a back paddle that turns the volume up. A button can carry one macro on press and another on hold. Universe ships a few for pads with spare buttons, and you bind your own in Settings › **Controller**.

A game that reads the same button sees the press too. In emulators, the [Emulator controls](/docs/controllers/emulator-controls/) module keeps HOME, Capture and the back paddles out of the emulator's mapping.

## Actions

| Action | Id | What it does |
|---|---|---|
| Volume up | `volume_up` | Raises the default output by the volume step and unmutes it. Repeats while held. |
| Volume down | `volume_down` | Lowers it by the volume step. Repeats while held. |
| Mute | `mute` | Mutes or unmutes the default output. |
| Screenshot | `screenshot` | Takes a screenshot. Needs the [Screenshots](/docs/add-ons/screenshots/) module, off until you turn it on in [Settings › Features](/docs/settings/features/). |
| Toggle MangoHud | `mangohud` | Shows or hides the running game's MangoHud for this session, leaving its settings alone. |
| Stop the game | `stop` | Rumbles the pad, then quits the game. Hold only. |
| Key combo… | `keys` | Types a key combination, such as `Super_L+F12`. |
| Command… | `command` | Runs a shell command. |

The volume macros go through the sound system (WirePlumber) and type no key, so nothing reaches the game. The new level shows over the game, or in the desktop's own volume popup.

A key combo joins key names with `+`. It takes modifier words (`ctrl`, `shift`, `alt`, `super`), X key names as MangoHud.conf spells them (`Super_L`, `Control_R`), letters, function keys, evdev names such as `KEY_VOLUMEUP`, and `XF86AudioRaiseVolume`, `XF86AudioLowerVolume`, `XF86AudioMute`, `XF86AudioPlay`, `XF86AudioNext` and `XF86AudioPrev`. Universe types it through `/dev/uinput`, holding the keys for 40 ms. Without access to `/dev/uinput`, key combos do nothing, and the [System check](/docs/controllers/supported/#system-check) says so.

A command runs with `sh -c`. While a game runs, it gets `GAME_ID`, `GAME_TITLE`, `SESSION_ID`, `SESSION_UNIT` and `SESSION_SCREEN` in its environment.

## Shipped macros

Universe ships these, all on a press:

| Controller | Button | Action |
|---|---|---|
| DualSense Edge | Left Fn `fn_left` | Screenshot |
| DualSense Edge | Right Fn `fn_right` | Toggle MangoHud |
| DualSense Edge | Left back button (LB) `paddle_left` | Volume down |
| DualSense Edge | Right back button (RB) `paddle_right` | Volume up |
| Xbox Elite Series 2 | P1 (right upper) `paddle_p1` | Toggle MangoHud |
| Xbox Elite Series 2 | P2 (right lower) `paddle_p2` | Volume up |
| Xbox Elite Series 2 | P3 (left upper) `paddle_p3` | Screenshot |
| Xbox Elite Series 2 | P4 (left lower) `paddle_p4` | Volume down |
| 8BitDo Pro 3 | R4 `paddle_r4` | Toggle MangoHud |
| 8BitDo Pro 3 | PR `paddle_pr` | Volume up |
| 8BitDo Pro 3 | PL `paddle_pl` | Volume down |
| Xbox | Share `share` | Screenshot |
| Switch Pro | Capture `capture` | Screenshot |
| Switch 2 Pro | Capture `capture` | Screenshot |

No standard button carries one, and the Steam Deck's back buttons ship empty. The Edge's four buttons need Linux 7.2 or newer, and the Switch 2 Pro's Capture waits on a kernel driver ([Supported controllers](/docs/controllers/supported/#switch-2-pro-controller)).

## Bind a macro

1. Connect the pad and open Settings › Controller.
2. Press **A** on the button's row, then **On press…** or **On hold…**.
3. Pick the action. **Key combo…** and **Command…** open a text field.

The macro holds for every pad of that model. To remove one, pick **Clear press** or **Clear hold** in the same menu.

To bind for a model with no pad connected, use Universe Desktop's Preferences › Controller ([Button setup](/docs/controllers/button-setup/#universe-desktop)).

## Press and hold

A button with only a press macro fires the moment you press it. With a hold macro too, release the button before the hold length (600 ms by default) and the press macro fires on the release; keep holding, and the hold macro fires once at the hold length.

Volume up and Volume down repeat while you hold the button, after 400 ms and then every 100 ms, unless the button also carries a hold macro.

## Rules

- HOME takes no macro. It is how you reach Universe over a game ([Playing](/docs/using/playing/)).
- Stop the game only fires on a hold. **On press…** doesn't offer it.
- From the command line you can also bind a macro for every pad (`*`), on a standard button only. A model's own macro on the same button and trigger wins over it.

## When macros don't fire

Macros fire whichever app has the focus, except:

- With the dock open over a game (the Control Center in the PS5 look), only the volume, mute and MangoHud macros fire.
- While Settings › Controller has the focus, none fire.
- In [Steam's Game Mode](/docs/getting-started/steam-deck/#steams-game-mode), Steam owns the volume, MangoHud and screenshots, so the volume, mute, MangoHud and Screenshot macros do nothing. Key combos, commands and Stop the game still work.

Big Screen runs them while it's open. For a game started from Universe Desktop or the command line, the game's session runs them, as long as `controller.enabled` is on in `config.toml` (the default).

## Timing

Settings › Controller's **Advanced** row opens two timing settings:

- **Hold length (ms)**: 400, 600, 800 or 1000, 600 by default. A press this long is a hold, and a hold macro fires then. It is also how long you hold HOME to leave a game for Universe.
- **Volume step (%)**: 1, 2, 5 or 10, 2 by default, how far one press of a volume macro moves the default output. `controller.volume_step` in `config.toml` takes any value from 1 to 100.

Next to them, the HOME card's **HOME opens Universe** decides what HOME does while another app has the focus: on, it brings Universe to the front; off, it does nothing there.

## From the command line

```sh
universe controller bind <family> <button> <press|hold> <action> [--keys K] [--command C]
universe controller unbind <family> <button> [press|hold]
```

The family and button ids are in [Supported controllers](/docs/controllers/supported/#controller-families). For example:

```sh
universe controller bind dualsense-edge paddle_left hold stop
universe controller bind xbox share press keys --keys Super_L+F12
universe controller bind '*' start hold command --command 'notify-send "Start held"'
universe controller unbind xbox share press
```

Without a trigger, [`universe controller unbind`](/docs/reference/cli/controller/) removes both. `bind` refuses what the menus refuse, and a `*` macro on an extra button, and says why. `universe controller ls` ends with the whole macro list.

In `config.toml`, macros are `[[controller.macros]]` tables, each with a `family`, `button`, `trigger`, `action`, and its `keys` or `command` ([config.toml](/docs/reference/config/)).

## Get the shipped macros back

Until you change a macro, `config.toml` holds none and the shipped ones apply. Your first change writes all of them into the file. To get the shipped set back, remove the list:

```sh
universe config set controller.macros
```

[`universe config set`](/docs/reference/cli/config/) with no value deletes the key; deleting every `[[controller.macros]]` table from `config.toml` by hand does the same. For no macros at all, even the shipped ones, write `macros = []` under `[controller]`.
