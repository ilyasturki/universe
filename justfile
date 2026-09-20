# Every recipe runs inside `nix develop` against .dev/; UNIVERSE_DEV names another profile (`UNIVERSE_DEV=.dev-empty just ui`).

dev := justfile_directory() / env("UNIVERSE_DEV", ".dev")
export UNIVERSE_DATA_HOME := dev / "data"
export UNIVERSE_CONFIG_HOME := dev / "config"
export UNIVERSE_STATE_HOME := dev / "state"
export UNIVERSE_CACHE_HOME := dev / "cache"
export UNIVERSE_MODULES_PATH := justfile_directory() / "modules"
export UNIVERSE_SOURCES_PATH := justfile_directory() / "sources"
export UNIVERSE_BIN := justfile_directory() / "target/debug/universe"
export RUST_LOG := env("RUST_LOG", "info")
export VIRTUAL_ENV := justfile_directory() / ".venv"

nix := "nix develop --quiet --command"
python := VIRTUAL_ENV / "bin/python"
ui_bin := VIRTUAL_ENV / "bin/universe-ui"

# First run: build, create .dev/ with a ready config.toml, wire the git hooks, run doctor
setup: build env
    @git config core.hooksPath .githooks
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

# The CLI against .dev/: just cli migrate --apply, just cli scan gog, just cli play <game>…
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

# Every suite, or the ones named: rust, python, or one of python's ui, modules, sources, bindings
test *suites: build develop env
    #!/usr/bin/env -S nix develop --quiet --command bash
    set -euo pipefail
    cd "{{ justfile_directory() }}"
    suites="{{ suites }}"
    [ -n "$suites" ] || suites="rust python"
    for suite in $suites; do
        case "$suite" in
            rust) cargo test ;;
            python) "{{ python }}" -m pytest -q ;;
            bindings) "{{ python }}" -m pytest -q crates/universe-py/tests ;;
            ui|modules|sources) "{{ python }}" -m pytest -q "$suite" ;;
            *) echo "test: unknown suite '$suite' (rust, python, ui, modules, sources, bindings)" >&2; exit 2 ;;
        esac
    done

# Format every tree (rust python qml js nix), the ones named, or `--files a.rs b.py`; `--check` only reports
fmt *args:
    @{{ nix }} tools/fmt {{ args }}

# Every linter (rust python fmt js shell actions) or the sections named; what CI runs
lint *args:
    @{{ nix }} tools/lint {{ args }}

# Every local gate, lint then every suite: what the pre-push hook runs (`just check` is the sandboxed version)
verify: lint test

# Against this machine: transient units on the user systemd, a scope around the test process, the InputPlumber daemon (pads hidden ~10 s), the DRM cards
test-live: build
    @{{ nix }} cargo test -- --ignored live

# Build the flake packages and run the sandboxed checks: what nixos-rebuild and CI run
check:
    nix build .#universe .#universe-ui --no-link
    nix flake check

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

# A stand-in game in the profile (a Vulkan window MangoHud sees), to drive the launch path without a real game: just sample [install|remove]
sample *args: build env
    @{{ nix }} tools/sample {{ args }}

# The app as a new user meets it: a blank profile, no config.toml, ~/Games ~/Videos ~/Documents under it. just fresh ui [flags] | cli doctor | sample install. FRESH=<name> is another such profile; FRESH=hm gets a home-manager install's read-only `schema = 1` config.toml.
fresh cmd *args: build develop
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}/.dev-fresh${FRESH:+-$FRESH}"
    case "{{ cmd }}" in
        ui) bin="{{ ui_bin }}" ;;
        cli) bin="{{ UNIVERSE_BIN }}" ;;
        sample) bin="{{ justfile_directory() }}/tools/sample" ;;
        *) echo "fresh: ui | cli | sample" >&2; exit 1 ;;
    esac
    mkdir -p "$root"/{data,config,state,cache}
    if [ "${FRESH:-}" = hm ] && [ ! -e "$root/config/config.toml" ]; then
        printf 'schema = 1\n' > "$root/config/config.toml"
        chmod a-w "$root/config/config.toml"
    fi
    export UNIVERSE_DATA_HOME="$root/data" UNIVERSE_CONFIG_HOME="$root/config" UNIVERSE_STATE_HOME="$root/state" UNIVERSE_CACHE_HOME="$root/cache"
    export XDG_GAMES_DIR="$root/home/Games" XDG_VIDEOS_DIR="$root/home/Videos" XDG_DOCUMENTS_DIR="$root/home/Documents"
    exec {{ nix }} "$bin" {{ args }}

# Trash the profile (config, data, recordings, journal) and .venv
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
    games_root = "{{ dev }}/data/games"
    recordings_root = "{{ dev }}/recordings"
    journal_root = "{{ dev }}/journal"
    overrides = "~/Dotfiles/home/config/pegasus-art"   # hand-picked art; drop the line to test without
    [modules]
    enabled = ["capture", "journal"]
    [modules.capture]
    min_duration_s = 20
    [sources]
    enabled = ["gog"]
    EOF
    echo "wrote $cfg"

# Release: rewrite every copy of the version, commit `chore(release): vX.Y.Z`, tag vX.Y.Z (no push).
bump level: check
    #!/usr/bin/env -S nix develop --quiet --command bash
    set -euo pipefail
    cd "{{ justfile_directory() }}"
    cur="$(sed -n '/^\[workspace.package\]/,/^\[/{s/^version = "\(.*\)"$/\1/p}' Cargo.toml)"
    IFS=. read -r maj min pat <<< "$cur"
    case "{{ level }}" in
        major) new="$((maj + 1)).0.0" ;;
        minor) new="$maj.$((min + 1)).0" ;;
        patch) new="$maj.$min.$((pat + 1))" ;;
        *) new="{{ level }}" ;;
    esac
    [[ "$new" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "bump: '{{ level }}' is not patch|minor|major|X.Y.Z" >&2; exit 1; }
    [[ "$new" != "$cur" ]] || { echo "bump: already at $cur" >&2; exit 1; }
    git diff --quiet && git diff --cached --quiet || { echo "bump: working tree is not clean" >&2; exit 1; }
    ! git rev-parse -q --verify "refs/tags/v$new" >/dev/null || { echo "bump: tag v$new exists" >&2; exit 1; }
    copies=(ui/pyproject.toml modules/*/module.toml sources/*/source.toml docs/api.md)
    sed -i "/^\[workspace.package\]/,/^\[/s/^version = \"$cur\"$/version = \"$new\"/" Cargo.toml
    sed -i "s/^version = \"$cur\"$/version = \"$new\"/" "${copies[@]}"
    cargo update --workspace --offline --quiet
    git add Cargo.toml Cargo.lock "${copies[@]}"
    git commit --quiet -m "chore(release): v$new"
    git tag -a "v$new" -m "v$new"
    echo "$cur -> $new: committed and tagged v$new (not pushed)"
