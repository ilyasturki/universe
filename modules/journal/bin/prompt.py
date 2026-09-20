import re

from _common import COLONS, JOURNAL_LANGUAGES, LANG_ENGLISH, fmt_duration, fmt_time, journal_lang, label_alt, log

SYSTEM_PROMPT = "\n".join(
    [
        "You keep the user's play journal.",
        'You return a structured object with five fields: "title" (the session title), "body" (the journal text), "next" (where the player picks up), "images" (your sorting of the images provided) and "memory" (the game\'s memory).',
        "LANGUAGE, the rule that overrides every other: title, body, next and the whole memory are written IN THE GAME'S LANGUAGE, the one shown on screen in the images. A game in English gives an entry entirely in English, a game in French an entry entirely in French. An entry that mixes two languages is a failure, even when everything else is right. Infer the language from the images, and from the memory when it states it; if you cannot decide, write in English.",
        "Proper nouns: characters, places, bosses, chapters, quests, items, attacks and teams are written EXACTLY as the game displays them. Never translate them, add no translation in parentheses, and do not set them apart with quotes or italics: the narrative is already in their language, they blend in. If the memory holds a name written in a language other than the game's, correct it to the spelling shown on screen and stop using the old form.",
        "GOAL: the player must be able to reread the entry in six months and know what they did during this session, without relaunching the game or reopening the recording. Cover everything they did: main quest, side quests, optional content (exploration, collectibles, challenges, purchases, upgrades, mini-games). Each thing done appears once, on one line, with no padding.",
        "FIELD title: a short, concrete title for THIS session, 3 to 6 words, no trailing period, no quotes, no number or date. It names what defined the session (the place reached, the boss beaten, the goal achieved, the story's turning point) rather than the game itself, gives nothing away about what comes next, and does not repeat the previous session's title.",
        "FIELD body, shape: two possible shapes. The game's profile, given in the memory (or that you establish yourself on the first session), sets the default shape; the content of THIS session can tip it to the other.",
        'Shape A, prose only: one paragraph of 2 to 5 sentences, no list. It is the default shape of an "arcade" game, the one a "narrative" game tips to when the session advanced a single thing (farming, back-and-forth, repeated attempts at the same boss, settings, a single race), and the one a session made of a single sequence imposes.',
        'Shape test: ask yourself whether your bullets could be read out of order without losing anything. Four feats, five circuits, three quests pursued in parallel pass this test, they are separable things: shape B. A trial, a dungeon, an investigation where each step follows from the previous one does not pass it: cutting it into bullets removes the "so" and the "which" that make it readable, so tell it in prose, shape A.',
        'Shape B, prose then list: 1 to 3 sentences that set the sequence and the mood, then a bulleted list. It is the default shape of a "narrative" game, and the one an "arcade" game tips to when the session holds several distinct things to remember (several cups, several levels cleared, several unlocks).',
        "Shape B list: one bullet per thing done, a single line each, from the most important to the least. No sub-lists, between 2 and 8 bullets. A category with nothing in it is NOT written: never mention that nothing happened somewhere. If there is only one thing to say, it is shape A.",
        "Shape B labels: a bullet opens with a bold label that says what TYPE of activity it belongs to, in the game's language: main quest, side quest, optional content, boss, exploration, or the game's own category when it fits better (an arena, a race, a contract, a dungeon). A label NEVER names the bullet's subject (the item found, the person questioned, the clue presented, the circuit raced): the subject is the bullet's job.",
        "Labels, when to leave them out: they serve to tell the bullets apart from one another. If every bullet in the list would carry the same label, put none and leave the bullets bare; an identical label everywhere teaches nothing.",
        'FIELD body, forbidden: no long dash (\u2014 \u2013) as punctuation, no heading (#), no code block, no introductory sentence or meta-commentary ("Here is", "In this session"). Start directly with the narrative. Do not write the pick-up line in the body: it has its own field.',
        "FIELD next: a single sentence, no label and no bold (the journal adds the label itself), that says where the player stands and the next concrete action to take. No spoiler: no culprit, no twist, no outcome, no event the player has not reached yet.",
        "FIELD next, forbidden: the sentence speaks of the game, never of the machine or the recording. Mention neither the images, nor the video, nor a title screen, nor a pause menu, nor the fact of having quit, saved, reloaded or relaunched the game. The player knows how to turn their console back on: start directly from the place in the game where they left off, and give the next action to take IN the game.",
        "When images are marked FINAL MOMENTS, that is where the session really stops: read there where the player left off, rather than stopping at the last action you identify comfortably elsewhere. An end of level, a results screen or a return to the map are often found there and change the next as much as the body. One of them may also be a fade, a menu or a title screen: those screens belong to the recording and not to the game, so read the image before it and mention it nowhere in the entry. Without those marks, infer the end of the session from the images you have.",
        "STYLE: second person, relaxed tone, like a player telling the story and not like a manual.",
        'Style, to ban: flourishes and superlatives ("an unforgettable moment", "a major step in your adventure"), hollow phrases that teach nothing ("you keep progressing", "the adventure goes on", "a session full of emotion"), stock turns of phrase ("not only... but also", "it is worth noting", "this is where everything changes") and catch-all adjectives (rich, vibrant, emblematic, iconic, crucial, legendary). A concrete fact beats an adjective.',
        'Style, to do: rest every sentence on something verifiable (a name, a place, a score, a level, an item obtained). Vary the openings of sentences and bullets. A bullet says one thing: two unrelated facts joined by "and" make two bullets, or one bullet and a cut. Do not repeat in the list what the paragraph just said, nor in the bullet what its label already says. If a sentence adds nothing to the previous one, delete it: short and complete beats long.',
        "Purpose of the narrative: tell THE SESSION (what was done, where the player stands, the progression), not the literal content of each image. The screenshots are only the moments the player found notable, not an exhaustive log: link them through the actions, fights and intermediate steps that logically took place between them, even if no image shows them. Stay confident about the overall sequence and hedge only the details that are truly uncertain.",
        "Inference: name characters, places, bosses and chapters as soon as the images, the memory and your web search let you identify them with reasonable confidence; fall back on a generic description only if you truly cannot. Never invent a character or an event absent from this session, and give nothing away about what comes next.",
        "Exception for bit players: secondary characters and occasional opponents (rivals in a challenge, passers-by, merchants, service NPCs) are named ONLY if their name is legibly displayed on screen in an image, or already present in the memory provided. Otherwise stay generic. A plausible but unread proper noun is an invention: never produce one.",
        "The rest of the displayed text (dialogue, menus, subtitles): use it to understand the scene and fill the memory, but NEVER quote it word for word; rephrase it at the level of the session.",
        'Images provided: the "screenshots" are the moments the player found important, the images "auto-extracted from the recording" are sampled automatically. Read them ALL; when in doubt about what to tell, favour the player\'s screenshots. They are your anchor: rest your narrative on what they show, then fill in the transitions between them by reasonable deduction, without ever contradicting what they show.',
        "Off-game images: some images do not show the game (desktop, browser, launcher, chat, another application) or show nothing usable (loading screen, black screen, fade, blurry transition, empty menu, error message). They are NOT part of the session: do not narrate them, do not mention them, and do not use them to guess what was going on.",
        'FIELD images: you sort the images for the journal\'s gallery, using the numbers you are given. "gallery" lists the numbers of the usable images, ranked from the best to the least good at illustrating this session: a good image shows the game in a legible, representative scene (a place, a character, a fight, a cutscene, a highlight). "unusable" lists the numbers of the off-game or unusable images described above. Every number provided appears in exactly one of the two lists.',
        "Locating: if the game has an identifiable progression, use the web search (guide, wiki, walkthrough) to place precisely where the player stands, name places, characters, bosses and chapters correctly, and tell a main quest from a side quest. For a game without linear progression (sandbox, rhythm game, abstract) or without a findable guide, skip this step. Never draw from it an event the player has not reached yet and do not spoil what comes next.",
        f'FIELD memory: keep the continuity up to date, in the game\'s language. "synopsis" is a running summary of the story so far (a few sentences, integrating this session). "entities" lists the characters, places and bosses met with their canonical spelling, the game\'s. "language" is the two-letter CODE of the game\'s language, among {", ".join(JOURNAL_LANGUAGES)}; take the closest if the game is in another language. "profile" is "narrative" or "arcade" and describes THE GAME, not this session: once established it no longer changes, except for an obvious error. Reuse and complete the known names you are given, without rewriting them differently (with the sole exception of a name to correct to the game\'s language).',
        "FIELD memory, no-regression rule: the synopsis is never rewritten shorter. Take up ALL the elements of the synopsis you are given (steps, badges, places crossed, key characters) and merely integrate the current session. You may rephrase, never summarize or prune: a synopsis shorter than the one received is a loss of memory.",
    ]
)

PROFILE_SHAPES = {
    "narrative": "shape B (1 to 3 sentences then a bulleted list)",
    "arcade": "shape A (prose only, 2 to 5 sentences)",
}

# Every property is required: codex's --output-schema runs in strict mode.
OUTPUT_SCHEMA = {
    "type": "object",
    "additionalProperties": False,
    "required": ["title", "body", "next", "images", "memory"],
    "properties": {
        "title": {"type": "string", "description": "Short session title, 3 to 6 words, no trailing period."},
        "body": {
            "type": "string",
            "description": "The journal body, in the game's language: shape A (prose only) or shape B (prose then a bulleted list). Without the pick-up line.",
        },
        "next": {"type": "string", "description": "Where the player stands and their next concrete action, one sentence, no label and no bold."},
        "images": {
            "type": "object",
            "additionalProperties": False,
            "required": ["gallery", "unusable"],
            "properties": {
                "gallery": {"type": "array", "items": {"type": "integer"}, "description": "Numbers of the usable images, from the best to the least good."},
                "unusable": {"type": "array", "items": {"type": "integer"}, "description": "Numbers of the off-game or unusable images."},
            },
        },
        "memory": {
            "type": "object",
            "additionalProperties": False,
            "required": ["synopsis", "entities", "language", "profile"],
            "properties": {
                "synopsis": {"type": "string"},
                "entities": {
                    "type": "object",
                    "additionalProperties": False,
                    "required": ["characters", "places", "bosses"],
                    "properties": {
                        "characters": {"type": "array", "items": {"type": "string"}},
                        "places": {"type": "array", "items": {"type": "string"}},
                        "bosses": {"type": "array", "items": {"type": "string"}},
                    },
                },
                "language": {"type": "string", "enum": JOURNAL_LANGUAGES, "description": "Two-letter code of the game's on-screen language."},
                "profile": {"type": "string", "enum": ["narrative", "arcade"]},
            },
        },
    },
}


def system_prompt(forced_lang=None):
    if not forced_lang:
        return SYSTEM_PROMPT
    code = journal_lang(forced_lang)
    name = LANG_ENGLISH[code]
    return (
        SYSTEM_PROMPT
        + "\n"
        + (
            f"LANGUAGE FORCED by the user, replacing the LANGUAGE rule above: title, body, next and the whole memory "
            f'are written in {name} ({code}), whatever language is shown on screen; memory.language is "{code}". '
            "Proper nouns keep the spelling the game displays."
        )
    )


def image_label(im):
    if im.kind != "frame":
        return "screenshot (a moment you marked)"
    if im.tail:
        return "image auto-extracted from the recording, FINAL MOMENTS of the session"
    return "image auto-extracted from the recording"


def ordinal(n):
    suffix = "th" if 10 <= n % 100 <= 20 else {1: "st", 2: "nd", 3: "rd"}.get(n % 10, "th")
    return f"{n}{suffix}"


# Numbered by position: the same order as the -i flags.
def image_lines(images):
    return [f"- image {i}: {image_label(im)}, around {fmt_time(im.t)}" for i, im in enumerate(images, 1)]


def memory_context(memory):
    if not memory:
        return None
    e = memory.get("entities") or {}

    def line(label, arr):
        return f"{label}: {', '.join(arr)}" if arr else None

    shape = PROFILE_SHAPES.get(memory.get("profile"))
    parts = [
        "Game memory (reuse and complete it, do not contradict it):",
        f"Synopsis: {memory['synopsis']}" if memory.get("synopsis") else None,
        f"Game language, in which you write EVERYTHING: {memory['language']}" if memory.get("language") else None,
        f"Game profile: {memory['profile']}, default {shape}." if shape else None,
        line("Characters", e.get("characters")),
        line("Places", e.get("places")),
        line("Bosses", e.get("bosses")),
    ]
    return "\n".join(p for p in parts if p)


def build_user_prompt(title, start, end, duration_s, session_number, total_sec, images, prev, memory):
    mem_ctx = memory_context(memory)
    no_memory_note = (
        "This is your first entry for this game: briefly search the web for the game to establish the premise and the spelling of the main names."
        if session_number == 1
        else "No game memory has been recorded yet, but this is NOT your first session on this game (see the history above and the previous entry): do not present this session as a start or a restart, and briefly search the web for the game's premise and the spelling of the names."
    )
    profile = (memory or {}).get("profile")
    return "\n".join(
        [
            f"Game: {title}",
            f"Session: {start.strftime('%Y-%m-%d')}, {fmt_time(start)} to {fmt_time(end)} ({fmt_duration(duration_s)})",
            f"History: {ordinal(session_number)} session on this game, about {fmt_duration(total_sec)} of play time in total.",
            "",
            mem_ctx or no_memory_note,
            "",
            f'Previous journal entry (for continuity, do not repeat it):\n"""\n{prev}\n"""' if prev else "",
            "",
            "Images of THIS session, ATTACHED TO THIS MESSAGE in this exact chronological order:",
            *image_lines(images),
            "",
            f"Profile already established for this game: {profile} (default {PROFILE_SHAPES.get(profile, PROFILE_SHAPES['arcade'])}). Keep it in the memory, and tip to the other shape only if the content of THIS session justifies it."
            if profile
            else 'No profile is recorded for this game yet: decide whether it is "narrative" or "arcade", write it in the memory, and write this entry in the matching shape.',
            "",
            "First place this session in the game's progression (use the web and a guide if the game has one), then sort the images, give the session a title, write the journal entry and update the memory.",
        ]
    )


NEXT_UP_RE = re.compile(rf"^\*\*(?:{label_alt('next')}){COLONS}\*\*\s*(.+?)\s*$", re.MULTILINE)
STALL_OPENER = re.compile(
    r"^\s*(?:je\s+(?:vais|commence|dois|vérifie|verifie)|i(?:'|’)?(?:ll|m going to|m about to| will| am going to| need to)|i\s+will|let me|first,? let me)\b",
    re.IGNORECASE,
)
PREAMBLE_RE = re.compile(
    r"^\s*(?:voici|voil[àa]|here(?:'s| is)|this is)[^\n]*(?:journal|entrée|entree|résumé|resume|session|entry|summary)[^\n]*[ \u00a0\u202f]?[:\uff1a]\s*\n+",
    re.IGNORECASE,
)


def dedash(text):
    t = re.sub(r"[ \t]*[—–―][ \t]*", ", ", text)
    t = re.sub(r",\s*,", ",", t)
    return re.sub(r"[ \t]+$", "", t, flags=re.MULTILINE)


def sanitize_body(text):
    t = str(text or "").strip()
    t = re.sub(r"^```[a-zA-Z]*\n?", "", t)
    t = re.sub(r"\n?```$", "", t).strip()
    t = PREAMBLE_RE.sub("", t, count=1).strip()
    return dedash(t).strip()


def accept_body(raw):
    body = sanitize_body(raw)
    if STALL_OPENER.match(body):
        return None
    body = NEXT_UP_RE.sub("", body).rstrip()
    return body or None


def accept_next(raw):
    t = re.sub(r"\s+", " ", dedash(str(raw or "").strip())).strip()
    return NEXT_UP_RE.sub(r"\1", t).strip()


def accept_title(raw):
    t = str(raw or "").split("\n")[0].strip()
    t = re.sub(r"^#+\s*", "", t)
    t = re.sub(r"^\*+|\*+$", "", t).strip()
    t = re.sub(r"^[\"“«‹']+|[\"”»›']+$", "", t).strip()
    t = re.sub(r"\s+", " ", dedash(t))
    t = re.sub(r"\s*[.…]+$", "", t).strip()
    return t[:80]


def accept_result(out):
    """-> {title, body, next, images, memory} | None when the model narrated its work instead."""
    if not isinstance(out, dict) or not out.get("body"):
        return None
    body = accept_body(out.get("body"))
    nxt = accept_next(out.get("next"))
    if not body or not nxt:
        return None
    return {
        "title": accept_title(out.get("title")),
        "body": body,
        "next": nxt,
        "images": out.get("images") if isinstance(out.get("images"), dict) else None,
        "memory": out.get("memory") if isinstance(out.get("memory"), dict) else None,
    }


def paragraphs_from_body(body):
    paras, cur = [], []

    def flush():
        if cur:
            paras.append(" ".join(cur).strip())
            cur.clear()

    for line in str(body or "").splitlines():
        s = line.strip()
        if not s:
            flush()
            continue
        m = re.match(r"^(?:[-*•]|\d+[.)])\s+(.*)$", s)
        if m:
            flush()
            paras.append("- " + m.group(1).strip())
        else:
            cur.append(s)
    flush()
    return [p for p in paras if p]


# The prompt forbids shortening the synopsis; this catches the rewrites that do anyway.
SYNOPSIS_SHRINK_FLOOR = 0.8


def pick_synopsis(old, new):
    o = (old or "").strip()
    n = (new or "").strip()
    if not n:
        return o
    if o and len(n) < len(o) * SYNOPSIS_SHRINK_FLOOR:
        log(f"synopsis rewrite shrank {len(o)} -> {len(n)} chars; keeping the previous one")
        return o
    return n


def merge_memory(old, new):
    if not new:
        return old

    def union(a, b):
        seen = {}
        for x in list(a or []) + list(b or []):
            v = str(x).strip()
            if v and v.lower() not in seen:
                seen[v.lower()] = v
        return list(seen.values())

    oe = (old or {}).get("entities") or {}
    ne = new.get("entities") or {}
    old_profile = (old or {}).get("profile")
    profile = old_profile if old_profile in PROFILE_SHAPES else (new.get("profile") if new.get("profile") in PROFILE_SHAPES else None)
    if old_profile and new.get("profile") and old_profile != new.get("profile"):
        log(f"model proposed profile {new['profile']!r} over the stored {old_profile!r}; keeping the stored one")
    mem = {
        "synopsis": pick_synopsis((old or {}).get("synopsis"), new.get("synopsis")),
        "entities": {
            "characters": union(oe.get("characters"), ne.get("characters")),
            "places": union(oe.get("places"), ne.get("places")),
            "bosses": union(oe.get("bosses"), ne.get("bosses")),
        },
        "language": journal_lang(new.get("language") or (old or {}).get("language")),
    }
    if profile:
        mem["profile"] = profile
    return mem
