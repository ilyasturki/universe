#!/bin/sh
# Universe for one user, no root: PREFIX/lib/universe holds the CLI, the modules and the UI's Python venv; PREFIX/bin gets universe and universe-ui.
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

say() { printf '%s\n' "$*"; }
die() { printf 'install.sh: %s\n' "$*" >&2; exit 1; }

remove_installed() {
    [ -f "$manifest" ] || return 0
    while IFS= read -r f; do rm -f "$prefix/$f"; done < "$manifest"
}

if [ "$uninstall" = 1 ]; then
    [ -d "$lib" ] || die "nothing installed under $prefix"
    remove_installed
    rm -rf "$lib"
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
[ -n "$python" ] || die "Python 3.11 to 3.14 is needed for the UI (PySide6); install python3.11 or later"
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
rm -rf "${lib:?}/bin" "$lib/modules" "$lib/sources"
install -Dm755 "$dist/bin/universe" "$lib/bin/universe"
cp -r "$dist/share/universe/modules" "$lib/modules"
cp -r "$dist/share/universe/sources" "$lib/sources"

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

: > "$manifest"
(cd "$dist" && find share -type f ! -path 'share/universe/*') | while IFS= read -r f; do
    install -Dm644 "$dist/$f" "$prefix/$f"
    printf '%s\n' "$f" >> "$manifest"
done
printf '%s\n%s\n' bin/universe bin/universe-ui >> "$manifest"
sed -i "s|^Exec=universe-ui|Exec=$prefix/bin/universe-ui|" "$prefix/share/applications/universe-ui.desktop"

say "installed Universe under $prefix"
case ":$PATH:" in
    *":$prefix/bin:"*) ;;
    *) say "note: $prefix/bin is not on PATH; add it to run universe from a terminal" ;;
esac
say ""
"$prefix/bin/universe" setup || true
