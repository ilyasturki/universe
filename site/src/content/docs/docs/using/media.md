---
title: Media
description: The Media tab, screenshots, recordings and the play journal, a game's own media pages, and the commands that list and remove them.
sidebar:
  order: 5
---

Media, the third tab, holds the screenshots, recordings and journal entries of all your games. Each comes from a feature that stays off until you turn it on in Settings › **Features**:

- [Screenshots](/docs/add-ons/screenshots/) takes the pictures.
- [Video capture](/docs/add-ons/video-capture/) records each session with gpu-screen-recorder.
- [Play journal](/docs/add-ons/play-journal/) has a model write an entry after each session, from its recording and your screenshots.

Each feature's page has what it needs and its settings.

## The Media tab

The grid runs newest first, every game mixed in, with no sort or filter.

- **A** on a screenshot shows it full screen, and Left and Right step through the others.
- **A** on a recording plays it on the game's Recordings page.
- **A** on a journal entry opens it on the game's Journal page.

**X** opens the game's page, and **Y** jumps to the journal entry of a screenshot or a recording that has one. On a screenshot, **Start** › **Remove screenshot…** sends it to the trash, after a question. **B** returns to Home.

## Screenshots

[Playing](/docs/using/playing/#screenshots) says how to take them. Each one lands in the game's folder as `~/.local/share/universe/games/<id>/screenshots/YYYYMMDD-HHMMSS.png`, and a PNG or JPEG you copy there under such a name shows up too. One taken with no game running goes to `~/.local/state/universe/screenshots/`, which no list shows. Removing a screenshot also takes it out of the journal entry that shows it.

## Recordings

Video capture records every session, the whole screen by default, and files each one under `~/Videos/universe/<id>/`. A recording shorter than four minutes goes to the trash; **Shortest recording kept**, an advanced setting of the feature, changes that. Settings › Game defaults › **Folders** › **Recordings**, an advanced row, moves the folder.

A game's **Recordings** page plays them. Left and Right seek 10 s, the right stick scrubs and **X** goes full screen. **Start** › **Remove recording…** offers **Trash the recording** alone or **Trash it and its journal entry**, and the hours played stay either way.

Removing a game from the library moves its recordings to an archive folder ([Removing a game](/docs/using/game-page/#removing-a-game)).

## The journal

Play journal writes an entry after each session from its recording and your screenshots; a session with neither gets no entry. Nothing happens until you pick its **Writing model** in the feature's settings. [Play journal](/docs/add-ons/play-journal/) lists the models and what is sent to them.

An entry holds a title, a few paragraphs, a **NEXT UP** line on where you left off, the recording and the screenshots. One put off is tried again later, never while a game is running.

A game's **Journal** page lists every session you played:

- **A** reads a written entry, writes a missing one (**Write it**), or shows **Being written** while the model works.
- **Start** adds **Write it again**, **Try again now** for an entry that failed, **Recording**, and **Remove entry…** (**Cancel the writing…** while it's being written). Removing an entry trashes it and its frames, and keeps your screenshots.

## A game's own media

A game's own **Screenshots**, **Recordings** and **Journal** open from its page's **Media** button and its game menu's **Media ›** ([A game's page](/docs/using/game-page/)), or from the dock over the running game ([Playing](/docs/using/playing/#the-dock)). While the game runs, its Screenshots page puts this session's first.

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
