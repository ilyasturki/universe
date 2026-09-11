# Every recipe runs inside `nix develop` against an isolated data dir (.dev/),
# so nothing here touches ~/.config/universe or ~/.local/share/universe.
# One daemon per session bus: stop a home-manager `universed` before using these.

dev := justfile_directory() / ".dev"
export UNIVERSE_DATA_HOME := dev / "data"
export UNIVERSE_CONFIG_HOME := dev / "config"
export UNIVERSE_STATE_HOME := dev / "state"
export UNIVERSE_CACHE_HOME := dev / "cache"
export UNIVERSE_MODULES_PATH := justfile_directory() / "modules"
export RUST_LOG := env("RUST_LOG", "info")

nix := "nix develop --quiet --command"
ui_py := "env PYTHONPATH=" + justfile_directory() / "ui" + " python3 -m universe_ui"

# First run: build, create .dev/ with a ready config.toml, run doctor
setup: build _env
    @{{ nix }} target/debug/universe doctor

# Build the daemon and the CLI (debug)
build:
    @{{ nix }} cargo build

# The CLI against .dev/ (the daemon is spawned on demand): just cli migrate --apply, just cli gog scan…
cli *args: build _env
    @{{ nix }} target/debug/universe {{ args }}

# Host UI on the dev daemon; flags pass through (--fullscreen, --no-gamepad, --keys "…")
ui *args: build _env
    @{{ nix }} target/debug/universe status >/dev/null
    @{{ nix }} {{ ui_py }} {{ args }}

# Host UI on a fixture library, no daemon — for UI work
ui-fake *args:
    @{{ nix }} {{ ui_py }} --fake {{ args }}

# Run the daemon in the foreground with logs (the CLI otherwise spawns it silently)
daemon: build _env
    -pkill -x universed
    @{{ nix }} target/debug/universed

# Kill the daemon after a rebuild; the next cli or ui call respawns it
restart:
    -pkill -x universed

# Follow the log of a CLI-spawned daemon
logs:
    tail -n 50 -f "{{ dev }}/state/universed.log"

test:
    @{{ nix }} cargo test
    @{{ nix }} python3 -m pytest -q ui
    @{{ nix }} python3 -m pytest -q modules

# Build the flake packages and run the sandboxed checks
check:
    nix build .#universe .#universe-ui --no-link
    nix flake check

# Trash .dev/ (config, data, recordings, journal, daemon log)
clean: restart
    trash "{{ dev }}"

_env:
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
    [modules]
    enabled = ["gog", "capture", "journal"]
    [modules.capture]
    min_duration_s = 20
    # [modules.tracker-md]
    # root = "~/Documents/notes/games"   # writes Hours into the real tracker; enable on purpose
    EOF
    echo "wrote $cfg"
