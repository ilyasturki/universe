import json
import os
import re
import subprocess
from datetime import datetime, timedelta

from _common import journal_lang, log, read_json, remove
from prompt import OUTPUT_SCHEMA, system_prompt

# A 20-image session measured 159 s; course-based games with one web lookup per level passed 420 s.
TIMEOUT_S = 900
ATTEMPTS = 2
# The quota wall lasts days; transient failures ("Reconnecting") must stay on the retry path.
LIMIT_RE = re.compile(r"hit your usage limit", re.I)
LIMIT_FALLBACK_HOURS = 6


class QuotaExceeded(Exception):
    def __init__(self, until):
        super().__init__(f"usage limit until {until}")
        self.until = until


_MONTHS = {m: i for i, m in enumerate(["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"], 1)}
_RESET_RE = re.compile(r"try again at ([A-Za-z]+) (\d{1,2})(?:st|nd|rd|th)?,? (\d{4}),? (\d{1,2}):(\d{2})(?: ?([AP]M))?", re.I)


# Codex prints the reset in English whatever the locale; strptime %b/%p would not.
def parse_limit_reset(text):
    m = _RESET_RE.search(text or "")
    if not m:
        return None
    month = _MONTHS.get(m.group(1)[:3].lower())
    if not month:
        return None
    hour = int(m.group(4)) % 12 if m.group(6) else int(m.group(4))
    if m.group(6) and m.group(6).upper() == "PM":
        hour += 12
    try:
        return datetime(int(m.group(3)), month, int(m.group(2)), hour, int(m.group(5)))
    except ValueError:
        return None


def codex_args(model, images, schema_path, out_path, prompt, cwd):
    args = [
        "codex", "exec",
        "--skip-git-repo-check",
        "--ignore-user-config",
        "--disable", "browser_use",
        "--disable", "computer_use",
        "--ephemeral",
        "-C", cwd,
        "-s", "read-only",
        "-c", "approval_policy=never",
        "-c", f"model={model}",
        "-c", "model_reasoning_effort=high",
        "-c", "model_verbosity=medium",
        "-c", "project_doc_max_bytes=0",
        "-c", "tools.web_search=true",
        "-c", "mcp_servers={}",
    ]
    for im in images:
        args += ["-i", im]
    args += ["--output-schema", schema_path, "-o", out_path, prompt]
    return args


# codex has no system-prompt flag, so the brief carries it.
def run_codex(model, brief, images, work_dir, forced_lang=None):
    prompt = f"{system_prompt(forced_lang)}\n\n---\n\n{brief}"
    schema_path = os.path.join(work_dir, "schema.json")
    out_path = os.path.join(work_dir, "entry.json")
    with open(schema_path, "w", encoding="utf-8") as f:
        json.dump(OUTPUT_SCHEMA, f)
    args = codex_args(model, [im.file for im in images], schema_path, out_path, prompt, work_dir)
    for attempt in range(1, ATTEMPTS + 1):
        remove(out_path)
        try:
            res = subprocess.run(args, cwd=work_dir, capture_output=True, text=True, timeout=TIMEOUT_S, stdin=subprocess.DEVNULL)
        except subprocess.TimeoutExpired:
            log(f"codex attempt {attempt}/{ATTEMPTS} timed out after {TIMEOUT_S}s")
            continue
        except OSError as e:
            log(f"codex could not start: {e}")
            return None
        if res.returncode != 0:
            output = f"{res.stdout or ''}\n{res.stderr or ''}"
            if LIMIT_RE.search(output):
                raise QuotaExceeded(parse_limit_reset(output) or datetime.now() + timedelta(hours=LIMIT_FALLBACK_HOURS))
            log(f"codex attempt {attempt}/{ATTEMPTS} returned exit={res.returncode}: {(res.stderr or '').strip()[-400:]}")
            continue
        out = read_json(out_path)
        if not out or not out.get("body"):
            log(f"codex attempt {attempt}/{ATTEMPTS}: no usable structured output")
            continue
        return out
    return None


def run_stub(title, images, forced_lang=None):
    n = len(images)
    return {
        "title": f"Stub session of {title}",
        "body": (
            f"You played {title} for a while and the stub provider watched {n} image(s).\n\n"
            "- **Main quest:** Reached the first checkpoint.\n"
            "- **Exploration:** Looked at every image — all of them."
        ),
        "next": "Resume at the first checkpoint and keep going.",
        "images": {"gallery": list(range(1, n + 1)), "unusable": []},
        "memory": {
            "synopsis": f"The player started {title} and reached the first checkpoint.",
            "entities": {"characters": ["Stub Hero"], "places": ["First Checkpoint"], "bosses": []},
            "language": journal_lang(forced_lang),
            "profile": "arcade",
        },
    }


def generate(provider, *, model, title, brief, images, work_dir, forced_lang=None):
    if provider == "stub":
        return run_stub(title, images, forced_lang)
    return run_codex(model, brief, images, work_dir, forced_lang)
