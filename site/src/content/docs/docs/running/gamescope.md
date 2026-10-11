---
title: Gamescope, HDR and frame rate
description: How games run inside gamescope, which display settings reach them, and how HDR, MangoHud and the frame rate limit work.
sidebar:
  order: 3
---

Games run inside gamescope by default. It gives a game a screen of its own at the resolution and refresh rate you pick, scales the picture to yours, and draws the MangoHud overlay. Without gamescope installed, a game opens on the desktop, and System check says so.

## Two cases

Universe Big Screen, fullscreen or as [the Universe session](/docs/getting-started/universe-session/), starts inside a gamescope of its own whenever gamescope is installed, and every game it launches lands on that one. That gamescope is built from the defaults alone, read as Universe starts: Settings › **Game defaults** › **Display** with its scaling rows and **Gamescope arguments**, and **HDR** from Settings › **Emulators & Proton** › **Proton**. A change to them applies from Universe's next start. A game's own resolution, refresh rate, scaler, filter, sharpness, adaptive sync, gamescope arguments and **Gamescope** switch don't reach it.

A game started outside it, by [`universe play`](/docs/reference/cli/play/), from Universe Desktop or from Big Screen run with `--windowed`, gets a gamescope of its own, built from the game's values over the defaults:

```text
gamescope -f -W <screen width> -H <screen height> -w <game width> -h <game height> -r <refresh>
    [-S <scaler>] [-F <filter>] [--sharpness <n>] [--adaptive-sync]
    [Gamescope arguments] [--mangoapp] [--hdr-enabled]
    -- universe splash -- <the game's command>
```

There the game's **Gamescope** switch decides, and a runner's page can turn gamescope off for all of its games, for an emulator that misbehaves under it.

If gamescope fails to start, Big Screen opens on the desktop instead, and the Universe session ends. Under [Steam's Game Mode](/docs/getting-started/steam-deck/#steams-game-mode), games land on Steam's gamescope, and Steam keeps the overlay and the frame rate limit.

## Resolution and refresh rate

Both sit in the **Display** card, whose heading shows the screen Universe read, such as `DP-1 3840×2160 @ 60 Hz`.

- **Resolution** is what the game renders at, upscaled to the screen. `auto` is the screen's own; the choices are the screen's size and the usual sizes below it, at its shape.
- **Refresh rate** is the rate the game sees: `auto` for the screen's, else the screen's own rate or one of the usual rates below it.
- **Adaptive sync** is variable refresh (FreeSync, G-Sync). `auto` turns it on when the screen supports it, and `on` or `off` force it.

When Universe can't read the screen, System check's **Screen resolution** fails, and gamescope stays at 1280×720 unless you set a resolution.

## Scaling

With Show advanced (**Y**), the Display card adds:

- **Scaler** sets how a smaller picture fills the screen. `integer` keeps pixels whole and `fit` keeps the aspect, while `fill` and `stretch` don't. Unset, gamescope picks (`auto`).
- **Filter** is the upscaling filter. `fsr` and `nis` sharpen, and `nearest` and `pixel` keep pixel art crisp. Unset, it's `linear`.
- **Sharpness** applies to `fsr` and `nis`, from 0, the sharpest, to 20, the softest. Unset, it's gamescope's 2.
- **Gamescope arguments** takes any other gamescope flag. Yours come after the fields above and a game's after the defaults', and gamescope takes the last of a repeated flag.
- **Discrete GPU**, on by default, renders the game and gamescope on the stronger GPU of a laptop that has two, rather than on the one driving the screen.

The gamescope binary itself is **Gamescope program**, in the **Programs** card of Game defaults.

## HDR

**HDR**, off by default, sends HDR to an HDR screen in games that support it. It sets Proton's `PROTON_ENABLE_HDR` for the game and starts gamescope with `--hdr-enabled`.

The default is on Settings › Emulators & Proton › **Proton**, and a Proton game's own switch is in the **Proton** card of its Game settings. First-run setup asks too, in its **Controller and screen** step (the **Graphics** card), when config.toml is writable. In the fullscreen app, where only the default reaches gamescope, a game's own switch only sets Proton's variable.

## MangoHud

**Show MangoHud**, in the **Overlay** card and off by default, draws the frame rate, frame time, and GPU and CPU load over the game. Off keeps MangoHud loaded but hidden, so the frame rate limit still holds.

Inside gamescope, mangoapp draws the overlay. It comes with MangoHud, except on Debian and Ubuntu, which package it apart as `mangoapp`. Without it, MangoHud's layer draws the overlay in the game, in Vulkan games only. Your `~/.config/MangoHud/MangoHud.conf` still sets the layout, since Universe copies its lines into its own files in `~/.local/state/universe/`, minus `fps_limit`, `no_display` and `control`, which it writes itself.

During a game, HOME › **Performance** › **MangoHud** or the **Toggle MangoHud** pad macro ([Macros and paddles](/docs/controllers/macros/)) shows or hides it until the game ends.

System check's **MangoHud in 32-bit games** looks for MangoHud's 32-bit layer, without which a 32-bit game gets neither the overlay nor the limit, and **HUD inside Gamescope (mangoapp)** looks for mangoapp.

## Frame rate limit

**Frame rate limit**, in the **Overlay** card, holds the game to that many frames per second through MangoHud.

- `auto`, the default, is the refresh rate the game sees. An emulator gets no limit on `auto`, since it paces itself, and neither does any game when Universe couldn't read the screen, unless its gamescope has a refresh rate set.
- `none` turns the limit off.
- A number sets it.

With a limit, Linux games and emulators start through the `mangohud` command, so OpenGL games are limited too. Proton and Wine games get MangoHud's Vulkan layer alone. Without MangoHud there is no limit, and System check's **Frame rate limit (MangoHud)** fails. Install MangoHud, or turn the limit off for every game to stop the check:

```sh
universe config set launch.fps_limit none
```

## What HOME changes for the running game

HOME › **Performance** sets **MangoHud**, **FPS limit**, **Filter** and **Sharpness** for the running game only, and the next launch starts from the settings again ([Playing](/docs/using/playing/#mangohud-the-frame-rate-limit-and-scaling)). Filter and Sharpness act on the fullscreen app's gamescope, which goes back to the default filter when the game ends. **Pause on HOME**, in the same card, is saved as the game's own value ([Pause on HOME](/docs/using/playing/#pause-on-home)).
