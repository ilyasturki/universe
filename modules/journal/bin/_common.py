import json
import os
import re
import subprocess
import sys
import unicodedata
from datetime import datetime, timedelta

DEFAULT_BUS = "io.github.ilyasturki.Universe"
DEFAULT_OBJECT = "/io/github/ilyasturki/Universe"
JOURNAL_IFACE = "io.github.ilyasturki.Universe.Journal1"

SETTINGS_DEFAULTS = {
    "enabled": True,
    "language": "auto",
    "provider": "codex",
    "model": "gpt-5.6-sol",
    "markdown_export": True,
    "journal_root": "~/Documents/notes/games/journal",
    "max_images": 40,
}

SESSION_ID_RE = re.compile(r"^(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})$")


def log(msg):
    print(f"[journal] {msg}", file=sys.stderr, flush=True)


def load_settings():
    raw = os.environ.get("MODULE_SETTINGS_JSON", "") or "{}"
    try:
        parsed = json.loads(raw)
    except json.JSONDecodeError as e:
        log(f"MODULE_SETTINGS_JSON invalid ({e}), using defaults")
        parsed = {}
    settings = dict(SETTINGS_DEFAULTS)
    settings.update({k: v for k, v in parsed.items() if v is not None})
    try:
        settings["max_images"] = max(1, int(settings["max_images"]))
    except (TypeError, ValueError):
        settings["max_images"] = SETTINGS_DEFAULTS["max_images"]
    if "journal_root" not in parsed and os.environ.get("UNIVERSE_JOURNAL_ROOT"):
        settings["journal_root"] = os.environ["UNIVERSE_JOURNAL_ROOT"]
    settings["journal_root"] = os.path.expanduser(str(settings["journal_root"]))
    return settings


def bus_and_object():
    return (
        os.environ.get("UNIVERSE_BUS") or DEFAULT_BUS,
        os.environ.get("UNIVERSE_OBJECT") or DEFAULT_OBJECT,
    )


# --- session ids and dates ---------------------------------------------------

def parse_session_id(s):
    m = SESSION_ID_RE.match(str(s or ""))
    if not m:
        return None
    return datetime(*(int(x) for x in m.groups()))


def session_id(d):
    return d.strftime("%Y%m%d-%H%M%S")


def to_local_naive(d):
    if d.tzinfo is not None:
        d = d.astimezone().replace(tzinfo=None)
    return d


def parse_rfc3339(s):
    s = (s or "").strip()
    if not s:
        return None
    try:
        return to_local_naive(datetime.fromisoformat(s.replace("Z", "+00:00")))
    except ValueError:
        return None


def rfc3339_local(d):
    if d.tzinfo is None:
        d = d.astimezone()
    return d.isoformat(timespec="seconds")


def date_fr(d):
    return d.strftime("%d/%m/%Y")


def heure_fr(d):
    return d.strftime("%Hh%M")


def duree_fr(total_sec):
    total_min = max(1, round((total_sec or 0) / 60))
    h, m = divmod(total_min, 60)
    if h and m:
        return f"{h} h {m} min"
    if h:
        return f"{h} h"
    return f"{m} min"


# The space before the unit tells "1 h 23 min" apart from the "14h30" time range.
def parse_duree_fr(s):
    h = re.search(r"(\d+)\s+h\b", str(s or ""))
    m = re.search(r"(\d+)\s+min\b", str(s or ""))
    return (int(h.group(1)) if h else 0) * 3600 + (int(m.group(1)) if m else 0) * 60


# --- names -------------------------------------------------------------------

# Mirror of sanitizeGameName (~/NixOs/bin/lib/game-session.mjs), which named the recording and journal folders.
def sanitize_game_name(name):
    if not name:
        return "unknown"
    s = unicodedata.normalize("NFKD", str(name).lower())
    s = re.sub(r"[\u0300-\u036f]", "", s)
    s = re.sub(r"['’]", "", s)
    s = re.sub(r"[^a-z0-9-]", "-", s)
    s = re.sub(r"-+", "-", s)
    return s.strip("-")


def game_note_name(title):
    safe = re.sub(r"[\\/:#^\[\]|]", " ", str(title or ""))
    safe = re.sub(r"\s+", " ", safe).strip()
    return safe or "Journal"


# --- languages and labels -------------------------------------------------------

LABELS = {
    "fr": {"journal": "Journal", "recording": "Enregistrement", "next": "Reprise", "frames": "Images extraites de l'enregistrement", "no_images": "L'enregistrement de cette session est vide et aucune capture ne la couvre : il n'y a rien à résumer.", "colon": " :"},
    "en": {"journal": "Journal", "recording": "Recording", "next": "Next up", "frames": "Frames from the recording", "no_images": "This session’s recording holds no picture and no screenshot covers it, so there is nothing to summarize.", "colon": ":"},
    "es": {"journal": "Diario", "recording": "Grabación", "next": "Retomar", "frames": "Imágenes extraídas de la grabación", "no_images": "La grabación de esta sesión no tiene imagen y ninguna captura la cubre, así que no hay nada que resumir.", "colon": ":"},
    "de": {"journal": "Journal", "recording": "Aufnahme", "next": "Weiter", "frames": "Bilder aus der Aufnahme", "no_images": "Die Aufnahme dieser Sitzung enthält kein Bild und kein Screenshot deckt sie ab, es gibt also nichts zusammenzufassen.", "colon": ":"},
    "it": {"journal": "Diario", "recording": "Registrazione", "next": "Ripresa", "frames": "Immagini estratte dalla registrazione", "no_images": "La registrazione di questa sessione non ha immagini e nessuna cattura la copre, quindi non c’è nulla da riassumere.", "colon": ":"},
    "pt": {"journal": "Diário", "recording": "Gravação", "next": "Retomar", "frames": "Imagens extraídas da gravação", "no_images": "A gravação desta sessão não tem imagem e nenhuma captura a cobre, portanto não há nada a resumir.", "colon": ":"},
    "ja": {"journal": "日誌", "recording": "録画", "next": "次回", "frames": "録画から抽出した画像", "no_images": "このセッションの録画に映像がなく、これを補うスクリーンショットもないため、まとめる内容がありません。", "colon": "："},
}
JOURNAL_LANGUAGES = list(LABELS)

LANG_NAMES = {
    "en": ["anglais", "english", "inglés", "ingles", "englisch", "inglese", "inglês", "英語"],
    "fr": ["français", "francais", "french", "französisch", "francés", "frances", "francese", "francês", "フランス語"],
    "es": ["espagnol", "spanish", "español", "espanol", "spanisch", "spagnolo", "espanhol", "スペイン語"],
    "de": ["allemand", "german", "deutsch", "alemán", "aleman", "tedesco", "alemão", "alemao", "ドイツ語"],
    "it": ["italien", "italian", "italiano", "italienisch", "イタリア語"],
    "pt": ["portugais", "portuguese", "português", "portugues", "portugiesisch", "portoghese", "ポルトガル語"],
    "ja": ["japonais", "japanese", "japonés", "japones", "japanisch", "giapponese", "japonês", "日本語", "nihongo"],
}
LANG_BY_NAME = {n: code for code, names in LANG_NAMES.items() for n in names}


def journal_lang(lang):
    raw = str(lang or "").strip().lower()
    if raw in LABELS:
        return raw
    return LANG_BY_NAME.get(raw, "en")


def labels(lang):
    return LABELS[journal_lang(lang)]


# --- sessions.jsonl --------------------------------------------------------------

MIGRATED_SESSIONS = ".migrated-sessions.jsonl"


def _read_jsonl(path):
    out = []
    try:
        with open(path, encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    out.append(json.loads(line))
                except json.JSONDecodeError:
                    continue
    except OSError:
        pass
    return out


def read_sessions(journal_dir, explicit=None):
    """The core's sessions.jsonl beside journal/ wins; the migration sidecar fills what it lacks."""
    sessions = {}
    paths = [explicit] if explicit else [os.path.join(os.path.dirname(os.path.abspath(journal_dir)), "sessions.jsonl")]
    paths.append(os.path.join(journal_dir, MIGRATED_SESSIONS))
    for path in paths:
        for s in _read_jsonl(path):
            sid = s.get("session")
            if sid and sid not in sessions:
                sessions[sid] = s
    return sessions


def session_span(session, sid):
    start = parse_rfc3339((session or {}).get("started_at")) or parse_session_id(sid)
    duration = (session or {}).get("duration_s")
    end = parse_rfc3339((session or {}).get("ended_at"))
    if start is None:
        return None, None, 0
    if duration is None:
        duration = int((end - start).total_seconds()) if end else 0
    if end is None:
        end = start + timedelta(seconds=int(duration or 0))
    return start, end, int(duration or 0)


# --- entries ---------------------------------------------------------------------

ENTRY_KEYS = ("session", "game", "written_at", "lang", "title", "provider", "paragraphs", "next_up", "images")


def validate_entry(entry):
    errors = []
    for k in ENTRY_KEYS:
        if k not in entry:
            errors.append(f"missing {k}")
    for k in ("session", "game", "written_at", "lang", "title", "provider", "next_up"):
        if k in entry and not isinstance(entry[k], str):
            errors.append(f"{k} must be a string")
    for k in ("paragraphs", "images"):
        if k in entry and (not isinstance(entry[k], list) or not all(isinstance(x, str) for x in entry[k])):
            errors.append(f"{k} must be a list of strings")
    if "session" in entry and not SESSION_ID_RE.match(str(entry["session"])):
        errors.append("session must be AAAAMMJJ-HHMMSS")
    extra = set(entry) - set(ENTRY_KEYS)
    if extra:
        errors.append(f"unexpected keys: {sorted(extra)}")
    return errors


def read_entries(journal_dir):
    entries = []
    try:
        names = os.listdir(journal_dir)
    except OSError:
        return entries
    for name in names:
        if not name.endswith(".json") or not SESSION_ID_RE.match(name[:-5]):
            continue
        try:
            with open(os.path.join(journal_dir, name), encoding="utf-8") as f:
                e = json.load(f)
        except (OSError, ValueError):
            continue
        if isinstance(e, dict) and e.get("session") == name[:-5]:
            entries.append(e)
    entries.sort(key=lambda e: e["session"], reverse=True)
    return entries


def write_entry_file(journal_dir, entry):
    os.makedirs(journal_dir, exist_ok=True)
    path = os.path.join(journal_dir, f"{entry['session']}.json")
    tmp = path + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(entry, f, ensure_ascii=False, indent=2)
        f.write("\n")
    os.replace(tmp, path)
    return path


def add_entry_via_bus(sid, entry_json):
    """-> 'ok' | 'invalid' (the core rejected the entry) | 'unavailable'."""
    bus, obj = bus_and_object()
    cmd = ["busctl", "--user", "call", bus, obj, JOURNAL_IFACE, "AddEntry", "ss", sid, entry_json]
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
    except FileNotFoundError:
        log("busctl not found")
        return "unavailable"
    except subprocess.TimeoutExpired:
        log("Journal1.AddEntry timed out")
        return "unavailable"
    if result.returncode == 0:
        return "ok"
    err = (result.stderr or "").strip()
    log(f"Journal1.AddEntry failed ({result.returncode}): {err}")
    if ".Error.Invalid" in err:
        return "invalid"
    return "unavailable"
