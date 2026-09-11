"""tracker-md: row matching, Hours max(), Journal link, games.json fill, create_missing, --all."""
import importlib.machinery
import importlib.util
import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

MODULE_DIR = Path(__file__).resolve().parents[1]
SYNC = MODULE_DIR / "bin" / "sync"


@pytest.fixture
def sync_mod():
    loader = importlib.machinery.SourceFileLoader("tracker_md_sync", str(SYNC))
    spec = importlib.util.spec_from_loader("tracker_md_sync", loader)
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module


# ---------------------------------------------------------------------------
# Fixture tracker: hand-built, same table shape as the real files, small enough
# to eyeball. Independent of bin/sync's own render() so the tests aren't
# checking the code against itself.
# ---------------------------------------------------------------------------

def _table(path, heading, header_cols, rows):
    widths = [max(len(header_cols[i]), *(len(r[i]) for r in rows)) for i in range(len(header_cols) - 1)]

    def render_row(cells):
        head = " | ".join(c.ljust(w) for c, w in zip(cells[:-1], widths))
        return f"| {head} | {cells[-1]} |"

    sep = "|" + "|".join("-" * (w + 2) for w in widths) + "|---|"
    lines = [f"# {heading}", "", render_row(header_cols), sep] + [render_row(r) for r in rows]
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


@pytest.fixture
def tracker(tmp_path):
    root = tmp_path / "tracker"
    root.mkdir()

    _table(root / "playing.md", "Playing", ["Game", "Hours", "Vibes", "Journal"], [
        ["Alpha Quest", "5.0", "story", ""],
        ["Bolt", "", "", ""],
        ["Cyber City", "12.0", "combat, atmosphere", ""],
    ])
    _table(root / "hold.md", "On hold", ["Game", "Hours", "Vibes", "Journal"], [
        ["Dune Voyager", "9.8", "", "[↗](journal/dune-voyager/)"],
        ["Echo Frontier", "3.0", "", ""],
        ["The Technomancer", "", "", ""],
    ])
    _table(root / "played.md", "Played", ["Game", "Rating", "Hours", "Vibes", "Journal"], [
        ["Ghost Protocol", "amazing", "20.0", "story, combat", ""],
        ["Halcyon Days", "great", "", "", ""],
    ])
    _table(root / "dropped.md", "Dropped", ["Game", "Rating", "Hours", "Vibes", "Journal"], [
        ["Iron Vale", "meh", "1.6", "", ""],
    ])
    _table(root / "backlog.md", "Backlog", ["Game", "Hours", "Vibes", "Journal"], [
        ["Jade Requiem", "", "", ""],
    ])

    data_dir = root / ".data"
    data_dir.mkdir()
    (data_dir / "games.json").write_text(json.dumps({
        "alpha-quest": {"title": "Alpha Quest", "developer": "Studio A"},
        "halcyon-days": {"title": "Halcyon Days"},
        "cyber-city": {
            "title": "Cyber City", "developer": "Studio C", "publisher": "Pub C",
            "genre": ["action"], "release_year": 2020, "summary": "Existing summary",
            "metacritic": 80, "rawg_id": 111, "sgdb_id": 222, "steam_appid": 333,
        },
    }, indent=2), encoding="utf-8")

    (root / "journal" / "the-technomancer").mkdir(parents=True)
    (root / "journal" / ".archive" / "echo-frontier").mkdir(parents=True)

    return root


def run_sync(tracker, *, game_id, game_title, game_json, settings=None, extra_env=None, args=None):
    env = dict(os.environ)
    for var in ("UNIVERSE_JOURNAL_ROOT", "UNIVERSE_DATA_HOME"):
        env.pop(var, None)
    env["GAME_ID"] = game_id
    env["GAME_TITLE"] = game_title
    env["UNIVERSE_GAME_JSON"] = json.dumps(game_json)
    env["MODULE_SETTINGS_JSON"] = json.dumps({"root": str(tracker), **(settings or {})})
    if extra_env:
        env.update(extra_env)
    return subprocess.run(
        [sys.executable, str(SYNC), *(args or [])],
        capture_output=True, text=True, env=env,
    )


def run_all(tracker, data_home, settings=None):
    env = dict(os.environ)
    for var in ("UNIVERSE_JOURNAL_ROOT", "GAME_ID", "GAME_TITLE", "UNIVERSE_GAME_JSON"):
        env.pop(var, None)
    env["UNIVERSE_DATA_HOME"] = str(data_home)
    env["MODULE_SETTINGS_JSON"] = json.dumps({"root": str(tracker), **(settings or {})})
    return subprocess.run([sys.executable, str(SYNC), "--all"], capture_output=True, text=True, env=env)


def changed_lines(before, after):
    b, a = before.split("\n"), after.split("\n")
    return [i for i in range(max(len(b), len(a)))
            if (b[i] if i < len(b) else None) != (a[i] if i < len(a) else None)]


def read(path):
    return Path(path).read_text(encoding="utf-8")


def games_json(tracker):
    return json.loads(read(tracker / ".data" / "games.json"))


def row_cells(text, game_name):
    """Parsed cells of the row whose first column equals game_name, or None."""
    for line in text.splitlines():
        if not line.strip().startswith("|"):
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if cells and cells[0] == game_name:
            return cells
    return None


# ---------------------------------------------------------------------------
# Hours: max(existing, computed), one decimal, never down.
# ---------------------------------------------------------------------------

def test_hours_bumped_when_computed_is_higher(tracker):
    before = read(tracker / "playing.md")
    result = run_sync(tracker, game_id="alpha-quest", game_title="Alpha Quest",
                       game_json={"stats": {"hours": 8.34}})
    assert result.returncode == 0
    after = read(tracker / "playing.md")
    diff = changed_lines(before, after)
    assert len(diff) == 1, f"expected exactly one changed line, got {diff}"
    cells = row_cells(after, "Alpha Quest")
    assert cells[1] == "8.3"
    assert cells[2] == "story"  # Vibes untouched


def test_hours_never_goes_down(tracker):
    before_stat = (tracker / "playing.md").stat().st_mtime_ns
    before = read(tracker / "playing.md")
    result = run_sync(tracker, game_id="cyber-city", game_title="Cyber City",
                       game_json={"stats": {"hours": 3.0}})
    assert result.returncode == 0
    assert read(tracker / "playing.md") == before
    assert (tracker / "playing.md").stat().st_mtime_ns == before_stat


def test_hours_fills_blank_cell(tracker):
    result = run_sync(tracker, game_id="the-technomancer", game_title="The Technomancer",
                       game_json={"stats": {"hours": 0.9}})
    assert result.returncode == 0
    cells = row_cells(read(tracker / "hold.md"), "The Technomancer")
    assert cells[1] == "0.9"


def test_zero_hours_does_not_fabricate_a_value(tracker):
    before = read(tracker / "backlog.md")
    result = run_sync(tracker, game_id="jade-requiem", game_title="Jade Requiem",
                       game_json={"stats": {"hours": 0}})
    assert result.returncode == 0
    assert read(tracker / "backlog.md") == before


# ---------------------------------------------------------------------------
# Journal link
# ---------------------------------------------------------------------------

def test_journal_link_added_when_folder_exists(tracker):
    result = run_sync(tracker, game_id="the-technomancer", game_title="The Technomancer",
                       game_json={"stats": {"hours": 0}})
    assert result.returncode == 0
    cells = row_cells(read(tracker / "hold.md"), "The Technomancer")
    assert cells[-1] == "[↗](journal/the-technomancer/)"


def test_journal_link_archived_fallback(tracker):
    result = run_sync(tracker, game_id="echo-frontier", game_title="Echo Frontier",
                       game_json={"stats": {"hours": 0}})
    assert result.returncode == 0
    cells = row_cells(read(tracker / "hold.md"), "Echo Frontier")
    assert cells[-1] == "[↗](journal/.archive/echo-frontier/)"


def test_journal_link_not_overwritten(tracker):
    before = read(tracker / "hold.md")
    result = run_sync(tracker, game_id="dune-voyager", game_title="Dune Voyager",
                       game_json={"stats": {"hours": 0}})
    assert result.returncode == 0
    assert read(tracker / "hold.md") == before


def test_journal_link_disabled_by_setting(tracker):
    before = read(tracker / "hold.md")
    result = run_sync(tracker, game_id="the-technomancer", game_title="The Technomancer",
                       game_json={"stats": {"hours": 0}}, settings={"journal_link": False})
    assert result.returncode == 0
    assert read(tracker / "hold.md") == before


def test_journal_root_override_checks_elsewhere_but_link_stays_relative(tracker, tmp_path):
    # Echo Frontier's archived folder only exists under the tracker root's journal/;
    # an elsewhere UNIVERSE_JOURNAL_ROOT with a live folder must be what's checked,
    # while the written link text is always the fixed "journal/<slug>/" form.
    elsewhere = tmp_path / "elsewhere-journal"
    (elsewhere / "echo-frontier").mkdir(parents=True)
    result = run_sync(tracker, game_id="echo-frontier", game_title="Echo Frontier",
                       game_json={"stats": {"hours": 0}}, extra_env={"UNIVERSE_JOURNAL_ROOT": str(elsewhere)})
    assert result.returncode == 0
    cells = row_cells(read(tracker / "hold.md"), "Echo Frontier")
    assert cells[-1] == "[↗](journal/echo-frontier/)"  # live, not the .archive fallback


# ---------------------------------------------------------------------------
# games.json — fill blanks only, never touch a missing slug.
# ---------------------------------------------------------------------------

def test_metadata_fills_only_missing_fields(tracker):
    game_json = {
        "stats": {"hours": 0}, "release_year": 1999,
        "metadata": {
            "developers": ["New Dev"], "publishers": ["New Pub"], "genres": ["rpg"],
            "summary": "New summary", "metacritic": 91, "rawg_id": 999, "sgdb_id": 888, "steam_appid": 777,
        },
    }
    result = run_sync(tracker, game_id="cyber-city", game_title="Cyber City", game_json=game_json)
    assert result.returncode == 0
    entry = games_json(tracker)["cyber-city"]
    # every field was already present -> none of them move
    assert entry["developer"] == "Studio C"
    assert entry["publisher"] == "Pub C"
    assert entry["genre"] == ["action"]
    assert entry["release_year"] == 2020
    assert entry["summary"] == "Existing summary"
    assert entry["metacritic"] == 80
    assert entry["rawg_id"] == 111
    assert entry["sgdb_id"] == 222
    assert entry["steam_appid"] == 333


def test_metadata_fills_blank_entry(tracker):
    game_json = {
        "stats": {"hours": 0},
        "metadata": {"developers": ["Studio A2"], "summary": "Filled in", "metacritic": 70},
    }
    result = run_sync(tracker, game_id="alpha-quest", game_title="Alpha Quest", game_json=game_json)
    assert result.returncode == 0
    entry = games_json(tracker)["alpha-quest"]
    assert entry["developer"] == "Studio A"   # already present, untouched
    assert entry["summary"] == "Filled in"    # was absent, now filled
    assert entry["metacritic"] == 70          # was absent, now filled


def test_metadata_maps_all_nine_fields_on_a_bare_entry(tracker):
    game_json = {
        "stats": {"hours": 0}, "release_year": 2021,
        "metadata": {
            "developers": ["Studio H1", "Studio H2"], "publishers": ["Pub H"], "genres": ["adventure", "indie"],
            "summary": "A halcyon tale.", "metacritic": 88, "rawg_id": 4242, "sgdb_id": 5353, "steam_appid": 6464,
        },
    }
    result = run_sync(tracker, game_id="halcyon-days", game_title="Halcyon Days", game_json=game_json)
    assert result.returncode == 0
    entry = games_json(tracker)["halcyon-days"]
    assert entry["developer"] == "Studio H1, Studio H2"
    assert entry["publisher"] == "Pub H"
    assert entry["genre"] == ["adventure", "indie"]
    assert entry["release_year"] == 2021  # top-level of UNIVERSE_GAME_JSON, not under metadata
    assert entry["summary"] == "A halcyon tale."
    assert entry["metacritic"] == 88
    assert entry["rawg_id"] == 4242
    assert entry["sgdb_id"] == 5353
    assert entry["steam_appid"] == 6464
    assert entry["title"] == "Halcyon Days"  # untouched, was already there


def test_metadata_never_creates_a_new_sidecar_entry(tracker):
    before = read(tracker / ".data" / "games.json")
    result = run_sync(tracker, game_id="the-technomancer", game_title="The Technomancer",
                       game_json={"stats": {"hours": 0.5}, "metadata": {"developers": ["Spiders"]}})
    assert result.returncode == 0
    assert read(tracker / ".data" / "games.json") == before
    assert "the-technomancer" not in games_json(tracker)


def test_metadata_disabled_by_setting(tracker):
    before = read(tracker / ".data" / "games.json")
    result = run_sync(tracker, game_id="alpha-quest", game_title="Alpha Quest",
                       game_json={"stats": {"hours": 0}, "metadata": {"summary": "should not land"}},
                       settings={"update_metadata": False})
    assert result.returncode == 0
    assert read(tracker / ".data" / "games.json") == before


# ---------------------------------------------------------------------------
# create_missing
# ---------------------------------------------------------------------------

def test_create_missing_false_leaves_tables_untouched(tracker):
    stats_before = {p.name: p.stat().st_mtime_ns for p in tracker.glob("*.md")}
    result = run_sync(tracker, game_id="new-game", game_title="New Game",
                       game_json={"stats": {"hours": 1.0}})
    assert result.returncode == 0
    assert "[tracker-md]" in result.stderr
    for p in tracker.glob("*.md"):
        assert p.stat().st_mtime_ns == stats_before[p.name]


def test_create_missing_true_inserts_alphabetically(tracker):
    result = run_sync(tracker, game_id="new-game", game_title="New Game",
                       game_json={"stats": {"hours": 1.0}}, settings={"create_missing": True})
    assert result.returncode == 0
    after = read(tracker / "playing.md")
    game_lines = [l for l in after.splitlines() if l.startswith("|") and "---" not in l and "| Game " not in l]
    names = [l.split("|")[1].strip() for l in game_lines]
    assert names == sorted(names, key=str.lower)
    cells = row_cells(after, "New Game")
    assert cells is not None and cells[1] == "1.0"


# ---------------------------------------------------------------------------
# No write when nothing changed (mtime stable, across all files).
# ---------------------------------------------------------------------------

def test_no_write_when_already_up_to_date(tracker):
    run_sync(tracker, game_id="the-technomancer", game_title="The Technomancer",
             game_json={"stats": {"hours": 0.9}})
    md_stats = {p.name: p.stat().st_mtime_ns for p in tracker.glob("*.md")}
    json_stat = (tracker / ".data" / "games.json").stat().st_mtime_ns

    result = run_sync(tracker, game_id="the-technomancer", game_title="The Technomancer",
                       game_json={"stats": {"hours": 0.9}})
    assert result.returncode == 0
    for p in tracker.glob("*.md"):
        assert p.stat().st_mtime_ns == md_stats[p.name], f"{p.name} was rewritten with no change"
    assert (tracker / ".data" / "games.json").stat().st_mtime_ns == json_stat


# ---------------------------------------------------------------------------
# Matching: case-insensitive title, or slug(Game cell) == GAME_ID.
# ---------------------------------------------------------------------------

def test_matches_by_title_case_insensitive(tracker):
    result = run_sync(tracker, game_id="ghost-protocol", game_title="ghost protocol",
                       game_json={"stats": {"hours": 25.0}})
    assert result.returncode == 0
    cells = row_cells(read(tracker / "played.md"), "Ghost Protocol")
    assert cells[1] == "amazing" and cells[2] == "25.0"


def test_matches_by_slug_when_title_text_differs(tracker):
    # "Dune  Voyager" (double space) fails a case-insensitive text match against the
    # row's "Dune Voyager" but slugifies to the same "dune-voyager" as GAME_ID.
    result = run_sync(tracker, game_id="dune-voyager", game_title="Dune  Voyager",
                       game_json={"stats": {"hours": 15.0}})
    assert result.returncode == 0
    cells = row_cells(read(tracker / "hold.md"), "Dune Voyager")
    assert cells[1] == "15.0"


# ---------------------------------------------------------------------------
# --all: sums sessions.jsonl durations, no D-Bus.
# ---------------------------------------------------------------------------

def _write_game(data_home, slug, title, durations_s):
    game_dir = data_home / "games" / slug
    game_dir.mkdir(parents=True)
    (game_dir / "game.toml").write_text(f'schema = 1\nid = "{slug}"\ntitle = "{title}"\n', encoding="utf-8")
    (game_dir / "sessions.jsonl").write_text(
        "\n".join(json.dumps({"duration_s": s}) for s in durations_s) + "\n", encoding="utf-8",
    )


def test_all_leaves_hours_unchanged_when_lower(tracker, tmp_path):
    data_home = tmp_path / "data-home"
    _write_game(data_home, "alpha-quest", "Alpha Quest", [3600 * 2, 1800])  # 2.5h < existing 5.0h
    result = run_all(tracker, data_home)
    assert result.returncode == 0
    cells = row_cells(read(tracker / "playing.md"), "Alpha Quest")
    assert cells[1] == "5.0"


def test_all_bumps_when_session_hours_exceed_existing(tracker, tmp_path):
    data_home = tmp_path / "data-home"
    _write_game(data_home, "cyber-city", "Cyber City", [3600 * 15])  # 15h > existing 12.0h
    result = run_all(tracker, data_home)
    assert result.returncode == 0
    cells = row_cells(read(tracker / "playing.md"), "Cyber City")
    assert cells[1] == "15.0"


# ---------------------------------------------------------------------------
# Pure-function unit tests
# ---------------------------------------------------------------------------

def test_slugify_matches_sanitize_game_name(sync_mod):
    assert sync_mod.slugify("Assassin's Creed IV Black Flag") == "assassins-creed-iv-black-flag"
    assert sync_mod.slugify("Pokémon HeartGold") == "pokemon-heartgold"
    assert sync_mod.slugify("de Blob") == "de-blob"
    assert sync_mod.slugify("") == "unknown"


def test_compute_hours_cell(sync_mod):
    assert sync_mod.compute_hours_cell("", 0.9) == "0.9"
    assert sync_mod.compute_hours_cell("5.0", 3.0) is None
    assert sync_mod.compute_hours_cell("5.0", 8.34) == "8.3"
    assert sync_mod.compute_hours_cell("", 0) is None
    assert sync_mod.compute_hours_cell("5.0", 5.0) is None
