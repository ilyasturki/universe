#!/bin/sh
# Universe for one user: PREFIX/lib/universe holds the CLI, the modules and the UI's Python venv; PREFIX/bin gets universe, universe-ui and, where GTK is new enough, universe-desktop. Root is only asked for (sudo) to put the udev rule and the uhid load under /etc, since /usr is read-only on SteamOS and Bazzite.
set -eu

usage() {
    cat <<'USAGE'
usage: install.sh [--prefix DIR] [--version vX.Y.Z] [--uninstall]

  --prefix DIR   install under DIR (default ~/.local)
  --version TAG  the release to download (default the latest)
  --uninstall    remove what an install put under DIR; config, library and
                 recordings stay

Run from a release tarball it installs that tarball; from a git checkout it
builds the checkout (cargo needed); piped from curl it downloads the release.

The virtual pads and the key macros need /dev/uhid and /dev/uinput opened to
your session: a udev rule and a module load under /etc, which it installs as
root through sudo (asking for your password) or prints the commands for.
USAGE
}

repo_url=https://github.com/ilyasturki/universe
prefix="$HOME/.local"
version=latest
uninstall=0
while [ $# -gt 0 ]; do
    case "$1" in
        --prefix) prefix="$2"; shift 2 ;;
        --version) version="$2"; shift 2 ;;
        --uninstall) uninstall=1; shift ;;
        -h | --help) usage; exit 0 ;;
        *) echo "install.sh: unknown argument $1" >&2; usage >&2; exit 2 ;;
    esac
done
lib="$prefix/lib/universe"
manifest="$lib/installed-files"
rules=/etc/udev/rules.d/70-universe.rules
load=/etc/modules-load.d/universe.conf

say() { printf '%s\n' "$*"; }
die() { printf 'install.sh: %s\n' "$*" >&2; exit 1; }

remove_installed() {
    [ -f "$manifest" ] || return 0
    while IFS= read -r f; do rm -f "$prefix/$f"; done < "$manifest"
}

# sudo prompts on /dev/tty, so a `curl | sh` still asks; with no terminal only a passwordless sudo goes on
as_root() {
    if [ "$(id -u)" = 0 ]; then
        "$@"
    elif command -v sudo >/dev/null && { sudo -n true 2>/dev/null || (: </dev/tty) 2>/dev/null; }; then
        sudo "$@"
    else
        return 1
    fi
}

install_system_files() {
    if [ ! -f "$lib/system/${rules##*/}" ] || [ -f "/usr/lib/udev/rules.d/${rules##*/}" ] \
        || { cmp -s "$lib/system/${rules##*/}" "$rules" && cmp -s "$lib/system/${load##*/}" "$load"; }; then
        return 0
    fi
    say "root (sudo) puts the pad rules in place: $rules and $load open /dev/uhid and /dev/uinput to your session for the virtual pads and the key macros"
    # shellcheck disable=SC2016
    if as_root sh -c 'install -Dm644 "$1" "$3" && install -Dm644 "$2" "$4" || exit 1
        udevadm control --reload 2>/dev/null
        modprobe uhid 2>/dev/null
        udevadm trigger --action=change --subsystem-match=misc --subsystem-match=hidraw 2>/dev/null
        exit 0' sh "$lib/system/${rules##*/}" "$lib/system/${load##*/}" "$rules" "$load"; then
        return 0
    fi
    say "note: without them the virtual pads and the key macros do nothing; as root, run"
    say "  install -Dm644 $lib/system/${rules##*/} $rules"
    say "  install -Dm644 $lib/system/${load##*/} $load"
    say "  then reboot"
}

if [ "$uninstall" = 1 ]; then
    [ -d "$lib" ] || die "nothing installed under $prefix"
    remove_installed
    rm -rf "$lib"
    if [ -f "$rules" ] || [ -f "$load" ]; then
        as_root rm -f "$rules" "$load" || say "note: $rules and $load stay; remove them as root"
    fi
    say "removed Universe from $prefix; your config, library and recordings stay under ~/.config/universe and ~/.local/share/universe"
    exit 0
fi

python=
for p in python3 python3.14 python3.13 python3.12 python3.11; do
    if command -v "$p" >/dev/null && "$p" -c 'import sys; sys.exit(not (3, 11) <= sys.version_info[:2] < (3, 15))' 2>/dev/null; then
        python="$p"
        break
    fi
done
[ -n "$python" ] || die "Python 3.11 to 3.14 is needed for the UI (PySide6); install python3.14 (sudo dnf install python3.14 on Fedora 45, whose python3 is 3.15)"
"$python" -c 'import venv, ensurepip' 2>/dev/null || die "$python cannot make a venv: install python3-venv (Debian, Ubuntu) or python3-pip"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
here="$(cd "$(dirname "$0")" 2>/dev/null && pwd || true)"

venv="$lib/venv"
if ! "$venv/bin/python" -c '' 2>/dev/null; then
    rm -rf "$venv"
    "$python" -m venv "$venv"
fi
pip() { "$venv/bin/python" -m pip --disable-pip-version-check --quiet "$@"; }

if [ -n "$here" ] && [ -x "$here/bin/universe" ] && [ -d "$here/wheels" ]; then
    dist="$here"
elif [ -n "$here" ] && [ -f "$here/Cargo.toml" ] && [ -x "$here/tools/dist" ]; then
    command -v cargo >/dev/null || die "building from a checkout needs cargo (rustup.rs)"
    say "building the checkout at $here"
    PYTHON="$venv/bin/python" "$here/tools/dist" "$work/dist" >/dev/null
    dist="$work/dist"
else
    [ "$(uname -m)" = x86_64 ] || die "releases are built for x86_64 only; build a checkout instead"
    if [ "$version" = latest ]; then
        url="$repo_url/releases/latest/download/universe-x86_64-linux.tar.gz"
    else
        url="$repo_url/releases/download/$version/universe-x86_64-linux.tar.gz"
    fi
    say "downloading $url"
    if command -v curl >/dev/null; then
        curl -fL --progress-bar -o "$work/universe.tar.gz" "$url"
    elif command -v wget >/dev/null; then
        wget -q -O "$work/universe.tar.gz" "$url"
    else
        die "curl or wget is needed to download the release"
    fi
    mkdir "$work/dist"
    tar -xzf "$work/universe.tar.gz" -C "$work/dist" --strip-components=1
    dist="$work/dist"
fi

say "installing the UI's Python packages into $venv (PySide6 is large the first time)"
pip install "$dist"/wheels/*.whl
pip install --force-reinstall --no-deps "$dist"/wheels/*.whl
if ! "$venv/bin/python" -c 'import xkbcommon' 2>/dev/null && ! pip install xkbcommon 2>/dev/null; then
    say "note: xkbcommon did not build (it needs libxkbcommon's headers and a C compiler); the on-screen keyboard shows qwerty"
fi
if ! "$venv/bin/python" -c 'import sdl2' 2>/dev/null; then
    pip install pysdl2-dll
fi

remove_installed
rm -rf "${lib:?}/bin" "$lib/modules" "$lib/sources" "$lib/system"
install -Dm755 "$dist/bin/universe" "$lib/bin/universe"
desktop=0
if [ -x "$dist/bin/universe-desktop" ]; then
    install -Dm755 "$dist/bin/universe-desktop" "$lib/bin/universe-desktop"
    if "$lib/bin/universe-desktop" --help >/dev/null 2>&1; then
        desktop=1
    else
        rm -f "$lib/bin/universe-desktop"
        say "note: Universe Desktop needs GTK 4.22 and libadwaita 1.9, older here: it is left out"
    fi
fi
cp -r "$dist/share/universe/modules" "$lib/modules"
cp -r "$dist/share/universe/sources" "$lib/sources"
# releases up to 0.0.9 ship neither
if [ -d "$dist/lib/udev" ]; then
    install -Dm644 "$dist/lib/udev/rules.d/${rules##*/}" "$dist/lib/modules-load.d/${load##*/}" -t "$lib/system"
fi

mkdir -p "$prefix/bin"
cat > "$prefix/bin/universe" <<EOF
#!/bin/sh
export UNIVERSE_BIN="$prefix/bin/universe" UNIVERSE_MODULES_PATH="$lib/modules" UNIVERSE_SOURCES_PATH="$lib/sources"
exec "$lib/bin/universe" "\$@"
EOF
cat > "$prefix/bin/universe-ui" <<EOF
#!/bin/sh
export UNIVERSE_BIN="$prefix/bin/universe" UNIVERSE_MODULES_PATH="$lib/modules" UNIVERSE_SOURCES_PATH="$lib/sources" QT_FORCE_STDERR_LOGGING=1
exec "$venv/bin/universe-ui" "\$@"
EOF
chmod 755 "$prefix/bin/universe" "$prefix/bin/universe-ui"
if [ "$desktop" = 1 ]; then
    cat > "$prefix/bin/universe-desktop" <<EOF
#!/bin/sh
export UNIVERSE_BIN="$prefix/bin/universe" UNIVERSE_MODULES_PATH="$lib/modules" UNIVERSE_SOURCES_PATH="$lib/sources"
exec "$lib/bin/universe-desktop" "\$@"
EOF
    chmod 755 "$prefix/bin/universe-desktop"
fi

: > "$manifest"
(cd "$dist" && find share -type f ! -path 'share/universe/*') | while IFS= read -r f; do
    case "$f" in
        *io.github.ilyasturki.UniverseDesktop* | */universe-desktop.1) [ "$desktop" = 1 ] || continue ;;
    esac
    install -Dm644 "$dist/$f" "$prefix/$f"
    printf '%s\n' "$f" >> "$manifest"
done
printf '%s\n%s\n' bin/universe bin/universe-ui >> "$manifest"
sed -i "s|^Exec=universe-ui|Exec=$prefix/bin/universe-ui|" "$prefix/share/applications/universe-ui.desktop"
if [ "$desktop" = 1 ]; then
    printf '%s\n' bin/universe-desktop >> "$manifest"
    sed -i "s|^Exec=universe-desktop|Exec=$prefix/bin/universe-desktop|" \
        "$prefix/share/applications/io.github.ilyasturki.UniverseDesktop.desktop" \
        "$prefix/share/dbus-1/services/io.github.ilyasturki.UniverseDesktop.service"
fi

say "installed Universe under $prefix"
install_system_files
if [ "$desktop" = 1 ]; then
    say "note: GNOME Shell reads search providers from XDG_DATA_DIRS only, which $prefix/share is not on by default: its search does not list Universe Desktop's games"
fi
case ":$PATH:" in
    *":$prefix/bin:"*) ;;
    *) say "note: $prefix/bin is not on PATH; add it to run universe from a terminal" ;;
esac
say ""
"$prefix/bin/universe" setup || true
