---
title: Play journal
description: After each session, a model writes a journal entry from its recording and your screenshots.
sidebar:
  order: 4
---

After each session, the Play journal has a model write an entry about it, from the session's recording when [Video capture](/docs/add-ons/video-capture/) is on, and from the screenshots you took. A session with neither gets no entry. Entries show in the [Media](/docs/using/media/#the-journal) tab and on the game's Journal page.

## Set it up

Turn it on in Settings › **Features** › **Play journal**, then pick a **Writing model**. Until you do, its row reads **Set it up**.

- **Codex** runs the `codex` command, which has to be installed and signed in (`codex login` in a terminal). Its **Model**, **Latest** by default, follows Codex's newest model.
- **OpenAI-compatible API** sends the request to the **API endpoint** with your **API key**. Pick its **Model** by name, one that reads images: **Latest** works with Codex only.
- **Stub (no model)** writes a placeholder entry, to check that the rest works.

The journal also needs ffmpeg.

## What the model gets

The session's pictures, the game's title, when you played and for how long, the previous entry, and what the journal remembers of the game so far, such as its characters and places. With **Let the model look the game up on the web** on, the model is also asked to search the web for the game.

## Settings

**Entry language**, on Auto, writes in the game's on-screen language. **Write a journal entry after each session** turns the journal off for one game, in its **Game settings**. **Y** shows the advanced rows, such as how hard the model thinks and how long a try may take.

## Retries

An entry that fails on a quota, a sign-out or a timeout is tried again later, never while a game runs. One that can't succeed, as with no API key or an unknown model, fails at once.

On the game's Journal page, **Start** on a session offers **Write the entry**, **Try again now** or **Write it again**, which replaces a written one.
