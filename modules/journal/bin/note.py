import json
import os
import re
import shutil
from urllib.parse import quote

from _common import COLONS, LABELS, fmt_date, fmt_duration, fmt_time, game_note_name, journal_lang, label_alt, session_span

# Format frozen to game-session-summary.mjs buildEntry; the core's journal.rs mirrors it.
SHOT_IMAGE_RE = re.compile(r"(?:^|/)\d{8}-\d{6}\.(?:png|jpe?g)$", re.I)
MARKER_RE = re.compile(r"<!-- session: (\d{8}-\d{6}) -->")
RECORDING_NUM_RE = re.compile(r"^(\d{1,4})-\d{8}-\d{6}")
DOC_TITLE_RE = re.compile(rf"^#\s*(?:{label_alt('journal')})\s*{COLONS}\s*(.+?)\s*$", re.M)


def file_uri(path):
    return "file://" + quote(path, safe="/;,?:@&=+$!~*'#").replace("(", "%28").replace(")", "%29")


def yaml_str(s):
    s = str(s)
    if re.search(r"(:\s|\s#|^[\s\"'\-*&!\[\]{}|>%@`?]|[\s]$|^$)", s) or s.lower() in ("true", "false", "null", "yes", "no", "~"):
        return json.dumps(s, ensure_ascii=False)
    try:
        float(s)
        return json.dumps(s)
    except ValueError:
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


def resolve_note_path(note_dir, title):
    """<Title>.md, or the folder's single marked note so a renamed game does not fork its history."""
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


def mirror_images(entries, journal_dir, note_dir):
    """Obsidian only follows links inside the vault, so referenced images are copied beside the note."""
    if os.path.abspath(journal_dir) == os.path.abspath(note_dir):
        return 0
    copied = 0
    for e in entries:
        for rel in e.get("images") or []:
            if os.path.isabs(rel) or ".." in rel.split("/"):
                continue
            src, dst = os.path.join(journal_dir, rel), os.path.join(note_dir, rel)
            if not os.path.isfile(src):
                continue
            if os.path.isfile(dst) and os.path.getsize(dst) == os.path.getsize(src):
                continue
            os.makedirs(os.path.dirname(dst), exist_ok=True)
            shutil.copyfile(src, dst)
            copied += 1
    return copied


def write_note(entries, sessions, title, journal_dir, note_dir):
    os.makedirs(note_dir, exist_ok=True)
    path = resolve_note_path(note_dir, title)
    text = render_note(entries, sessions, title)
    mirror_images(entries, journal_dir, note_dir)
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
