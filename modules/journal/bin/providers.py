"""The writing models. Each run returns the raw object of prompt.OUTPUT_SCHEMA, or raises Quota/Transient/Permanent."""

import base64
import json
import os
import re
import select
import subprocess
import urllib.error
import urllib.request
from dataclasses import dataclass
from datetime import datetime, timedelta

import images as img
from _common import journal_lang, log, read_json, remove
from prompt import OUTPUT_SCHEMA

# A 20-image session measured 159 s; course-based games with one web lookup per level passed 420 s.
TIMEOUT_S = 900
ATTEMPTS = 2
# The quota wall lasts days; transient failures ("Reconnecting") must stay on the retry path.
LIMIT_RE = re.compile(r"hit your usage limit", re.IGNORECASE)
LIMIT_FALLBACK_HOURS = 6
PROVIDERS = ("codex", "openai", "stub")


class QuotaExceeded(Exception):
    """The account's window is spent: nothing to try until `until`."""

    def __init__(self, until, provider="codex"):
        super().__init__(f"{provider} usage limit until {until}")
        self.until = until
        self.provider = provider


class Transient(Exception):
    """The try failed for a reason that may not repeat: worth another run later."""


class Permanent(Exception):
    """The setup is wrong (no key, unknown model, missing binary): a retry changes nothing."""


@dataclass
class Options:
    provider: str = ""
    model: str = ""
    base_url: str = ""
    api_key: str = ""
    effort: str = "high"
    web_search: bool = True
    timeout_s: int = TIMEOUT_S
    attempts: int = ATTEMPTS
    image_long_edge: int = img.FRAMES_LONG_EDGE


_MONTHS = {m: i for i, m in enumerate(["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"], 1)}
_RESET_RE = re.compile(r"try again at ([A-Za-z]+) (\d{1,2})(?:st|nd|rd|th)?,? (\d{4}),? (\d{1,2}):(\d{2})(?: ?([AP]M))?", re.IGNORECASE)


# codex's own account API, one-shot over stdio: the reset instant, where the message only carries a same-day time or "later".
APP_SERVER_TIMEOUT_S = 15


def read_limit_reset(timeout_s=APP_SERVER_TIMEOUT_S):
    """When the exhausted window resets, from `account/rateLimits/read`; None when codex cannot say."""
    try:
        proc = subprocess.Popen(
            ["codex", "app-server", "--listen", "stdio://"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, bufsize=0
        )
    except OSError as e:
        log(f"codex app-server could not start: {e}")
        return None
    assert proc.stdin is not None and proc.stdout is not None
    try:
        for msg in (
            {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {"clientInfo": {"name": "universe-journal", "title": "Universe", "version": "0.0.2"}},
            },
            {"jsonrpc": "2.0", "method": "initialized", "params": {}},
            {"jsonrpc": "2.0", "id": 2, "method": "account/rateLimits/read", "params": {}},
        ):
            proc.stdin.write((json.dumps(msg) + "\n").encode())
        proc.stdin.flush()
        # Unbuffered reads: with a buffered pipe, lines already read ahead would leave select() waiting on an empty fd
        deadline = datetime.now() + timedelta(seconds=timeout_s)
        pending = b""
        while (remaining := (deadline - datetime.now()).total_seconds()) > 0:
            if not select.select([proc.stdout], [], [], remaining)[0]:
                break
            chunk = os.read(proc.stdout.fileno(), 65536)
            if not chunk:
                break
            pending += chunk
            while b"\n" in pending:
                line, pending = pending.split(b"\n", 1)
                try:
                    reply = json.loads(line)
                except ValueError:
                    continue
                if isinstance(reply, dict) and reply.get("id") == 2:
                    return limit_reset_from(reply.get("result"))
        log("codex app-server gave no rate limits in time")
        return None
    except (OSError, ValueError) as e:
        log(f"codex app-server: {e}")
        return None
    finally:
        proc.kill()


def limit_reset_from(result):
    """The latest reset among the windows at 100 %: every one of them has to pass before a request goes through."""
    limits = (result or {}).get("rateLimits") or {}
    windows = [w for w in (limits.get("primary"), limits.get("secondary")) if isinstance(w, dict) and w.get("resetsAt")]
    exhausted = [w for w in windows if (w.get("usedPercent") or 0) >= 100]
    if not exhausted:
        return None
    return datetime.fromtimestamp(max(w["resetsAt"] for w in exhausted))


def failure_messages(stdout):
    """The `turn.failed` and `error` events of `codex exec --json`; anything else on stdout is left alone."""
    out = []
    for line in (stdout or "").splitlines():
        try:
            ev = json.loads(line)
        except ValueError:
            continue
        if not isinstance(ev, dict):
            continue
        if ev.get("type") == "turn.failed":
            out.append(str((ev.get("error") or {}).get("message") or ""))
        elif ev.get("type") == "error":
            out.append(str(ev.get("message") or ""))
    return [m for m in out if m]


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


def codex_args(model, images, schema_path, out_path, prompt, cwd, effort="high", web_search=True):
    args = [
        "codex",
        "exec",
        "--json",
        "--skip-git-repo-check",
        "--ignore-user-config",
        "--disable",
        "browser_use",
        "--disable",
        "computer_use",
        "--ephemeral",
        "-C",
        cwd,
        "-s",
        "read-only",
        "-c",
        "approval_policy=never",
    ]
    if model:
        args += ["-c", f"model={model}"]
    args += [
        "-c",
        f"model_reasoning_effort={effort}",
        "-c",
        "model_verbosity=medium",
        "-c",
        "project_doc_max_bytes=0",
        "-c",
        f"tools.web_search={'true' if web_search else 'false'}",
        "-c",
        "mcp_servers={}",
    ]
    for im in images:
        args += ["-i", im]
    args += ["--output-schema", schema_path, "-o", out_path, prompt]
    return args


# codex has no system-prompt flag, so the brief carries it.
def run_codex(opts, system, brief, images, work_dir, heartbeat=None):
    prompt = f"{system}\n\n---\n\n{brief}"
    schema_path = os.path.join(work_dir, "schema.json")
    out_path = os.path.join(work_dir, "entry.json")
    with open(schema_path, "w", encoding="utf-8") as f:
        json.dump(OUTPUT_SCHEMA, f)
    args = codex_args(opts.model, [im.file for im in images], schema_path, out_path, prompt, work_dir, opts.effort, opts.web_search)
    attempts = max(1, opts.attempts)
    last = "codex gave no usable entry"
    for attempt in range(1, attempts + 1):
        remove(out_path)
        if heartbeat:
            heartbeat()
        try:
            res = subprocess.run(args, cwd=work_dir, capture_output=True, text=True, timeout=opts.timeout_s, stdin=subprocess.DEVNULL, check=False)
        except subprocess.TimeoutExpired:
            last = f"codex timed out after {opts.timeout_s}s"
            log(f"codex attempt {attempt}/{attempts} timed out after {opts.timeout_s}s")
            continue
        except OSError as e:
            raise Permanent(f"codex could not start: {e}") from e
        if res.returncode != 0:
            failures = failure_messages(res.stdout)
            output = "\n".join(failures) if failures else f"{res.stdout or ''}\n{res.stderr or ''}"
            if LIMIT_RE.search(output):
                raise QuotaExceeded(read_limit_reset() or parse_limit_reset(output) or datetime.now() + timedelta(hours=LIMIT_FALLBACK_HOURS))
            last = f"codex exited {res.returncode}: {output.strip()[-200:]}"
            log(f"codex attempt {attempt}/{attempts} returned exit={res.returncode}: {output.strip()[-400:]}")
            continue
        out = read_json(out_path)
        if not out or not out.get("body"):
            last = "codex returned no structured output"
            log(f"codex attempt {attempt}/{attempts}: no usable structured output")
            continue
        return out
    raise Transient(last)


# --- OpenAI-compatible endpoint -------------------------------------------------------------

JSON_SCHEMA_NAME = "journal_entry"
# Inline images are base64 in the request body; past this the endpoint refuses the payload.
PAYLOAD_BUDGET_BYTES = 20 * 1024 * 1024
JPEG_QUALITY = 5
# Fields an endpoint may not know: dropped one by one when it says so, rather than failing the session.
OPTIONAL_FIELDS = ("web_search_options", "reasoning_effort")
RETRY_AFTER_RE = re.compile(r"(?:try again|retry) in (\d+(?:\.\d+)?)\s*(ms|s|m|h)", re.IGNORECASE)
SPENT_RE = re.compile(r"insufficient_quota|billing|credit|exceeded your current quota", re.IGNORECASE)
RATE_LIMIT_MINUTES = 10
SPENT_HOURS = 12


def api_key(opts):
    key = (opts.api_key or "").strip()
    if not key:
        raise Permanent("no API key: set journal.api_key or journal.api_key_file")
    return key


def endpoint(opts, path):
    base = (opts.base_url or "").strip().rstrip("/")
    if not base:
        raise Permanent("no API endpoint: set journal.base_url")
    return f"{base}{path}"


def data_url(path):
    with open(path, "rb") as f:
        return "data:image/jpeg;base64," + base64.b64encode(f.read()).decode("ascii")


def inline_images(images, work_dir, long_edge):
    """Every image re-encoded to JPEG and base64'd; the least useful frames go when the payload is too big."""
    if not images:
        return []
    out_dir = os.path.join(work_dir, "inline")
    os.makedirs(out_dir, exist_ok=True)
    urls = []
    for i, im in enumerate(images, 1):
        jpeg = img.as_jpeg(im.file, os.path.join(out_dir, f"i_{i:03d}.jpg"), long_edge, JPEG_QUALITY)
        if not jpeg:
            log(f"could not encode {os.path.basename(im.file)} for the request; leaving it out")
            continue
        urls.append((im, data_url(jpeg)))
    while len(urls) > 1 and sum(len(u) for _, u in urls) > PAYLOAD_BUDGET_BYTES:
        frames = [i for i, (im, _) in enumerate(urls) if im.kind == "frame"]
        drop = frames[len(frames) // 2] if frames else len(urls) // 2
        urls.pop(drop)
        log(f"payload over {PAYLOAD_BUDGET_BYTES // (1024 * 1024)} MB; feeding {len(urls)} image(s) instead")
    return urls


def chat_payload(opts, system, brief, urls, schema=True):
    content = [{"type": "text", "text": brief}]
    content += [{"type": "image_url", "image_url": {"url": url}} for _, url in urls]
    payload = {
        "model": opts.model,
        "messages": [{"role": "system", "content": system}, {"role": "user", "content": content}],
        "response_format": {"type": "json_schema", "json_schema": {"name": JSON_SCHEMA_NAME, "strict": True, "schema": OUTPUT_SCHEMA}}
        if schema
        else {"type": "json_object"},
    }
    if opts.effort:
        payload["reasoning_effort"] = opts.effort
    if opts.web_search:
        payload["web_search_options"] = {}
    return payload


def quota_until(body, headers):
    retry_after = (headers or {}).get("Retry-After") or ""
    if retry_after.strip().isdigit():
        return datetime.now() + timedelta(seconds=int(retry_after.strip()))
    m = RETRY_AFTER_RE.search(body or "")
    if m:
        scale = {"ms": 1 / 1000, "s": 1, "m": 60, "h": 3600}[m.group(2).lower()]
        return datetime.now() + timedelta(seconds=float(m.group(1)) * scale)
    hours = SPENT_HOURS if SPENT_RE.search(body or "") else 0
    return datetime.now() + (timedelta(hours=hours) if hours else timedelta(minutes=RATE_LIMIT_MINUTES))


def error_message(body):
    try:
        data = json.loads(body)
    except ValueError:
        return (body or "").strip()[:300]
    err = data.get("error") if isinstance(data, dict) else None
    if isinstance(err, dict):
        return str(err.get("message") or err)[:300]
    return str(err or data)[:300]


def post_json(url, key, payload, timeout_s):
    """The decoded reply, or a raised Quota/Transient/Permanent telling the caller what kind of failure it was."""
    req = urllib.request.Request(
        url,
        data=json.dumps(payload).encode("utf-8"),
        method="POST",
        headers={"Content-Type": "application/json", "Accept": "application/json", "Authorization": f"Bearer {key}"},
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout_s) as res:
            return json.loads(res.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        body = e.read().decode("utf-8", "replace")
        message = error_message(body)
        if e.code == 429:
            raise QuotaExceeded(quota_until(body, dict(e.headers or {})), "openai") from e
        if e.code in (401, 403):
            raise Permanent(f"the endpoint refused the key ({e.code}): {message}") from e
        if e.code in (400, 404, 422):
            raise Permanent(f"the endpoint refused the request ({e.code}): {message}") from e
        raise Transient(f"the endpoint answered {e.code}: {message}") from e
    except (urllib.error.URLError, TimeoutError, OSError) as e:
        raise Transient(f"the endpoint could not be reached: {e}") from e
    except ValueError as e:
        raise Transient(f"the endpoint answered with something other than JSON: {e}") from e


def unsupported_field(message):
    """The optional payload field an endpoint says it does not know, so the next try can go without it."""
    low = (message or "").lower()
    for field in OPTIONAL_FIELDS:
        if field in low:
            return field
    if "json_schema" in low or "response_format" in low:
        return "response_format"
    return None


def content_of(reply):
    choices = (reply or {}).get("choices") or []
    message = (choices[0] or {}).get("message") if choices else None
    if not isinstance(message, dict):
        raise Transient("the endpoint answered without a message")
    if message.get("refusal"):
        raise Transient(f"the model refused: {str(message['refusal'])[:200]}")
    text = message.get("content")
    if isinstance(text, list):  # some endpoints answer in parts
        text = "".join(part.get("text", "") for part in text if isinstance(part, dict))
    if not text or not str(text).strip():
        raise Transient("the endpoint answered with an empty message")
    return str(text)


def parse_entry(text):
    text = text.strip()
    if text.startswith("```"):
        text = re.sub(r"^```[a-zA-Z]*\n?", "", text)
        text = re.sub(r"\n?```$", "", text).strip()
    try:
        out = json.loads(text)
    except ValueError as e:
        raise Transient(f"the model's answer is not JSON: {e}") from e
    if not isinstance(out, dict) or not out.get("body"):
        raise Transient("the model's answer has no body")
    return out


def run_openai(opts, system, brief, images, work_dir, heartbeat=None):
    if not opts.model:
        raise Permanent("no model set: pick one in the journal's settings")
    key = api_key(opts)
    url = endpoint(opts, "/chat/completions")
    urls = inline_images(images, work_dir, opts.image_long_edge)
    dropped = set()

    def payload():
        schema = "response_format" not in dropped
        body = chat_payload(opts, system if schema else f"{system}\n\n{schema_reminder()}", brief, urls, schema=schema)
        for field in dropped - {"response_format"}:  # that one is downgraded, not removed: the answer still has to be JSON
            body.pop(field, None)
        return body

    attempts = max(1, opts.attempts)
    tries = 0
    last = "the endpoint gave no usable entry"
    while tries < attempts:
        if heartbeat:
            heartbeat()
        try:
            return parse_entry(content_of(post_json(url, key, payload(), opts.timeout_s)))
        except Permanent as e:
            # An endpoint that does not know an optional field says so: drop it and ask again, rather than losing the session.
            field = unsupported_field(str(e))
            if not field or field in dropped:
                raise
            dropped.add(field)
            log(f"the endpoint does not take {field}; asking again without it")
        except Transient as e:
            tries += 1
            last = str(e)
            log(f"openai attempt {tries}/{attempts}: {last}")
    raise Transient(last)


def schema_reminder():
    return (
        "Answer with JSON only, no prose around it, exactly this shape: "
        '{"title": string, "body": string, "next": string, '
        '"images": {"gallery": [int], "unusable": [int]}, '
        '"memory": {"synopsis": string, "entities": {"characters": [string], "places": [string], "bosses": [string]}, '
        '"language": string, "profile": "narrative" | "arcade"}}.'
    )


def openai_models(opts):
    """The endpoint's model ids for the settings page; an empty list when it cannot say."""
    try:
        req = urllib.request.Request(endpoint(opts, "/models"), headers={"Authorization": f"Bearer {api_key(opts)}", "Accept": "application/json"})
        with urllib.request.urlopen(req, timeout=15) as res:
            data = json.loads(res.read().decode("utf-8"))
    except (Permanent, urllib.error.URLError, urllib.error.HTTPError, TimeoutError, OSError, ValueError) as e:
        log(f"{getattr(opts, 'base_url', '')}/models failed: {e}")
        return []
    rows = data.get("data") if isinstance(data, dict) else data
    ids = [str(r["id"]) for r in rows or [] if isinstance(r, dict) and r.get("id")]
    return sorted(ids)


def codex_models():
    try:
        out = subprocess.run(["codex", "debug", "models"], capture_output=True, text=True, check=True, timeout=15).stdout
        models = json.loads(out).get("models") or []
    except (OSError, subprocess.SubprocessError, ValueError, AttributeError) as e:
        log(f"codex debug models failed: {e}")
        return []
    listed = [m for m in models if isinstance(m, dict) and m.get("slug") and m.get("visibility", "list") == "list"]
    listed.sort(key=lambda m: m["priority"] if isinstance(m.get("priority"), int) else 1 << 30)
    return [m["slug"] for m in listed]


def models(opts):
    if opts.provider == "codex":
        return codex_models()
    if opts.provider == "openai":
        return openai_models(opts)
    return []


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


def generate(opts, *, title, system, brief, images, work_dir, forced_lang=None, heartbeat=None):
    if opts.provider == "stub":
        return run_stub(title, images, forced_lang)
    if opts.provider == "codex":
        return run_codex(opts, system, brief, images, work_dir, heartbeat)
    if opts.provider == "openai":
        return run_openai(opts, system, brief, images, work_dir, heartbeat)
    raise Permanent(f"unknown writing model {opts.provider!r}")
