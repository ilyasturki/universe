# Cargo.toml strips, so a debuginfo package would be empty
%global debug_package %{nil}
# -flto C objects (aws-lc's) are GCC bitcode that rust-lld cannot link
%global _lto_cflags %{nil}
%global extension universe@ilyasturki.github.io
%global app_id io.github.ilyasturki.UniverseDesktop

Name:           universe
Version:        0.0.11
Release:        1%{?dist}
Summary:        Gamepad-first game launcher: a Rust core, a PySide6 UI, games inside gamescope
License:        MIT
URL:            https://github.com/ilyasturki/universe
Source0:        %{url}/archive/v%{version}/%{name}-%{version}.tar.gz
# the flake's galaxyServiceStub
Source1:        https://github.com/imLinguin/comet/releases/download/v0.3.2/GalaxyCommunication-dummy.exe
ExclusiveArch:  x86_64

# cargo fetches the crates: the COPR project builds with the network on
BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  gcc
BuildRequires:  maturin
BuildRequires:  python3-devel
BuildRequires:  python3-build
BuildRequires:  python3-installer
BuildRequires:  python3-setuptools
BuildRequires:  scdoc
BuildRequires:  systemd-rpm-macros
BuildRequires:  tzdata
BuildRequires:  blueprint-compiler
BuildRequires:  glib2-devel
BuildRequires:  pkgconfig(gtk4) >= 4.22
BuildRequires:  pkgconfig(libadwaita-1) >= 1.9

Requires:       hicolor-icon-theme
Requires:       python3
Requires:       python3-pyside6 >= 6.11
Requires:       python3-pysdl2
Requires:       python3-qrcode
Requires:       python3-xkbcommon
Requires:       qt6-qt5compat
Requires:       qt6-qtdeclarative
Requires:       qt6-qtimageformats
Requires:       qt6-qtmultimedia
Requires:       qt6-qtsvg
Requires:       qt6-qtwayland
Requires:       SDL2
Requires:       systemd
Recommends:     gamescope
Suggests:       mangohud
Suggests:       mangohud(x86-32)
Suggests:       umu-launcher
Suggests:       ludusavi
Suggests:       steam
Suggests:       gpu-screen-recorder
Suggests:       /usr/bin/ffmpeg
Suggests:       trash-cli
Suggests:       steam-devices
Suggests:       grim
Suggests:       spectacle
Suggests:       %{name}-desktop

%description
Universe launches games from GOG, Epic, Steam, itch.io and emulators inside
gamescope, behind a gamepad-first PySide6 UI, a GNOME Shell extension and the
universe CLI. It ships the udev rule and the uhid module load that open
/dev/uhid and /dev/uinput to the session, for the virtual pads and the key
macros.

%package desktop
Summary:        The Universe game library for mouse and keyboard: a GTK 4 and libadwaita app
Requires:       %{name}%{?_isa} = %{version}-%{release}
Requires:       gtk4%{?_isa} >= 4.22
Requires:       libadwaita%{?_isa} >= 1.9
Requires:       hicolor-icon-theme
Requires:       gstreamer1-plugins-base
Requires:       gstreamer1-plugins-good
Suggests:       gstreamer1-plugins-bad-free
Suggests:       gstreamer1-plugin-libav

%description desktop
Universe Desktop shows the Universe game library in a GTK 4 and libadwaita
app for mouse and keyboard, with a GNOME Shell search provider for the games.

%prep
%autosetup
# maturin's cargo metadata resolves every platform's crates, not only the host's
cargo fetch --locked

%build
export CARGO_TARGET_DIR=target
cargo build --frozen --release -p universe -p universe-desktop
maturin build --frozen --release --compatibility linux -m crates/universe-py/Cargo.toml -o dist
%{python3} -m build --wheel --no-isolation --outdir dist ui
target/release/universe __generate gen
scdoc < ui/universe-ui.1.scd > gen/man/universe-ui.1
scdoc < crates/universe-desktop/universe-desktop.1.scd > gen/universe-desktop.1

%install
for wheel in dist/*.whl; do
  %{python3} -m installer --destdir=%{buildroot} $wheel
done
install -Dm755 target/release/universe -t %{buildroot}%{_bindir}
install -Dm755 target/release/universe-system-install -t %{buildroot}%{_prefix}/lib/universe
for kind in modules sources; do
  install -d %{buildroot}%{_datadir}/universe/$kind
  cp -r $kind/. %{buildroot}%{_datadir}/universe/$kind
  rm -rf %{buildroot}%{_datadir}/universe/$kind/*/tests %{buildroot}%{_datadir}/universe/$kind/conftest.py %{buildroot}%{_datadir}/universe/$kind/test_*.py
done
install -Dm644 %{SOURCE1} %{buildroot}%{_datadir}/universe/sources/gog/GalaxyCommunication.exe
install -Dm644 extension/metadata.json extension/extension.js -t %{buildroot}%{_datadir}/gnome-shell/extensions/%{extension}
install -Dm644 gen/man/*.1 -t %{buildroot}%{_mandir}/man1
install -Dm644 gen/universe.fish %{buildroot}%{_datadir}/fish/vendor_completions.d/universe.fish
install -Dm644 gen/universe.bash %{buildroot}%{_datadir}/bash-completion/completions/universe
install -Dm644 gen/_universe %{buildroot}%{_datadir}/zsh/site-functions/_universe
install -Dm644 packaging/system/70-universe.rules -t %{buildroot}%{_udevrulesdir}
install -Dm644 packaging/system/universe.conf -t %{buildroot}%{_modulesloaddir}
install -Dm644 packaging/system/universe.desktop -t %{buildroot}%{_datadir}/wayland-sessions
install -Dm644 packaging/system/io.github.ilyasturki.universe.policy -t %{buildroot}%{_datadir}/polkit-1/actions
install -Dm644 ui/universe-ui.desktop -t %{buildroot}%{_datadir}/applications
install -Dm644 ui/icons/hicolor/scalable/apps/universe-ui.svg -t %{buildroot}%{_datadir}/icons/hicolor/scalable/apps
install -Dm644 ui/icons/hicolor/symbolic/apps/universe-ui-symbolic.svg -t %{buildroot}%{_datadir}/icons/hicolor/symbolic/apps

install -Dm755 target/release/universe-desktop -t %{buildroot}%{_bindir}
data=crates/universe-desktop/data
install -Dm644 $data/%{app_id}.desktop -t %{buildroot}%{_datadir}/applications
install -Dm644 $data/%{app_id}.service -t %{buildroot}%{_datadir}/dbus-1/services
sed -i 's|^Exec=universe-desktop|Exec=%{_bindir}/universe-desktop|' %{buildroot}%{_datadir}/dbus-1/services/%{app_id}.service
install -Dm644 $data/%{app_id}.search-provider.ini -t %{buildroot}%{_datadir}/gnome-shell/search-providers
install -Dm644 $data/%{app_id}.metainfo.xml -t %{buildroot}%{_metainfodir}
install -Dm644 $data/icons/apps/%{app_id}.svg -t %{buildroot}%{_datadir}/icons/hicolor/scalable/apps
install -Dm644 $data/icons/apps/%{app_id}-symbolic.svg -t %{buildroot}%{_datadir}/icons/hicolor/symbolic/apps
install -Dm644 gen/universe-desktop.1 -t %{buildroot}%{_mandir}/man1

%check
export CARGO_TARGET_DIR=target
# the journal tests expect Paris local time, as the flake's check does
TZ=Europe/Paris cargo test --frozen -p universe -p universe-desktop

%files
%license LICENSE
%{_bindir}/universe
%{_bindir}/universe-ui
%{python3_sitearch}/universe_core/
%{python3_sitearch}/universe_core-*.dist-info/
%{python3_sitelib}/universe_ui/
%{python3_sitelib}/universe_ui-*.dist-info/
%{_datadir}/universe/
%{_datadir}/gnome-shell/extensions/%{extension}/
%{_mandir}/man1/universe*.1*
%exclude %{_mandir}/man1/universe-desktop.1*
%{_datadir}/fish/vendor_completions.d/universe.fish
%{_datadir}/bash-completion/completions/universe
%{_datadir}/zsh/site-functions/_universe
%{_udevrulesdir}/70-universe.rules
%{_modulesloaddir}/universe.conf
%{_datadir}/wayland-sessions/universe.desktop
%{_prefix}/lib/universe/
%{_datadir}/polkit-1/actions/io.github.ilyasturki.universe.policy
%{_datadir}/applications/universe-ui.desktop
%{_datadir}/icons/hicolor/scalable/apps/universe-ui.svg
%{_datadir}/icons/hicolor/symbolic/apps/universe-ui-symbolic.svg

%files desktop
%license LICENSE
%{_bindir}/universe-desktop
%{_datadir}/applications/%{app_id}.desktop
%{_datadir}/dbus-1/services/%{app_id}.service
%{_datadir}/gnome-shell/search-providers/%{app_id}.search-provider.ini
%{_metainfodir}/%{app_id}.metainfo.xml
%{_datadir}/icons/hicolor/scalable/apps/%{app_id}.svg
%{_datadir}/icons/hicolor/symbolic/apps/%{app_id}-symbolic.svg
%{_mandir}/man1/universe-desktop.1*
