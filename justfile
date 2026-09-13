# Every recipe runs inside `nix develop` against an isolated data dir (.dev/), so nothing
# here touches ~/.config/universe or ~/.local/share/universe. Nothing runs in the background:
# a game is a transient systemd unit that closes its own session (`universe session-end`).

dev := justfile_directory() / ".dev"
export UNIVERSE_DATA_HOME := dev / "data"
export UNIVERSE_CONFIG_HOME := dev / "config"
export UNIVERSE_STATE_HOME := dev / "state"
export UNIVERSE_CACHE_HOME := dev / "cache"
export UNIVERSE_MODULES_PATH := justfile_directory() / "modules"
export UNIVERSE_BIN := justfile_directory() / "target/debug/universe"
export RUST_LOG := env("RUST_LOG", "info")
export VIRTUAL_ENV := justfile_directory() / ".venv"

nix := "nix develop --quiet --command"
python := VIRTUAL_ENV / "bin/python"
ui_bin := VIRTUAL_ENV / "bin/universe-ui"

# First run: build, create .dev/ with a ready config.toml, run doctor
setup: build env
    @{{ nix }} target/debug/universe doctor

# Build the CLI (debug)
build:
    @{{ nix }} cargo build

# .venv on the dev shell's Python: the core extension via maturin, universe_ui editable
develop:
    #!/usr/bin/env -S nix develop --quiet --command bash
    set -euo pipefail
    home="$(dirname "$(command -v python3)")"
    if ! grep -qxF "home = $home" "$VIRTUAL_ENV/pyvenv.cfg" 2>/dev/null; then
        rm -rf "$VIRTUAL_ENV"
        python3 -m venv --system-site-packages "$VIRTUAL_ENV"
    fi
    env -u RUST_LOG maturin develop --quiet -m crates/universe-py/Cargo.toml
    "$VIRTUAL_ENV/bin/pip" install --quiet --no-index --no-build-isolation --no-deps -e ui

# The CLI against .dev/: just cli migrate --apply, just cli gog scan, just cli play <game>…
cli *args: build env
    @{{ nix }} target/debug/universe {{ args }}

# Host UI on the in-process core; flags pass through (--windowed, --no-gamepad, --keys "…")
ui *args: build develop env
    @{{ nix }} {{ ui_bin }} {{ args }}

# Host UI on a fixture library, no core — for UI work
ui-fake *args: develop
    @{{ nix }} {{ ui_bin }} --fake {{ args }}

# Follow the units of games, hooks and session ends
logs:
    journalctl --user -f -u 'universe-*'

# Open the dev shell (cargo, PySide6, Qt paths, module runtime on PATH)
shell:
    nix develop

test: build develop env
    @{{ nix }} cargo test
    @{{ nix }} {{ python }} -m pytest -q ui
    @{{ nix }} python3 -m pytest -q modules

# Build the flake packages and run the sandboxed checks
check:
    nix build .#universe .#universe-ui --no-link
    nix flake check

# game.toml, sessions.jsonl, media/ and journal/ are copied from ~/.local/share/universe; the
# recordings stay where they are (sessions.jsonl points at them by absolute path, nothing in dev writes there).
# Copy real games into .dev/ to test the recording player and the journal: just seed [id…]
seed *ids: env
    #!/usr/bin/env bash
    set -euo pipefail
    cfg="{{ dev }}/config/config.toml"
    if grep -E '^\s*recordings_root\s*=' "$cfg" | grep -qv '{{ dev }}/'; then
        echo "recordings_root in $cfg leaves {{ dev }}/: refusing to seed" >&2; exit 1
    fi
    src="${XDG_DATA_HOME:-$HOME/.local/share}/universe/games"
    ids="{{ ids }}"
    [ -n "$ids" ] || ids="super-mario-bros-wonder xenoblade-chronicles-x-definitive-edition assassins-creed-mirage"
    for id in $ids; do
        [ -d "$src/$id" ] || { echo "no such game: $src/$id" >&2; exit 1; }
        dest="{{ dev }}/data/games/$id"
        rm -rf "$dest"
        mkdir -p "$dest"
        for f in game.toml sessions.jsonl media journal; do
            [ -e "$src/$id/$f" ] && cp -r "$src/$id/$f" "$dest/"
        done
        echo "seeded $id ($(grep -c '"recording":"/' "$dest/sessions.jsonl" 2>/dev/null || echo 0) recordings, $(ls "$dest/journal"/*.json 2>/dev/null | wc -l) entries)"
    done

# Trash .dev/ (config, data, recordings, journal) and .venv
clean:
    trash "{{ dev }}" "{{ VIRTUAL_ENV }}"

env:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p "{{ dev }}"/{data,config,state,cache,recordings,journal}
    cfg="{{ dev }}/config/config.toml"
    [ -e "$cfg" ] && exit 0
    cat > "$cfg" <<EOF
    schema = 1
    [paths]
    recordings_root = "{{ dev }}/recordings"
    journal_root = "{{ dev }}/journal"
    overrides = "~/Dotfiles/home/config/pegasus-art"   # hand-picked art; drop the line to test without
    [modules]
    enabled = ["gog", "capture", "journal"]
    [modules.capture]
    min_duration_s = 20
    EOF
    echo "wrote $cfg"
