"""Journal module: image curation, the stub pipeline end to end, the Markdown
round trip (entries -> note -> migrate -> entries) and codex argument composition."""
import json
import os
import shutil
import stat
import subprocess
import sys
from datetime import datetime
from pathlib import Path

import pytest

MODULE_DIR = Path(__file__).resolve().parents[1]
BIN_DIR = MODULE_DIR / "bin"
sys.path.insert(0, str(BIN_DIR))

import images as img  # noqa: E402
import note  # noqa: E402
import prompt as pr  # noqa: E402
import providers  # noqa: E402
from _common import read_entries, read_sessions, validate_entry  # noqa: E402

SID = "20260911-120000"
needs_ffmpeg = pytest.mark.skipif(shutil.which("ffmpeg") is None, reason="ffmpeg not on PATH")


def ffmpeg(*args):
    subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", *args], check=True)


def make_png(path, color="blue"):
    path.parent.mkdir(parents=True, exist_ok=True)
    ffmpeg("-f", "lavfi", "-i", f"color=c={color}:size=64x48:duration=0.1:rate=10", "-frames:v", "1", "-update", "1", str(path))


def make_mkv(path, source, seconds):
    path.parent.mkdir(parents=True, exist_ok=True)
    sep = ":" if "=" in source else "="
    ffmpeg("-f", "lavfi", "-i", f"{source}{sep}duration={seconds}:size=320x240:rate=10", "-c:v", "mpeg4", "-q:v", "5", str(path))


def write_shim(path, body):
    path.write_text(f"#!{shutil.which('bash')}\n{body}\n")
    path.chmod(path.stat().st_mode | stat.S_IEXEC | stat.S_IXGRP | stat.S_IXOTH)


# --- image curation ---------------------------------------------------------------

@needs_ffmpeg
def test_select_screenshots_window(tmp_path):
    att = tmp_path / "attachments"
    for name in ("20260911-115900.png", "20260911-120130.png", "20260911-120245.jpg", "20260911-130000.png", "20260911-120000-1.png"):
        make_png(att / name)
    (att / "notes.txt").write_text("not an image")
    start, end = datetime(2026, 9, 11, 12, 0, 0), datetime(2026, 9, 11, 12, 3, 20)
    shots = img.select_screenshots(str(att), start, end)
    # the head/tail grace admits 11:59:00, the frame file and the 13:00 shot stay out
    assert [os.path.basename(s.file) for s in shots] == ["20260911-115900.png", "20260911-120130.png", "20260911-120245.jpg"]
    assert all(s.kind == "shot" for s in shots)


@needs_ffmpeg
def test_extract_frames_from_mkv(tmp_path):
    rec = tmp_path / "rec.mkv"
    make_mkv(rec, "testsrc", 120)
    start = datetime(2026, 9, 11, 12, 0, 0)
    frames = img.extract_frames(str(rec), [], start, 8, str(tmp_path / "frames"), 120)
    assert 1 <= len(frames) <= 8
    offs = [f.off for f in frames]
    assert offs == sorted(offs)
    tail_start = 120 - min(img.TAIL_WINDOW_SEC, 120 / 4)
    assert any(f.tail for f in frames) and all(f.tail == (f.off >= tail_start) for f in frames)
    assert all(os.path.getsize(f.file) > 0 and f.kind == "frame" for f in frames)
    for i, a in enumerate(frames):
        for b in frames[i + 1:]:
            assert img.hamming(a.hash, b.hash) >= img.DUP_DISTANCE


@needs_ffmpeg
def test_extract_frames_dedupes_static_and_drops_black(tmp_path):
    static = tmp_path / "static.mkv"
    make_mkv(static, "smptebars", 100)
    start = datetime(2026, 9, 11, 12, 0, 0)
    assert len(img.extract_frames(str(static), [], start, 6, str(tmp_path / "f1"), 100)) == 1
    black = tmp_path / "black.mkv"
    make_mkv(black, "color=c=black", 100)
    assert img.extract_frames(str(black), [], start, 6, str(tmp_path / "f2"), 100) == []


def test_dhash_and_review_normalization():
    flat = bytes([10] * 72)
    ramp = bytes(range(71, -1, -1))
    assert img.dhash(flat) == 0
    assert img.hamming(img.dhash(flat), img.dhash(ramp)) == 64
    assert img.dhash(b"short") is None
    order, unusable = img.normalize_review({"gallery": [3, 1, 9, 3, True], "unusable": [2, 3, 0]}, 4)
    assert (order, unusable) == ([1, 4], {2, 3})
    assert img.normalize_review(None, 2) == ([1, 2], set())
    assert img.even_sample(list(range(10)), 4) == [0, 3, 6, 9]


# --- the stub pipeline, end to end ------------------------------------------------------

@pytest.fixture
def fakebin(tmp_path):
    bindir = tmp_path / "fakebin"
    bindir.mkdir()
    write_shim(bindir / "busctl", f'''printf "%s\\n" "$@" > "{bindir}/busctl.args"
if [ "${{FAKE_BUSCTL_EXIT:-0}}" != "0" ]; then echo "${{FAKE_BUSCTL_STDERR:-Failed to connect to bus: No such file or directory}}" >&2; exit "${{FAKE_BUSCTL_EXIT}}"; fi
exit 0''')
    return bindir


def run_process(tmp_path, fakebin, settings, extra_env=None, recording=None):
    journal_dir = tmp_path / "games" / "testgame" / "journal"
    journal_dir.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ)
    env.update({
        "PATH": f"{fakebin}:{env.get('PATH', '')}",
        "GAME_ID": "testgame", "GAME_SLUG": "testgame", "GAME_TITLE": "Test Game: Redux",
        "SESSION_ID": SID, "SESSION_STARTED_AT": "2026-09-11T12:00:00+02:00",
        "SESSION_ENDED_AT": "2026-09-11T12:03:20+02:00", "SESSION_DURATION_S": "200",
        "RECORDING_PATH": str(recording or ""),
        "JOURNAL_DIR": str(journal_dir), "MODULE_DATA_DIR": str(tmp_path / "data"), "MODULE_DIR": str(MODULE_DIR),
        "MODULE_SETTINGS_JSON": json.dumps({"provider": "stub", "journal_root": str(tmp_path / "root"), **settings}),
    })
    env.update(extra_env or {})
    res = subprocess.run([sys.executable, str(BIN_DIR / "process")], env=env, capture_output=True, text=True)
    return res, journal_dir


@needs_ffmpeg
def test_stub_pipeline_writes_entry_note_and_memory(tmp_path, fakebin):
    rec = tmp_path / "rec.mkv"
    make_mkv(rec, "testsrc", 200)
    att = tmp_path / "games" / "testgame" / "journal" / "attachments"
    for name in ("20260911-120130.png", "20260911-120245.png", "20260911-130000.png"):
        make_png(att / name)
    (tmp_path / "games" / "testgame" / "sessions.jsonl").write_text(json.dumps({
        "session": SID, "game": "testgame", "started_at": "2026-09-11T12:00:00+02:00",
        "ended_at": "2026-09-11T12:03:20+02:00", "duration_s": 200, "source": "daemon", "recording": str(rec),
    }) + "\n")

    res, journal_dir = run_process(tmp_path, fakebin, {}, {"FAKE_BUSCTL_EXIT": "1"}, recording=rec)
    assert res.returncode == 0, res.stderr
    assert "bus unavailable" in res.stderr

    entry = json.loads((journal_dir / f"{SID}.json").read_text())
    assert validate_entry(entry) == []
    assert entry["session"] == SID and entry["game"] == "testgame" and entry["provider"] == "stub" and entry["lang"] == "en"
    assert entry["title"] == "Stub session of Test Game: Redux"
    assert entry["paragraphs"][0].startswith("You played Test Game: Redux")
    assert [p for p in entry["paragraphs"] if p.startswith("- ")] == ["- **Main quest:** Reached the first checkpoint.", "- **Exploration:** Looked at every image, all of them."]
    assert entry["next_up"] == "Resume at the first checkpoint and keep going."
    assert entry["written_at"][:10] == datetime.now().strftime("%Y-%m-%d") and entry["written_at"][19] in "+-"
    shots = [i for i in entry["images"] if note.SHOT_IMAGE_RE.search(i)]
    frames = [i for i in entry["images"] if not note.SHOT_IMAGE_RE.search(i)]
    assert shots == ["attachments/20260911-120130.png", "attachments/20260911-120245.png"]
    assert 1 <= len(frames) <= img.GALLERY_FRAME_TARGET - 2
    assert frames == [f"attachments/{SID}-{n}.png" for n in range(1, len(frames) + 1)]
    assert all((journal_dir / f).stat().st_size > 0 for f in entry["images"])

    args = (fakebin / "busctl.args").read_text().splitlines()
    assert args[:8] == ["--user", "call", "io.github.ilyasturki.Universe", "/io/github/ilyasturki/Universe", "io.github.ilyasturki.Universe.Journal1", "AddEntry", "ss", SID]
    assert json.loads(args[8]) == entry

    memory = json.loads((tmp_path / "data" / "memory" / "testgame.json").read_text())
    assert memory["profile"] == "arcade" and memory["language"] == "en" and "Stub Hero" in memory["entities"]["characters"]

    note_path = tmp_path / "root" / "testgame" / "Test Game Redux.md"
    text = note_path.read_text()
    assert text.startswith('---\ngame: "Test Game: Redux"\nsessions: 1\nfirst_played: 2026-09-11\nlast_played: 2026-09-11\ncover: attachments/20260911-120130.png\n---\n\n# Journal: Test Game: Redux\n\n')
    assert f"## #1 · Stub session of Test Game: Redux\n*11/09/2026 · 12h00 à 12h03 · 3 min*\n<!-- session: {SID} -->\n\n" in text
    assert "\n\n**Next up:** Resume at the first checkpoint and keep going.\n\n**Recording:** [rec.mkv](file://" in text
    assert "\n\n*Frames from the recording*\n\n![](attachments/" in text
    assert all((note_path.parent / f).exists() for f in entry["images"])
    assert list((tmp_path / "data" / "work").iterdir()) == []

    # a second run finds the entry and does nothing
    res2, _ = run_process(tmp_path, fakebin, {}, {"FAKE_BUSCTL_EXIT": "1"}, recording=rec)
    assert res2.returncode == 0 and "already exists" in res2.stderr
    assert note_path.read_text() == text


def test_stub_pipeline_hands_off_to_the_core(tmp_path, fakebin):
    res, journal_dir = run_process(tmp_path, fakebin, {"markdown_export": False})
    assert res.returncode == 0, res.stderr
    assert not (journal_dir / f"{SID}.json").exists()
    assert "Journal1.AddEntry" in res.stderr and not (tmp_path / "root").exists()
    entry = json.loads((fakebin / "busctl.args").read_text().splitlines()[8])
    assert entry["provider"] == "none" and entry["images"] == [] and entry["title"] == ""
    assert entry["paragraphs"] == ["This session’s recording holds no picture and no screenshot covers it, so there is nothing to summarize."]


def test_core_rejection_writes_nothing(tmp_path, fakebin):
    res, journal_dir = run_process(tmp_path, fakebin, {}, {"FAKE_BUSCTL_EXIT": "1", "FAKE_BUSCTL_STDERR": "Call failed: io.github.ilyasturki.Universe.Error.Invalid: bad"})
    assert res.returncode == 1 and not (journal_dir / f"{SID}.json").exists()
    assert not (tmp_path / "data" / "memory").exists() and not (tmp_path / "root").exists()


def test_disabled_and_forced_language(tmp_path, fakebin):
    res, journal_dir = run_process(tmp_path, fakebin, {"enabled": False})
    assert res.returncode == 0 and not (fakebin / "busctl.args").exists()
    res, journal_dir = run_process(tmp_path, fakebin, {"language": "fr"}, {"FAKE_BUSCTL_EXIT": "1"})
    assert res.returncode == 0, res.stderr
    entry = json.loads((journal_dir / f"{SID}.json").read_text())
    assert entry["lang"] == "fr" and entry["paragraphs"][0].startswith("L'enregistrement de cette session est vide")
    text = (tmp_path / "root" / "testgame" / "Test Game Redux.md").read_text()
    assert "# Journal : Test Game: Redux\n\n## 11/09/2026 · 12h00 à 12h03 · 3 min\n<!-- session:" in text
    assert "*L'enregistrement de cette session est vide" in text


# --- render <-> migrate round trip ------------------------------------------------------------

def sample_entries():
    return [
        {"session": "20260301-210000", "game": "sample", "written_at": "2026-03-01T22:30:00+01:00", "lang": "en",
         "title": "Into the Dome", "provider": "import",
         "paragraphs": ["Zachariah reached the Source after three failed runs.", "- **Main quest:** Cleared the gate.", "- **Side quest:** Talked to Amelia.", "Then the patrol reset."],
         "next_up": "Return to the Exchange and talk to Amelia.",
         "images": ["attachments/20260301-211500.png", "attachments/20260301-210000-1.png", "attachments/frames/frame-20260301-210000-02.jpg"]},
        {"session": "20260215-183000", "game": "sample", "written_at": "2026-02-15T19:45:00+01:00", "lang": "fr",
         "title": "Trois contrats et Port-péril", "provider": "import",
         "paragraphs": ["Le duo a enchaîné les sauvetages.", "- **Boss :** Tu as vaincu Corbin Claquebec."],
         "next_up": "Tu reprendras dans le Mausolée III.", "images": ["attachments/frames/frame-20260215-183000-01.jpg"]},
        {"session": "20260110-000500", "game": "sample", "written_at": "2026-01-10T00:07:00+01:00", "lang": "en",
         "title": "", "provider": "import", "paragraphs": [], "next_up": "", "images": []},
        {"session": "20251220-120000", "game": "sample", "written_at": "2025-12-20T12:01:00+01:00", "lang": "en",
         "title": "", "provider": "none",
         "paragraphs": ["This session’s recording holds no picture and no screenshot covers it, so there is nothing to summarize."],
         "next_up": "", "images": []},
        {"session": "20251201-230000", "game": "sample", "written_at": "2025-12-02T00:10:00+01:00", "lang": "en",
         "title": "First Glimpse", "provider": "import", "paragraphs": ["You reached the title screen."],
         "next_up": "Press any key.", "images": ["attachments/20251201-230100.png"]},
    ]


def sample_sessions():
    def s(sid, started, ended, dur, rec):
        return {"session": sid, "game": "sample", "started_at": started, "ended_at": ended, "duration_s": dur, "source": "import-journal", "recording": rec}
    return [
        s("20260301-210000", "2026-03-01T21:00:00+01:00", "2026-03-01T22:30:00+01:00", 5400, "/mnt/recordings/games/sample/20260301-210000.mkv"),
        s("20260215-183000", "2026-02-15T18:30:00+01:00", "2026-02-15T19:45:00+01:00", 4500, "/mnt/recordings/games/sample/003-20260215-183000-1h15m.mkv"),
        s("20260110-000500", "2026-01-10T00:05:00+01:00", "2026-01-10T00:07:00+01:00", 120, "/mnt/recordings/games/sample/002-20260110-000500-2m.mkv"),
        s("20251220-120000", "2025-12-20T12:00:00+01:00", "2025-12-20T12:01:00+01:00", 60, "/mnt/recordings/games/sample/001-20251220-120000-1m.mkv"),
        s("20251201-230000", "2025-12-01T23:00:00+01:00", "2025-12-02T00:10:00+01:00", 4200, None),
    ]


def test_render_then_migrate_round_trip(tmp_path):
    entries, sessions = sample_entries(), sample_sessions()
    journal_dir = tmp_path / "games" / "sample" / "journal"
    journal_dir.mkdir(parents=True)
    with open(tmp_path / "games" / "sample" / "sessions.jsonl", "w") as f:
        for s in sessions:
            f.write(json.dumps(s) + "\n")
    for e in entries:
        (journal_dir / f"{e['session']}.json").write_text(json.dumps(e))
    res = subprocess.run([sys.executable, str(BIN_DIR / "render"), "sample", "--journal-dir", str(journal_dir), "--journal-root", str(tmp_path / "root"), "--title", "Sample: The Game"], capture_output=True, text=True)
    assert res.returncode == 0, res.stderr
    note_path = Path(res.stdout.strip())
    text = note_path.read_text()
    assert note_path == tmp_path / "root" / "sample" / "Sample The Game.md"
    assert "\n## #4 · Into the Dome\n*01/03/2026 · 21h00 à 22h30 · 1 h 30 min*\n" in text
    assert "\n## #3 · Trois contrats et Port-péril\n" in text and "\n**Reprise :** Tu reprendras" in text and "\n**Enregistrement :** [003-" in text
    assert "\n## #2 · 10/01/2026 · 00h05 à 00h07 · 2 min\n<!-- session: 20260110-000500 -->\n\n**Recording:** [002-" in text
    assert "\n## #1 · 20/12/2025 · 12h00 à 12h01 · 1 min\n<!-- session: 20251220-120000 -->\n\n*This session’s recording" in text
    assert "\n## First Glimpse\n*01/12/2025 · 23h00 à 00h10 · 1 h 10 min*\n" in text
    assert "![](attachments/20260301-211500.png)\n\n*Frames from the recording*\n\n![](attachments/20260301-210000-1.png)\n![](attachments/frames/frame-20260301-210000-02.jpg)\n" in text

    out_dir = tmp_path / "migrated"
    res = subprocess.run([sys.executable, str(BIN_DIR / "migrate"), str(note_path), str(out_dir), "--game", "sample"], capture_output=True, text=True)
    assert res.returncode == 0, res.stderr
    assert read_entries(str(out_dir)) == sorted(entries, key=lambda e: e["session"], reverse=True)
    migrated = read_sessions(str(out_dir))
    for s in sessions:
        assert migrated[s["session"]] == s
    # rendering the migrated data reproduces the note; a second render is a no-op
    res = subprocess.run([sys.executable, str(BIN_DIR / "render"), "--journal-dir", str(out_dir), "--journal-root", str(tmp_path / "root2"), "--title", "Sample: The Game"], capture_output=True, text=True)
    assert res.returncode == 0 and Path(res.stdout.strip()).read_text() == text
    res = subprocess.run([sys.executable, str(BIN_DIR / "render"), "--journal-dir", str(out_dir), "--journal-root", str(tmp_path / "root2"), "--title", "Sample: The Game"], capture_output=True, text=True)
    assert "unchanged" in res.stderr


LEGACY_NOTE = """---
game: "Dishonored: Definitive Edition"
sessions: 3
first_played: 2024-03-09
last_played: 2026-04-25
cover: attachments/20260425-000349.png
---

# Journal: Dishonored: Definitive Edition
## First Glimpse of Dunwall
*25/04/2026 · 00h03 à 00h03 · 1 min*
<!-- session: 20260425-000349 -->

You launched Dishonored for the first time and reached the title screen overlooking Dunwall.

**Next up:** Resume from the title screen, press any key, and begin a new campaign.

![](attachments/20260425-000349.png)

## #9 · 22/12/2025 · 22h19 à 22h25 · 6 min
<!-- session: 20251222-221927 -->

**Recording:** [009-20251222-221927-6m.mkv](file:///mnt/recordings/games/dishonored/009-20251222-221927-6m.mkv)

## #2 · Trois contrats
*09/03/2024 · 18h58 à 20h27 · 1 h 28 min*
<!-- session: 20240309-185826 -->

Le duo a enchaîné les sauvetages au dernier souffle.

- **Boss :** Tu as vaincu Corbin Claquebec.
- **Niveau :** Tu as terminé Port-péril.

**Reprise :** Tu reprendras dans le Mausolée III.

**Enregistrement :** [002-20240309-185826-1h28m.mkv](file:///mnt/recordings/games/dishonored/002-20240309-185826-1h28m.mkv)

*Images extraites de l'enregistrement*

![](attachments/frames/frame-20240309-185826-01.jpg)
![](attachments/frames/frame-20240309-185826-02.jpg)
"""


def test_migrate_legacy_note_shapes(tmp_path):
    src = tmp_path / "legacy" / "Dishonored Definitive Edition.md"
    src.parent.mkdir()
    src.write_text(LEGACY_NOTE)
    (src.parent / "attachments").mkdir()
    (src.parent / "attachments" / "20260425-000349.png").write_bytes(b"png")
    out = tmp_path / "journal"
    res = subprocess.run([sys.executable, str(BIN_DIR / "migrate"), str(src), str(out)], capture_output=True, text=True)
    assert res.returncode == 0, res.stderr
    entries = read_entries(str(out))
    assert [e["session"] for e in entries] == ["20260425-000349", "20251222-221927", "20240309-185826"]
    virtual, bodyless, french = entries
    assert virtual["game"] == "dishonored-definitive-edition" and virtual["title"] == "First Glimpse of Dunwall" and virtual["lang"] == "en"
    assert virtual["images"] == ["attachments/20260425-000349.png"] and virtual["written_at"] == "2026-04-25T00:03:00+02:00"
    assert bodyless["title"] == "" and bodyless["paragraphs"] == [] and bodyless["next_up"] == "" and bodyless["lang"] == "en"
    assert french["lang"] == "fr" and french["paragraphs"] == ["Le duo a enchaîné les sauvetages au dernier souffle.", "- **Boss :** Tu as vaincu Corbin Claquebec.", "- **Niveau :** Tu as terminé Port-péril."]
    assert french["next_up"] == "Tu reprendras dans le Mausolée III." and french["title"] == "Trois contrats"
    assert (out / "attachments" / "20260425-000349.png").read_bytes() == b"png"
    sessions = read_sessions(str(out))
    assert sessions["20251222-221927"]["recording"] == "/mnt/recordings/games/dishonored/009-20251222-221927-6m.mkv"
    assert sessions["20260425-000349"]["recording"] is None and sessions["20240309-185826"]["duration_s"] == 5280
    title, parsed, parsed_sessions = note.parse_note(LEGACY_NOTE)
    rendered = note.render_note(parsed, {s["session"]: s for s in parsed_sessions}, title)
    assert rendered == LEGACY_NOTE.replace("# Journal: Dishonored: Definitive Edition\n## First", "# Journal: Dishonored: Definitive Edition\n\n## First")


# --- codex provider ------------------------------------------------------------------------

class FakeImage:
    def __init__(self, file, t, kind="frame", tail=False):
        self.file, self.t, self.kind, self.tail = file, t, kind, tail


def test_codex_exec_arguments(tmp_path, monkeypatch):
    calls = []
    answer = {"title": "Into the Dome", "body": "You did things.\n\n- **Boss:** Beat it.", "next": "Go on.",
              "images": {"gallery": [2, 1], "unusable": []},
              "memory": {"synopsis": "s", "entities": {"characters": [], "places": [], "bosses": []}, "language": "en", "profile": "narrative"}}

    def fake_run(args, **kw):
        calls.append((args, kw))
        Path(args[args.index("-o") + 1]).write_text(json.dumps(answer))
        return subprocess.CompletedProcess(args, 0, "", "")

    monkeypatch.setattr(providers.subprocess, "run", fake_run)
    ims = [FakeImage("/tmp/a.png", datetime(2026, 9, 11, 12, 1)), FakeImage("/tmp/b.png", datetime(2026, 9, 11, 12, 2), tail=True)]
    brief = pr.build_user_prompt("Test", datetime(2026, 9, 11, 12, 0), datetime(2026, 9, 11, 12, 30), 1800, 1, 1800, ims, "", None,
                                 providers.IMAGE_INTRO, pr.image_lines(ims))
    out = providers.run_codex("gpt-5.6-sol", brief, ims, str(tmp_path))
    assert out == answer
    args, kw = calls[0]
    schema, outp = str(tmp_path / "schema.json"), str(tmp_path / "entry.json")
    assert args[:-1] == ["codex", "exec", "--skip-git-repo-check", "--ignore-user-config", "--disable", "browser_use", "--disable", "computer_use",
                         "--ephemeral", "-C", str(tmp_path), "-s", "read-only", "-c", "approval_policy=never", "-c", "model=gpt-5.6-sol",
                         "-c", "model_reasoning_effort=high", "-c", "model_verbosity=medium", "-c", "project_doc_max_bytes=0",
                         "-c", "tools.web_search=true", "-c", "mcp_servers={}", "-i", "/tmp/a.png", "-i", "/tmp/b.png",
                         "--output-schema", schema, "-o", outp]
    assert args[-1].startswith(pr.SYSTEM_PROMPT + "\n\n---\n\nJeu : Test\nSession : 11/09/2026, de 12h00 a 12h30 (30 min)\nHistorique : 1re session")
    assert "- image 1 : image auto-extraite de l'enregistrement, vers 12h01\n- image 2 : image auto-extraite de l'enregistrement, DERNIERS INSTANTS de la session, vers 12h02" in args[-1]
    assert kw["cwd"] == str(tmp_path) and kw["timeout"] == providers.TIMEOUT_S
    assert json.loads(Path(schema).read_text()) == pr.OUTPUT_SCHEMA


def test_codex_quota_wall_and_retry(tmp_path, monkeypatch):
    attempts = []

    def walled(args, **kw):
        attempts.append(1)
        return subprocess.CompletedProcess(args, 1, "", "You've hit your usage limit. Try again at Sep 7th, 2026 12:07 PM.")

    monkeypatch.setattr(providers.subprocess, "run", walled)
    with pytest.raises(providers.QuotaExceeded) as info:
        providers.run_codex("m", "brief", [], str(tmp_path))
    assert info.value.until == datetime(2026, 9, 7, 12, 7) and len(attempts) == 1

    def flaky(args, **kw):
        attempts.append(2)
        return subprocess.CompletedProcess(args, 1, "", "Reconnecting... 5/5")

    attempts.clear()
    monkeypatch.setattr(providers.subprocess, "run", flaky)
    assert providers.run_codex("m", "brief", [], str(tmp_path)) is None
    assert len(attempts) == providers.RETRIES + 1


def test_acceptance_of_model_fields():
    assert pr.accept_result({"body": "Je vais vérifier les noms.", "next": "x"}) is None
    r = pr.accept_result({"title": '"The Dome — Again."', "body": "Voici le résumé de la session :\nTu as fait X — puis Y.\n\n**Reprise :** Continue.", "next": "**Next up:** Go on – now."})
    assert r["title"] == "The Dome, Again" and r["body"] == "Tu as fait X, puis Y." and r["next"] == "Go on, now."
    assert pr.paragraphs_from_body("Intro.\nMore.\n\n- a\n* b\n\nOutro.") == ["Intro. More.", "- a", "- b", "Outro."]
    merged = pr.merge_memory({"synopsis": "A long synopsis about the whole story so far.", "entities": {"characters": ["Zach"], "places": [], "bosses": []}, "language": "en", "profile": "narrative"},
                             {"synopsis": "Short.", "entities": {"characters": ["zach", "Amelia"], "places": ["Ophir"], "bosses": []}, "language": "french", "profile": "arcade"})
    assert merged == {"synopsis": "A long synopsis about the whole story so far.", "entities": {"characters": ["Zach", "Amelia"], "places": ["Ophir"], "bosses": []}, "language": "fr", "profile": "narrative"}
