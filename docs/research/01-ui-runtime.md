# 01 — Le runtime UI : sur quoi faire tourner Reprise

## 1. Verdict

**Route (c) : un hôte Qt 6 neuf, en Python (PySide6 6.11), qui charge le QML de Reprise quasi tel quel.** Vérifié sur cette machine ce jour, depuis nixpkgs, sans compilation : les quatre effets `QtGraphicalEffects` que Reprise utilise existent dans `Qt5Compat.GraphicalEffects`, `SoundEffect` marche, le `SortFilterProxyModel` officiel de Qt 6.10+ reproduit les proxies de Reprise sur un modèle Python, un `QKeyEvent` synthétique arrive dans `Keys.onPressed` et passe par `api.keys.isAccept`, et un enregistrement AV1 10 bits 3840×2160 60 fps se décode à 60 fps via VAAPI dans `MediaPlayer`. Le port de Reprise, c'est cinq lignes d'import, six blocs de proxy à renommer dont trois à réécrire, et quatorze handlers `Keys.on*` à déclarer avec un paramètre formel.

Pegasus « tel quel + démon » (a) plafonne dès qu'on veut D-Bus, un rafraîchissement live ou lancer autre chose qu'un jeu ; le fork Pegasus (b) rend la personne dépendante de C++/Qt 5 qu'elle n'écrit pas, avec un port Qt 6 upstream bloqué depuis un an ; OpenGamepadUI (d) ne laisse pas un plugin remplacer son UI et impose de réécrire les 5,4 k lignes de Reprise en GDScript. L'a priori « garder mon QML » était le bon.

## 2. Les faits

### 2.1 Ce que Reprise consomme réellement (mesuré dans `~/Projects/pegasus-ui`)

- Imports : `QtQuick 2.15` (29 fichiers), `QtGraphicalEffects 1.15` (5), `SortFilterProxyModel 0.2` (4), `QtQuick.Window 2.15` (1), `QtMultimedia 5.15` (1). Aucun `QtQuick.Controls`, aucun `Qt.labs`, aucun `XMLHttpRequest`, aucune lecture vidéo (`grep VideoOutput|MediaPlayer|assets.video` : 0).
- Effets : `FastBlur` (`ui/BackgroundStage.qml:195`, `ui/LaunchFrame.qml:70`), `OpacityMask` (`ui/CoverCard.qml:62`, `ui/LaunchFrame.qml:105`), `ColorOverlay` (`ui/PlatformIcon.qml:40`), `RectangularGlow` (`ui/ChipPicker.qml:130`, `ui/CoverCard.qml:47`).
- Son : un pool de `SoundEffect` sur des WAV (`sound/Sound.qml:18`, `:53`).
- Proxies : `ValueFilter{inverted}` + `RoleSorter` (`pages/HomePage.qml:111-124`), `IndexFilter{maximumIndex: 11}` (`pages/HomePage.qml:127-130`), `ExpressionFilter` qui lit `index` et `page.pinned` (`pages/FavouritesPage.qml:57-62`), quatre sorters commutés par `enabled` (`pages/LibraryPage.qml:99-122`), `RegExpFilter` lié au texte tapé (`ui/SearchOverlay.qml:77-86`).
- `api` : `keys.is{Accept,Cancel,Filters,Details,PageUp,PageDown,PrevPage,NextPage,Menu}`, `allGames(.count,.get)`, `collections(.count,.get)`, `memory.{get,set,has}`. Champs jeu : `favorite, assets, extra["metacritic"|"hltb-*"], playTime, title, releaseYear, playCount, collections, publisherList, players, lastPlayed, developerList, summary, launch, genreList, description`. Assets : `logo, boxFront, screenshotList, background, tile`.
- 14 handlers `Keys.onPressed/onReleased:` sans paramètre formel (comptés par grep). Qt 6.11 les exécute encore, avec un avertissement `Injection of parameters into signal handlers is deprecated` (testé, voir 2.4).
- Le splash externe (`launch/pegasus-launch-splash`, `launch/Splash.qml`, `os/gaming/pegasus.nix:44-80`, xdotool/xprop) n'existe que parce que Pegasus démonte sa fenêtre au lancement (`ProcessLauncher.cpp:264-268`, `onTeardownComplete` → `waitForFinished`). Un hôte qu'on contrôle garde sa fenêtre : `ui/LaunchFrame.qml` devient le splash, en process.

### 2.2 Pegasus Frontend aujourd'hui (github.com/mmatyas/pegasus-frontend, interrogé via `gh api` le 2026-09-09)

- 766 k octets de C++, 180 k de QML, CMake + qmake, GPLv3 (`src/app/qmlplugins.qml:1-15`), 1 888 étoiles, 152 issues ouvertes, dernier tag oct. 2024.
- 2026 : 13 commits, trois auteurs (mmatyas : localisations, SDL 2.32.10, quit menu ; « bdm »/« L » : GameDataCache et « scan on launch » en juin-juillet). PR ouvertes : 5, dont deux de 2023. Les PR sont fusionnées en semaines à mois quand mmatyas est là (#1184 ouvert 15/06, fusionné 18/07).
- **Port Qt 6** : issue épinglée #1167 (mmatyas, 2025-09-16) : code, proxy model, thème par défaut et CMake desktop cochés (branche `qt6_v2`, dernier commit 2025-09-06) ; restent Android (Qt Gamepad supprimé de Qt 6), CI, Flatpak, doc. Issue #1188 (2026-09-04) : port complet par ChuLiqiang sur Qt 6.11.2 macOS (branche `qt6-port`, dernier commit 2026-09-06 « Fix crash when SDL reports removal of an untracked gamepad » — le même crash que le patch `overlays/pegasus-frontend-gamepad-unknown-iid.patch` de `~/NixOs/overlays/default.nix:40-42`). Fermée par mmatyas le jour même : « There's already a Qt 6 port, see #1167. The problem is that Qt 6 dropped Qt Gamepad ». https://github.com/mmatyas/pegasus-frontend/issues/1167 · https://github.com/mmatyas/pegasus-frontend/issues/1188
- Manette : SDL2 in-process (`src/backend/model/internal/GamepadManagerSDL2.cpp`, 737 lignes). Chaque bouton devient un `QKeyEvent` envoyé à `qApp->focusWindow()` avec des `Qt::Key` privés (`GamepadButtonNavigation.cpp:30-38`, `:65-81`) ; les axes passent un seuil ±0.5 et émettent press/release (`GamepadAxisNavigation.cpp:32-34`, `:95-102`) ; la répétition est un `QTimer` par bouton, 50 ms, événements flaggés `autorep` (`GamepadButtonNavigation.cpp:28`, `:46-61`). `api.keys.isX(event)` compare `event.key` à ces ids (`src/backend/model/keys/Keys.h:40-55` ; `isMenu` existe, absent de la doc).
- `api.allGames` et `api.collections` sont `CONSTANT` (`src/backend/model/Api.h:43-44`) : pas de rechargement depuis un thème. Les hooks `game-start/game-end/quit/reboot/shutdown/config-changed/settings-changed/controls-changed` sont lancés par `QProcess::execute` synchrone, uniquement sur événement interne (`src/backend/ScriptRunner.cpp:75-100`).
- `QML_XHR_ALLOW_FILE_READ` **et** `QML_XHR_ALLOW_FILE_WRITE` existent dans Qt 5.15 (`qtdeclarative` branche 5.15, `src/qml/qml/qqmlxmlhttprequest.cpp:81-82`, `:1206-1223`). Un thème peut donc écrire un fichier, pas seulement le lire.
- nixpkgs : `pegasus-frontend 0-unstable-2024-11-11`, buildInputs `qtbase qtmultimedia qtsvg qtgraphicaleffects qtx11extras sqlite sdl2-compat` (SDL2 émulé sur SDL3), `wrap-qt5-apps-hook`. Aucun signal de suppression de `libsForQt5` trouvé ; Qt 5.15 est en fin de vie (les 5.15.x open source sortent en retard sur le commercial : Pegasus a fusionné « Update to Qt 5.15.10 » en 2025-09, Homebrew est à 5.15.19) et nixpkgs recommande `qt6Packages.callPackage` (https://github.com/NixOS/nixpkgs/blob/master/doc/languages-frameworks/qt.section.md).

### 2.3 Qt 6 sur NixOS (nixpkgs `26.05pre-git`, `nix eval` ce jour)

- `python3Packages.pyside6 6.11.0` lié à `qt6 6.11.1`, buildInputs `qtbase qtdeclarative qtmultimedia qtshadertools qtquick3d …` (pas `qt5compat`, pas `qtwayland` : ce sont des modules QML/plugins, il suffit de les mettre dans le chemin d'import). `dontWrapQtApps = true` dans la dérivation (`pkgs/development/python-modules/pyside6/default.nix:119`) : **l'application finale doit être enveloppée par `qt6Packages.wrapQtAppsHook`**, sinon rien ne trouve les modules. Mes probes ont construit `QML_IMPORT_PATH`/`QT_PLUGIN_PATH` à la main pour cette raison.
- `qt6.qt5compat 6.11.1` fournit `Qt5Compat.GraphicalEffects` avec `FastBlur, GaussianBlur, OpacityMask, ColorOverlay, RectangularGlow, Glow, DropShadow, …` ; la doc le dit « primarily included for compatibility with Qt 5 applications » et renvoie vers `MultiEffect` (https://doc.qt.io/qt-6/qtgraphicaleffects5-index.html).
- `QtQml.Models` (Qt ≥ 6.10) : `SortFilterProxyModel, Filter, RoleFilter, ValueFilter, FunctionFilter, Sorter, RoleSorter, StringSorter, FunctionSorter`, marqués « under development and subject to change ». `Filter.inverted/enabled`, `Sorter.enabled/priority/sortOrder` existent. `FunctionFilter` ne voit que les rôles déclarés dans un composant inline, pas l'index source, et exige `invalidate()` quand une propriété externe change (https://doc.qt.io/qt-6/qml-qtqml-models-sortfilterproxymodel.html · https://doc.qt.io/qt-6/qml-qtqml-models-functionfilter.html · https://doc.qt.io/qt-6/qml-qtqml-models-sorter.html). Pas d'`IndexFilter`, pas de `RegExpFilter`, pas d'`ExpressionFilter`.
- Le SortFilterProxyModel d'oKcerG (celui que Pegasus embarque en sous-module, `.gitmodules`) est figé depuis 2023-07 ; PR #100 « Qt6 & Qt5 version » ouverte, non fusionnée. Forks Qt 6 : `mitchcurtis/SortFilterProxyModel` (2026-04), `richardhozak/SortFilterProxyModelQt6`. Solution de repli, pas la voie principale.
- `qt6.qtmultimedia 6.11.1` est compilé avec `ffmpeg 8.1.2, libva, pipewire, libpulseaudio` ; backend FFmpeg par défaut sur Linux (https://doc.qt.io/qt-6/qtmultimedia-index.html), VAAPI sélectionnable par `QT_FFMPEG_DECODING_HW_DEVICE_TYPES=vaapi` (https://doc.qt.io/qt-6/advanced-ffmpeg-configuration.html).
- Manette : `QtGamepad` n'est pas porté sur Qt 6 ; Qt Universal Input est expérimental (https://www.arnorehn.de/blog/2023/10/31/qtgamepad-ported-to-qt-6/). nixpkgs a `python3Packages.pysdl2 0.9.17`, `evdev 1.9.3`, `dbus-next 0.2.3`, `SDL2 2.32.68`. InputPlumber 0.77.1 expose une cible `dbus` : interface `org.shadowblip.Input.DBusDevice`, signal `InputEvent(s event, d value)` (https://raw.githubusercontent.com/ShadowBlip/InputPlumber/main/bindings/dbus-xml/org.shadowblip.Input.DBusDevice.xml).
- Rust : cxx-qt v0.10.0 (2026-08-24), qmetaobject-rs dernier push 2026-04. Viables, mais la personne n'écrit pas de Rust ; PySide6 couvre tout ce dont l'hôte a besoin.

### 2.4 Ce que j'ai fait tourner (probes dans `scratchpad/research/probe/`, `QT_QPA_PLATFORM=offscreen`)

`probe.py` + `probe.qml` (PySide6 6.11.0 / Qt 6.11.1) :

```
qml: games.count=4
qml: sound status=2 (Ready=2)
qml: keysSeen=2 acceptSeen=1
```

`FastBlur`, `RectangularGlow`, `ColorOverlay`, `OpacityMask` (Qt5Compat) et `MultiEffect` instanciés sans erreur ; `SoundEffect` sur `assets/sounds/tick.wav` prêt ; un `QKeyEvent` avec un id privé (`0x01100000`) posté depuis Python sur `app.focusWindow()` arrive dans `Keys.onPressed` et `api.keys.isAccept(event)` — un `Slot` Python lisant `event.property("key")` — répond vrai.

`proxy.qml` sur un `QAbstractListModel` Python à quatre lignes, comparé à un `ListModel` QML :

```
pyRecent=3 [Zeta,Iota,Omega]  qmlRecent=3 [Zeta,Iota,Omega]     # ValueFilter{playCount,0,inverted} + RoleSorter lastPlayed desc
pyFav=2 [Iota,Omega]          qmlFav=2 [Iota,Omega]             # FunctionFilter(favorite) + StringSorter title
pySorted=4 [Alpha,Iota,Omega,Zeta]
```

`av1.qml` sur `/mnt/recordings/games/grand-theft-auto-v/012-20260909-182430-46m.mkv` (av1, 3840×2160, yuv420p10le, 60 fps) :

```
qt.multimedia.ffmpeg.hwaccel: Found potential codec "av1" for hw accel 3 ; Checking the hw device...
qt.multimedia.ffmpeg.hwaccel:     Checking HW context: vaapi  /  HW device is OK
qml: hasVideo=true playing=true pos=5983ms frames=362 res=3840 x 2160 codec=AV1
```

362 frames en 5,98 s = 60 fps. En logiciel (`QT_FFMPEG_DECODING_HW_DEVICE_TYPES=""`, dav1d sur le 7900X) : 362 frames aussi.

`inject.qml` : `Keys.onPressed: { if (event.key) … }` sans paramètre déclaré compte bien l'événement en 6.11, avec l'avertissement de dépréciation.

Détail utile pour plus tard : la Qt de nixpkgs est compilée avec journald et envoie `console.log` et `qWarning` au journal, pas sur stderr, dès que stdout n'est pas un tty. `QT_FORCE_STDERR_LOGGING=1` dans `tools/shot` évitera une heure perdue.

### 2.5 OpenGamepadUI (github.com/ShadowBlip/OpenGamepadUI, arbre `main` et docs lus ce jour)

- `core/main.gd:29` charge `res://core/ui/card_ui/card_ui.tscn` en dur (ou `card_ui_overlay_mode` avec `--overlay-mode`). Le `--vapor-ui` de l'aide (`main.gd:47`) n'a aucun code derrière.
- L'API plugin est la classe `Plugin` (`core/systems/plugin/plugin.gd`, 106 lignes) : `unload()`, `get_settings_menu()`, `add_library()`, `add_store()`, `add_boxart()`, `add_to_quick_bar()`, `add_overlay()`, plus `add_to_qam()` déjà déprécié en place. Rien pour remplacer ou retirer un menu. Le `PluginManager` est un enfant de `CardUI` (`card_ui.tscn:108`), donc un plugin naît **à l'intérieur** de l'UI qu'il voudrait remplacer ; les menus sont des scènes instanciées au démarrage et commutées par visibilité via des `StateMachine` globales (`docs/documentation/contributing/creating_a_menu.md`), l'`InputManager`, la `ContextBar`, l'OSK et les fondus appartiennent à `CardUI` (`card_ui.tscn:240`, `:298-318`, `:337-348`). « Modify nearly all aspects » (`docs/documentation/plugins/introduction.md`) veut dire : le zip peut écraser des ressources `res://` — c'est-à-dire forker l'UI par substitution de fichiers, pas via une API.
- `PLUGIN_API_VERSION = "2.0.0"` (`core/global/plugin_loader.gd:21`) ; compatibilité vérifiée sur le majeur seulement (`:509-510`). 12 releases entre 2025-10 et 2026-09 (v0.42.1 → v0.46.1), 40 commits en 2026 ; le gabarit de plugin n'a pas bougé depuis 2023-10-03 ; le store est un seul `plugins.json`. `OpenGamepadUI-lutris` : dernier commit janv. 2026.
- Ce qu'OGUI donne gratuitement et qui compte ici : `core/systems/{bluetooth, input (81 fichiers, InputPlumber), overlay, gamescope, launcher, mangoapp, notification, power, performance}`, profils manette par jeu (`launch_manager.gd:467-487 set_app_gamepad_profile / set_gamepad_profile`), hooks de cycle de vie (`library.gd:114 get_app_lifecycle_hooks`), session gamescope et mode overlay (`rootfs/…/ogui-overlay-mode.service`), rendu GNOME corrigé en v0.46.1. `LibraryLaunchItem` = `name, command, args, env, cwd, tags, categories, installed, hidden, metadata` (`library_launch_item.gd:13-25`).
- Le prior « jamais de souris » survit à OGUI : l'UI est pensée manette d'abord (`FocusGroup`, `focus_neighbors`, OSK intégré, `creating_a_menu.md`). C'est le prior « garder mon QML » qui casse, entièrement.
- Godot peut techniquement refaire les mouvements de Reprise : `Tween.TRANS_SPRING/ELASTIC/BACK` (https://docs.godotengine.org/en/stable/classes/class_tween.html), un shader écran pour le flou (OGUI a `assets/shaders/simple_blur.gdshader`), un OSK existe. Mais c'est une réécriture : zéro ligne des 5,4 k de QML ne survit.
- nixpkgs : `opengamepadui 0.45.0`, `godot 4.6.3` alors que le projet déclare `config/features=PackedStringArray("4.7", …)` (`project.godot:27`). Un plugin développé ici tournerait sur une version en retard.

### 2.6 Licences

- Pegasus : GPLv3 (en-têtes de chaque fichier). Reprise : MIT. SortFilterProxyModel oKcerG : MIT. Qt et PySide6 : LGPLv3, liaison dynamique. Godot : MIT. OGUI : GPLv3.
- Un hôte qui **copie** du C++ de Pegasus (parser, launcher, mapping manette) devient GPLv3. Un hôte qui réimplémente depuis la doc publique (https://pegasus-frontend.org/docs/themes/api/, format `metadata.pegasus.txt`) et depuis le comportement observé reste MIT. Les formats de fichiers ne sont pas du code. Les ids de touches privés sont des entiers : en choisir d'autres.
- Un plugin GDScript chargé dans le process d'OGUI est vraisemblablement une œuvre combinée GPLv3 ; Reprise-en-Godot ne resterait MIT que de nom.
- Le ffmpeg que nixpkgs lie à Qt Multimedia est en variante GPL (le probe a affiché « FFmpeg version 8.1.2 GPL version 3 or later »). L'hôte distribué en source reste MIT ; un bundle autonome qui embarquerait ce ffmpeg serait lié par la GPL.

## 3. Le design recommandé

### 3.1 Découpage

```
reprise-host   (Python 3.13 + PySide6, un process, une fenêtre, MIT)
├── host/app.py         QGuiApplication, QQmlApplicationEngine, main.qml enveloppe theme.qml
├── host/api.py         objet `api` : keys, allGames, collections, memory, device
├── host/model.py       Game, Collection, Assets (QObject), GameListModel (QAbstractListModel)
├── host/library.py     lecture metadata.pegasus.txt + media/ (v1), puis library.db (v2)
├── host/launch.py      launch(): hooks pré/post, QProcess, signaux gameStarted/gameStopped
├── host/pad.py         thread SDL2 → QKeyEvent posté sur focusWindow (repeat 50 ms, autorep)
├── host/dbus.py        QtDBus : org.reprise.Host sur le bus session
├── host/proxies.py     LimitProxy(QIdentityProxyModel) pour l'IndexFilter de HomePage
└── modules/            opt-in, découverts par entry point `reprise.modules`
```

Taille à reproduire. Côté Pegasus, la couche modèle fait 187 k octets de C++ sur 67 fichiers (`src/backend/model`), et ce que Reprise en exerce côté données (`providers/pegasus_metadata` + `pegasus_favorites` + `pegasus_playtime` + `Provider*`/`SearchContext`) 89 k octets, soit 7 à 8 k lignes de C++ en tout, dont une bonne part est du boilerplate Qt et des providers inutiles ici. En Python, pour la surface de 3.2 seulement : `model.py` ≈ 300 lignes, `library.py` (parser `metadata.pegasus.txt` + résolution des assets dans `media/`) ≈ 250, `api.py` (keys, memory, device, listes) ≈ 200, `launch.py` (hooks, QProcess, signaux) ≈ 200, `pad.py` ≈ 200, `dbus.py` ≈ 120, `proxies.py` ≈ 40, `app.py` ≈ 100 : **1 400 à 1 800 lignes**, tests compris on approche 2 500.

Les fonctions longues (enregistrement, journal, raccourcis manette, Bluetooth) restent des unités `systemd --user` séparées et parlent au host par D-Bus ; leurs sections le détaillent. L'hôte ne porte que l'UI, le modèle et le lancement.

### 3.2 Le contrat `api` (ce que Reprise attend, rien de plus au départ)

- `api.keys.isAccept/isCancel/isDetails/isFilters/isNextPage/isPrevPage/isPageUp/isPageDown/isMenu(event)` : `@Slot("QVariant", result=bool)`, compare `event.property("key")` à une table `{accept: [KEY_A, Qt.Key_Return], cancel: [KEY_B, Qt.Key_Escape], …}` chargée depuis `~/.config/reprise/keys.toml`.
- `api.allGames`, `api.collections` : `QAbstractListModel` avec rôles nommés + `count` (Property, notify) + `get(i)` (Slot → QObject) + `toVarArray()`.
- `Game` : les 16 champs listés en 2.1, `favorite` en écriture (persisté dans `~/.local/share/reprise/favorites.txt`, une clé par ligne), `extra` en `QVariantMap`, `assets.*` en `QUrl`, `assets.screenshotList` en `QStringList`, `launch()`.
- `Collection` : Reprise lit `name`, `shortName`, `games` (`pages/LibraryPage.qml:54-78`, `ui/PlatformIcon.qml:14`) ; `games` est un `GameListModel` filtré sur la collection.
- `api.memory` : `get/set/has/unset` sur `~/.local/state/reprise/memory/<theme>.json`, écrit à chaque `set` (Reprise l'appelle deux fois).
- `api.device.batteryPercent/batteryCharging` : `NaN`/`false` (Reprise ne les lit pas, la compat coûte trois lignes).

### 3.3 Port de Reprise (une journée, en branche `qt6`)

1. `import QtGraphicalEffects 1.15` → `import Qt5Compat.GraphicalEffects` (5 fichiers). `QtMultimedia 5.15` → `QtMultimedia`. `SortFilterProxyModel 0.2` → `QtQml.Models`.
2. `sourceModel:` → `model:` dans les 6 proxies (HomePage 3, LibraryPage, FavouritesPage, SearchOverlay).
3. `pages/HomePage.qml:127-130` : `IndexFilter{maximumIndex: 11}` → `LimitProxy { model: byRelease; limit: 12 }` fourni par l'hôte (~30 lignes Python, `QIdentityProxyModel` qui tronque `rowCount`).
4. `pages/FavouritesPage.qml:60` : `ExpressionFilter` avec `index` → ajouter un rôle `key` au modèle, `page.pinned` devient une liste de clés, `FunctionFilter { component D: QtObject { property bool favorite; property string key } function filter(d: D): bool { return d.favorite || page.pinned.indexOf(d.key) !== -1 } }` et `onPinnedChanged: favourites.invalidate()`.
5. `ui/SearchOverlay.qml:80-84` : `RegExpFilter` → `FunctionFilter` avec `property var re: new RegExp(overlay.pattern, "i")` et `onPatternChanged: matches.invalidate()`.
6. Les 14 `Keys.onPressed: {` → `Keys.onPressed: function(event) {`. Optionnel en 6.11, obligatoire un jour.
7. Supprimer `launch/` : `ui/LaunchOverlay.qml` garde son dip à 300 ms (`:11`, `:71-77`), mais `overlay.game.launch()` (`:90`) n'a plus besoin de bloquer : l'hôte émet `launchStarted` quand le `QProcess` a démarré, l'overlay reste affiché en process jusqu'à ce que la fenêtre du jeu prenne le premier plan. Le `resetTimer` (`:95-97`) reste comme filet.

Repli si `QtQml.Models` change en 6.12 : un `ReprisePages`-`QSortFilterProxyModel` Python (~150 lignes) qui expose `filters`/`sorters` en propriétés, ou le fork mitchcurtis compilé en module QML. Les quatre blocs QML ne bougent pas.

### 3.4 Manette

`host/pad.py` : thread `SDL_Init(SDL_INIT_GAMECONTROLLER)`, `SDL_GameControllerAddMappingsFromFile(gamecontrollerdb.txt)`, boucle `SDL_WaitEventTimeout`. Sémantique à reproduire, celle sur laquelle Reprise s'appuie (`CLAUDE.md`) :

- bouton : `KeyPress` puis, tant que tenu, toutes les 50 ms un couple `KeyRelease`+`KeyPress` avec `autorep=True` ; au relâchement un `KeyRelease` avec `autorep=False` ;
- axe (sticks, L2/R2) : press quand `|v|` franchit 0,5, release quand il repasse dessous ; jamais de press sans release apparié ;
- ids : une plage privée à nous (`0x01200000 + n`), `Qt.Key_Up/Down/Left/Right` pour le d-pad et le stick gauche ;
- `QCoreApplication.postEvent(app.focusWindow(), QKeyEvent(...))` depuis le thread (thread-safe, contrairement à `sendEvent`).

Second backend, `--pad=inputplumber`, pour le jour où l'hôte tourne en overlay pendant un jeu : s'abonner à `org.shadowblip.Input.DBusDevice.InputEvent` sur le `CompositeDevice` courant et traduire `event` → même table. Pas nécessaire au premier jour ; les pads passent déjà par InputPlumber → `ds5-edge` virtuel, que SDL voit comme une DualSense Edge.

### 3.5 Lancement et sessions

- `launch()` : `pre-launch.d/*` synchrones (300 ms de budget, sinon on les passe en D-Bus), puis `QProcess.start(cmd, args)` avec `ForwardedChannels`, `finished` → `gameStopped(key, seconds, exitCode)`, `post-launch.d/*`. L'hôte ne cache pas sa fenêtre : sous gamescope la nouvelle fenêtre XWayland prend le focus ; sous GNOME la fenêtre du jeu passe devant, et `window.requestActivate()` à `finished` ramène Reprise.
- Session gamescope : `os/gaming/pegasus.nix:73-90` reste, `pegasus-fe` → `reprise-host`, `QT_QPA_PLATFORM=xcb` comme aujourd'hui. Sous GNOME : `QT_QPA_PLATFORM=wayland`, `Window { visibility: FullScreen }`.
- Vidéo : `MediaPlayer + VideoOutput + AudioOutput` avec `QT_FFMPEG_DECODING_HW_DEVICE_TYPES=vaapi` dans le wrapper. Le clip H.264 de 20 s produit par `pegasus-sync` n'est plus nécessaire : la page détail peut lire l'AV1 source.

### 3.6 D-Bus (nom, chemin, surface minimale)

`org.reprise.Host` sur le bus session, objet `/org/reprise/Host` :

- propriétés `CurrentGame (s)`, `State (s: idle|launching|in-game)` ;
- signaux `GameStarted(s key, i pid)`, `GameStopped(s key, u seconds, i exit)` ;
- méthodes `Launch(s key)`, `Notify(s title, s body, s icon)`, `Reload()` (remplace le redémarrage de Pegasus : relit la bibliothèque et émet `modelReset`) ;
- `Reload()` est ce que `pegasus-sync` et le post-traitement des enregistrements appellent à la fin, au lieu de rien.

### 3.7 Packaging NixOS

```nix
python3Packages.buildPythonApplication {
  pname = "reprise-host"; pyproject = true;
  nativeBuildInputs = [ qt6Packages.wrapQtAppsHook ];
  buildInputs = with qt6; [ qtbase qtdeclarative qt5compat qtmultimedia qtwayland qtshadertools ];
  propagatedBuildInputs = with python3Packages; [ pyside6 pysdl2 ];
  dontWrapQtApps = false;
  preFixup = ''
    qtWrapperArgs+=(--set-default QT_FFMPEG_DECODING_HW_DEVICE_TYPES vaapi
                    --set-default QT_QPA_PLATFORM "wayland;xcb"
                    --prefix QML_IMPORT_PATH : $out/share/reprise/qml)
  '';
}
```

`wrapQtAppsHook` construit `QT_PLUGIN_PATH` et `QML2_IMPORT_PATH` à partir des `buildInputs` Qt ; c'est exactement ce que mes probes ont dû faire à la main. Le thème est installé en `share/reprise/theme/`, surchargeable par `--theme <dir>` pour développer dans `~/Projects/pegasus-ui` sans rebuild.

### 3.8 Modèle de données (v1 → v2)

- v1 (première semaine) : lire `metadata.pegasus.txt` + `media/` là où `pegasus-sync` les écrit, `favorites.txt`, et remplacer `stats.db` par `~/.local/share/reprise/sessions.sqlite` (`sessions(key, started, ended, seconds, recording)`) alimentée par `GameStopped`. `playTime/playCount/lastPlayed` = agrégats de cette table. `pegasus-sync` cesse de réécrire les plays.
- v2 : `library.sqlite` possédée par l'hôte (jeux, collections, assets, extra), `pegasus-sync` devient un module d'import. Hors périmètre de cette section.

## 4. Coût et risques

| Route | Jours (un dev expérimenté) | Ce qu'on paie ensuite |
|---|---|---|
| (a) Pegasus + démon | 3–5 pour le démon, les entrées factices, XHR read/write et l'inotify | Plafond atteint le premier jour ; chaque action passe par un fichier ; `stats.db` pollué par les pseudo-jeux ; splash externe conservé ; Qt 5 |
| (b) Fork Pegasus | 12–20 : rebase sur `qt6_v2` ou `ChuLiqiang/qt6-port`, pont QtDBus (~400 lignes C++), packaging CMake/Qt 6, manette Android à ignorer | 2–4 j/an de C++ que la personne n'écrit pas ; upstream à un mainteneur ; port Qt 6 non fusionné depuis 2025-09 |
| **(c) Hôte PySide6** | **15–25** : noyau 5 (engine, api, modèles, parser, memory, keys), manette 3, lancement + hooks + sessions 3, port Reprise 1, D-Bus + frontière modules 3, Nix 2, QA 3–5 | Python + QML, les deux langages déjà maintenus ; Qt 6.11 suivi dans nixpkgs (statut LTS non vérifié) |
| (d) OGUI + plugins | 40–60 pour réécrire Reprise en Control/GDScript sous `CardUI`, ou 0 en abandonnant Reprise | API 2.0.0 avec dépréciations en place, 12 releases/an, Godot 4.7 requis vs 4.6.3 packagé, plugin = fork par substitution |

Risques de (c), dans l'ordre où je les vois :

1. `QtQml.Models.SortFilterProxyModel` « subject to change » : repli Python prêt (3.3), quatre fichiers touchés.
2. `Qt5Compat.GraphicalEffects` figé, non optimisé pour l'empilement : Reprise n'empile pas (un flou par scène), et `MultiEffect` est un remplacement ligne à ligne pour `FastBlur`/`ColorOverlay` si besoin.
3. Retour de focus après un jeu sous GNOME Wayland : `requestActivate()` peut être refusé par Mutter ; sous gamescope le problème n'existe pas. À tester la première semaine.
4. PySide6 en retard d'un patch sur Qt (6.11.0 vs 6.11.1 aujourd'hui) : sans conséquence ; un mineur de retard casserait le chargement, d'où le pin explicite dans le flake.
5. Démarrage Python (estimation, non mesuré : de l'ordre de la demi-seconde avant le premier frame) : acceptable pour une session dédiée ; QML compilé (`qmlcachegen`) n'est pas disponible pour un thème chargé depuis un dossier, il faudra vivre avec.

Non vérifié, à dire clairement : rendu des effets contrôlé hors écran uniquement (aucune capture pixel) ; Qt 6 sous gamescope en xcb non testé (Qt 5 y tourne aujourd'hui avec le même plugin) ; PySDL2 non exercé ; AV1 via GStreamer dans Pegasus Qt 5 pour la route (a) non testé (Reprise ne lit aucune vidéo aujourd'hui) ; comportement d'OGUI en mode GNOME imbriqué connu par la release note seulement.

## 5. Ce que je ne ferais pas

- **Pas la route (a)**, même comme étape intermédiaire. Ce qui y est impossible : lancer un process autrement que par un `launch()` de jeu (qui déclenche teardown, splash, `game-start`, et compte une partie), parler à D-Bus, Bluetooth ou InputPlumber, recharger `allGames` sans redémarrer (`CONSTANT`, et le « scan on launch » de juin 2026 est interne à Pegasus). Ce qui y est seulement pénible : lire du JSON par `XMLHttpRequest` avec `QML_XHR_ALLOW_FILE_READ=1` et un `Timer` de polling, écrire une requête avec `QML_XHR_ALLOW_FILE_WRITE=1` qu'un démon ramasse par inotify. Ça marche, ça produit un thème qui ment sur son état la moitié du temps.
- **Pas le fork (b)**, malgré le patch déjà porté : le pont QtDBus est facile, mais on hérite de 766 k de C++, d'un port Qt 6 bloqué sur Android et d'un mainteneur unique. Le seul argument pour (b) — récupérer les providers Steam/GOG/ES2 de Pegasus — ne sert pas à une bibliothèque que `pegasus-sync` génère déjà.
- **Pas OGUI (d) comme hôte de l'UI.** Ses briques système sont vraies et bonnes ; son plugin API ne remplace pas de menu, son UI est un `CardUI` monolithique aux machines d'état globales, et réécrire Reprise en GDScript jette l'unique chose que la personne a demandé de garder. Ce qu'il faut prendre chez OGUI, c'est la liste de ce qu'un hôte doit offrir (profils InputPlumber par jeu, overlay, gamescope), pas le process.
- **Pas Rust ni C++ pour l'hôte** : cxx-qt 0.10 est mûr, mais le seul code natif nécessaire (SDL2, D-Bus, QML) a déjà ses bindings Python, et l'hôte fait quelques milliers de lignes.
- **Pas copier le C++ de Pegasus** : GPLv3 contaminerait l'hôte ; tout ce dont on a besoin est dans la doc publique et dans le comportement mesuré.
