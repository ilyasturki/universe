---
title: Media
description: The Media tab, screenshots, recordings and the play journal, a game's own media pages, and the commands that list and remove them.
sidebar:
  order: 5
---

Media, the third tab, holds the screenshots, recordings and journal entries of all your games in one grid. Universe makes none of them out of the box. Each comes from a feature that stays off until you turn it on in Settings › **Features**:

- **Screenshots** takes the pictures.
- **Video capture** records each session with gpu-screen-recorder.
- **Play journal** has a model write an entry after each session, from its recording and your screenshots.

[Screenshots](/docs/add-ons/screenshots/), [Video capture](/docs/add-ons/video-capture/) and [Play journal](/docs/add-ons/play-journal/) each have a page with what the feature needs and its settings.

## The Media tab

The grid runs four to a row, newest first, every game mixed in, with no sort or filter. The title above it names the focused item's game. Until you have something, it reads "Screenshots, recordings and journal entries land here as you play."

- A screenshot is the picture itself. **A** (**View**) shows it full screen, and Left and Right step through the others.
- A recording shows a frame under a play button, with the session's length. **A** (**Play**) opens it on the game's Recordings page.
- A journal entry shows its heading, the start of its text and the session's length. **A** (**Read**) opens it on the game's Journal page.

**X** opens the game's page, and **Y** jumps to the journal entry of a screenshot or a recording that has one. **Start** lists the same actions, with **Details** for the game's page, and on a screenshot **Remove screenshot…**, which asks before it sends the file to the trash. **B** returns to Home.

## Screenshots

You take them from the dock, with a pad button or with `universe screenshot` ([Playing](/docs/using/playing/#screenshots) has the details). Each one lands in the game's folder as a PNG named by the date and time, `~/.local/share/universe/games/<id>/screenshots/YYYYMMDD-HHMMSS.png`. A PNG or JPEG you copy there under such a name shows up as well. Removing a screenshot sends it to the trash and takes it out of the journal entry that shows it.

## Recordings

Video capture records every session, the whole screen by default, into an MKV file. The recording pauses while the game is frozen behind the dock, so the menus stay out of the video, and the dock shows **REC** with the time while it runs. At the end of the session, a recording shorter than four minutes goes to the trash; **Shortest recording kept**, an advanced setting of the feature, changes that. The others are filed under `~/Videos/universe/<id>/`, one file per session. Settings › Game defaults › **Folders** › **Recordings**, an advanced row, moves that folder.

A game's **Recordings** page plays them. **A** plays and pauses, Left and Right seek 10 s (faster while held), the right stick scrubs and **X** goes full screen. The session's journal entry sits under the video. **Start** › **Remove recording…** offers **Trash the recording** alone or **Trash it and its journal entry**, and the hours played stay either way.

Removing a game from the library moves its recordings to an archive folder ([Removing a game](/docs/using/game-page/#removing-a-game)).

## The journal

With Play journal on, a model writes an entry after each session from its recording, when Video capture is on, and from the screenshots you took. A session with neither gets no entry. Nothing happens until you pick its **Writing model** in the feature's settings, one of **Codex**, an **OpenAI-compatible API** and **Stub (no model)**. What goes to the model is the session's screenshots and frames from its recording, the game's title, the session's times, the previous entry and a short memory the journal keeps for each game.

An entry holds a title, a few paragraphs, a **NEXT UP** line on where you left off and what to do next, the recording and the screenshots. A message says when an entry is being written, and another when it's done, put off or failed. One put off is tried again later, never while a game is running.

A game's **Journal** page lists every session you played:

- **A** reads a written entry, writes a missing one (**Write it**), or shows **Being written** while the model works.
- **Start** adds **Write it again**, **Try again now** for an entry that failed, **Recording**, and **Remove entry…** (**Cancel the writing…** while it's being written). Removing an entry trashes it and its frames, and keeps your screenshots.

## A game's own media

On a game's page, **Media** opens its screenshots, recordings or journal, and shows only once the game has one of them. The game menu's **Media ›** lists the same pages, **Screenshots**, **Recordings** and **Journal**, each one only when it holds something. While the game is running, its Screenshots page puts this session's first. The screenshots on the game's page itself are the store's pictures, not yours.

Over the running game, the dock's **Game** card opens its **Journal** and **Recordings**, and the tray under the dock holds its screenshots.

## From the command line

- [`universe screenshots`](/docs/reference/cli/screenshots/) lists your screenshots, newest first, of one game or of all; `--remove` trashes one.
- [`universe recordings`](/docs/reference/cli/recordings/) lists a game's recordings; `--remove` trashes a session's recording and keeps its hours.
- [`universe journal`](/docs/reference/cli/journal/) lists a game's entries. `--write` writes a session's entry now, `--force` with it replaces the one it has, `--retry` starts the oldest entry waiting for another try, and `--remove` trashes one.
- `universe module enable screenshot`, `capture` or `journal` turns a feature on ([`universe module`](/docs/reference/cli/module/)).

`universe media` handles artwork, not captures ([Artwork](/docs/library/artwork/)).

## In the other looks

The Switch 2 look keeps screenshots and recordings in its **Album**, with **Show**, **Filter** and **Sort** in the rail, and the journal in **News**. A game's **Software Options** open both filtered to that game.

The PS5 look's **Media** tab holds **Media Gallery**, with **All**, **Screenshots**, **Recordings** and **Journal** tabs and a sort and a filter by game, then **Journal**, then a card for each game you captured.

Universe Desktop has a **Media** page in its sidebar ([Universe Desktop](/docs/using/universe-desktop/)).
