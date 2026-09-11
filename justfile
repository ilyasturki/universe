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

nix := "nix develop --quiet --command"
ui_py := "env PYTHONPATH=" + justfile_directory() / "ui" + ":" + dev / "py" + " python3 -m universe_ui"

# First run: build, create .dev/ with a ready config.toml, run doctor
setup: build env
    @{{ nix }} target/debug/universe doctor

# Build the CLI and the Python module of the core (debug)
build:
    @{{ nix }} cargo build

# The CLI against .dev/: just cli migrate --apply, just cli gog scan, just cli play <game>…
cli *args: build env
    @{{ nix }} target/debug/universe {{ args }}

# Host UI on the in-process core; flags pass through (--fullscreen, --no-gamepad, --keys "…")
ui *args: build env
    @{{ nix }} {{ ui_py }} {{ args }}

# Host UI on a fixture library, no core — for UI work
ui-fake *args:
    @{{ nix }} {{ ui_py }} --fake {{ args }}

# Follow the units of games, hooks and session ends
logs:
    journalctl --user -f -u 'universe-*'

# Open the dev shell (cargo, PySide6, Qt paths, module runtime on PATH)
shell:
    nix develop

test: build env
    @{{ nix }} cargo test
    @{{ nix }} env PYTHONPATH={{ dev }}/py python3 -m pytest -q ui
    @{{ nix }} python3 -m pytest -q modules

# Build the flake packages and run the sandboxed checks
check:
    nix build .#universe .#universe-ui --no-link
    nix flake check

# Trash .dev/ (config, data, recordings, journal)
clean:
    trash "{{ dev }}"

env:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p "{{ dev }}"/{data,config,state,cache,recordings,journal,py}
    ln -sfn "{{ justfile_directory() }}/target/debug/libuniverse_core.so" "{{ dev }}/py/universe_core.so"
    cfg="{{ dev }}/config/config.toml"
    [ -e "$cfg" ] && exit 0
    cat > "$cfg" <<EOF
    schema = 1
    [paths]
    recordings_root = "{{ dev }}/recordings"
    journal_root = "{{ dev }}/journal"
    [modules]
    enabled = ["gog", "capture", "journal"]
    [modules.capture]
    min_duration_s = 20
    # [modules.tracker-md]
    # root = "~/Documents/notes/games"   # writes Hours into the real tracker; enable on purpose
    EOF
    echo "wrote $cfg"
