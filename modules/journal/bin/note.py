"""The Obsidian note, rendered from Entry JSON and parsed back (format of game-session-summary.mjs buildEntry).
The meta line follows the LC_TIME of the environment; the parser also reads the legacy French form."""
import json
import os
import re
import shutil
from datetime import timedelta
from urllib.parse import quote, unquote

from _common import (LABELS, fmt_date, fmt_duration, fmt_time, game_note_name, journal_lang, labels, parse_duration,
                     parse_session_id, rfc3339_local, sanitize_game_name, session_span)

SHOT_IMAGE_RE = re.compile(r"(?:^|/)\d{8}-\d{6}\.(?:png|jpe?g)$", re.I)
MARKER_RE = re.compile(r"<!-- session: (\d{8}-\d{6}) -->")
META_RE = re.compile(r"^(?P<date>.+?) · (?P<sh>\d{2})[:h](?P<sm>\d{2})(?:–| à )(?P<eh>\d{2})[:h](?P<em>\d{2}) · (?P<duration>.+)$")
HEADER_RE = re.compile(r"^## (?:#(\d+) · )?(.*)$")
IMAGE_LINE_RE = re.compile(r"^!\[[^\]]*\]\((.+?)\)\s*$")
COLONS = r"[ \u00a0\u202f]?[:\uff1a]"
RECORDING_NUM_RE = re.compile(r"^(\d{1,4})-\d{8}-\d{6}")


def _by_label(key):
    out = {}
    for code, l in LABELS.items():
        out.setdefault(l[key], code)
    return out


def _alt(key):
    return "|".join(re.escape(v) for v in _by_label(key))


NEXT_LINE_RE = re.compile(rf"^\*\*({_alt('next')}){COLONS}\*\*\s*(.*?)\s*$")
RECORDING_LINE_RE = re.compile(rf"^\*\*({_alt('recording')}){COLONS}\*\* \[([^\]]*)\]\(([^)]*)\)\s*$")
FRAMES_CAPTION_RE = re.compile(rf"^\*({_alt('frames')})\*$")
DOC_TITLE_RE = re.compile(rf"^#\s*(?:{_alt('journal')})\s*{COLONS}\s*(.+?)\s*$", re.M)


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


def yaml_unstr(s):
    s = s.strip()
    if s.startswith('"'):
        try:
            return json.loads(s)
        except ValueError:
            return s.strip('"')
    if s.startswith("'") and s.endswith("'") and len(s) >= 2:
        return s[1:-1].replace("''", "'")
    return s


# --- rendering -------------------------------------------------------------------

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
    """NNN from the recording name, else the rank among recorded sessions; none without footage."""
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
    lab = labels(entry.get("lang"))
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


# --- parsing (migration) ---------------------------------------------------------

def parse_frontmatter(text):
    m = re.match(r"^---\n(.*?)\n---\n+", text, re.S)
    if not m:
        return {}, text
    data = {}
    for line in m.group(1).splitlines():
        k, sep, v = line.partition(":")
        if sep:
            data[k.strip()] = yaml_unstr(v)
    return data, text[m.end():]


def _lang_of(block_lines):
    for regex, key in ((RECORDING_LINE_RE, "recording"), (FRAMES_CAPTION_RE, "frames"), (NEXT_LINE_RE, "next")):
        table = _by_label(key)
        for line in block_lines:
            m = regex.match(line.strip())
            if m:
                return table[m.group(1)]
    return None


def parse_note(text, game=None):
    """-> (title, [Entry], [Session]); written_at is the session's end, the only clock the note has."""
    fm, body = parse_frontmatter(text)
    m = DOC_TITLE_RE.search(body)
    title = fm.get("game") or (m.group(1) if m else None) or "Journal"
    doc_lang = None
    if m:
        first = body[m.start():].split("\n", 1)[0]
        doc_lang = _by_label("journal").get(re.match(r"^#\s*(\S+)", first).group(1))
    starts = [h.start() for h in re.finditer(r"^## .*$", body, re.M)]
    entries, sessions = [], []
    for i, s in enumerate(starts):
        block = body[s:starts[i + 1] if i + 1 < len(starts) else len(body)]
        e, sess = parse_block(block, title, game, doc_lang)
        if e:
            entries.append(e)
            sessions.append(sess)
    return title, entries, sessions


def parse_block(block, title, game, doc_lang):
    lines = block.rstrip("\n").split("\n")
    hm = HEADER_RE.match(lines[0])
    sm = MARKER_RE.search(block)
    if not hm or not sm:
        return None, None
    sid = sm.group(1)
    header_rest = hm.group(2).strip()
    meta = header_rest if META_RE.match(header_rest) else ""
    entry_title = "" if meta else header_rest
    body_lines, images, next_up, recording = [], [], "", None
    for raw in lines[1:]:
        line = raw.strip()
        if not line or MARKER_RE.search(line):
            if not line:
                body_lines.append("")
            continue
        if not meta and META_RE.match(line.strip("*")) and line.startswith("*") and line.endswith("*"):
            meta = line.strip("*")
            continue
        im = IMAGE_LINE_RE.match(line)
        if im:
            images.append(im.group(1))
            continue
        if FRAMES_CAPTION_RE.match(line):
            continue
        nm = NEXT_LINE_RE.match(line)
        if nm:
            next_up = nm.group(2)
            continue
        rm = RECORDING_LINE_RE.match(line)
        if rm:
            uri = rm.group(3)
            recording = unquote(uri[7:]) if uri.startswith("file://") else uri
            continue
        body_lines.append(line)
    paragraphs = _paragraphs(body_lines)
    provider = "import"
    if len(paragraphs) == 1 and not next_up and re.fullmatch(r"\*[^*].*\*", paragraphs[0]):
        provider = "none"
        paragraphs = [paragraphs[0][1:-1]]
    lang = _lang_of(lines) or doc_lang or "en"

    start = parse_session_id(sid)
    mm = META_RE.match(meta) if meta else None
    if mm:
        end = start.replace(hour=int(mm["eh"]), minute=int(mm["em"]), second=0)
        if end < start.replace(second=0):
            end += timedelta(days=1)
        duration = parse_duration(mm["duration"])
    else:
        end, duration = start, 0
    entry = {
        "session": sid,
        "game": game or sanitize_game_name(title),
        "written_at": rfc3339_local(end),
        "lang": lang,
        "title": entry_title,
        "provider": provider,
        "paragraphs": paragraphs,
        "next_up": next_up,
        "images": images,
    }
    session = {
        "session": sid,
        "game": entry["game"],
        "started_at": rfc3339_local(start),
        "ended_at": rfc3339_local(end),
        "duration_s": duration,
        "source": "import-journal",
        "recording": recording,
    }
    return entry, session


def _paragraphs(lines):
    paras, cur = [], []

    def flush():
        if cur:
            paras.append(" ".join(cur))
            cur.clear()

    for line in lines:
        if not line:
            flush()
            continue
        m = re.match(r"^(?:[-*•]|\d+[.)])\s+(.*)$", line)
        if m:
            flush()
            paras.append("- " + m.group(1).strip())
        else:
            cur.append(line)
    flush()
    return paras
