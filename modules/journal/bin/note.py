import json
import os
import re
import shutil
from urllib.parse import quote

from _common import COLONS, LABELS, fmt_date, fmt_duration, fmt_time, game_note_name, journal_lang, label_alt, session_span

# The core's journal.rs renders the same note byte for byte; keep the two in step.
SHOT_IMAGE_RE = re.compile(r"^\d{8}-\d{6}\.(?:png|jpe?g)$", re.I)
MARKER_RE = re.compile(r"<!-- session: (\d{8}-\d{6}) -->")
RECORDING_NUM_RE = re.compile(r"^(\d{1,4})-\d{8}-\d{6}")
DOC_TITLE_RE = re.compile(rf"^#\s*(?:{label_alt('journal')})\s*{COLONS}\s*(.+?)\s*$", re.M)


def file_uri(path):
    # Parentheses encoded for Markdown; ? and # would start a query or a fragment
    return "file://" + quote(path, safe="/;,:@&=+$!~*'").replace("(", "%28").replace(")", "%29")


YAML_WORDS = ("true", "false", "null", "yes", "no", "on", "off", "y", "n", "~", ".inf", "-.inf", "+.inf", ".nan")
YAML_RADIX = re.compile(r"^[-+]?0(?:x[0-9a-fA-F_]+|o[0-7_]+|b[01_]+)$")
YAML_SEXAGESIMAL = re.compile(r"^[-+]?[0-9.][0-9_.:]*$")
YAML_DATE = re.compile(r"^[0-9]{4}-[0-9]+-[0-9]{1,2}(?:[Tt ]|$)")


def yaml_typed(s):
    """A plain scalar YAML 1.1 (PyYAML) or 1.2 (js-yaml, Obsidian) would type as something other than a string."""
    if s.lower() in YAML_WORDS or YAML_RADIX.match(s) or YAML_DATE.match(s):
        return True
    t = s[1:] if s[:1] in "+-" else s
    if t[:1] in "0123456789." and t[:1]:
        if ":" in t and YAML_SEXAGESIMAL.match(s):
            return True
        try:
            float(s.replace("_", ""))
            return True
        except ValueError:
            pass
    return False


def yaml_str(s):
    s = str(s)
    if re.search(r"(:\s|\s#|^[\s\"'\-*&!\[\]{}|>%@`?#]|[\s]$|^$)", s) or yaml_typed(s):
        return json.dumps(s, ensure_ascii=False)
    return s


def body_from_paragraphs(paragraphs, italic=False):
    blocks, bullets = [], []
    for p in paragraphs:
        if p.startswith("- "):
            bullets.append(p)
            continue
        if bullets:
            blocks.append("\n".join(bullets))
            bullets = []
        blocks.append(f"*{p}*" if italic else p)
    if bullets:
        blocks.append("\n".join(bullets))
    return "\n\n".join(blocks)


def entry_number(entry, sessions, entries):
    sid = entry["session"]
    rec = (sessions.get(sid) or {}).get("recording")
    if not rec:
        return None
    m = RECORDING_NUM_RE.match(os.path.basename(rec))
    if m:
        return int(m.group(1))
    return 1 + sum(1 for e in entries if e["session"] < sid and (sessions.get(e["session"]) or {}).get("recording"))


def render_block(entry, sessions, entries):
    sid = entry["session"]
    lab = LABELS[journal_lang(entry.get("lang"))]
    start, end, duration = session_span(sessions.get(sid), sid)
    meta = f"{fmt_date(start)} · {fmt_time(start)}–{fmt_time(end)} · {fmt_duration(duration)}"
    n = entry_number(entry, sessions, entries)
    prefix = f"#{n} · " if n else ""
    title = entry.get("title") or ""
    head = f"## {prefix}{title}\n*{meta}*" if title else f"## {prefix}{meta}"
    parts = [f"{head}\n<!-- session: {sid} -->"]
    body = body_from_paragraphs(entry.get("paragraphs") or [], italic=entry.get("provider") == "none")
    if body:
        parts.append(body)
    if entry.get("next_up"):
        parts.append(f"**{lab['next']}{lab['colon']}** {entry['next_up']}")
    rec = (sessions.get(sid) or {}).get("recording")
    if rec:
        parts.append(f"**{lab['recording']}{lab['colon']}** [{os.path.basename(rec)}]({file_uri(rec)})")
    images = entry.get("images") or []
    shots = [i for i in images if SHOT_IMAGE_RE.search(i)]
    frames = [i for i in images if not SHOT_IMAGE_RE.search(i)]
    if shots:
        parts.append("\n".join(f"![]({i})" for i in shots))
    if frames:
        parts.append(f"*{lab['frames']}*\n\n" + "\n".join(f"![]({i})" for i in frames))
    return "\n\n".join(parts)


def frontmatter(title, body):
    sids = sorted(MARKER_RE.findall(body))
    cover = re.search(r"^!\[\]\((.+?)\)", body, re.M)
    lines = [f"game: {yaml_str(title)}", f"sessions: {len(re.findall(r'^## ', body, re.M))}"]
    if sids:
        lines.append(f"first_played: {sids[0][:4]}-{sids[0][4:6]}-{sids[0][6:8]}")
        lines.append(f"last_played: {sids[-1][:4]}-{sids[-1][4:6]}-{sids[-1][6:8]}")
    if cover:
        lines.append(f"cover: {cover.group(1)}")
    return "---\n" + "\n".join(lines) + "\n---\n\n"


def render_note(entries, sessions, title):
    entries = sorted(entries, key=lambda e: e["session"], reverse=True)
    lang = journal_lang(entries[0].get("lang")) if entries else "en"
    lab = LABELS[lang]
    blocks = [render_block(e, sessions, entries) for e in entries]
    body = f"# {lab['journal']}{lab['colon']} {title}\n\n" + "\n\n".join(blocks) + "\n"
    body = re.sub(r"\n{3,}", "\n\n", body)
    return frontmatter(title, body) + body


# <Title>.md, or the folder's single marked note so a renamed game does not fork its history.
def resolve_note_path(note_dir, title):
    preferred = os.path.join(note_dir, f"{game_note_name(title)}.md")
    try:
        names = os.listdir(note_dir)
    except OSError:
        return preferred
    if os.path.basename(preferred) in names:
        return preferred
    marked, headed = [], []
    for name in names:
        if not name.lower().endswith(".md"):
            continue
        try:
            with open(os.path.join(note_dir, name), encoding="utf-8") as f:
                text = f.read()
        except OSError:
            continue
        if "<!-- session:" in text:
            marked.append(name)
        elif DOC_TITLE_RE.search(text):
            headed.append(name)
    notes = marked or headed
    return os.path.join(note_dir, notes[0]) if len(notes) == 1 else preferred


# The player's own shots are named bare and live in the game's screenshots/; the rest is relative to the journal.
def image_path(journal_dir, screenshots_dir, rel):
    if SHOT_IMAGE_RE.search(rel):
        return os.path.join(screenshots_dir, rel)
    return os.path.join(journal_dir, rel)


# Obsidian only follows links inside the vault, so referenced images are copied beside the note.
def mirror_images(entries, journal_dir, screenshots_dir, note_dir):
    if os.path.abspath(journal_dir) == os.path.abspath(note_dir):
        return
    for e in entries:
        for rel in e.get("images") or []:
            if os.path.isabs(rel) or ".." in rel.split("/"):
                continue
            src, dst = image_path(journal_dir, screenshots_dir, rel), os.path.join(note_dir, rel)
            if not os.path.isfile(src):
                continue
            if os.path.isfile(dst) and os.path.getsize(dst) == os.path.getsize(src):
                continue
            os.makedirs(os.path.dirname(dst), exist_ok=True)
            shutil.copyfile(src, dst)


def write_note(entries, sessions, title, journal_dir, screenshots_dir, note_dir):
    os.makedirs(note_dir, exist_ok=True)
    path = resolve_note_path(note_dir, title)
    text = render_note(entries, sessions, title)
    mirror_images(entries, journal_dir, screenshots_dir, note_dir)
    try:
        with open(path, encoding="utf-8") as f:
            if f.read() == text:
                return path, False
    except OSError:
        pass
    tmp = path + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        f.write(text)
    os.replace(tmp, path)
    return path, True
