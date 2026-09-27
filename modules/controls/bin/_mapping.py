import os
from pathlib import Path

# Universe's buttons: HOME, the share/capture/mic button, the back paddles and an Edge's Fn keys.
TAKEN = ("guide", "misc1", "misc2", "misc3", "misc4", "misc5", "misc6", "paddle1", "paddle2", "paddle3", "paddle4")


def stripped(mapping, guide=False):
    """SDL skips a file line with no `platform:` field."""
    fields = [f for f in mapping.split(",") if f]
    if len(fields) < 2:
        return ""
    taken = [t for t in TAKEN if not (guide and t == "guide")]
    body = [f for f in fields[2:] if f.split(":", 1)[0] not in taken]
    if not any(f.startswith("platform:") for f in body):
        body.append("platform:Linux")
    return ",".join([*fields[:2], *body]) + ","


def lines(pads, guide=False):
    out, seen = [], set()
    for pad in pads:
        line = stripped(pad.mapping, guide)
        guid = line.split(",", 1)[0]
        if line and guid not in seen:
            seen.add(guid)
            out.append(line)
    return out


def path(session_id):
    runtime = Path(os.environ.get("XDG_RUNTIME_DIR") or f"/run/user/{os.getuid()}")
    return runtime / "universe" / f"controls-{session_id}.txt"


def text(pads, guide=False, databases=()):
    """SDL keeps the last line per GUID."""
    parts = []
    for db in databases:
        try:
            parts.append(Path(db).read_text(errors="replace").rstrip("\n"))
        except OSError:
            continue
    parts += lines(pads, guide)
    return "\n".join(parts) + "\n"
