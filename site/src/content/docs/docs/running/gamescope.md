---
title: Gamescope, HDR and frame rate
description: How games run inside gamescope, which display settings reach them, and how HDR, MangoHud and the frame rate limit work.
sidebar:
  order: 3
---

Games run inside gamescope by default. It gives a game a screen of its own at the resolution and refresh rate you pick and scales the picture to fit yours, and the MangoHud overlay draws inside it. Without gamescope installed, a game opens on the desktop, and System check tells you so.

## Two cases

Universe Big Screen, fullscreen or as [the Universe session](/docs/getting-started/universe-session/), starts inside a gamescope of its own whenever gamescope is installed, and every game it launches lands on that one. This gamescope is built from the defaults alone, read as Universe starts: Settings › **Game defaults** › **Display** with its scaling rows and **Gamescope arguments**, and **HDR** from Settings › **Emulators & Proton** › **Proton**. A change to them applies from Universe's next start. A game's own resolution, refresh rate, scaler, filter, sharpness, adaptive sync and gamescope arguments don't reach it, and neither does its **Gamescope** switch.

A game started outside it, by [`universe play`](/docs/reference/cli/play/) in a terminal, from Universe Desktop or from Big Screen run with `--windowed`, gets a gamescope of its own, built from the game's values over the defaults:

```text
gamescope -f -W <screen width> -H <screen height> -w <game width> -h <game height> -r <refresh>
    [-S <scaler>] [-F <filter>] [--sharpness <n>] [--adaptive-sync]
    [Gamescope arguments] [--mangoapp] [--hdr-enabled]
    -- universe splash -- <the game's command>
```

Here the game's **Gamescope** switch decides. A runner's page can also turn gamescope off for all of its games, which helps with an emulator that misbehaves under it.

If gamescope fails to start, Big Screen opens on the desktop instead, and the Universe session, with no desktop to fall back to, ends. Under Steam's Game Mode, games land on Steam's gamescope, and Steam keeps the overlay and the frame rate limit (see [Steam's Game Mode](/docs/getting-started/steam-deck/#steams-game-mode)).

## Resolution and refresh rate

Both sit in the **Display** card, whose heading shows the screen Universe read, such as `DP-1 3840×2160 @ 60 Hz`.

- **Resolution** is what the game renders at. `auto` is the screen's own; a lower one is upscaled to the screen and lighter on the GPU. The choices follow your screen, with its own size, then the 2160, 1800, 1440, 1080 and 720-line sizes below it, at its shape.
- **Refresh rate** is the rate the game sees: `auto` for the screen's, else the screen's own rate or one of the usual rates below it.
- **Adaptive sync** is variable refresh (FreeSync, G-Sync). `auto` turns it on when the screen supports it, and `on` or `off` force it.

The resolution and refresh pickers end on **Type a value…**. When Universe can't read the screen, System check's **Screen resolution** fails, and gamescope stays at its own 1280×720 unless you set a resolution.

## Scaling

With Show advanced (**Y**), the Display card adds the scaling rows:

- **Scaler** sets how a smaller picture fills the screen. `integer` keeps pixels whole and `fit` keeps the aspect, while `fill` and `stretch` don't. Unset, gamescope picks (`auto`).
- **Filter** is the upscaling filter. `fsr` and `nis` sharpen, and `nearest` and `pixel` keep pixel art crisp. Unset, it's `linear`.
- **Sharpness** applies to `fsr` and `nis`, from 0, the sharpest, to 20, the softest. Unset, it's gamescope's 2.
- **Gamescope arguments** takes any other gamescope flag. They come after the fields above, and since gamescope takes the last of a repeated flag, yours win. A game's own come after the default ones.
- **Discrete GPU**, on by default, makes the game and gamescope render on the stronger GPU of a laptop that has two, rather than on the one driving the screen. With a single GPU it changes nothing.

The gamescope binary itself is **Gamescope program**, in the **Programs** card of Game defaults.

## HDR

**HDR**, off by default, sends HDR to an HDR screen in games that support it. It sets Proton's `PROTON_ENABLE_HDR` for the game and starts gamescope with `--hdr-enabled`.

The default is on Settings › Emulators & Proton › **Proton**, and a Proton game's own switch is in the **Proton** card of its Game settings. First-run setup asks too, in its **Controller and screen** step (the **Graphics** card), when config.toml is writable.

In the fullscreen app, gamescope outputs HDR only when the default is on as Universe starts. A game's own switch then only sets Proton's variable for that game.

## MangoHud

**Show MangoHud**, in the **Overlay** card and off by default, draws the frame rate, frame time, and GPU and CPU load over the game from launch. Off keeps MangoHud loaded but hidden, so the frame rate limit still holds.

Inside gamescope, mangoapp draws the overlay. It comes with MangoHud, except on Debian and Ubuntu, which package it apart as `mangoapp`. Without it, MangoHud's layer in the game draws the overlay there, in Vulkan games only. Your `~/.config/MangoHud/MangoHud.conf` still sets the layout, since Universe copies its lines into its own files in `~/.local/state/universe/`, minus `fps_limit`, `no_display` and `control`, which it writes itself.

During a game, HOME › **Performance** › **MangoHud** shows or hides it. So does the **Toggle MangoHud** pad macro, set out of the box on a DualSense Edge's **Right Fn**, an Xbox Elite Series 2's **P1 (right upper)** paddle and an 8BitDo Pro 3's **R4** (see [Macros and paddles](/docs/controllers/macros/)). Either way the change lasts for the running game, and the next launch starts from the setting again.

System check has two rows for it. **MangoHud in 32-bit games** looks for MangoHud's 32-bit layer, without which a 32-bit game gets neither the overlay nor the limit. **HUD inside Gamescope (mangoapp)** looks for mangoapp.

## Frame rate limit

**Frame rate limit**, in the **Overlay** card, holds the game to that many frames per second through MangoHud, with the overlay shown or not.

- `auto`, the default, is the refresh rate the game sees. An emulator gets no limit on `auto`, since it paces itself, and neither does any game when Universe couldn't read the screen, unless its gamescope has a refresh rate set.
- `none` turns the limit off.
- A number sets it.

With a limit, Linux games and emulators start through the `mangohud` command, so OpenGL games are limited too. Proton and Wine games get MangoHud's Vulkan layer alone. Without MangoHud there is no limit, and System check's **Frame rate limit (MangoHud)** fails. Install MangoHud, or turn the limit off for every game to stop the check:

```sh
universe config set launch.fps_limit none
```

## What HOME changes for the running game

Press HOME over a game and open **Performance** on the dock (more on the dock in [Playing](/docs/using/playing/)):

- **MangoHud** reads **Shown** or **Hidden**.
- **FPS limit** reads "Auto · 144" (what `auto` comes to), **None** or a number.
- **Filter** steps through **Default**, **Linear**, **Nearest**, **FSR**, **NIS** and **Pixel**.
- **Sharpness** appears while the filter is FSR or NIS, from "0 · sharpest" to "20 · softest".

These four change the running game only. Nothing is saved, and the next launch starts from the settings again. Filter and Sharpness act on the fullscreen app's gamescope, which goes back to the default filter when the game ends.

**Pause on HOME**, in the same card, is saved as the game's own value. Under it the dock reads "For Dead Cells only" on Dead Cells, and **Y** (**All games**) gives that value to every game without its own. Under Steam's Game Mode there is no dock, and Show MangoHud, Frame rate limit and Pause on HOME leave the settings pages, since Steam owns all three.
