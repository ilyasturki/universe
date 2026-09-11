# 07 — Contrat de module, absorption de `game` / `pegasus-sync` / tracker, disposition des données, lancement GOG via umu

Faits vérifiés le 2026-09-10 : code de l'utilisateur lu en entier (`~/Dotfiles/home/scripts/game` 1 316 l., `pegasus-sync.py` 2 494 l., tracker `_lib.py` + `sync_lutris.py`, `~/NixOs/bin/lib/game-session.mjs`, les 55 yml Lutris), sources dans le store Nix (gogdl 1.3.0, umu-launcher 1.4.0, protonfixes du GE-Proton installé), dépôts amont pour les précédents. Ce qui n'a pas pu être vérifié est en fin de document. Le nom du cœur est **Coulisses** (04 §7) ; le nom de bus devient `com.ilyasturki.Coulisses` et remplace le `com.ilyasturki.Reprise` de 04 (« renommable avant publication »).

## Verdict

Un module = un dossier avec un `module.toml` (identité, hooks, schéma de réglages) et des exécutables que le cœur lance avec un contrat d'environnement ; rien d'autre dans le MVP. Pas de chargement de code dans le processus du cœur (le précédent OGUI/Decky, « mêmes privilèges que l'application », est exactement ce qu'on refuse), pas de seconde interface D-Bus à implémenter côté module : un module long-vécu est un client ordinaire de l'API du cœur, et s'il expose quelque chose au frontend il possède un nom sous le préfixe `com.ilyasturki.Coulisses.Module.` que le frontend découvre comme un lecteur MPRIS. Les réglages par jeu sont déclarés en schéma dans le manifeste et stockés par le cœur dans `[modules.<id>]` du `game.toml`, donc rendus génériquement par n'importe quel frontend. `game` se dissout presque entièrement dans le cœur (source GOG, mises à jour, archive/restore/remove), `pegasus-sync` aussi (artwork, métadonnées) sauf l'extrait vidéo (module) et la galerie web (parquée, l'API reste) ; tout ce qui parle à Pegasus tombe. La vérité est `games/<slug>/game.toml` + `sessions.jsonl` + le journal Markdown ; SQLite n'est qu'un index dans le cache, reconstruit en quelques secondes.

---

## 1. Précédents

| Système | Unité de module | Manifeste | Langage / isolation | Ce que le module peut toucher | Découverte / sélection | Réglages | Ce qu'on retient |
|---|---|---|---|---|---|---|---|
| **Decky Loader** (Steam Deck) | dossier dans `~/homebrew/plugins` : `plugin.json` + `package.json` + `dist/index.js` (obligatoires) + `main.py` (backend Python) + `bin/` (natif, sorti de `backend/out`) | `plugin.json` : `name`, `author`, `flags` (`debug`, `_root`), `api_version: 1`, `publish{tags, description, image}` | Python in-process côté loader, React injecté dans le client Steam ; drapeaux `_root`/`root` (sens exact non vérifié) | classe `Plugin` : `_main` (« Asyncio-compatible long-running code, executed in a task when the plugin is loaded »), `_unload`, `_uninstall`, `_migration` ; toute méthode ordinaire est appelable du TS par `callable<[first: number, second: number], number>("add")` ; événements `decky.emit()` → `addEventListener` ; dossiers `DECKY_PLUGIN_SETTINGS_DIR`, `_LOG_DIR`, `_RUNTIME_DIR` | dossier + boutique ; « install from URL » | libres (dossier settings) | le couple « méthodes nommées + événements nommés » comme toute la surface ; le drapeau root est un précédent à ne pas suivre |
| **OpenGamepadUI** | zip dans `~/.local/share/opengamepadui/plugins` chargé par `ProjectSettings.load_resource_pack` | `plugin.json` : `plugin.id/name/version/min-api-version/link/source/description`, `store.tags/images`, `author.name/email`, `entrypoint` ; `REQUIRED_META = ["plugin.name","plugin.version","plugin.min-api-version","entrypoint"]`, `PLUGIN_API_VERSION = "2.0.0"` (`plugin_loader.gd:21-24`) | GDScript in-process : « executed with the same privileges as OpenGamepadUI itself […] can be written to modify nearly all aspects » | `Plugin` : `unload()`, `get_settings_menu() -> Control`, `add_library(Library)`, `add_store(Store)`, `add_boxart(BoxArtProvider)`, `add_to_quick_bar`, `add_overlay(OverlayProvider)` (`plugin.gd`) ; un `Library` implémente `get_library_launch_items() -> Array[LibraryLaunchItem]{name, command, args, tags, installed}` | boutique JSON distante + dossier ; semver du plugin vs API | un `Control` Godot par plugin : réglages non génériques | les *rôles* (library, store, boxart, overlay) sont le bon découpage ; le modèle d'isolation est le mauvais |
| **Playnite** (Windows, C#) | DLL .NET 4.6.2 dans `%AppData%\Playnite\Extensions` | `extension.yaml` : `Id`, `Name`, `Author`, `Version`, `Module` (dll), `Type ∈ Script \| GenericPlugin \| GameLibrary \| MetadataProvider`, `Icon`, `Links` | in-process | `GenericPlugin` : `OnGameInstalled/Starting/Started/Stopped(game, elapsedSeconds)/Uninstalled`, `OnApplicationStarted/Stopped`, `OnLibraryUpdated` ; `LibraryPlugin` : `Id`, `Name`, `GetGames → GameMetadata{GameId, PluginId, PlayAction, InstallDirectory}`, `Properties{CanShutdownClient, HasCustomizedGameImport}` | dossier | `GetSettings/GetSettingsView` (vue WPF) | les **kinds** de plugin et la liste d'événements de session ; c'est la liste de hooks la plus proche de la nôtre |
| **Lutris** | runner = classe Python (`lutris/runners/runner.py`) ; service = classe (`lutris/services/base.py`) ; hooks = commandes shell | pas de manifeste : `game_options`/`runner_options` = listes de dicts `{"option", "type", "label", "default", "advanced"}` dont l'UI dérive les formulaires ; `play()` doit « Return the information needed to launch the game: at minimum a 'command' key » | in-process (arbre Lutris) ; hooks = processus | `BaseService` : `id, name, icon, online, local, drm_free, medias, load(), install(db_game), generate_installers()`, `OnlineService.login/logout` ; hooks `prelaunch_command`/`postexit_command`/`prefix_command` avec `GAME_NAME`, `GAME_DIRECTORY` (03 §2.1, `game.py:566-582, 697-699`) | in-tree | schéma d'options → UI générée | le **schéma d'options déclaratif** rendu génériquement, et le contrat d'env des hooks ; on garde les deux |
| **Cartridges** (GTK) | classes `Source` in-tree (`cartridges/importer/{steam,lutris,heroic,bottles,flatpak,desktop,itch,legendary,retroarch}_source.py`) | aucun : attributs `source_id`, `name`, `variant`, `available_on`, `locations`, `iterable_class` ; `SourceIterable.__iter__` « yield `None` when an iteration hasn't produced a game », rend `Game` ou `tuple[Game, tuple[Any]]` ; `ExecutableFormatSource.executable_format` | in-process | rien d'externe : **pas d'API de plugin tiers** | — | — | un patron d'importateur, pas un système de modules ; utile comme modèle de la classe `Source` du cœur |
| **umu-protonfixes** | un module Python par jeu : `gamefixes-<store>/umu-<id>.py` ou `gamefixes-steam/<appid>.py` | `umu-database.csv` : `TITLE,STORE,CODENAME,UMU_ID,…` | exécuté dans le Proton du jeu | `early()`/`main()` (+ `_with_id`) ; `default.py` par store ; local `~/.config/protonfixes/localfixes/<id>.py` — « local fixes prevent global fixes from being executed » (`fix.py`, `run_fix`) | par env `UMU_ID` + `STORE` (`get_module_name`) ; **aucun repli** d'un store à l'autre : `ImportError` → « No … protonfix found » | `config.main.enable_global_fixes` | la règle **local écrase global** et la résolution par (store, id) ; on copie la première |
| **xdg-desktop-portal** | service D-Bus activable implémentant `org.freedesktop.impl.portal.*` | `<DATADIR>/xdg-desktop-portal/portals/<name>.portal` : `DBusName=org.freedesktop.impl.portal.desktop.<name>`, `Interfaces=…;`, `UseIn=` (déprécié) | processus séparé, sans privilège partagé | seulement les interfaces déclarées | `portals.conf` (`$XDG_CONFIG_HOME`, `/etc/xdg`, `$XDG_DATA_DIRS`, variante `<DESKTOP>-portals.conf`) : `[preferred] default=gnome;gtk`, clés par interface `org.freedesktop.impl.portal.Screenshot=…`, valeurs spéciales `*` et `none`, « in the same order as specified » | — | le **manifeste qui déclare des interfaces** + la config qui choisit l'implémentation ; le modèle de notre `[dbus]` réservé |
| **MPRIS** | processus qui possède `org.mpris.MediaPlayer2.<name>` (« must only contain the ASCII characters [A-Z][a-z][0-9]_- and must not begin with a digit ») ; instances multiples `.instance<pid>` | aucun : le nom *est* le manifeste | processus séparé | objet fixe `/org/mpris/MediaPlayer2` ; `org.mpris.MediaPlayer2` et `.Player` obligatoires, `.TrackList`/`.Playlists` facultatives | `ListNames` + préfixe | — | la **découverte par préfixe de nom** sans registre ; c'est ce que le frontend fait pour trouver l'écran d'un module |

Lectures amont maigres (à noter) : le README de Decky et la page wiki « Getting Started » n'ont rendu qu'un titre (site rendu en JS) — le contrat Decky ci-dessus vient du dépôt `decky-plugin-template` (`main.py`, `plugin.json`, `src/index.tsx`, `README.md`) ; le README de gogdl dit seulement « This is **not** user friendly cli, it's meant to be used by some other application » — la surface CLI vient de `gogdl --help` et de `args.py` du store ; `docs/installers.rst` de Lutris ne décrit pas les hooks — ils viennent de la source citée en 03.

---

## 2. Contrat de module

### 2.1 Forme

Un module est un dossier `<id>/` contenant `module.toml` et ce qu'il veut (scripts, un binaire, un `ui/`). Deux emplacements, le premier écrase le second sur un même `id` (règle protonfixes) :

```
$XDG_CONFIG_HOME/coulisses/modules/<id>/     # modules de l'utilisateur, activés ou non
$XDG_DATA_DIRS/coulisses/modules/<id>/       # modules livrés (paquet, Nix, distro)
```

`<id>` : `[a-z][a-z0-9_-]*`, même alphabet que MPRIS pour pouvoir devenir un segment de nom D-Bus. L'activation est une liste dans `config.toml` (`[modules] enabled = ["tracker-md", "trailer"]`) ; un module présent mais non listé est inerte.

### 2.2 `module.toml`

```toml
api = 1                       # version du contrat ; le cœur refuse ce qu'il ne connaît pas
id = "trailer"
name = "Extrait vidéo"
version = "0.1.0"
kind = "hooks"                # hooks | daemon ; un module peut être les deux
description = "Coupe 20 s de gameplay dans le dernier enregistrement."

[requires]
core = ">=0.1"                # semver du cœur
bins = ["ffmpeg", "ffprobe"]  # sur PATH, sinon le module est marqué « indisponible », pas chargé
dbus = []                     # noms de bus système/session attendus (ex. "org.shadowblip.InputPlumber")

[hooks]                       # chemins relatifs au dossier du module ; chaque clé est facultative
pre-launch   = ""             # bloquant, avant la création du scope ; timeout_s ; peut écrire $COULISSES_ENV_FILE
post-launch  = ""             # asynchrone, après SessionStarted
session-end  = ""             # bloquant court, après la disparition du scope
post-process = "bin/cut-clip" # asynchrone, après RecordingFiled ; reçoit RECORDING_PATH
daemon       = ""             # long-vécu, lancé avec le cœur (unité transitoire coulisses-module@<id>)
timeout_s    = 30             # pour les hooks bloquants ; dépassé = tué, journalisé
required     = false          # true : un pre-launch qui échoue annule le lancement

[dbus]                        # réservé ; le cœur ne fait qu'en vérifier la présence sur le bus
name = ""                     # doit commencer par com.ilyasturki.Coulisses.Module.<id>

[frontend.qml]                # facultatif ; ignoré par un frontend GTK
screen = ""                   # ui/Screen.qml, chargé par Reprise sous « Modules › <name> »

[[settings]]                  # schéma de réglages, rendu générique par tout frontend
key = "seconds"
type = "int"                  # bool | int | float | string | enum | path
default = 20
min = 5
max = 60
label = "Durée de l'extrait"
help = "Secondes de gameplay coupées dans le dernier enregistrement."
scope = "global"              # global (config.toml) | game (game.toml [modules.<id>])

[[settings]]
key = "enabled"
type = "bool"
default = true
label = "Couper un extrait pour ce jeu"
scope = "game"
```

`enum` ajoute `choices = ["a", "b"]`, `path` ajoute `mode = "file" | "dir"`. Rien d'autre : c'est la liste d'options de Lutris (`option/type/label/default`) avec un `scope`.

### 2.3 Contrat d'environnement des hooks

Exporté à chaque hook, en plus de l'env de session de l'utilisateur ; remplace la liste de 03 §3.5 (`GAME_SESSION_ID`, `GAME_SCOPE`, `GAME_ROOT`) :

| Variable | Valeur | Présente dans |
|---|---|---|
| `COULISSES_API` | `1` | tous |
| `GAME_ID` | slug (= `GAME_SLUG` ; les deux existent parce que la clé de dossier et l'identifiant sont la même chaîne, et que les scripts existants lisent déjà un « key ») | tous |
| `GAME_SLUG` | `sanitizeGameName(title)` (§3.6) | tous |
| `GAME_TITLE`, `GAME_PLATFORM` | `title`, `platform` du `game.toml` | tous |
| `GAME_DIR`, `GAME_EXE` | `source.dir`, `launch.exe` | tous |
| `GAME_TOML` | chemin absolu du `game.toml` (lecture seule pour le module) | tous |
| `WINEPREFIX` | `launch.prefix` | backends proton/wine seulement |
| `SESSION_ID` | `YYYYMMDD-HHMMSS` (le même contrat que `game-session.mjs`) | tous sauf `daemon` |
| `SESSION_UNIT` | nom du scope (`coulisses-game-<slug>-<epoch>.scope`) | post-launch, session-end |
| `SESSION_STARTED_AT`, `SESSION_ENDED_AT`, `SESSION_DURATION_S` | ISO 8601 / secondes | session-end, post-process |
| `RECORDING_PATH` | mkv classé ; vide si la session n'a pas été enregistrée | post-process |
| `JOURNAL_DIR` | `<journal_root>/<slug>` (peut ne pas exister) | tous |
| `MODULE_DIR`, `MODULE_DATA_DIR` | dossier du module ; `$XDG_DATA_HOME/coulisses/modules/<id>/` (seul endroit où il écrit) | tous |
| `MODULE_SETTINGS_JSON` | réglages résolus (globaux fusionnés avec `[modules.<id>]` du jeu), en JSON | tous |
| `COULISSES_ENV_FILE` | fichier vide ; des lignes `KEY=VALUE` écrites là par `pre-launch` sont fusionnées dans l'env du jeu, **avant** `launch.env` (le jeu a le dernier mot) | pre-launch |
| `COULISSES_BUS` | `com.ilyasturki.Coulisses` | tous |

Codes de retour : `0` ok ; autre = échec journalisé, et pour `pre-launch` avec `required = true`, lancement annulé avec le message du stderr dans le frontend. Un hook asynchrone tourne dans une unité transitoire `coulisses-hook-<id>-<session>.service` (`CPUWeight=20`, `MemoryHigh=2G` par défaut, réglables dans `[hooks]`) — la protection cgroup de 04 §1 vaut pour les modules aussi.

### 2.4 Ce que le cœur expose

- **Événements** (signaux D-Bus de 04, inchangés) : `Session1.SessionStarted(session_id, game_id)`, `SessionEnded(session_id, duration_s)`, `Recording1.RecordingFiled(session_id, path)`, `Journal1.EntryWritten(session_id, note_path)`, `Library1.LibraryChanged`, `Settings1.SettingChanged(key)`. Un `daemon` s'y abonne ; les hooks n'en ont pas besoin, ils *sont* les événements.
- **Lecture de bibliothèque** : les fichiers eux-mêmes (`games/<slug>/game.toml`, `sessions.jsonl`, `media/`), plus `Library1.Get(slug) → s` (le TOML résolu en JSON, réglages de modules inclus) et `Library1.List() → as`.
- **État de session** : `Session1.State`, `Session1.CurrentSession` et le fichier `$XDG_STATE_HOME/coulisses/current-session.json`.
- **Stockage des réglages** : le cœur seul écrit `[modules.<id>]` ; un module change une valeur par `Library1.SetModuleSetting(slug, module, key, value)` / `Settings1.SetModuleSetting(module, key, value)`, validées contre le schéma.
- **Introspection pour les frontends** : `Modules1.List() → s` (JSON : manifestes, disponibilité, erreurs de `requires`), `Modules1.Enable(id)`, `Disable(id)`. Un frontend rend la page « Modules » depuis ce JSON seul ; s'il est QML et que `[frontend.qml] screen` existe, il l'ajoute derrière le formulaire générique.

### 2.5 Ce que le cœur interdit

Écrire dans `game.toml`, `sessions.jsonl`, l'index SQLite ou le journal autrement que par l'API ; lancer ou arrêter un jeu (`Session1.Launch/Stop` sont réservés aux frontends — un module qui veut lancer est un frontend) ; toucher au dossier d'un autre module ; demander root ou installer une règle polkit (ces prérequis appartiennent au paquet du cœur, 04 §6) ; bloquer au-delà de `timeout_s` ; enregistrer un nom hors de son préfixe. Aucun code de module n'est importé dans le processus du cœur, jamais : c'est la seule règle qui rend « tiers » possible sans audit.

### 2.6 Trois modules pour vérifier la taille du contrat

| Module | `kind` | Hooks | Réglages | Ce qu'il utilise |
|---|---|---|---|---|
| `trailer` (extrait 20 s, port de `make_recording_clip`) | hooks | `post-process` | `seconds` global, `enabled` par jeu | `RECORDING_PATH`, écrit `Media1.SetSlot(slug, "video", path)` |
| `tracker-md` (alimente `~/Documents/notes/games`) | hooks | `session-end`, et `LibraryChanged` via un `daemon` ou un timer | `root` (path), `create_missing` (bool), `vibes_map` (string) | `Library1.Get` (heures = Σ sessions), `JOURNAL_DIR` |
| `inputplumber` (profil par jeu, cible, remap) | daemon | `pre-launch` (charge le profil), `session-end` (profil par défaut) | `profile` (path, jeu), `targets` (enum multiple, jeu) | bus système InputPlumber ; expose `com.ilyasturki.Coulisses.Module.inputplumber` avec `CaptureSource(timeout)` pour l'écran de remap QML (04 §3) |

Les trois tiennent dans le manifeste ci-dessus sans clé supplémentaire ; le mode session gamescope est un `pre-launch` qui écrit `COULISSES_ENV_FILE` ; l'overlay Guide est un `daemon` qui possède son nom et parle à InputPlumber. Ce qui ne tient pas : un module qui voudrait dessiner *dans* Reprise autrement que par `[frontend.qml]` — c'est voulu.

---

## 3. Carte d'intégration

### 3.1 `~/Dotfiles/home/scripts/game`

| Fonction (lignes) | Destination | Notes |
|---|---|---|
| `list_gog` 154-170 (`find … goggame-*.info`, `.playTasks[] \| select(.isPrimary and .type=="FileTask")`, `.buildId`) | **cœur** : scanner `Sources1.Scan("gog")` / `coulisses scan` | L'identité GOG vient du disque, pas de pga.db : 12 jeux de base ont un `.info` à la racine de leur dossier sous `/mnt/games/PC` (Batman AO, Control, Cuphead, Cyberpunk, Dead Cells, de Blob, INSIDE, LEGO Batman, Mini Metro, Mirror's Edge, Bannerlord, Technomancer), plus Dishonored dont le `.info` est cinq niveaux sous son préfixe (`/mnt/games/gog/dishonored/drive_c/GOG Games/Dishonored/`) : **13** contre **3** lignes `service='gog'` dans pga.db. `The Witcher 2 Enhanced Edition` et `007 First Light` ont un dossier sous `/mnt/games/PC` sans `goggame-*.info` à la racine et aucune entrée Lutris ; le scanner les liste « dossier sans manifeste », il n'invente pas de source. Alternative `gogdl import <dir>` (`imports.py`) : rend `{appName, buildId, title, tasks, installedLanguage, dlcs, platform, versionName}` mais appelle le endpoint `builds` pour `versionName` — hors-ligne, on garde le `jq` |
| `is_gog_dir` 176-184, `config_target` 193-210, `list_lutris` 254-266 | **outil de migration**, puis abandonné | Le rejoint d'un `main_file` plié par PyYAML (l. 190-192) est un détail à porter dans l'import |
| `versions.tsv` : `load_manifest`/`manifest_set` 212-248, `pin`/`link`/`seen`/`final` 1143-1262, `remote_steam_build` 402-411, `remote_ps3_ver` 428-435, titledb 457-558, `game-probe` | **parqué** (repacks/Switch/PS3 hors MVP) | Les colonnes survivent comme clés réservées `[source] pinned_version`, `steam_appid`, `seen_build`, `final`, importées par la migration pour ne rien perdre ; un module `update-tracker` les relira |
| `remote_gog_build` 384-392 (`content-system.gog.com/products/<id>/os/windows/builds?generation=2`, sans auth, `branch == null`, dernier `date_published`) | **cœur** : `Sources1.CheckUpdates("gog")` | Même requête que `gogdl` (`manager.py:39-52`) ; résultat en cache `$XDG_CACHE_HOME/coulisses/gog-builds.json` |
| `cmd_update` 877-927 → `gogdl update <id> --platform windows --with-dlcs --path <dir>` | **cœur** : `Sources1.Update(slug)` + frontend (confirmation) | `update` est un alias de `download` (`args.py`) ; seule différence `should_append_folder_name = self.arguments.command == "download"` (`manager.py:22`) — le commentaire du script l. 1107-1109 est exact. Après mise à jour, relire `.info` et mettre `[source] build_id` à jour |
| `archive`/`restore` 955-980 + `park` 934-946 (`hidden = 1` en base, `mv` vers `.archive/`) | **cœur** : `Library1.SetHidden(slug, b)` avec parcage des enregistrements et du journal | Comportement conservé (rename même FS) ; `journal_cell()` du tracker résout déjà `journal/.archive/<slug>/` |
| `remove` 1010-1043 (trash fichiers, préfixe, enregistrements, journal ; ligne pga.db) | **cœur** `coulisses remove` + frontend | Toujours `trash`, jamais `rm` ; la « ligne » devient `games/<slug>/` déplacé vers `$XDG_DATA_HOME/coulisses/.trash/` |
| `status`/`library`/`search` 1047-1101 (`embed.gog.com/userData.json`, `account/getFilteredProducts?mediaType=1&page=N`, `catalog.gog.com/v1/catalog?query=like:`) | **cœur** : source `gog` (`Sources1.Login`, `Library()`, `Search()`) | Le jeton vient de `gogdl auth` (rafraîchit seul, l. 372-378) |
| `login` 1127-1139 (URL d'auth `client_id=46899977096215655`, `gogdl auth --code`) | **cœur** + **CLI / frontend bureau** | Exige un navigateur et un collage de code : impossible à la manette ; Reprise affiche l'URL et attend que la CLI ait fini |
| `install` 1103-1120 (`gogdl download <id> --platform windows --with-dlcs --path /mnt/games/PC`) | **cœur** : `Sources1.Install(id)` | Après téléchargement : créer `game.toml` (§4.4) avec `exe` = tâche primaire, `prefix = <prefixes_dir>/<slug>`, `umu_id` résolu hors-ligne (§5), puis `Session1.Setup(slug)` (03 §3.5, `winetricks isolate_home`) |
| `info` 1122-1125 | **cœur** → frontend (taille, DLC) | `gogdl info --with-dlcs` |
| `exe` 1266-1278 | **cœur** `Library1.Get` | — |
| `resolve` 316-334 (exact › mot › sous-chaîne › chemin) | **CLI** seulement | — |
| `dl`/`ensure_auth` 358-369 (fichier `auth.json` vide = crash de gogdl) | **cœur** | Garder le `{}` initial ; chemin `$XDG_CONFIG_HOME/coulisses/gog/auth.json` (aujourd'hui `~/.config/gogdl/auth.json`, migré) |

### 3.2 `~/Dotfiles/home/scripts/pegasus-sync.py`

| Fonction (lignes) | Destination | Notes |
|---|---|---|
| `load_games` 270-302 (pga.db, `.hidden`) | **abandonné** | La bibliothèque du cœur est la source |
| `PLATFORM_MAP`/`collection_for` 68-93, 305-309 | **cœur** (énumération `platform`) ; collections = **frontend** | Les noms courts (`windows`, `switch`, …) deviennent les valeurs de `platform` |
| `sort_title` 312-317 | **cœur** (défaut dérivé, surchargeable) | — |
| classe `SGDB` 193-267 (recherche nom+année avec la règle des jumeaux l. 228-239, `types=static`, filtre `language == "en"`, tri `upvotes`, `platformdata` → appid Steam) | **cœur** : `media/sgdb.py` | Artwork automatique = MVP |
| `ASSET_TARGETS` 136-142 (boxFront 600x900…, steam 920x430, tile 1024x1024, hero, logo) | **cœur** | Slots = `box-front`, `tile`, `background`, `logo`, `steam`, `screenshots/NN`, `video` |
| `pinned_id` 941-946 (`ART_DIR/<slug>/sgdb_id`, `rawg_id`) | **`game.toml [metadata] sgdb_id / rawg_id`** | Un fichier de pin devient une clé ; la règle « un pin qui change invalide tout ce que SGDB a dit » (l. 990-1008) reste dans le cœur |
| `ART_DIR` 54 (`~/Dotfiles/home/config/pegasus-art/<slug>/<stem>.<ext>`, `screenshotNN`, `video.mp4`) | **cœur** : `config.toml [media] overrides_dir` | Par défaut `$XDG_DATA_HOME/coulisses/overrides/<slug>/` ; l'utilisateur pointe sur ses Dotfiles. **Migration obligatoire** : ces dossiers sont nommés par slug Lutris (`resident-evil-4-remake`, `lego-batman`, `xenoblade-chronicles-x-definitive-edition-citron`), le cœur les nomme par slug sanitisé (§3.4) |
| `fetch_assets_for` 977-1164 (override › lutris › sgdb par slot, empreintes `size:mtime_ns`, `absent`, `sources`) | **cœur** | Le tier `lutris` (coverart/banners) disparaît après migration (copié une fois dans `media/`) ; `.sync.json` reste un cache (§4.3) |
| `SHOT_TIERS` 132 (`override › journal › journal-frames › mkv › steam`), `journal_shots` 814-825, `copy_shots` 828-838 | **cœur** | `journal` = `attachments/*.png` du journal |
| `steam_appdetails` 706-716 (`store.steampowered.com/api/appdetails?appids=&l=english`, pause 1 s), `save_steam_screenshots` 719-727, `STEAM_COLLECTIONS` 121 | **cœur** | Seulement `platform ∈ {windows, linux, mac}` |
| `pick_clip` 748-758, `probe_busyness`/`pick_offset` 775-806, `make_recording_clip` 868-897 (20 s, 720p, x264 CRF 23, `+faststart`), `grab_recording_frames` 848-865 | **module `trailer`** (`post-process`) | Décision CONTEXT-v2 ; les cadres extraits du mkv vont avec (même source, même ffmpeg) |
| `fetch_rawg` 386-411, `rawg_search` 370-383 (`search_precise`, année ±1), `html_text` 414-416 | **cœur** : `metadata/rawg.py` | Résultat écrit dans `[metadata]` du `game.toml` |
| `game_metadata` 428-454 (`TRACKER_DATA` en amorce, `metacritic` du tracker prioritaire, `hltb_*`) | **cœur**, inversé : le cœur alimente le tracker | Voir §3.3 |
| `write_split_metadata` 488-523 (`launch: lutris lutris:rungame/{file.basename}` l. 502), stubs `.lutris`, `write_game_dirs` 571-575, `relocate_media`/`prune_*` 526-568 | **abandonnés** dès que Reprise tourne sur le cœur | Transition : `coulisses export pegasus` (un après-midi) si Pegasus 5.15 reste le shell quelques semaines ; supprimé ensuite |
| `sync_play_stats` 626-668 (réécriture de `stats.db`, heures Lutris réparties sur le nombre de mkv), `count_recordings` 609-623 | **abandonnés** | `sessions.jsonl` est la vérité ; 04 §2 |
| `sanitize_game_name` 589-599, `recording_key` 602-606 | **cœur** `slug()` ; `recording_key` **abandonné** après migration | §3.4 |
| `link_marquee` 900-914 (shinretro) | **abandonné** | Spécifique à un thème Pegasus |
| `--gallery` : `GALLERY_CSS/JS` 1167-2039, `GalleryServer`/`GalleryHandler` 2210-2333, `candidates` 2270-2285, `put_art`/`drop_art` 2179-2202, `art_dest` 2146-2151 | **parqué** ; l'API reste dans le cœur : `Media1.Candidates(slug, slot, page) → JSON`, `Media1.SetSlot(slug, slot, url\|path)`, `Media1.Unset(slug, slot)` ; CLI `coulisses media set/unset` | Deviendra un écran de frontend (sélecteur d'artwork à la manette) ; la galerie HTML peut être ressuscitée telle quelle au-dessus de l'API si l'envie revient |
| `mark_custom` 2154-2162 (`has_custom_coverart_big` dans pga.db) | **abandonné** | — |
| `acquire_lock` 157-166, `write_if_changed` 320-327 | **cœur** | Un seul écrivain (le démon) ; ne pas toucher un mtime pour rien (Syncthing) |

### 3.3 Tracker (`~/Documents/notes/games`)

| Fonction | Destination | Notes |
|---|---|---|
| `sync_lutris.update_row` : « `new_value = game.hours if existing is None else round(max(existing, game.hours), 1)` » (« max(), never assignment: Lutris only knows the hours it launched itself ») | **module `tracker-md`** (livré avec le cœur, opt-in, premier utilisateur du contrat) | `Hours = max(existing, round(Σ sessions.duration_s / 3600, 1))` ; même règle, nouvelle source |
| `CATEGORY_TO_VIBE` (`Multiplayer → multiplayer`, `Poadcast → background`), `merge_vibes` | module, réglage `vibes_map` | Les catégories Lutris deviennent `tags` du `game.toml` (migration) |
| `run_add_game` (shell vers `add_game.py`, ambiguïtés RAWG) | module, réglage `create_missing` | Le module appelle les scripts du skill (chemin réglable) plutôt que de réécrire `write_table` |
| `_lib.journal_cell` (`[↗](journal/<slug>/)` ou `journal/.archive/<slug>/`) | inchangé, côté tracker | Le module ne fait que déclencher la resynchronisation ; la cellule se calcule depuis le disque |
| `.data/games.json` (162 entrées : developer, publisher, genre, release_year, summary, metacritic, hltb_*, rawg_id, sgdb_id, steam_appid, title) | **le cœur possède les métadonnées des jeux installés** et les *pousse* dans `games.json` (mêmes clés) ; le tracker garde son propre scraping pour les ~110 jeux non installés | Réponse à « qui possède » : le cœur pour ce qu'il lance, le skill pour le reste ; `backfill.py` ne réécrit jamais une valeur existante, donc l'alimentation ne se fait pas écraser |
| HLTB (`howlongtobeatpy`) | reste au tracker ; le cœur lit `hltb_*` de `games.json` comme aujourd'hui (`game_metadata` l. 453) | Pas de scraping HLTB dans le cœur : la lib casse quand le site change, et le tracker sait déjà le faire |

### 3.4 Compatibilité des slugs

Les deux fonctions, telles quelles :

```python
# ~/Documents/notes/games/.claude/skills/game-tracker/scripts/_lib.py
def slugify(name: str) -> str:
    s = unicodedata.normalize("NFKD", str(name or "").lower())
    s = "".join(c for c in s if not unicodedata.combining(c))
    s = re.sub(r"['’]", "", s)
    s = re.sub(r"[^a-z0-9-]", "-", s)
    return re.sub(r"-+", "-", s).strip("-") or "unknown"
```

```js
// ~/NixOs/bin/lib/game-session.mjs:122-131
export function sanitizeGameName(name) {
    if (!name) return 'unknown';
    return name
        .toLowerCase()
        .normalize('NFKD')
        .replace(/[\u0300-\u036f]/g, '')
        .replace(/['’]/g, '')
        .replace(/[^a-z0-9-]/g, '-')
        .replace(/-+/g, '-')
        .replace(/^-|-$/g, '');
}
```

Différences : `lower()` avant/après NFKD (sans effet sur le résultat, les deux minusculent avant de filtrer sur `[a-z0-9-]`), et `combining(c)` (toute marque) contre `[\u0300-\u036f]` (diacritiques latins) — une marque hors de cette plage est *supprimée* par l'un et *remplacée par `-`* par l'autre, ce que le `-+ → -` et le `strip` absorbent sauf en début/fin de mot. Sur l'alphabet de sortie `[a-z0-9-]` et les titres de la bibliothèque, les deux sont identiques ; `pegasus-sync.sanitize_game_name` (l. 589-599) est la copie Python de la version JS. **Le cœur embarque la version JS transcrite** (plage `U+0300–U+036F`), parce que c'est celle qui a frappé les dossiers d'enregistrement et de journal ; le tracker doit garder la sienne alignée, ce que son docstring exige déjà.

Ce que le slug **n'est pas** : le slug Lutris. Divergences constatées entre le `slug` de pga.db et le dossier d'enregistrements/journal (= `sanitizeGameName(name)`) : `resident-evil-4-remake` → `resident-evil-4` (name « Resident Evil 4 ») ; `lego-batman` → `lego-batman-the-videogame` (name « LEGO Batman: The Videogame »). Fausse piste à ne pas suivre : le `slug:` de tête d'un yml d'installeur (`cyberpunk-2077-setup`, `the_technomancer`, `dishonored-gog`, `hollow-knight-setup`, `resident-evil-4-remake-setup`) et le nom du fichier yml (`mario-odyssey-…`, `mario-smash-football-…`) ne sont pas le slug Lutris (pga.db dit `cyberpunk-2077`, `the-technomancer`, `super-mario-odyssey`, `super-mario-strikers`) ; la migration se clé sur pga.db (`slug`, `name`, `configpath`), jamais sur ces champs. `pegasus-sync` pontait avec `recording_key()` ; le cœur n'a qu'une clé, `slug = sanitizeGameName(title)`, immuable après création (un renommage de titre ne renomme pas le dossier — `resolveJournalPath` de `game-session.mjs:156-188` fait déjà ce choix pour la note). La migration renomme les dossiers d'overrides `pegasus-art/<slug Lutris>` en conséquence.

---

## 4. Disposition des données

### 4.1 Arborescence

```
$XDG_DATA_HOME/coulisses/
  games/<slug>/
    game.toml                 # vérité : identité, source, lancement, métadonnées, réglages de modules
    sessions.jsonl            # vérité : une ligne par session close
    media/                    # rendu final lu par les frontends
      box-front.jpg tile.png background.jpg logo.png steam.png
      screenshots/01.jpg … 06.jpg
      video.mp4
      .sync.json              # cache de provenance/empreintes/misses (ex-.sync.json de pegasus-sync) ; supprimable
    lutris-installer.yml      # bloc script: archivé par la migration (5 jeux), jamais relu
  overrides/<slug>/           # défaut de [media] overrides_dir ; ici l'utilisateur pointe ~/Dotfiles/home/config/pegasus-art
  proton/<name>/              # liens vers les arbres Proton (03 §3.6.1) : proton-ge, proton-em, proton-cachyos
  modules/<id>/               # MODULE_DATA_DIR de chaque module
  gog/                        # cache de bibliothèque GOG (getFilteredProducts), catalogue
  .trash/<slug>-<date>/       # `coulisses remove` sans corbeille système pour les dossiers de données

$XDG_CONFIG_HOME/coulisses/
  config.toml                 # chemins, défauts de lancement, [modules] enabled, [modules.<id>] réglages globaux
  modules/<id>/module.toml    # modules de l'utilisateur
  gog/auth.json               # jetons gogdl (--auth-config-path)
  keys/{steamgriddb,rawg}     # ex ~/.config/steamgriddb/api_key, ~/.config/rawg/api_key

$XDG_STATE_HOME/coulisses/
  current-session.json        # session ouverte (crash-safe)
  provider-limits.json        # mur codex (ex .codex-limit.json)

$XDG_CACHE_HOME/coulisses/
  index.db (+wal, shm)        # l'index SQLite — dans le cache parce qu'il est jetable ; 04 le mettait dans data
  thumbnails/ gog-builds.json umu-warm/

<recordings_root>/<slug>/<session_id>.mkv        # défaut /mnt/recordings/games ; anciens NNN-… conservés tels quels
<journal_root>/<slug>/<Titre>.md + attachments/  # défaut ~/Documents/notes/games/journal ; machine-owned
```

`config.toml` (extrait) :

```toml
[paths]
games_dir      = "/mnt/games/PC"                 # cible des installations GOG
scan_dirs      = ["/mnt/games/PC", "/mnt/games/gog"]
prefixes_dir   = "/mnt/games/prefixes"
recordings_root = "/mnt/recordings/games"
journal_root   = "~/Documents/notes/games/journal"

[launch]                       # défauts, surchargés par [launch] du jeu
proton = "proton-ge"
mangohud = true
env = { PROTON_ENABLE_WAYLAND = "1" }

[session]
record = true
capture_cursor = false
hide_cursor = true             # GNOME : bascule l'extension hide-cursor@elcste.com (CONTEXT-v2)
journal = true

[media]
overrides_dir = "~/Dotfiles/home/config/pegasus-art"
max_screenshots = 6

[modules]
enabled = ["tracker-md", "trailer"]

[modules.tracker-md]
root = "~/Documents/notes/games"
create_missing = true
```

Le journal reste où il est : c'est un artefact humain synchronisé par Syncthing et lu par Obsidian (04 §2), et `journal_root` est un chemin de config, pas une convention. Le `.no-journal` (`journal/.no-journal`, `parseExcludeList` de `game-session.mjs:234-252`) devient `[session] journal = false` dans le `game.toml`, importé une fois.

### 4.2 `sessions.jsonl`

Une ligne par session **close**, ajoutée à la fin ; jamais réécrite. La session ouverte vit dans `current-session.json` ; au redémarrage du démon, elle est retrouvée par le nom d'unité (04 §1) ; si l'unité n'existe plus (coupure), elle est close avec `"truncated": true` et `ended_at` = mtime du mkv si présent, sinon l'heure de reprise.

```json
{"id":"20260910-213045","started_at":"2026-09-10T21:30:45+02:00","ended_at":"2026-09-10T23:02:11+02:00","duration_s":5486,"exit_code":0,"unit":"coulisses-game-cyberpunk-2077-1757532645.scope","source":"daemon","recording":{"path":"/mnt/recordings/games/cyberpunk-2077/20260910-213045.mkv","capture":"kms","duration_s":5461},"journal":"written","player":null,"core":"0.1.0"}
{"id":"20250702-203351","started_at":"2025-07-02T20:33:51+02:00","ended_at":"2025-07-02T21:32:51+02:00","duration_s":3540,"source":"import-recording","recording":{"path":"/mnt/recordings/games/cyberpunk-2077-2/007-20250702-203351-59m.mkv","legacy_seq":7}}
{"id":"20260803-164529","started_at":null,"ended_at":"2026-08-03T16:45:29+02:00","duration_s":708,"source":"import-lutris","note":"pga.db 10.93 h = 39 348 s, moins 38 640 s d'enregistrements importés depuis cyberpunk-2077-2"}
```

`source ∈ {daemon, import-lutris, import-recording}`. Garde-fou contre le double compte : la ligne synthétique Lutris vaut `max(0, playtime × 3600 − Σ durées des enregistrements importés)` et n'est pas écrite quand elle vaut 0 ; donc `Σ duration_s ≥ heures Lutris`, avec égalité quand les enregistrements couvrent moins que Lutris, et le `max()` du tracker ne redescend jamais. Cyberpunk montre les deux cas : les 9 mkv de `cyberpunk-2077-2` (13 juin → 4 juillet 2025, durées des noms arrondies à la minute) font 38 640 s contre 39 348 s dans pga.db — l'entrée Lutris a été recréée le 13 juin 2025 (`configpath` 1749830442) et ne compte que cette partie — d'où la ligne Lutris de 708 s ci-dessus ; si les 21 mkv archivés (`.archive/cyberpunk-2077`, 30 juillet 2024 → 10 avril 2025, 107 820 s) sont rattachés au même jeu, le reliquat est négatif, aucune ligne Lutris n'est écrite et le total passe à 40,7 h, ce que Lutris avait perdu en recréant l'entrée. `id` est le contrat `YYYYMMDD-HHMMSS` de `game-session.mjs` (le marqueur `<!-- session: id -->` du journal reste la jointure, 04 §2).

### 4.3 L'index SQLite et sa reconstruction

`$XDG_CACHE_HOME/coulisses/index.db`, `PRAGMA user_version = <n>`, WAL, **seul écrivain : le démon** ; les frontends ouvrent en `?mode=ro`. Contenu : le schéma de 04 §2 réduit à ce qui se dérive des fichiers — `games` (colonnes de `game.toml` aplaties, `modules` en JSON), `game_stats` (Σ/COUNT/MAX de `sessions.jsonl`), `sessions`, `recordings` (nom, durée, seq legacy), `assets` (un `stat()` de `media/`), `journal_entries` (marqueurs `<!-- session: … -->` + titres `## ` des notes), `screenshots` (`attachments/`), `games_fts` (FTS5 sur `title`, `sort_title`, `developers`, `genres`). Ni corps de note, ni réglages : tout ce qui n'est pas dans un fichier n'existe pas.

Reconstruction : `coulisses index rebuild` (ou automatique si `user_version` diffère, si le fichier manque, ou si un `game.toml` ne parse pas) parcourt `games/*/game.toml`, `games/*/sessions.jsonl`, `games/*/media/`, `<journal_root>/*/*.md`, dans une transaction, ~50 jeux en moins d'une seconde. Incrémental : le démon surveille `games/` et `journal_root` par inotify et compare `mtime` par fichier ; une écriture par l'API met à jour la ligne dans la même transaction que le fichier. Si l'index et le fichier divergent, le fichier gagne, toujours.

### 4.4 Schéma `game.toml` et correspondance Lutris

| Clé Lutris (occurrences sur 55 yml) | `game.toml` | Note |
|---|---|---|
| pga.db `name`, `slug` ; yml `slug:` / `game_slug:` | `title` / `id` = `slug(title)` | La migration se clé sur pga.db (`slug`, `name`, `configpath`) ; le `slug:` de tête d'un yml est celui de l'installeur (`cyberpunk-2077-setup`, `the_technomancer`, `dishonored-gog`…) et ne vaut rien ; aucun n'est la clé du cœur |
| `game.exe` (30) | `[launch] exe` | absolu |
| `game.prefix` (30) | `[launch] prefix` | 2 Kingdom Hearts partagent un préfixe (03 §3.3) : c'est une valeur, pas une fonction du slug |
| `game.main_file` (23) | `[launch] rom` | émulation hors MVP ; importé quand même, `backend = "emulator"`, non lançable tant que le module n'existe pas |
| `game.working_dir` (3) | `[launch] working_dir` | défaut `dirname(exe)` |
| `game.arch` (8, tous `win64`) | `[launch] arch` | umu refuse `win32` (03 §2.2) |
| `game.args` (1, `-nolauncher`) | `[launch] args = ["-nolauncher"]` | tableau, plus de shlex |
| `system.env` (15 blocs : `PROTON_SONY_HIDRAW_XINPUT` ×8, `LC_ALL=''` ×5, `PROTON_SONY_DUALSENSE_AS_DUALSHOCK4` ×2, `WINE_CPU_TOPOLOGY`, `RADV_DEBUG`) | `[launch] env` | appliqué en dernier |
| `system.prelaunch_command` (2, dont Bannerlord vide), `postexit_command` (1, vide) | `[launch] prelaunch` / `postexit` | AC Odyssey : `ac-odyssey-prelaunch` réduit à sa moitié « effacer `1.save` » (03 §3.3), le hook d'enregistrement n'étant plus écrasable |
| `system.prefix_command` (1, RE4 `WINEDLLOVERRIDES="amd_ags_x64.dll=n,b"`) | `[launch] env.WINEDLLOVERRIDES` | 03 §4 |
| `wine.version` (`proton-em` ×2, `system` ×1) | `[launch] proton = "proton-em"` ; `system` → `backend = "wine"` | défaut `proton-ge` en config |
| `wine.esync`/`fsync: false` (1) | `[launch] esync/fsync` | AC Odyssey |
| `wine.overrides` (1, GTA V `socialclub: n,b`, `version: n,b`) | `[launch] dll_overrides = { socialclub = "n,b", version = "n,b" }` | → `WINEDLLOVERRIDES` |
| `wine.dxvk_version: manual` (1) | rien | le préfixe contient déjà les DLL (03 §2.2) |
| `system.fps_limit` (1) | rien | mort dans Lutris 0.5.22 (03 §2.1) |
| `rpcs3.nogui`, `dolphin.platform` | `[launch] emulator_args` | émulation, parqué |
| `service: gog` + `service_id` (3) | `[source] kind = "gog"`, `gog_id` | mais la vérité est le `.info` sur disque : 12 jeux, pas 3 |
| `script:` (5) | `games/<slug>/lutris-installer.yml` | archivé, jamais interprété |
| pga.db `hidden`, catégorie `.hidden` | `hidden = true` | Cyberpunk, 6 jeux |
| catégorie `favorite` (4) | `favorite = true` | — |
| autres catégories (Poadcast, Nintendo, Multiplayer, Finished, …) | `tags = [...]` | le module `tracker-md` en tire les vibes |
| pga.db `playtime`, `lastplayed` | ligne `import-lutris` de `sessions.jsonl` | §4.2 |
| pga.db `year` | `release_year` | RAWG le corrige |
| `journal/.no-journal` | `[session] journal = false` | — |
| `pegasus-art/<slug>/sgdb_id`, `rawg_id` | `[metadata] sgdb_id`, `rawg_id` | — |

Où va `input_profile` : **pas** au premier niveau. InputPlumber est un module (CONTEXT-v2 : « son but est l'émulation ») ; une clé de module au premier niveau du schéma du cœur est exactement la fuite que le contrat interdit. Elle vit dans `[modules.inputplumber] profile`, avec `targets`. Le cœur ne sait pas ce que c'est ; il la stocke, la valide contre le schéma du module et la sert.

### 4.5 Exemple complet : Cyberpunk 2077

Valeurs réelles : `~/.config/lutris/games/cyberpunk-2077-setup-1749830442.yml` (exe, prefix, `LC_ALL`), `/mnt/games/PC/Cyberpunk 2077/goggame-1423049311.info` (`gameId`, `buildId`, `playTasks`), pga.db ligne 51 (`hidden = 1`, `playtime 10.93`, `lastplayed 1785768329` = 2026-08-03T16:45:29+02:00, catégories `Action`, `favorite`, `.hidden`), tracker `.data/games.json["cyberpunk-2077"]` (ids, HLTB, metacritic), `umu-database.csv` du GE-Proton installé (`Cyberpunk 2077,gog,1423049311,umu-1091500`).

```toml
schema = 1
id = "cyberpunk-2077"                 # = slug(title), nom du dossier, immuable
title = "Cyberpunk 2077"
sort_title = "cyberpunk 2077"
platform = "windows"
release_year = 2020
language = "en"                       # langue à l'écran, alimente le journal (04 §5)
hidden = true                         # pga.db hidden = 1 ; enregistrements et journal parqués dans .archive/
favorite = true                       # catégorie Lutris `favorite`
tags = ["Action"]                     # autres catégories Lutris
added_at = "2025-06-13"               # date du fichier Lutris (1749830442)

[source]
kind = "gog"                          # le dossier porte goggame-1423049311.info ; Lutris, lui, y voit encore un repack CODEX
gog_id = "1423049311"
dir = "/mnt/games/PC/Cyberpunk 2077"
build_id = "58809037770927044"        # lu dans le .info ; le fichier gagne s'ils divergent
dlcs = ["1256837418", "1597316373"]   # Phantom Liberty, REDmod (rootGameId = 1423049311)
primary_task = "REDprelauncher.exe"   # isPrimary, category "launcher" ; l'exe ci-dessous est la tâche category "game"

[launch]
backend = "proton"
exe = "/mnt/games/PC/Cyberpunk 2077/bin/x64/Cyberpunk2077.exe"   # tel que Lutris le lance : sans le REDprelauncher
args = []
working_dir = ""                      # défaut : dirname(exe)
prefix = "/mnt/games/prefixes/cyberpunk-2077"
proton = "proton-ge"                  # défaut de config, écrit ici par la migration pour figer le choix
arch = "win64"
esync = true
fsync = true
dll_overrides = {}
env = { LC_ALL = "" }                 # system.env du yml
prelaunch = ""
postexit = ""
umu_id = "umu-1091500"                # umu-database.csv : (gog, 1423049311) → umu-1091500
store = "gog"
mangohud = true

[session]
record = true
capture_cursor = false
hide_cursor = true
journal = true

[metadata]
developers = ["CD PROJEKT RED", "CD PROJEKT"]
publishers = ["CD PROJEKT RED"]
genres = ["Action", "Shooter", "RPG"]
summary = "Cyberpunk 2077 is a science fiction game loosely based on the role-playing game Cyberpunk 2020."
description = ""                      # RAWG description_raw, à remplir au premier scrape
metacritic = 73
hltb = { main = 26.0, extra = 63.1, completionist = 108.4 }
players = 1
rawg_id = 41494
sgdb_id = 5209422
steam_appid = 1091500

[media]                               # pins de l'utilisateur seulement ; la provenance des autres slots est dans media/.sync.json
# tile = "override"                   # aucun override dans pegasus-art/ pour ce jeu

[modules.trailer]
enabled = true

[modules.inputplumber]
profile = ""                          # vide = profil par défaut
targets = ["ds5-edge"]
```

Ce que l'exemple montre : la migration garde l'exe de Lutris (`Cyberpunk2077.exe`, tâche `category: game`) et non la tâche primaire GOG (`REDprelauncher.exe`) ; règle générale du cœur pour une installation neuve = tâche primaire, surcharge utilisateur toujours gagnante. `description` vide et `[media]` sans pin : rien n'est inventé, le premier `Media1.Refresh` les remplit.

---

## 5. Lancement GOG via umu

### 5.1 Faits (source installée, `umu-launcher 1.4.0`, `$U = …/site-packages/umu`)

- `umu_run.py:132-138` : « GAMEID is strictly required and the client is responsible for setting this. When the client only sets the GAMEID, the WINE prefix directory will be created as $HOME/Games/umu/$GAMEID. » puis `if not os.environ.get("GAMEID"): log.info("No GAMEID set, using umu-default")`.
- `umu_run.py:146-151` : `env["STORE"] = os.environ.get("STORE", "")` ; **sans `WINEPREFIX`, avec `STORE` posé, le préfixe devient `~/Games/<STORE>`** — un seul préfixe pour tous les jeux du store. Le cœur pose toujours `WINEPREFIX` ; ce piège est la raison de ne jamais laisser un module ou un script appeler `umu-run` sans.
- `umu_run.py:273-279` : `env["UMU_ID"] = env["GAMEID"]` ; si `UMU_ID` matche `^umu-[\d\w]+$`, `STEAM_COMPAT_APP_ID = <ce qui suit le tiret>`, recopié dans `SteamAppId`/`SteamGameId`. Donc `GAMEID=umu-1091500` fait voir à Proton l'appid Steam de Cyberpunk.
- umu-run **n'interroge jamais** `umu.openwinecomponents.org` (grep de `openwinecomponents|umu_api|umu-database` dans `$U/*.py` : rien). La correspondance codename GOG → umu-id est à la charge du client — Lutris le fait avec `~/.local/share/lutris/runtime/umu-games/umu-games.json` (03 §2.1).
- Le README amont confirme la sémantique : « GAMEID designates a corresponding umu-id from the umu-database for games that have fixes that need to be applied », « STORE designates what storefront to use. UMU uses GAMEID and STORE to search the umu-database for fixes to apply to a game », exemple `WINEPREFIX=$HOME/Games/epic-games-store GAMEID=umu-dauntless STORE=egs PROTONPATH="…/GE-Proton8-28" umu-run`. La page ne cite pas `PROTON_VERB` (03 §2.3 l'a vérifié dans la source : défaut `waitforexitandrun`).
- **umu-database** (README amont) : colonnes `TITLE, STORE, CODENAME, UMU_ID, COMMON ACRONYM (Optional), NOTE (Optional)` ; pour GOG « go to https://www.gogdb.org/, search the game title, find ID correlating to the title and Type 'Game' » — le CODENAME GOG est l'id produit numérique, le même que `goggame-<id>.info` ; stores `amazon, battlenet, ea, egs, gog, humble, itchio, steam, ubisoft, umu, zoomplatform` ; API `https://umu.openwinecomponents.org/umu_api.php?store=…&codename=…`. Copie locale : `<GE-Proton>/protonfixes/umu-database.csv` (1 200 lignes), avec une 7e colonne `EXE_STRINGS (Optional)` que le README ne documente pas. Lignes pertinentes : `Cyberpunk 2077,gog,1423049311,umu-1091500,,,` ; `Mount & Blade II: Bannerlord,gog,1564781494,umu-261550,,,` ; `The Witcher 2: Assassins of Kings Enhanced Edition,gog,1207658930,umu-20920,,,` (Witcher 2 est sur le disque, pas dans Lutris).
- **protonfixes** (`<GE-Proton>/protonfixes/fix.py`) : `get_game_id()` lit `UMU_ID`, sinon `SteamAppId`, sinon `SteamGameId`, sinon les chiffres de `STEAM_COMPAT_DATA_PATH` ; `get_module_name()` : `store = 'umu'` ; `if game_id.isnumeric(): store = 'steam'` ; `elif os.environ.get('STORE'): store = STORE.lower()` ; un store inconnu de `get_store_name()` retombe sur `umu` ; module = `protonfixes.gamefixes-<store>.<game_id>`. `_run_fix()` : `import_module` → `except ImportError: log.info('No … protonfix found')` — **aucun repli** vers `gamefixes-umu` ou `gamefixes-steam`. `get_game_title()` cherche la ligne `(UMU_ID, STORE)` dans le CSV pour le titre des logs. Ordre : `default.py` local › global, puis `<id>.py` local › global ; « local fixes prevent global fixes from being executed » (`~/.config/protonfixes/localfixes/`).
- Sur le disque : `gamefixes-gog/` contient 68 fichiers `umu-*.py` (03 en comptait 78 sur un autre arbre), **ni `umu-1091500.py` ni `umu-261550.py`** ; `gamefixes-umu/umu-271590.py` (GTA V, non GOG) et `gamefixes-steam/{1091500,271590}.py` existent mais sont inatteignables avec `STORE=gog`. Bilan identique à 03 §2.4 : zéro fix appliqué aujourd'hui à la bibliothèque, quel que soit le réglage.

### 5.2 Ce que le cœur exécute (backend `proton`, jeu GOG)

```
# env hérité de la session, plus (03 §3.3 inchangé) :
WINEPREFIX=/mnt/games/prefixes/cyberpunk-2077
PROTONPATH=$(readlink -f ~/.local/share/coulisses/proton/proton-ge)
GAMEID=umu-1091500 STORE=gog                    # [launch] umu_id / store ; sinon GAMEID=umu-default sans STORE
PROTON_VERB=waitforexitandrun                   # runinprefix si une session ouverte partage le préfixe
WINEARCH=win64 PROTON_ENABLE_WAYLAND=1 UMU_RUNTIME_UPDATE=0
WINEESYNC=1 WINEFSYNC=1                         # ou PROTON_NO_ESYNC=1 PROTON_NO_FSYNC=1
WINEDEBUG=-all DXVK_LOG_LEVEL=error PROTON_DXVK_D3D8=1 MANGOHUD=1 MANGOHUD_DLSYM=1
LC_ALL=                                         # [launch] env, dernier mot
umu-run "/mnt/games/PC/Cyberpunk 2077/bin/x64/Cyberpunk2077.exe"   # cwd = working_dir ou dirname(exe)
```

dans le scope `systemd-run --user --scope --collect --unit=coulisses-game-cyberpunk-2077-<epoch>.scope -p KillMode=control-group -p TimeoutStopSec=20` de 03 §3.2. Règles :

1. `umu_id` est résolu **hors-ligne à l'installation** (et au `scan`) : lecture de `<proton>/protonfixes/umu-database.csv`, ligne `row[1] == "gog" and row[2] == gog_id` → `row[3]`. Pas d'appel réseau, pas de dépendance à Lutris. Un jeu sans ligne : `umu_id = ""` → `GAMEID=umu-default`, **sans** `STORE` (poser `STORE=gog` avec `umu-default` ne change que le titre des logs et le nom du préfixe par défaut, que l'on fixe de toute façon).
2. `STORE` n'est posé que si `umu_id` l'est : c'est le couple qui choisit `gamefixes-gog/`.
3. `PROTONFIXES_DISABLE=1` reste une clé `[launch] env` par jeu, pas un réglage du cœur.
4. `gogdl launch` (`args.py` : `launch <path> <id> --platform --prefer-task --no-wine --wine --wine-prefix --wrapper --override-exe`) **n'est pas utilisé** : `launch.py` construit `[wrapper…] [wine] <exe> <args>` avec `WINEPREFIX` seul dans `envvars`, sans `PROTONPATH`/`GAMEID`/`STORE`, puis `Popen` en subreaper ; passer par lui reviendrait à `--wine umu-run` avec l'env posé par nous, une couche de plus pour rien. Il sert de référence pour la lecture des `playTasks` (`get_preferred_task`, `workingDir`, `arguments` en shlex, chemins `\` → `/`, résolution insensible à la casse) que le scanner du cœur reprend.

Cohérence avec 03 : §2.3 (variables, verbe, conteneur non contournable, `UMU_RUNTIME_UPDATE=0`) et §2.4 (résolution des fixes, candidats) sont confirmés par la même source ; ce document ajoute le piège du préfixe par store, la résolution hors-ligne par le CSV embarqué, et le rejet argumenté de `gogdl launch`.

---

## 6. Coût (un dev expérimenté, Python)

| Lot | Jours | Contenu |
|---|---|---|
| **Runtime de modules dans le cœur** | 3,5–4 | parse + validation de `module.toml` (schéma TOML, `requires`), découverte deux dossiers avec règle local › global, activation, exécution des hooks avec env, timeouts, `COULISSES_ENV_FILE`, unités transitoires pour l'asynchrone et `daemon`, stockage/validation des réglages dans `[modules.<id>]`, `Modules1.List/Enable/Disable`, `Library1.SetModuleSetting`, CLI `coulisses module ls/enable/disable/doctor`, tests. Hors lot : la page « Modules » générique en QML (1,5 j côté frontend) |
| **Absorption de `game`** | 3 | source GOG (auth gogdl, bibliothèque, catalogue, install, info, vérification et application des mises à jour, relecture `.info`), scanner `goggame-*.info` avec la règle « une ligne par dossier, la tâche primaire gagne » (`list_gog` l. 164-168), archive/restore avec parcage, remove |
| **Absorption de `pegasus-sync`** | 3 + 1 | SGDB (recherche, jumeaux, slots, `absent`, pins), RAWG, captures Steam, tiers de captures, overrides avec empreintes, `.sync.json`, `Media1.*`, CLI `media` ; + 1 j pour le module `trailer` (port de `pick_offset`/`make_recording_clip`/`grab_recording_frames`) |
| **Module `tracker-md`** | 1 | Hours `max()`, lien journal, vibes depuis `tags`, création via `add_game.py`, alimentation de `games.json` |
| **Outil de migration** | 2 + 1 | `coulisses import lutris` : 55 yml + pga.db → `game.toml` (table §4.4), `sessions.jsonl` (ligne Lutris + enregistrements, garde-fou de double compte), archivage des `script:`, renommage des overrides `pegasus-art`, `.no-journal`, jetons/clés, `versions.tsv` → clés réservées ; + 1 j de validation par diff d'env (03 §3.6.3, 29 jeux Proton) |
| **Total** | **≈ 14–15 jours** | s'ajoute aux ~33 j de 04 en remplaçant ses lots « Bibliothèque : importateurs, port des scrapers » (4 j) et une partie de « Squelette » ; net ≈ +9 j sur le plan global |

Risques propres à ce lot : (1) la migration des slugs (overrides, enregistrements `cyberpunk-2077-2`, journaux archivés) se fait à la main pour une poignée de cas — prévoir un rapport « non résolu » plutôt qu'une heuristique ; (2) un module `pre-launch` lent (InputPlumber 7-9 s, 04 §3) fait attendre le splash : le timeout par défaut de 30 s est là pour ça, et le frontend doit afficher « module X en cours » ; (3) le schéma `[[settings]]` est petit à dessein — le premier module qui demande un type de plus (liste, table) le demandera pour de bonnes raisons, et c'est le moment d'ajouter `api = 2`.

---

## 7. Verdict

- **Contrat** : manifeste TOML + hooks exécutables + schéma de réglages ; pas de code chargé dans le cœur ; les modules long-vécus sont des clients D-Bus du cœur et possèdent, s'ils exposent quelque chose, un nom sous `com.ilyasturki.Coulisses.Module.`. C'est la taille de la moitié d'un runner Lutris et ça couvre les sept modules listés par l'utilisateur.
- **`game`** : tout entre dans le cœur sauf le suivi de versions des repacks/ROM/PS3, parqué avec ses données préservées. **`pegasus-sync`** : artwork et métadonnées dans le cœur, extrait vidéo en module, galerie parquée derrière une API, tout le reste (Pegasus, `stats.db`, pga.db) supprimé. **Tracker** : alimenté par un module livré, jamais remplacé ; le cœur possède les métadonnées des jeux installés et les pousse dans `games.json`.
- **Données** : `game.toml` + `sessions.jsonl` + Markdown ; index SQLite dans le cache, reconstruit depuis les fichiers ; `slug = sanitizeGameName(title)`, le slug Lutris disparaît à la migration.
- **GOG via umu** : `GAMEID=umu-<id>` + `STORE=gog` résolus hors-ligne dans le CSV embarqué, `WINEPREFIX` toujours posé ; aucun fix ne s'applique aujourd'hui, la valeur est de ne pas avoir à y revenir ; `gogdl launch` n'apporte rien.

## 8. Ce que je ne ferais pas

- Charger des plugins Python dans le démon (Decky, OGUI) : la ligne « mêmes privilèges » de la doc OGUI est un avertissement, pas une fonctionnalité ; et un module qui plante ne doit pas emporter la session.
- Une interface D-Bus « Module1 » que chaque module devrait implémenter : un exécutable avec un env suffit pour six modules sur sept, et le septième (overlay) est un client de nos signaux.
- Un `input_profile` au premier niveau du `game.toml` : la première fuite d'un module dans le schéma du cœur en appelle dix.
- Garder le slug Lutris quelque part « pour compatibilité » : `recording_key()` existe précisément parce qu'il y avait deux clés ; on n'en garde qu'une.
- Réutiliser `gogdl launch` ou `gogdl import` dans le chemin de lancement/scan : le premier ne connaît pas Proton, le second téléphone à GOG pour un nom de version.

## Non vérifié

- Le texte de `umu(1)` pour `STORE`/`GAMEID` : la page de manuel du store contient les exemples (`GAMEID=umu-starcitizen`, `umu-genshin`) mais mon extraction des paragraphes `.B` a échoué ; les faits ci-dessus viennent de `umu_run.py` et du README amont.
- La sémantique exacte du drapeau `root` de Decky (le template n'a que `debug` et `_root`) ; sans importance pour notre contrat.
- Que `gogdl update` réécrive `goggame-<id>.info` (`buildId`) en place après mise à jour — le script `game` le suppose (`row_gog` compare `.buildId` au build distant) ; à confirmer sur une vraie mise à jour, sinon le cœur relit le build par `gogdl import`.
- L'origine du dossier `/mnt/recordings/games/cyberpunk-2077-2` (9 mkv, juin-juillet 2025) à côté de `.archive/cyberpunk-2077` (21 mkv, juillet 2024 → avril 2025, jusqu'à 62 Go l'un) : `sanitizeGameName` ne produit jamais de suffixe `-2`, donc il a été créé ou renommé à la main ; la migration devra demander lequel rattacher à la session.
- `description` RAWG et la provenance des slots média de Cyberpunk : aucun `.sync.json` n'existe pour ce jeu (`pegasus-sync` saute les jeux cachés), d'où les champs vides dans l'exemple.
- L'ordre d'application `COULISSES_ENV_FILE` › `launch.env` › umu : logique, non testé (umu recopie l'env sans le filtrer sauf `LD_PRELOAD` sous gamescope, 03 §2.3).
- Le coût de 14-15 jours est une estimation sur le code lu, pas sur un prototype.

## Sources

- Local : `/home/yasso/Dotfiles/home/scripts/game` (l. citées), `/home/yasso/Dotfiles/home/scripts/pegasus-sync.py` (l. citées, numérotation du fichier), `/home/yasso/Documents/notes/games/{README.md,.claude/skills/game-tracker/SKILL.md,.claude/skills/game-tracker/scripts/{_lib.py,sync_lutris.py},.data/games.json,playing.md,journal/}`, `/home/yasso/NixOs/bin/lib/game-session.mjs:122-131,156-188,234-252`, `/home/yasso/.config/lutris/games/*.yml` (55), `~/.local/share/lutris/pga.db` (`select … where service='gog'`, `games_categories` du jeu 51), `/mnt/games/{PC,gog}/**/goggame-*.info` (13 jeux de base), `/mnt/recordings/games/{cyberpunk-2077-2,.archive/cyberpunk-2077}`, `~/Dotfiles/home/config/pegasus-art/` (38 dossiers).
- Store Nix : `gogdl-1.3.0` (`gogdl --help`, `args.py`, `dl/managers/manager.py:22,39-52`, `launch.py`, `imports.py`) — attribut nixpkgs `gogdl` (`pkgs/by-name/go/gogdl/package.nix`, homepage `github.com/Heroic-Games-Launcher/heroic-gogdl`), déjà dans `~/NixOs/home/gaming/gog.nix` (`unstable.gogdl`) ; `umu-launcher-unwrapped-1.4.0/…/umu/umu_run.py:126-156,260-285` ; `proton-ge-steamcompattool/protonfixes/{fix.py,umu-database.csv,gamefixes-gog/}`.
- Amont : https://raw.githubusercontent.com/Heroic-Games-Launcher/heroic-gogdl/main/README.md ; https://raw.githubusercontent.com/Open-Wine-Components/umu-launcher/main/README.md ; https://raw.githubusercontent.com/Open-Wine-Components/umu-database/main/README.md ; https://raw.githubusercontent.com/SteamDeckHomebrew/decky-plugin-template/main/{main.py,plugin.json,src/index.tsx,README.md} ; OpenGamepadUI (`docs/documentation/plugins/{introduction,getting_started,tutorials/library_plugin}.md`, `core/systems/plugin/plugin.gd`, `core/global/plugin_loader.gd`, copies dans `scratchpad/research/ogui/`) ; https://api.playnite.link/docs/tutorials/extensions/{intro,plugins,genericPlugins,libraryPlugins,extensionsManifest}.html ; https://raw.githubusercontent.com/lutris/lutris/master/lutris/{runners/runner.py,services/base.py} ; https://raw.githubusercontent.com/kra-mo/cartridges/main/cartridges/importer/source.py et le dossier `importer/` ; https://flatpak.github.io/xdg-desktop-portal/docs/{writing-a-new-backend,portals.conf}.html ; http://specifications.freedesktop.org/mpris/latest/ ; 03-launcher.md et 04-architecture.md de ce dossier.
