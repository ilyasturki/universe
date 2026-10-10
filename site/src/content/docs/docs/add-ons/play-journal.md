---
title: Play journal
description: After each session, the Play journal has a model write an entry from its recording and your screenshots. What it needs, its settings, its retries.
sidebar:
  order: 4
---

After each session, the Play journal (`journal`) has a model write an entry about it. The model works from frames of the session's recording, when [Video capture](/docs/add-ons/video-capture/) is on, and from the screenshots you took. A session with neither gets no entry.

## An entry

An entry has a title, a few paragraphs on the session, a "next up" line on where to pick up, and pictures from the session. It shows in the [Media](/docs/using/media/) tab and on the game's Journal page, and lives in the game's folder as `~/.local/share/universe/games/<id>/journal/<session>.json`.

Before writing, the model reads the previous entry and the journal's memory of the game (its characters, places and bosses so far), so each entry picks up where the last one stopped. That memory is kept per game in `~/.local/share/universe/modules/journal/memory/`.

## What it needs

- A recording or screenshots of the session, from [Video capture](/docs/add-ons/video-capture/), [Screenshots](/docs/add-ons/screenshots/), or both.
- ffmpeg.
- A **Writing model**, which has no default:
  - **Codex** runs the `codex` command, which has to be installed and signed in (`codex login` in a terminal). Without `codex`, the module reads Unavailable. Leave **Model** empty to use Codex's own default.
  - **OpenAI-compatible API** sends the request to **API endpoint** (`https://api.openai.com/v1` unless you change it), at `/chat/completions`, with the **API key** or the key in **API key file**. It needs a **Model**, and since the pictures go inline in the request, that model has to read images.
  - **Stub (no model)** writes a placeholder entry without asking any model. It shows whether the rest works.

The provider gets the session's pictures, the game's title, when you played and for how long, the previous entry and the memory of the game. With **Let the model look the game up on the web** on, the model is also asked to search the web for the game.

## Set it up

Turned on with no writing model chosen, the module does nothing. Its row in Settings › **Features** reads **Set it up**, with "On, waiting on writing model" under it, and [System check](/docs/settings/system-check/) has a **Writing model** line: "not chosen yet: the Play journal module does nothing until it is". In the Universe look, turning the module on puts the cursor on **Writing model**.

From a terminal, with [`universe module`](/docs/reference/cli/module/):

```sh
universe module enable journal
universe module set journal provider=codex
```

With Codex chosen, System check also has a **Codex sign-in** line, which says "run codex login in a terminal" when Codex is signed out.

## Settings

**Write a journal entry after each session** and **Entry language** belong to each game, on its **Game settings** page under **Play journal**. The module's page sets the language for every game in its **Game defaults** card, and holds the rest.

| Setting | Default | |
| --- | --- | --- |
| **Write a journal entry after each session** | On | Per game. |
| **Entry language** | Auto | Auto writes in the game's on-screen language. Or French, English, Spanish, German, Italian, Portuguese, Japanese. |
| **Writing model** | none | Codex, OpenAI-compatible API, Stub (no model). |
| **Model** | empty | The model the provider is asked for. The choices come from the provider. |
| **API endpoint** | `https://api.openai.com/v1` | For the OpenAI-compatible API. |
| **API key** | empty | For the OpenAI-compatible API. |
| **API key file** | empty | A file read in place of the API key. |

Behind Advanced:

| Setting | Default | |
| --- | --- | --- |
| **Let the model look the game up on the web** | On | |
| **Reasoning effort** | High | Minimal, Low, Medium, High. |
| **Tries per session** | 2 | How many times one run asks the model before the entry is put off. |
| **Seconds a single try may take** | 900 | |
| **Maximum images sent to the model** | 40 | |
| **Frame size** | 1568 | The long edge of the frames taken from the recording, in pixels. |
| **Prompt file** | empty | Its text replaces the built-in writing prompt. |

## Retries

The entry shows as being written while the model works, and the Universe and PS5 looks say "Journal: writing" and the game's title. Then the entry is written, put off, or failed for good.

It's put off when the provider hits a quota, Codex is signed out, the endpoint times out or errors, the model answers nothing usable, or the recording sits on a drive that isn't mounted. Universe tries again 15 minutes later, then an hour after that, and the third failed run gives up with "gave up after 3 tries". A quota and a Codex sign-out don't count as tries. Universe waits for the quota to reset, or 15 minutes after a sign-out, then tries again.

It fails at once when waiting won't help, as with no API key, an unknown model, a missing program, or a session left with no picture to write from. An entry stuck in writing for 30 minutes is listed as failed, "timed out".

Universe retries one entry at a time, and never while a game runs.

To ask for a try yourself, open the game's Journal page and press **Start** on the session for **Write the entry** (a session that has none), **Try again now** (one that was put off or failed) or **Write it again** (to replace a written one). The PS5 look's Journal page has the same three; the Switch 2 look can't write or retry an entry.

From a terminal, with a session id that [`universe sessions`](/docs/reference/cli/sessions/) lists for the game:

```sh
universe journal "Dead Cells" --write 20261010-201500           # write or retry that session's entry
universe journal "Dead Cells" --write 20261010-201500 --force   # write it again over the old one
universe journal --retry                                        # start the oldest entry still owed
```

[`universe journal`](/docs/reference/cli/journal/) lists every option.
