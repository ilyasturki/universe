---
title: Play journal
description: After each session, the Play journal has a model write an entry from its recording and your screenshots. What it needs, its settings, its retries.
sidebar:
  order: 4
---

After each session, the Play journal (`journal`) has a model write an entry about it, from frames of the session's recording when [Video capture](/docs/add-ons/video-capture/) is on, and from the screenshots you took. A session with neither gets no entry.

## An entry

An entry has a title, a few paragraphs on the session, a "next up" line on where to pick up, and pictures from the session. It shows in the [Media](/docs/using/media/) tab and on the game's Journal page, and lives in `~/.local/share/universe/games/<id>/journal/<session>.json`.

The model also reads the previous entry and the journal's memory of the game (its characters, places and bosses so far), kept in `~/.local/share/universe/modules/journal/memory/`.

## What it needs

- A recording or screenshots of the session, from [Video capture](/docs/add-ons/video-capture/), [Screenshots](/docs/add-ons/screenshots/), or both.
- ffmpeg.
- A **Writing model**, which has no default:
  - **Codex** runs the `codex` command, which has to be installed and signed in (`codex login` in a terminal). Without `codex`, the module reads Unavailable. Leave **Model** empty for Codex's own default.
  - **OpenAI-compatible API** sends the request to **API endpoint**, at `/chat/completions`, with the **API key** or the key in **API key file**. It needs a **Model** that reads images, since the pictures go inline in the request.
  - **Stub (no model)** writes a placeholder entry without asking any model, to check that the rest works.

The provider gets the session's pictures, the game's title, when you played and for how long, the previous entry and the memory of the game. With **Let the model look the game up on the web** on, the model is also asked to search the web for the game.

## Set it up

Until a writing model is chosen, the module does nothing. Its row in Settings › **Features** reads **Set it up**, and [System check](/docs/settings/system-check/) has a **Writing model** line. With Codex chosen, System check also has a **Codex sign-in** line.

From a terminal, with [`universe module`](/docs/reference/cli/module/):

```sh
universe module enable journal
universe module set journal provider=codex
```

## Settings

**Write a journal entry after each session** and **Entry language** belong to each game, on its **Game settings** page under **Play journal**. The module's page holds the rest, and the language for every game in its **Game defaults** card.

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

While the model works, the entry shows as being written. Then it's written, put off, or failed for good.

It's put off when the provider hits a quota, Codex is signed out, the endpoint times out or errors, the model answers nothing usable, or the recording sits on a drive that isn't mounted. Universe tries again 15 minutes later, then an hour after that, and gives up after the third failed run. A quota or a Codex sign-out doesn't count as a try. Universe waits for the quota to reset, or 15 minutes after a sign-out.

It fails at once when waiting won't help, as with no API key, an unknown model, a missing program, or a session left with no picture to write from. An entry stuck in writing for 30 minutes is listed as failed.

Universe retries one entry at a time, and never while a game runs.

To ask for a try yourself, press **Start** on the session in the game's Journal page, for **Write the entry**, **Try again now** or **Write it again** (which replaces a written one). The PS5 look's Journal page has the same three; the Switch 2 look can't write or retry an entry.

From a terminal, with a session id that [`universe sessions`](/docs/reference/cli/sessions/) lists for the game:

```sh
universe journal "Dead Cells" --write 20261010-201500           # write or retry that session's entry
universe journal "Dead Cells" --write 20261010-201500 --force   # write it again over the old one
universe journal --retry                                        # start the oldest entry still owed
```

[`universe journal`](/docs/reference/cli/journal/) lists every option.
