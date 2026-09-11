# 05 — La pile : quel langage pour le cœur, quel hôte pour Reprise

Tout ce qui suit a été vérifié le 2026-09-10 sur cette machine (NixOS, nixpkgs `26.05pre-git`, Qt 6.11.1, PySide6 6.11.0, rustc 1.97.1, go 1.26.7) ou dans les dépôts cités. Les sondes et le harnais de mesure sont dans `scratchpad/research/probe-rust/` (`Cargo.toml`, `build.rs`, `src/{main,api,library,memory}.rs`, `qml/{main,bench,pyside_main}.qml`, `bench/`, `measure.py`, `run_main.py`, `run_pegasus.sh`, `env.sh`).

## 1. Verdict

**Cœur en Rust, hôte Reprise en PySide6, GOG par `gogdl` en sous-processus.** Le cœur (démon + lanceur + API D-Bus + données) est la pièce que les autres frontends et les modules tiers consomment : un binaire, des interfaces D-Bus typées (zbus émet `PropertiesChanged` tout seul), un écosystème complet (tokio, evdev, rusqlite, notify, zbus_systemd) et un langage que la personne a déjà publié. L'hôte QML reste PySide6 parce que la mesure ne donne pas raison au soupçon « Python n'est pas natif » : sur la même scène, l'hôte Rust (cxx-qt, construit et exécuté ici) gagne 70 ms au premier frame et 24 Mo de RSS, et le modèle Python coûte 3 ms au lieu de 1 ms pour instancier la bibliothèque entière ; Pegasus (C++ Qt 5) met aujourd'hui 479 ms et 201 Mo avec Reprise, parce que ce sont les images 4K et le QML qui coûtent, pas le langage. En face, cxx-qt-lib 0.10 n'expose ni `QKeyEvent`/`postEvent` ni `setContextProperty` : l'injection manette y passe par QtTest ou par un shim C++. Go ne peut pas héberger Reprise (miqt ne lie pas QtQuick) et son export D-Bus est une table de pointeurs.

## 2. Faits

### 2.1 Hôtes QML en Rust

**cxx-qt (KDAB)** — dépôt `KDAB/cxx-qt`, Rust, 1 538 étoiles, 102 issues ouvertes, dernier push 2026-08-24 (`gh api repos/KDAB/cxx-qt`).

- Releases : **v0.10.0 le 2026-08-24**, 0.9.1 (2026-07-03), 0.9.0 (2026-06-23), 0.8.1 (2026-02-17), 0.8.0 (2025-12-18) (`gh api repos/KDAB/cxx-qt/releases`). Trois mineures en huit mois ; 0.8.1 change la signature de `QObjectExt::set_parent` (CHANGELOG), 0.7 a supprimé la conversion automatique des noms (« Starting with CXX-Qt 0.7, this is no longer the case! », `book/src/bridge/attributes.md`).
- Qt supporté : README « We currently aim to support the Qt versions that have official support by the Qt company. At the moment this means: Qt 5.15 LTS » et « All versions of Qt 6 » (https://github.com/KDAB/cxx-qt/blob/main/README.md). CI à v0.10.0 : Qt 5.15.2 et **6.10.1** (Linux/macOS), 6.2.4 (wasm) (`.github/workflows/github-cxx-qt-tests.yml`). **Qt 6.11.1 : vérifié ici**, cxx-qt-lib 0.10 compile et se lie contre le Qt du store, la sonde tourne.
- Enregistrement QML : `#[qml_element]`, `#[qml_singleton]`, `#[qml_uncreatable]` sur un `#[qobject]` ; le module est déclaré dans `build.rs` par `CxxQtBuilder::new_qml_module(QmlModule::new("org.reprise.probe")).files([...]).build()` (`book/src/getting-started/4-cargo-executable.md`). « This allows for attributes such as `#[qml_element]` to register the `QObject` with the QML type system without any C++ code » (`book/src/concepts/build_systems.md`). Le plugin est statique, lié dans le binaire ; **un fichier QML chargé depuis le disque (`file://`) importe le module sans rien d'autre** (vérifié : `qml/main.qml` chargé par `engine.load(QUrl "file://…")`, `import org.reprise.probe` résolu).
- `QAbstractListModel` : `unsafe extern "C++Qt" { include!(<QtCore/QAbstractListModel>); #[qobject] type QAbstractListModel; }` puis `#[base = QAbstractListModel]`, `#[cxx_override]` pour `data`/`roleNames`/`rowCount`, `#[inherit]` pour `beginResetModel`/`endResetModel`/`beginInsertRows`, `#[qenum]` pour les rôles (`examples/qml_features/rust/src/custom_base_class.rs`, 476 lignes pour deux rôles). Ma version à 15 rôles + `count` + `fill()` + `title(i)` : `src/library.rs`, 193 lignes.
- Signaux, propriétés, threads : `#[qsignal]`, `#[qproperty(T, name, cxx_name = "…")]` avec setter et `*_changed` générés ; `impl cxx_qt::Threading for T {}` donne `qt_thread().queue(|this| …)` depuis un `std::thread` (vérifié dans `src/api.rs` : thread → signal `padKey` → QML).
- QtMultimedia et Qt5Compat depuis le QML seul : vérifié, `SoundEffect` sur `assets/sounds/tick.wav` → `status=2 (Ready)`, `FastBlur` instancié, avec `QML_IMPORT_PATH` pour toute configuration, zéro ligne Rust.
- Build : Cargo seul, sans CMake (le CMake 3.24 de la page « Getting started » sert à la route CMake ; ma construction n'en a pas eu besoin). « It is vital that the `qmake` executable can be found by CXX-Qt » ; variable `QMAKE` ou `PATH` (`book/src/getting-started/4-cargo-executable.md`). Compilateur C++ requis (crate `cc`). Temps : **47,6 s** de mur, 6 min 37 s CPU sur 24 threads pour la première construction (dépendances, dont le C++ de cxx-qt-lib), 4–5 s en incrémental (`build.log`, `build3.log`). Binaire : 2,3 Mo, PIE, **83 bibliothèques partagées**, Qt en dynamique (`ldd target/release/reprise-probe`).
- **Trous de cxx-qt-lib 0.10** (`crates/cxx-qt-lib/src/{core,gui,qml}` à v0.10.0) : `gui/` = qcolor, qfont, qgenericmatrix, qguiapplication, qimage, qpainter, qpainterpath, qpen, qpolygon, qquaternion, qregion, qvector2d/3d/4d ; `qml/` = qqmlapplicationengine, qqmlengine, qqmlimageproviderbase ; `core/` n'a que qcoreapplication comme application. **Ni `QEvent`, ni `QKeyEvent`, ni `postEvent`/`sendEvent`, ni `QQmlContext`/`setContextProperty`, ni `focusWindow`.** Conséquences : l'objet `api` doit être un `#[qml_singleton]` (fait), et l'injection manette passe par `import QtTest; TestEvent { }` + `keyPress(key, 0, -1)` piloté par un signal Rust (fait, ça marche : `key-seen key=18874369 isAccept=true autorep=false` puis `key-released`) ou par un shim C++ compilé via `cc_builder` — le C++ que la personne refuse. QtTest est un module de test (`lib/qt-6/qml/QtTest/libquicktestplugin.so`), pas une API d'exécution.
- Nix : `qt6.full` a été retiré (« qt6.full has been removed. Please use individual packages instead », ajouté 2025-10-21, `nix eval nixpkgs#qt6.full`). Le `qmake` du store ne connaît que son propre préfixe (`qmake -query QT_INSTALL_HEADERS` → `…-qtbase-6.11.1/include`), donc cxx-qt-build ne trouve ni les en-têtes ni `libQt6Qml` de qtdeclarative. Solution qui a marché : un préfixe fusionné (`cp -rs` de qtbase, qtdeclarative, qtmultimedia, qt5compat, qtwayland dans `qtmerged/`, copie du binaire `qmake`, `bin/qt.conf` = `[Paths] Prefix=..`), puis `-lGLX -lOpenGL` venus des `.prl` de Qt exigent le `lib/` de libglvnd (`EXTRA_LINK_DIRS` dans `build.rs`). C'est le `symlinkJoin` qu'un paquet Nix devrait reconstruire pour la route Cargo. **Un seul précédent nixpkgs** (`rg -l 'cxx-qt|cxx_qt|qmetaobject' <nixpkgs>/pkgs`) : `pkgs/by-name/id/idescriptor/package.nix` (« A cross-platform iDevice management tool », AGPL), qui prend la **route CMake** — `corrosion` + `cxx-qt-cmake` récupéré de GitHub (`-DFETCHCONTENT_SOURCE_DIR_CXXQT=…`), `rustPlatform.cargoSetupHook` + `fetchCargoVendor`, `qt6.wrapQtAppsHook`, `qt6.qtbase/qtmultimedia/qtwayland/…` en `buildInputs`. CMake trouve les modules Qt un par un et évite le préfixe fusionné, au prix d'un `CMakeLists.txt` dans le projet. Personne n'y packagé la route Cargo seule. **4 dépendants sur crates.io** (`crates.io/api/v1/crates/cxx-qt/reverse_dependencies`).

**qmetaobject-rs (woboq)** — Rust, 735 étoiles, 94 issues, dernier push 2026-04-13 (« chore: bump syn to 2 »). **Dernière version publiée : 0.2.10 le 2023-11-03** (`crates.io/api/v1/crates/qmetaobject/versions`). README : « The qmetaobject crate is currently only being passively maintained as focus has shifted towards developing Slint ». Issue #310 « Unable to load qmetaobject based QML Plugins in KDE6 » ouverte depuis 2024-03-14. 7 dépendants. À écarter.

### 2.2 Go

- **therecipe/qt** : dernier commit 2020-09-04, 371 issues ouvertes, Qt 5 (`gh api repos/therecipe/qt`). Mort.
- **mappu/miqt** : MIT, v0.14.0 (2026-05-31), push 2026-09-04, 780 étoiles. README : « newly started in August 2024 », modules liés « QtCore, QtGui, QtWidgets, … QtMultimedia, … QML, … there is subclassing support » ; « Qt 5.15 / Qt 6.4+ ». Sous-répertoires de `qt6/` (API `git/trees`) : `cbor designer mainthread multimedia network pdf positioning printsupport qml spatialaudio sql statemachine svg uiplugin uitools webchannel webengine websockets` — **pas de `quick`, aucun fichier `gen_qquick*`**. `qt6/qml/` = 31 fichiers Go (QQmlApplicationEngine, QQmlComponent, `QQmlContext.SetContextProperty(string, *QObject)`). `QAbstractListModel` sous-classable par `OnData`, `OnRowCount`, `OnRoleNames` (`qt6/gen_qabstractitemmodel.go:5587-6453`) ; `NewQKeyEvent`, `QCoreApplication_PostEvent`, `QGuiApplication_FocusWindow` existent (`gen_qevent.go:2340`, `gen_qcoreapplication.go:217`, `gen_qguiapplication.go:177`). Contraintes README : cgo, `runtime.LockOSThread()` sur le thread Qt, objets Qt libérés par finaliseurs Go.
- Verdict Go-hôte : sur le papier, un moteur QML + modèles par propriétés de contexte + `postEvent` suffisent à charger Reprise sans jamais toucher un `QQuickItem` depuis Go. Personne ne l'a fait, rien n'est testé au-delà de 6.8 dans le README, et le jour où l'hôte a besoin d'un `QQuickImageProvider` ou d'un `QQuickItem` (le `LimitProxy`, un fournisseur d'images), Go n'a pas la classe. Pas construit, pas retenu.

### 2.3 Écosystèmes démon

**Rust.** `zbus` 5.19.0 (2026-08-09, MSRV 1.87) : `#[interface]` avec `#[zbus(property)]` (« A property setter named set_foo will be called to set the property 'Foo', and will emit the PropertiesChanged signal ») et `#[zbus(signal)]`, méthodes `async`, `<prop>_changed()` générés (https://docs.rs/zbus/latest/zbus/attr.interface.html). `tokio` 1.53.1 (2026-07-20), `tokio::process` pour lancer et attendre. `evdev` 0.13.2 (2025-09-15, `EventStream` tokio). `rusqlite` 0.40.2 (2026-08-08, SQLite embarqué). `toml` 1.1.6 (2026-09-10), `serde` 1.0.229 (2026-07-18). `notify` 8.2.0 (2026-08-30) / `inotify` 0.11.5 (2026-08-14). `zbus_systemd` 0.26100.0 (2026-06-26) : proxies typés de systemd 261, `start_transient_unit` inclus. Trou : aucun bloquant ; `evdev` a un rythme d'un an.

**Python.** `dbus-fast` 5.0.22 (2026-06-05, ≥3.10) : `ServiceInterface` + `@dbus_property`/`@dbus_signal`/`@method`, bus `aio.MessageBus`, mais `PropertiesChanged` est **manuel** (`self.emit_properties_changed({...})`, https://dbus-fast.readthedocs.io/en/latest/high-level-service/index.html). `asyncio` subprocess. `evdev` 2.0.0 (2026-08-23, ≥3.11, extension C). `sqlite3` et `tomllib` (lecture seule, 3.11+) dans la stdlib ; `tomli-w` 1.2.0 (2025-01-15) pour écrire. `asyncinotify` 4.4.4 (2026-04-13) ou `watchfiles` 1.2.0 (2026-05-18, `notify` Rust dessous). systemd : `StartTransientUnit` en appel brut `a(sv)` avec `Variant`, pas d'enveloppe typée. Trous : `PropertiesChanged` à la main, pas de wrapper systemd, distribution hors Nix (interpréteur + extensions C).

**Go.** `godbus/dbus` v5.2.2 (2025-12-29) : propriétés par le paquet `prop` — `Export(conn, path, props Map)` avec `Map = map[string]map[string]*Prop`, émission réglée par `EmitTrue`/`EmitInvalidates`/`EmitConst` (https://pkg.go.dev/github.com/godbus/dbus/v5/prop) ; introspection à écrire soi-même. `gopsutil` v4.26.8 (2026-08-29). SQLite : `modernc.org/sqlite` v1.58.0 (2026-09-01, sans cgo) ou `mattn/go-sqlite3` v1.14.52 (cgo). `BurntSushi/toml` v1.6.0 (2025-12-18). `fsnotify` v1.10.1 (2026-05-04). `coreos/go-systemd` v22.7.0 (2026-01-27) : `StartTransientUnitContext(ctx, name, mode, []Property, ch)`, `PropExecStart`, `PropPids`, `PropSlice`, `SubscribeUnits`/`SetPropertiesSubscriber` (https://pkg.go.dev/github.com/coreos/go-systemd/v22/dbus). evdev : `holoplot/go-evdev` sans tag (pseudo-version 2026-09-09), `gvalkov/golang-evdev` arrêté 2022-08-15. Trous : export D-Bus non typé, evdev non versionné.

**Commun aux trois.** `cgroup.events` : « Unless specified otherwise, a value change in this file generates a file modified event » et `populated` = « 1 if the cgroup or its descendants contains any live processes » (https://docs.kernel.org/admin-guide/cgroup-v2.html) : inotify `IN_MODIFY` suffit, avec n'importe laquelle des bibliothèques ci-dessus. `cgroup.kill` (écrire `1`) tue l'arbre.

### 2.4 Précédents (langue GitHub, octets, dernier push ; `gh api repos/<r>` et `/languages`)

| Projet | Langage | Push | Note |
|---|---|---|---|
| ShadowBlip/InputPlumber | Rust 2,11 Mo | 2026-09-10 | démon D-Bus, zbus |
| ShadowBlip/OpenGamepadUI | GDScript 1,33 Mo + **Rust 560 ko** | 2026-09-03 | `core/Cargo.toml` : GDExtension Rust |
| ChimeraOS/gamescope-session | Shell 16 ko | 2026-01-06 | scripts de session |
| ValveSoftware/gamescope | C++ | 2026-09-10 | |
| lutris/lutris | Python 2,79 Mo | 2026-09-10 | GTK |
| Open-Wine-Components/umu-launcher | Python 371 ko | 2026-09-05 | le lanceur qu'on appelle de toute façon |
| kra-mo/cartridges | Python 232 ko + Blueprint | 2026-09-03 | GTK4/libadwaita |
| bottlesdevs/Bottles | Python 2,53 Mo | 2026-09-10 | GTK4 |
| Heroic-Games-Launcher | TypeScript 1,94 Mo (Electron) | 2026-09-09 | GOG par gogdl (Python) en sous-processus |
| SteamDeckHomebrew/decky-loader | TypeScript 230 ko + **Python 166 ko** | 2026-09-02 | `backend/pyproject.toml` : le démon est en Python |
| JosefNemec/Playnite | C# 5,0 Mo | 2026-09-10 | Windows |
| flightlessmango/MangoHud | C 2,27 Mo + C++ 1,48 Mo | 2026-09-08 | |
| gpu-screen-recorder | C, meson, GPL-3.0-only | — | https://git.dec05eba.com/gpu-screen-recorder/about/ |
| nicohman/wyvern | Rust 88 ko | **2020-09-10** | mort ; sa crate `gog` 0.5.0 date du 2023-02-23 |
| Heroic-Games-Launcher/heroic-gogdl | Python 218 ko | 2026-09-08 | v1.3.0 le 2026-08-07 |
| Sude-/lgogdownloader | C++ 508 ko | 2026-09-09 | v3.18 le 2025-11-03 |

Lecture : les lanceurs Linux vivants sont en Python (Lutris, umu, Cartridges, Bottles, gogdl, le backend de Decky) ; le seul démon système comparable au nôtre, InputPlumber, est en Rust ; personne n'héberge du QML depuis Rust ou Go dans ce domaine.

### 2.5 GOG

- `gogdl` 1.3.0 installé (`/nix/store/…-gogdl-1.3.0/bin/gogdl`, nixpkgs `pkgs/by-name/go/gogdl/package.nix`, `dependencies = [ requests ]`, licence GPL-3, `heroic-unwrapped` 2.22.1 en dépend). `gogdl --help` : `import, redist (dependencies), auth, download (repair, update), info, launch, save-sync, save-clear, lang-match`, option globale `--auth-config-path`. **`update` et `repair` sont des alias de `download`.**
- `download` : `id`, `--path/-p` (obligatoire), `--platform/--os {windows,osx,linux}`, `--lang/-l`, `--build/-b`, `--branch`, `--password`, `--with-dlcs`/`--skip-dlcs`/`--dlcs LISTE`/`--dlc-only`, `--support`, `--max-workers`. `info` : « Calculates estimated download size and list of DLCs », mêmes filtres. `auth` : `--client-id`, `--client-secret`, `--code`. `launch path id --platform … [--no-wine] [--wine] [--wine-prefix] [--wrapper] [--override-exe] [--prefer-task]`. `import path`. `save-sync path id --ts --os [--name] [--skip-download|--skip-upload|--force-*]` (`gogdl <cmd> --help`).
- README : « This is **not** user friendly cli, it's meant to be used by some other application wanting to download game files, manage cloud saves or conveniently launch the game » ; « The only python dependency needed at this moment is `requests` » ; Heroic garde les jetons dans `$XDG_CONFIG_HOME/heroic/gog_store/auth.json` (le script `game` utilise `~/.config/gogdl/auth.json`, CONTEXT-v2).
- Alternatives : `lgogdownloader` (C++, `--galaxy-platform windows --galaxy-install <slug>/<build>`, `--galaxy-show-builds`, README) ; crate Rust `gog` 0.5.0 (2023-02-23, sr.ht nicohman) — même auteur que wyvern, arrêté.

### 2.6 Distribution

- PySide6 sur PyPI 6.11.2 (2026-08-18) : `pyside6_essentials` **80,1 Mo** + `pyside6_addons` **175,1 Mo** + `shiboken6` 0,3 Mo (roues `cp310-abi3-manylinux_2_34_x86_64`, `pypi.org/pypi/<p>/json`). Arch `extra/pyside6 6.11.2-1` (2026-08-20), dépend de `qt6-base`, `qt6-declarative`, `qt6-multimedia` en option (https://archlinux.org/packages/extra/x86_64/pyside6/). nixpkgs `python3Packages.pyside6 6.11.0` avec `dontWrapQtApps = true` (ligne 122 de la dérivation) : l'application doit passer par `wrapQtAppsHook`.
- Hôte Rust : compile avec Qt-dev + `qmake` + C++ ; binaire dynamique (83 `.so`, ci-dessus) ; PKGBUILD/COPR standards `cargo build` ; Nix = `buildRustPackage` + préfixe fusionné + `wrapQtAppsHook`, à écrire soi-même (aucun précédent nixpkgs).
- Démon Rust : binaire unique, SQLite embarqué, ne dépend que de glibc/D-Bus/systemd déjà présents ; musl possible. Démon Python : interpréteur ≥3.11 + `dbus-fast` + `evdev` (extension C) + `tomli-w` ; pipx/venv ou paquets distro. Démon Go : binaire statique, parité avec Rust.
- « Natif » perçu : le démon est un service `systemd --user`, son temps de démarrage n'existe pas pour l'utilisateur ; seul l'hôte se voit, chiffres ci-dessous.

## 3. Mesures

Harnais : `probe-rust/measure.py` — lance N fois, horodate la première ligne `first-frame` (émise par `Window.onFrameSwapped`), lit `/proc/<pid>/status` (VmRSS à cet instant et 2–3 s plus tard, VmHWM), médiane sur 3. `QT_QPA_PLATFORM=offscreen`, `QT_FORCE_STDERR_LOGGING=1`, mêmes `QML_IMPORT_PATH`/`QT_PLUGIN_PATH` (`env.sh`). Pas de vsync hors écran : le temps de mur reste valide, l'horloge QML non (CLAUDE.md).

| Hôte | Scène | 1er frame (médiane) | RSS au frame | RSS +2 s | Lignes hôte |
|---|---|---|---|---|---|
| `qml` (C++, qtdeclarative 6.11.1) | `first.qml` : fenêtre + texte | **57 ms** | 69 Mo | 69 Mo | 0 |
| PySide6 6.11.0 | `first.qml` | **119 ms** | 90 Mo | 90 Mo | 8 |
| Rust cxx-qt 0.10 (sonde) | `main.qml` : modèle 57×15 rôles, `FastBlur`, `SoundEffect`, injection manette, `memory` | **133 ms** | 143 Mo | 143 Mo | 340 (dont `build.rs` 17) |
| PySide6 6.11.0 | `pyside_main.qml` : même scène, `postEvent` réel | **203 ms** | 165 Mo | 167 Mo | 73 |
| pegasus-fe (C++, Qt 5.15.19, build `-gst`) | Reprise complet, vraie bibliothèque (46 jeux, art 4K) | **479 ms** | 138 Mo | 201 Mo (+3 s) | — |

Import Python (chaud, 3 runs, `time python3 -c 'import PySide6.QtQml, PySide6.QtQuick, PySide6.QtMultimedia'`) : **0,159 s / 0,085 s / 0,086 s** ; `python3 -c pass` : 0,010 s. `-X importtime` : `PySide6.QtQml` 57 ms cumulés dont 36 ms de `shiboken6` (chargeur de signatures), `QtQuick` 8 ms.

Coût du modèle — `Repeater` sur un modèle à 15 rôles, chaque délégué lit les 15 (`bench/bench.qml`, `qml/bench.qml`), temps entre l'affectation du modèle et le dernier `Component.onCompleted`, 2 runs :

| Lignes | Python `QAbstractListModel` | Rust `QAbstractListModel` (cxx-qt) | `ListModel` QML (C++ moteur) |
|---|---|---|---|
| 57 (taille réelle) | **3 ms** (855 appels `data()`) | **1 ms** | 1 ms |
| 500 | 19 / 18 ms (7 500 appels) | 5 / 5 ms | 4 / 5 ms |
| 2 000 | 75 / 66 ms (30 000 appels) | 20 / 20 ms | 18 / 17 ms |

Soit, à 2 000 lignes, (70 − 18) ms / 30 000 ≈ **1,7 µs de surcoût Python par accès `data()`** (GIL compris) au-dessus des ≈ 0,6 µs du moteur ; le Rust est indiscernable du moteur.

Asymétrie à signaler : `main.qml` (Rust) charge en plus `QtTest` (`libquicktestplugin.so`) pour l'injection, `pyside_main.qml` non. L'écart de 70 ms / 24 Mo en faveur du Rust est donc un plancher, pas un plafond.

Ce que ça dit : la couche qui coûte (graphe de scène, rendu, décodage d'images, QML JIT) est en C++ quelle que soit la langue de l'hôte ; Python ajoute ~60–70 ms de démarrage (import + shiboken), ~20–25 Mo, et des microsecondes par accès au modèle. Reprise-sous-Pegasus est aujourd'hui 2,4× plus lent au premier frame que l'hôte Python le plus lourd mesuré (479 / 203 ms) et 3,6× plus lent que l'hôte Rust (479 / 133 ms), parce qu'il charge 46 jaquettes ; c'est là que va le temps, pas dans l'interpréteur. Non mesuré : le GIL sous charge (thread manette + décodage) — le thread manette poste des `QKeyEvent` (`postEvent` est thread-safe) et ne tient le GIL que quelques µs.

## 4. Hôte QML : cxx-qt vs PySide6

**PySide6.** Ce qui a décidé :

1. **Complétude.** L'hôte fait quatre choses toute la journée : injecter des `QKeyEvent` sur `focusWindow()`, exposer `api`/`allGames`/`collections`/`memory`, sous-classer des modèles et des proxies, fournir des images. PySide6 a tout (`probe.py` de 01 et `run_main.py` ici : `postEvent`, `setContextProperty`, `QAbstractListModel`, `Property`/`Slot`, `QQuickImageProvider`). cxx-qt-lib 0.10 n'a ni `QKeyEvent`, ni `postEvent`, ni `QQmlContext` (2.1) : la sonde n'a marché qu'avec `QtTest.TestEvent` piloté par un signal, un module de test dans un binaire de production, ou alors un shim C++ — précisément ce que la personne ne veut pas maintenir. Un `QQuickImageProvider` existe (0.9.0), un `QSortFilterProxyModel` sous-classable non.
2. **Prix de la surface.** 340 lignes de Rust contre 73 de Python pour la même surface (modèle 15 rôles, `memory`, singleton, thread manette) : ×4,7, dont la moitié est du pont mécanique (`#[inherit]`, `#[cxx_override]`, `#[cxx_name]`, types `QHash_i32_QByteArray`). L'hôte estimé à 1 400–1 800 lignes Python (01 §3.1) ferait 5–7 k lignes de Rust.
3. **Ce que le Rust achète : 70 ms et 24 Mo**, une fois, au démarrage d'une session dédiée. À côté des 479 ms / 201 Mo de Pegasus aujourd'hui, ce n'est pas la variable qui compte ; les jaquettes et le QML de Reprise le sont.
4. **Emballage.** PySide6 est dans nixpkgs, Arch, PyPI ; cxx-qt exige sur Nix soit un préfixe Qt fusionné (route Cargo, 2.1), soit la route CMake du seul précédent nixpkgs (`idescriptor` : corrosion + `cxx-qt-cmake` + `CMakeLists.txt`), 4 dépendants crates.io, et trois versions mineures avec ruptures en huit mois.

Ce que cxx-qt fait bien et qu'il faut noter : la sonde compile en 48 s contre Qt 6.11.1, le modèle Rust est aussi rapide que le moteur, `Threading` rend le thread manette propre, et un QML sur disque importe le module Rust. Si KDAB ajoute `QKeyEvent`/`postEvent`/`QQmlContext`, le calcul change ; en 0.10 il n'y est pas.

**qmetaobject-rs** : passif depuis 2023, à écarter. **Go** : pas de QtQuick, à écarter pour l'hôte.

Design de l'hôte inchangé par rapport à 01 §3 (PySide6, `host/{app,api,model,library,launch,pad,dbus,proxies}.py`, 1 400–1 800 lignes), avec une différence : l'hôte **ne possède plus** la bibliothèque ni les sessions — il lit la base SQLite en lecture seule et appelle `org.reprise.Core` (04). Ce qui reste en Python dans l'hôte : le pont QML, la manette (SDL2/evdev → `postEvent`), la lecture des fichiers de données.

## 5. Cœur : Rust vs Go vs Python

**Rust**, un binaire `reprise-core` (ou `coulisses`), `tokio` + `zbus` 5 + `rusqlite` + `evdev` + `notify` + `zbus_systemd` + `toml`/`serde`. Raisons, dans l'ordre :

1. **L'API D-Bus est le produit.** Les frontends (Reprise, un GTK plus tard) et les modules tiers ne voient que `org.reprise.Core`. zbus génère propriétés, signaux, `PropertiesChanged`, introspection depuis une `impl` typée ; dbus-fast demande d'émettre `PropertiesChanged` à la main, godbus une `map[string]map[string]*Prop` et une introspection manuelle (2.3). L'interface publiée doit être exacte et stable ; c'est là que le typage paie, pas dans l'hôte.
2. **Distribution.** Un binaire statique, SQLite embarqué, zéro interpréteur : PKGBUILD, COPR, `buildRustPackage`, un `curl | install` pour les autres. Le cœur ne traîne ni PySide6 ni un venv ; seul le frontend Reprise a besoin de Python.
3. **La personne.** Elle a publié whisrs (Rust, crates.io) et phasionary (Go) ; « Python = quick hacks » (CONTEXT-v2). Le cœur sera le plus gros morceau de code (≈ 8–10 k lignes à terme : sessions, unités transitoires, cgroups, bibliothèque, importateurs, scrapers, InputPlumber, BlueZ) et vivra des années ; c'est le code qu'on veut typé.
4. **Écosystème complet** (2.3) : evdev asynchrone, inotify sur `cgroup.events`, `StartTransientUnit` typé, `tokio::process` avec `kill_on_drop`, `rusqlite` bundled. Rien n'est à écrire soi-même.

**Ce que Rust ne donne pas** : de la vitesse perceptible (le démon dort) ; et il coûte les 3 400 lignes Python existantes (`pegasus-sync`, `controller_*.py`) qui ne se portent pas gratuitement — elles deviennent des **modules** hors process, parlant D-Bus (le contrat de 04 §5 le prévoit déjà pour le journal zx), et se portent quand on les touche.

**Go** : viable pour le démon (binaire statique, go-systemd typé, fsnotify), écarté parce que l'export D-Bus de godbus est une table non typée sans introspection générée, l'evdev n'a pas de version, et rien ne serait partagé avec un hôte impossible en Go.

**Python** : c'était le plan v1 (04 §Langage). Il tient — dbus-fast, evdev, asyncio suffisent — mais il livre un démon qui exige un interpréteur et des extensions C partout où on le publie, avec un `PropertiesChanged` à ne jamais oublier. Le seul argument v1 qui survit, « la moitié du code métier est déjà en Python », est neutralisé par la frontière modules.

## 6. GOG

**Sous-processus `gogdl`, pas de réimplémentation.** `gogdl` est fait pour ça (« meant to be used by some other application »), il ne dépend que de `requests`, nixpkgs le paquette (`gogdl 1.3.0`), Heroic le fait vivre (v1.3.0 en août 2026, correctif de `launch` le 2026-09-08), et le script `game` l'utilise déjà avec `--with-dlcs` (CONTEXT-v2). Le cœur Rust l'appelle avec `--auth-config-path $XDG_CONFIG_HOME/reprise/gog-auth.json`, `info <id> --platform windows --lang <l>` pour la taille et les DLC, `download <id> --path /mnt/games/PC --platform windows --with-dlcs` pour installer, le même `download` pour mettre à jour (alias), `import <path>` pour adopter un dossier existant, `redist` pour les dépendances, et lit sa sortie ligne par ligne. **Licence** : GPL-3 ; un sous-processus garde le cœur sous sa propre licence, un `import gogdl` depuis Python ne le garderait pas. `launch` de gogdl n'est pas utilisé : c'est `umu-run` qui lance (03).

Rejetés : `lgogdownloader` (C++, second magasin de jetons, installeurs hors ligne comme voie principale, `--galaxy-install` en second) ; crate Rust `gog` (2023, auteur de wyvern, mort) ; réimplémenter l'API Galaxy (manifestes v1/v2, CDN, dépôts, DLC, branches) = les 218 ko de Python de heroic-gogdl à refaire et à suivre à chaque changement de GOG.

Coût : 1–2 jours pour l'enveloppe Rust (`tokio::process`, parsing de progression, état `installing/updating` exposé en D-Bus) contre 15–25 jours pour du natif plus la dette de suivi.

## 7. Coût (delta contre le plan v1 tout-Python)

Base v1 : hôte PySide6 15–25 j (01 §4, milieu 20 j retenu) ; démon + modules ≈ 33 j = lots démon 24 + journal 4 + vidéo 2 + packaging 3 (04 §Coût) ; lanceur ≈ 5 j (03). Les fourchettes sont prises à leur milieu pour que les totaux se recalculent.

| Lot | v1 Python | Pile recommandée | Delta | Pourquoi |
|---|---|---|---|---|
| Hôte QML | 20 j (15–25) | PySide6, 20 j | 0 | inchangé ; perd la bibliothèque et les sessions au profit du démon (−1 j), gagne le client D-Bus (+1 j) |
| Démon (squelette, bibliothèque, orchestration, capture, InputPlumber, BlueZ, raccourcis : 5+4+3+3+4+3+2) | 24 j | Rust, ≈ 31 j | **+7 j** | pont typé zbus (−), portage des scrapers de `pegasus-sync` vers `reqwest`/`serde` (+), evdev/cgroups équivalents |
| Journal (frontière + codex) | 4 j | 4 j | 0 | reste le script zx derrière D-Bus |
| Lecture vidéo (Qt Multimedia + mpv + focus gamescope) | 2 j | 2 j | 0 | dans l'hôte, inchangé |
| Lanceur (`umu-run`, scope, hooks, 4 backends) | 5 j | Rust, 6,5 j (6–7) | +1,5 j | même logique, plus de types |
| GOG | 0 (dans `game`) | enveloppe gogdl 1,5 j (1–2) | +1,5 j | nouveau dans le cœur |
| Packaging | 3 j | 4 j | +1 j | deux chaînes (cargo + pyproject), un flake |
| **Total** | **≈ 58 j** | **≈ 69 j** | **≈ +11 j** | |

Variante tout-Rust (cxx-qt) : hôte 32,5 j (25–40) au lieu de 20 (×4,7 lignes, pont mécanique, port de Reprise identique), + 2,5 j de build/Nix (préfixe fusionné, libglvnd, `wrapQtAppsHook`), + le shim d'injection (0 j en QtTest, 0,5 j en C++), soit ≈ +15 j sur l'hôte en plus des +11 j du cœur : **≈ +26 j** contre v1 pour 70 ms et 24 Mo. Variante tout-Go : non chiffrée, l'hôte n'est pas faisable proprement.

Risques de la pile recommandée : (1) deux langages dans un dépôt — la frontière est D-Bus, qui existe de toute façon pour les autres frontends ; (2) `pegasus-sync` reste Python jusqu'à son portage — acceptable, c'est un module ; (3) zbus 5 → 6 un jour (zbus a déjà renommé ses macros d'interface entre majeures — non vérifié pour 4 → 5, voir §9) ; (4) PySide6 en retard d'un patch sur Qt dans nixpkgs (6.11.0 / 6.11.1) — déjà noté en 01.

## 8. Verdict

**Pile retenue : cœur Rust (`reprise-core`, un binaire : démon tokio + zbus, lanceur, API `org.reprise.Core`, données TOML/JSONL/Markdown + index SQLite via rusqlite) ; frontend Reprise en Qt 6 hébergé par PySide6, client D-Bus, lecteur SQLite en lecture seule ; GOG par `gogdl` en sous-processus ; les scripts existants (journal zx, `pegasus-sync`, contrôleurs Python) deviennent des modules hors process jusqu'à leur portage.** Python n'est pas retenu par confort : la mesure dit que l'hôte Python coûte 70 ms et 24 Mo de plus que le même hôte en Rust, 2 ms de plus pour instancier la bibliothèque, sur une UI dont le plafond réel est ailleurs — et il apporte la seule liaison Qt complète que la personne acceptera de maintenir.

**Second : tout-Rust avec cxx-qt** pour l'hôte. Non retenu pour une seule raison : cxx-qt-lib 0.10 n'expose ni `QKeyEvent`/`postEvent` ni `QQmlContext::setContextProperty`, si bien que l'injection manette et l'objet `api` d'un hôte de production passent par QtTest ou par un shim C++.

## 9. Non vérifié

- Rendu contrôlé hors écran seulement ; ni Wayland, ni gamescope, ni frame pacing réel pour les trois hôtes.
- cxx-qt sur le vrai `theme.qml` de Reprise (5,4 k lignes) : seulement la surface d'API (modèle 15 rôles, `memory`, singleton, injection, Qt5Compat, QtMultimedia) ; ni `QSortFilterProxyModel` sous-classé, ni `QQuickImageProvider`, ni auto-répétition via `TestEvent`.
- cxx-qt avec Qt 6.12 (la CI KDAB teste 6.10.1 ; 6.11.1 vérifié ici).
- Aucun hôte Go construit ; miqt jugé sur son arbre de fichiers et son README (Qt ≤ 6.8 côté upstream).
- PySide6 sur Fedora/COPR, `python-dbus-fast`/`python-evdev` sur Arch : non consultés.
- Contention du GIL sous charge (thread manette + décodage vidéo) : non mesurée ; `postEvent` est thread-safe, le reste est une hypothèse.
- Facilité réelle du portage des scrapers de `pegasus-sync` (SteamGridDB, RAWG, Steam) en Rust : estimée, pas prototypée.
- Comportement de `gogdl` en flux (progression, codes retour) au-delà de `--help` : à lire dans `gogdl/dl/` avant d'écrire l'enveloppe.
- crates.io liste 0.2.10 comme dernière version de `qmetaobject` alors que le dépôt a un tag v0.2.12 : non résolu, sans effet sur le verdict.
- Le détail des ruptures zbus entre majeures (renommage des macros d'interface) est de mémoire, pas relu dans le CHANGELOG de zbus.
