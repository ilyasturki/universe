import json
import locale
import os
import re
import subprocess
import sys
from datetime import datetime, timedelta

SESSION_ID_RE = re.compile(r"^(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})$")


def log(msg):
    print(f"[journal] {msg}", file=sys.stderr, flush=True)


def load_settings():
    return json.loads(os.environ.get("MODULE_SETTINGS_JSON") or "{}")


def read_json(path):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError):
        return None


def write_json(path, data):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    tmp = path + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, indent=2)
        f.write("\n")
    os.replace(tmp, path)


def remove(path):
    try:
        os.remove(path)
    except OSError:
        pass


def parse_session_id(s):
    m = SESSION_ID_RE.match(str(s or ""))
    if not m:
        return None
    return datetime(*(int(x) for x in m.groups()))


def parse_rfc3339(s):
    s = (s or "").strip()
    if not s:
        return None
    try:
        d = datetime.fromisoformat(s.replace("Z", "+00:00"))
    except ValueError:
        return None
    return d.astimezone().replace(tzinfo=None) if d.tzinfo else d


def rfc3339_local(d):
    if d.tzinfo is None:
        d = d.astimezone()
    return d.isoformat(timespec="seconds")


def use_system_locale():
    try:
        locale.setlocale(locale.LC_TIME, "")
    except locale.Error:
        pass


def fmt_date(d):
    return d.strftime("%x")


def fmt_time(d):
    return d.strftime("%H:%M")


def fmt_duration(total_sec):
    total_min = max(1, round((total_sec or 0) / 60))
    h, m = divmod(total_min, 60)
    if h and m:
        return f"{h} h {m} min"
    if h:
        return f"{h} h"
    return f"{m} min"


def game_note_name(title):
    safe = re.sub(r"[\\/:#^\[\]|]", " ", str(title or ""))
    safe = re.sub(r"\s+", " ", safe).strip()
    return safe or "Journal"


LABELS = {
    "fr": {"journal": "Journal", "recording": "Enregistrement", "next": "Reprise", "frames": "Images extraites de l'enregistrement", "colon": " :"},
    "en": {"journal": "Journal", "recording": "Recording", "next": "Next up", "frames": "Frames from the recording", "colon": ":"},
    "es": {"journal": "Diario", "recording": "Grabación", "next": "Retomar", "frames": "Imágenes extraídas de la grabación", "colon": ":"},
    "de": {"journal": "Journal", "recording": "Aufnahme", "next": "Weiter", "frames": "Bilder aus der Aufnahme", "colon": ":"},
    "it": {"journal": "Diario", "recording": "Registrazione", "next": "Ripresa", "frames": "Immagini estratte dalla registrazione", "colon": ":"},
    "pt": {"journal": "Diário", "recording": "Gravação", "next": "Retomar", "frames": "Imagens extraídas da gravação", "colon": ":"},
    "ja": {"journal": "日誌", "recording": "録画", "next": "次回", "frames": "録画から抽出した画像", "colon": "："},
}
JOURNAL_LANGUAGES = list(LABELS)
COLONS = r"[   ]?[:：]"

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

LANG_ENGLISH = {"en": "English", "fr": "French", "es": "Spanish", "de": "German", "it": "Italian", "pt": "Portuguese", "ja": "Japanese"}


def journal_lang(lang):
    raw = str(lang or "").strip().lower()
    if raw in LABELS:
        return raw
    return LANG_BY_NAME.get(raw, "en")


def label_alt(key):
    return "|".join(re.escape(v) for v in dict.fromkeys(l[key] for l in LABELS.values()))


def read_jsonl(path):
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


def read_sessions(journal_dir):
    sessions = {}
    for s in read_jsonl(os.path.join(os.path.dirname(os.path.abspath(journal_dir)), "sessions.jsonl")):
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


ENTRY_KEYS = ("session", "game", "written_at", "lang", "title", "provider", "paragraphs", "next_up", "images")
OPTIONAL_ENTRY_KEYS = ("started_at", "ended_at", "duration_s")


def validate_entry(entry):
    errors = []
    for k in ENTRY_KEYS:
        if k not in entry:
            errors.append(f"missing {k}")
    for k in ("session", "game", "written_at", "lang", "title", "provider", "next_up", "started_at", "ended_at"):
        if k in entry and not isinstance(entry[k], str):
            errors.append(f"{k} must be a string")
    for k in ("paragraphs", "images"):
        if k in entry and (not isinstance(entry[k], list) or not all(isinstance(x, str) for x in entry[k])):
            errors.append(f"{k} must be a list of strings")
    if "duration_s" in entry and (isinstance(entry["duration_s"], bool) or not isinstance(entry["duration_s"], int)):
        errors.append("duration_s must be an integer")
    if "session" in entry and not SESSION_ID_RE.match(str(entry["session"])):
        errors.append("session must be YYYYMMDD-HHMMSS")
    extra = set(entry) - set(ENTRY_KEYS) - set(OPTIONAL_ENTRY_KEYS)
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


def add_entry_via_core(sid, entry_json):
    cmd = [os.environ.get("UNIVERSE_BIN") or "universe", "journal-add", sid, entry_json]
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
    except FileNotFoundError:
        log(f"{cmd[0]} not found")
        return "unavailable"
    except subprocess.TimeoutExpired:
        log("universe journal-add timed out")
        return "unavailable"
    if result.returncode == 0:
        return "ok"
    err = (result.stderr or "").strip()
    log(f"universe journal-add failed ({result.returncode}): {err}")
    if "invalid:" in err:
        return "invalid"
    return "unavailable"
