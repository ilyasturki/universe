# 04 — Architecture des modules, modèle de données, intégrations, distribution

Faits vérifiés le 2026-09-09 (sources en fin de document). Ce qui n'a pas pu être vérifié est marqué **non vérifié**.

## Verdict

Un seul démon utilisateur en **Python asyncio** (`dbus-fast` + `evdev` + `sqlite3`), exposant une API D-Bus de session, propriétaire d'une base **SQLite** que l'UI lit directement ; les modules lourds (capture, journal, post-traitement) tournent dans des **unités systemd transitoires** que le démon lance et surveille, les modules légers (état de session, bibliothèque, InputPlumber, BlueZ, raccourcis manette) sont des tâches asyncio du même processus. InputPlumber passe en gestion **permanente** (`auto_manage: true`) et se pilote par `LoadProfilePath` / `TargetDevices`, ce qui supprime le redémarrage du service à chaque lancement. Le journal reste en Markdown, la base l'indexe ; `codex exec` reste le fournisseur par défaut derrière une interface à quatre champs. Pas de Flatpak : l'unité publiable est « app + démon + module NixOS/home-manager + prérequis système documentés », en MIT, la frontière D-Bus/SQLite/processus servant aussi de frontière de licence.

## Hypothèse sur l'hôte de l'UI

Le démon, la base SQLite et l'API D-Bus ne dépendent pas de l'hôte QML : ils marchent que le shell soit une app PySide6 (Qt 6.11.2 et PySide6 6.11.0 sont déjà dans le store Nix de la machine, avec `QtMultimedia`, `QtDBus`, `QtQuick`) ou un fork de Pegasus. Le choix du shell appartient à la section voisine. Si Pegasus 5.15 reste le shell pendant la transition, le pont est un export JSON du démon (`library.json`, `sessions/<game_key>.json`) lu par le thème via `XMLHttpRequest` avec `QML_XHR_ALLOW_FILE_READ=1` (déjà noté dans CONTEXT) ; c'est un mode dégradé, pas la cible.

---

## 1. Forme du backend

### Faits

- Le code existant à absorber : ~1 440 lignes de zx JS dans `bin/game-session.mjs` + `bin/process-game-recording.mjs`, 1 676 lignes dans `bin/game-session-summary.mjs`, 1 023 dans `bin/game-journal.mjs`, 858 lignes Python dans `bin/controller_sdl.py` + `bin/controller_evdev.py`, ~2 500 lignes Python dans `pegasus-sync`. Donc ~3 400 lignes Python et ~4 700 lignes JS déjà écrites par la personne qui maintiendra le démon.
- La logique actuelle est déjà « un orchestrateur + des unités » : `game-recording.service` (ExecStartPre attend la fenêtre, ExecStopPost classe le fichier), `game-session-summary-queue.service` en série avec `MemoryHigh=4G` / `MemoryMax=6G` parce qu'« a parallel drain of 2-4 GB sessions put a 32 GB desktop into the OOM killer » (`home/gaming/game-session-summary.nix`). Ces bornes cgroup sont la raison de garder les jobs lourds hors du processus du démon.
- `dbus-fast` 5.0.22 (2026-06-05, MIT, Python ≥ 3.10) : asyncio, export de services (`ServiceInterface`, `@dbus_method/@dbus_property/@dbus_signal`), bus système et session, proxys introspectés. `evdev` 2.0.0 (2026-08-23, BSD-3). Côté Node, `dbus-next` npm est figé à 0.10.2 (2021-10-10), dépôt `dbusjs/node-dbus-next` sans commit depuis 2024-06 : pas de base saine pour un démon D-Bus en JS.
- Rust/`zbus` est la voie d'InputPlumber et d'OGUI (leur extension `extensions/core/src/dbus/**` est du zbus généré), mais il n'y a pas une ligne de Rust dans l'arbre de l'utilisateur.

### Design

**Un démon, `reprise-daemon`** (nom de bus proposé `com.ilyasturki.Reprise`, domaine possédé par l'utilisateur ; renommable avant publication), unité utilisateur :

```ini
# ~/.config/systemd/user/reprise-daemon.service (généré par le module home-manager)
[Unit]
Description=Reprise session daemon
PartOf=graphical-session.target
After=graphical-session.target
[Service]
Type=dbus
BusName=com.ilyasturki.Reprise
ExecStart=%h/.nix-profile/bin/reprise-daemon
Restart=on-failure
[Install]
WantedBy=graphical-session.target
```

Objets et interfaces (un objet par module, tous sous `/com/ilyasturki/Reprise`) :

| Objet | Interface | Méthodes | Propriétés / signaux |
|---|---|---|---|
| `/Session` | `…Session1` | `Launch(game_id) → session_id`, `Stop()` | `State` (idle/launching/running/stopping/processing), `CurrentSession` ; `SessionStarted`, `SessionEnded(session_id, duration_s)` |
| `/Library` | `…Library1` | `Rescan()`, `Import(source)`, `SetFavorite(game_id, b)` | `LibraryChanged` (l'UI relit SQLite) |
| `/Recording` | `…Recording1` | `Start(session_id)`, `Stop()`, `SaveReplay()` | `Mode` (window/kms/off), `Active` ; `RecordingFiled(session_id, path)` |
| `/Journal` | `…Journal1` | `Enqueue(session_id)`, `Regenerate(session_id)`, `Remove(session_id)` | `Provider`, `QueueLength`, `PausedUntil` ; `EntryWritten(session_id, note_path)` |
| `/Input` | `…Input1` | `ApplyGameProfile(game_id)`, `SetTargets(as)`, `CaptureSource(timeout_ms) → s` | `Pads` (aa{sv}) ; `PadsChanged` |
| `/Bluetooth` | `…Bluetooth1` | `StartDiscovery()`, `StopDiscovery()`, `Pair(address)`, `Connect(address)`, `Forget(address)` | `Devices` (aa{sv}), `Discovering` ; `DevicesChanged` |
| `/Screenshots` | `…Screenshots1` | `Capture() → path` | `ScreenshotTaken(path)` |
| `/Settings` | `…Settings1` | `Get(key) → v`, `Set(key, v)` | `SettingChanged(key)` |

Règle : **un seul écrivain SQLite, le démon**. L'UI ouvre la base en lecture seule et appelle D-Bus pour toute action ; les signaux lui disent quand relire.

**Répartition in-process / unités transitoires :**

- In-process (tâches asyncio, un module Python par fonction, activé par `settings`) : machine d'état de session, bibliothèque et importateurs, client InputPlumber, client BlueZ + agent, raccourcis manette (evdev), notifications, splash.
- Unités transitoires lancées par `org.freedesktop.systemd1.Manager.StartTransientUnit` avec propriétés cgroup, surveillées par `JobRemoved` + `ActiveState` :
  - `reprise-record@<session_id>.service` : le processus `gpu-screen-recorder` (choix de la source dans la section capture) ; `ExecStopPost` disparaît, c'est le démon qui classe le fichier sur `SessionEnded`.
  - `reprise-journal@<session_id>.service` : extraction ffmpeg + appel modèle, `MemoryHigh=4G`, `CPUWeight=20`, un à la fois (le démon sérialise, plus de boucle shell `seen[]`).
  - `reprise-scrape.service` : métadonnées (SteamGridDB, RAWG, Steam), à la demande.
- Le jeu lui-même tourne dans une unité transitoire `reprise-game@<session_id>.service` (pas un simple scope) : la durée de session est la vie de l'unité, pas celle de l'enregistrement ; et si le démon redémarre (`Restart=on-failure`), il retrouve la partie en cours par le nom de l'unité + `ActiveState` et par `$XDG_STATE_HOME/reprise/current-session.json`, puis se rattache à `JobRemoved`/`PropertiesChanged` pour en voir la fin. Une session ne meurt donc pas avec le démon.

**Langage : Python.** Raisons pesées : la moitié du code métier est déjà en Python, l'UI sera vraisemblablement hébergée par PySide6 (même environnement, mêmes tests), `dbus-fast` est le seul binding D-Bus asyncio maintenu dans les trois langages candidats, `evdev`/`sqlite3`/`pyyaml` sont natifs. Le pipeline journal en zx (`game-session-summary.mjs`) **n'est pas réécrit en phase 1** : il devient le corps du fournisseur `codex` derrière l'interface de la section 5 (entrée JSON, sortie JSON), et ne sera porté que si l'on touche à sa logique.

### Ce que je ne ferais pas

- Un démon Rust : personne pour le maintenir dans cet arbre, et l'avantage (types zbus) ne pèse rien face à 3 400 lignes Python existantes.
- Un démon Node : binding D-Bus figé depuis 2021, pas d'evdev sérieux.
- Tout dans un processus : le journal a déjà mis la machine dans l'OOM killer ; les cgroups des unités sont la protection, pas une option.
- Un service **système** à nous : tout ce qui a besoin de root existe déjà (InputPlumber, gsr-kms-server, udev) ; notre démon reste en session et parle au bus système via polkit.

---

## 2. Modèle de données

### Faits

- Aujourd'hui l'identité de session est la chaîne `YYYYMMDD-HHMMSS` et « MUST round-trip identically everywhere » (`bin/lib/game-session.mjs`) : nom de fichier, instance `%i` de l'unité, marqueur `<!-- session: id -->`. Elle est déjà la bonne clé primaire.
- Le classement `NNN-<id>-<durée>.mkv` sert de registre (« this replaces the old data.json ledger », `process-game-recording.mjs`), avec un `--renumber` dans `game-journal` pour réparer les index. Quatre formats de nom historiques coexistent (`NNN-YYYYMMDD-HHMMSS-…`, `<game>-NNN-context-…`, `NNN_date…`, `NNN.mkv`).
- Le frontmatter du journal est **entièrement dérivé du corps** (`frontmatterData` : `sessions` = nombre de `## `, `first/last_played` = min/max des marqueurs, `cover` = première image) ; il existe pour Dataview dans Obsidian.
- Playtime/lastplayed n'existent que dans Lutris `pga.db` ; `pegasus-sync` réécrit `stats.db` en répartissant ces heures sur le nombre d'enregistrements (CONTEXT).
- Qt 6.11.2 `qtmultimedia` du store référence `ffmpeg-9.0.1-lib`, qui référence `dav1d-1.5.3`, `libaom-3.12.1`, `libva-2.24.1` et `vulkan-loader` : décodage AV1 logiciel (dav1d) et VA-API disponibles sans rien construire. Qt : « With the VAAPI hardware backend, hardware texture conversion is disabled by default. Set `QT_XCB_GL_INTEGRATION=xcb_egl` to enable it » ; `vulkan` et `drm` sont « not tested with Qt Multimedia by the Qt maintainers ».
- mpv : `--wid` « On X11, the ID is interpreted as a Window » ; aucune entrée Wayland dans le paragraphe. `--input-gamepad=<yes|no>` « Enable/disable SDL2 Gamepad support. Disabled by default. » `--hwdec=vaapi` « requires --vo=gpu, --vo=gpu-next, --vo=vaapi or --vo=dmabuf-wayland (Linux only) », `--hwdec=vulkan` « requires --vo=gpu-next ». `--input-ipc-server=<socket>` pour le pilotage.
- python-mpv : le seul exemple QML est un gist tiers PyQt5/OpenGL (README « Using OpenGL from PyQt5/QML », Robozman) ; rien pour PySide6/Qt 6 RHI. **Non vérifié** qu'un `QQuickFramebufferObject` sous-classé en Python fonctionne.

### Schéma

`$XDG_DATA_HOME/reprise/library.db`, `PRAGMA journal_mode=WAL`, `foreign_keys=ON` ; l'UI ouvre avec `?mode=ro` et `busy_timeout=2000`.

```sql
CREATE TABLE games (
  id INTEGER PRIMARY KEY,
  key TEXT UNIQUE NOT NULL,            -- sanitizeGameName(title), clé des dossiers médias/enregistrements/journal
  title TEXT NOT NULL, sort_title TEXT,
  platform TEXT NOT NULL,              -- 'windows','switch','gamecube','wii','ps3','nds',…
  summary TEXT, description TEXT, release_year INT,
  developers TEXT, publishers TEXT, genres TEXT,   -- JSON arrays
  players INT, favorite INT NOT NULL DEFAULT 0, hidden INT NOT NULL DEFAULT 0,
  metacritic INT, hltb_main_min INT, hltb_extra_min INT, hltb_complete_min INT,
  language TEXT,                       -- langue à l'écran, alimente le journal
  journal_enabled INT NOT NULL DEFAULT 1,   -- remplace .no-journal
  added_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE launchers (               -- remplace Lutris games.yml + system.yml
  game_id INTEGER PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('wine','emulator','native')),
  exec TEXT NOT NULL, args TEXT, cwd TEXT,
  prefix TEXT, runner TEXT,            -- /mnt/games/prefixes/<key>, 'proton-ge'|'proton-em'|'system'
  env TEXT,                            -- JSON {PROTON_SONY_HIDRAW_XINPUT:"1", …}
  prelaunch TEXT, postexit TEXT,
  record INT NOT NULL DEFAULT 1,
  input_profile TEXT,                  -- chemin YAML InputPlumber ou NULL = défaut
  target_devices TEXT                  -- JSON ["ds5-edge","mouse","keyboard"] ou NULL = défaut
);
CREATE TABLE collections (id INTEGER PRIMARY KEY, name TEXT UNIQUE NOT NULL, kind TEXT NOT NULL); -- 'platform'|'user'
CREATE TABLE game_collections (game_id INT REFERENCES games(id) ON DELETE CASCADE,
  collection_id INT REFERENCES collections(id) ON DELETE CASCADE, position INT, PRIMARY KEY (game_id, collection_id));
CREATE TABLE assets (
  game_id INT REFERENCES games(id) ON DELETE CASCADE,
  role TEXT NOT NULL CHECK (role IN ('box_front','tile','background','logo','screenshot','trailer')),
  position INT NOT NULL DEFAULT 0, path TEXT NOT NULL, source TEXT, width INT, height INT,
  PRIMARY KEY (game_id, role, position)
);
CREATE TABLE sessions (
  id TEXT PRIMARY KEY,                 -- YYYYMMDD-HHMMSS, heure locale, = ancien startStr
  game_id INT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
  started_at TEXT NOT NULL, capture_started_at TEXT, ended_at TEXT, duration_s INT,
  source TEXT NOT NULL CHECK (source IN ('daemon','imported-lutris','imported-recording')),
  player TEXT                          -- 'Yasso'|'Dina' (profils émulateur)
);
CREATE TABLE recordings (
  session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
  game_id INT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
  seq INT NOT NULL,                    -- l'ancien NNN, désormais ici et nulle part ailleurs
  path TEXT UNIQUE NOT NULL, duration_s REAL, size_bytes INT, width INT, height INT,
  codec TEXT, capture TEXT CHECK (capture IN ('window','kms')), thumbnail TEXT,
  is_trailer_source INT NOT NULL DEFAULT 0, filed_at TEXT NOT NULL
);
CREATE TABLE screenshots (
  id INTEGER PRIMARY KEY, game_id INT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
  session_id TEXT REFERENCES sessions(id) ON DELETE SET NULL,
  path TEXT UNIQUE NOT NULL, taken_at TEXT NOT NULL,
  source TEXT NOT NULL CHECK (source IN ('pad','frame','manual')), curated INT NOT NULL DEFAULT 0
);
CREATE TABLE journal_entries (
  session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
  game_id INT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
  note_path TEXT NOT NULL, seq INT,    -- le "## #N" de l'entrée
  title TEXT, lang TEXT, next_up TEXT, images TEXT,   -- JSON array de chemins
  provider TEXT, model TEXT, written_at TEXT,
  status TEXT NOT NULL CHECK (status IN ('pending','written','failed','excluded')), error TEXT
);
CREATE TABLE journal_memory (game_id INTEGER PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
  synopsis TEXT, entities TEXT, language TEXT, profile TEXT, updated_at TEXT);   -- ex .game-memory.json
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL, updated_at TEXT NOT NULL);  -- value = JSON
CREATE VIEW game_stats AS
  SELECT g.id AS game_id, COUNT(s.id) AS play_count, COALESCE(SUM(s.duration_s),0) AS play_time_s,
         MAX(s.ended_at) AS last_played
  FROM games g LEFT JOIN sessions s ON s.game_id = g.id GROUP BY g.id;
CREATE INDEX sessions_game ON sessions(game_id, started_at DESC);
CREATE INDEX recordings_game ON recordings(game_id, seq DESC);
CREATE INDEX screenshots_game ON screenshots(game_id, taken_at DESC);
CREATE INDEX journal_game ON journal_entries(game_id, seq DESC);
```

### Disposition des fichiers

```
$XDG_DATA_HOME/reprise/
  library.db (+ -wal, -shm)
  media/<key>/{box-front.jpg, tile.jpg, background.jpg, logo.png, screenshots/NN.jpg, trailer.mp4}
  input-profiles/<key>.yaml                # profils InputPlumber par jeu (section 3)
$XDG_STATE_HOME/reprise/{current-session.json, thumbnails/, provider-limits.json}
<recordings_root>/<key>/<session_id>.mkv   # défaut /mnt/recordings/games, réglable
<journal_root>/<key>/<Display Name>.md + attachments/<YYYYMMDD-HHMMSS>.png   # inchangé, artefact humain
```

### Les trois artefacts remplacés

1. **Réécritures de `stats.db`.** Disparaissent avec Pegasus : `game_stats` est la vérité (durée = vie du processus du jeu, plus la durée d'enregistrement ni les heures Lutris divisées par le nombre de vidéos). Importateur unique `Import('lutris')` : `pga.db` → `games`/`launchers`, et une session synthétique par jeu (`source='imported-lutris'`, `duration_s = playtime*3600`, `ended_at = lastplayed`) pour ne pas perdre l'historique. Tant que Pegasus reste le shell, `pegasus-sync` se réduit à un exporteur qui **lit** la base (metadata.pegasus.txt + stats.db) au lieu d'être la source.
2. **Le registre `NNN-<id>-<durée>.mkv`.** Le nom devient une étiquette : nouveaux fichiers `<session_id>.mkv`, `seq` et `duration_s` en base, plus de `--renumber`. Importateur `Import('recordings')` avec les regex déjà écrites : `^(\d{1,4})-(\d{8}-\d{6})-(?:(\d+)h)?(?:(\d+)m)?\.mkv$` (seq, id, durée), `recording-(\d{8}-\d{6})\.mkv` (orphelin), et pour `<game>-NNN-context`/`NNN_date` l'id vient de `mtime − durée` (`recordingStartId`). Les anciens fichiers ne sont pas renommés : `path` les référence tels quels.
3. **Marqueurs `<!-- session: id -->` et frontmatter.** Le marqueur **reste** : c'est la jointure entre la note et `sessions.id`, et il round-trip déjà. Le frontmatter reste aussi (Dataview), mais il devient un rendu de la base au moment de l'écriture, pas un re-parse du corps. `journal_entries` indexe (titre, seq, langue, images, statut) ; le corps Markdown n'est jamais dupliqué en base. L'UI obtient le texte d'une entrée par `Journal1.GetEntry(session_id) → s` (le démon découpe le bloc `## ` par marqueur, comme `journalEntries()` aujourd'hui), ou bien lit la note et coupe elle-même sur le marqueur.

### « Enregistrements par jeu », « entrées par jeu » dans l'UI

Deux requêtes, un modèle de liste chacun :

```sql
SELECT r.session_id, r.seq, r.path, r.duration_s, r.thumbnail, s.started_at, j.title
FROM recordings r JOIN sessions s ON s.id = r.session_id
LEFT JOIN journal_entries j ON j.session_id = r.session_id
WHERE r.game_id = ? ORDER BY r.seq DESC;

SELECT j.session_id, j.seq, j.title, j.next_up, j.images, s.started_at, s.duration_s
FROM journal_entries j JOIN sessions s ON s.id = j.session_id
WHERE j.game_id = ? AND j.status = 'written' ORDER BY j.seq DESC;
```

Sous PySide6 : un `QAbstractListModel` par requête, rafraîchi sur `LibraryChanged`/`EntryWritten`/`RecordingFiled`. Sous Pegasus : mêmes lignes exportées en JSON par le démon.

### Lecture d'un mkv AV1 4K dans l'UI

Décision : **Qt Multimedia (`MediaPlayer` + `VideoOutput` QML) comme lecteur intégré**, pour les extraits de 20 s, les vignettes et la lecture des sessions ; **mpv en processus enfant plein écran comme « salle »** quand on veut la 4K60 10 bits sans compromis. Justification : le backend FFmpeg de Qt 6.11 a dav1d (le 7900X décode l'AV1 4K en logiciel sans peine) et VA-API ; sa conversion de texture matérielle est désactivée par défaut et ne s'active que sous xcb (`QT_XCB_GL_INTEGRATION=xcb_egl`), donc sous gamescope oui, sous GNOME Wayland non. mpv, lui, fait `--hwdec=vaapi --vo=gpu-next` partout et prend la manette (`--input-gamepad=yes`) :

```
mpv --fs --vo=gpu-next --hwdec=vaapi --input-gamepad=yes --osc=no \
    --input-ipc-server=$XDG_RUNTIME_DIR/reprise-mpv.sock --input-conf=<reprise>/mpv-gamepad.conf <path>
```

Le démon pilote la fenêtre par le socket IPC (`{"command":["quit"]}` sur Cancel). `--wid` (incrustation dans une fenêtre de l'UI) n'est documenté que pour X11 : possible sous gamescope (Qt en xcb), pas sous GNOME Wayland ; je le garde comme optimisation gamescope, pas comme plan. **Risque à tester en premier** : le comportement de focus/empilement d'une deuxième fenêtre XWayland sous gamescope (non vérifié). libmpv via l'API render depuis Python/QML : non, pas de chemin documenté pour PySide6/Qt 6.

### Ce que je ne ferais pas

- Garder l'index dans le nom de fichier (la base sait compter, le renumérotage a coûté un outil).
- Mettre le corps des entrées en base (le `.md` est l'artefact, Obsidian le lit, Syncthing le synchronise).
- Laisser l'UI écrire dans SQLite.

---

## 3. InputPlumber depuis l'UI

### Faits (API vérifiée dans `bindings/dbus-xml/*.xml` et `src/`, v0.79.2 du 2026-09-09)

- Bus système `org.shadowblip.InputPlumber`, préfixe `/org/shadowblip/InputPlumber` (`src/constants.rs`) ; composites `/org/shadowblip/InputPlumber/CompositeDevice<N>`, cibles `/org/shadowblip/InputPlumber/devices/target/<kind><N>`. Vérifié sur le bus de la machine (`busctl --system tree/introspect`, InputPlumber 0.77.1 en local, 0.79.2 upstream) : le Manager est `/org/shadowblip/InputPlumber/Manager`, la racine `/org/shadowblip/InputPlumber` implémente `org.freedesktop.DBus.ObjectManager` (`GetManagedObjects`, `InterfacesAdded`, `InterfacesRemoved`), et `ManageAllDevices` y vaut `false` aujourd'hui.
- `org.shadowblip.InputManager` : méthodes `CreateCompositeDevice(config_path)`, `CreateTargetDevice(kind)`, `StopTargetDevice(path)`, `AttachTargetDevice(target_path, composite_path)`, `HookSleep`, `HookWake` ; propriétés `ManageAllDevices` (b, rw), `GamepadOrder` (as, rw), `SupportedTargetDeviceIds`/`SupportedTargetDevices` (as, ro), `Version`. Le `docs/dbus-interface/manager.md` est périmé (il ne liste que `CreateCompositeDevice`) : les XML font foi.
- `org.shadowblip.Input.CompositeDevice` : `LoadProfilePath(path)`, `LoadProfileFromYaml(yaml)`, `GetProfileYaml()`, `SendEvent`, `SendButtonChord`, `SetInterceptActivation(activation_events, target_event)`, `Stop()` ; propriétés `InterceptMode` (u, rw : 0 NONE, 1 PASS, 2 ALL, 3 GAMEPAD_ONLY), `TargetDevices` (as, rw), `Capabilities`, `TargetCapabilities`, `OutputCapabilities`, `SourceDevicePaths`, `DbusDevices`, `ProfilePath`, `PersistentId`.
- `org.shadowblip.Input.DBusDevice` : signaux `InputEvent(event: s, value: d)` et `TouchEvent(...)`. Chaque composite reçoit **automatiquement** une cible `dbus` à sa création (`src/input/manager.rs`, `start_composite_device`, l. 863-869 : `create_target_device("dbus")` puis `set_dbus_devices`).
- Routage (`src/input/composite_device/mod.rs`, `write_event`, l. 1083-1103) : en mode ALL « only send events to DBus target devices » ; en GAMEPAD_ONLY les événements manette partent vers D-Bus ; en PASS le chord d'activation (Guide par défaut, `intercept_mode_target_cap`) bascule en ALL (`is_intercept_event_single`, l. 2026). Donc l'interception **détourne** l'entrée du jeu : utile pour un overlay ou un écran de capture, inutilisable pour des raccourcis pendant la partie.
- Identifiants de cibles (`src/input/target/mod.rs`) : `dbus, deck, deck-uhid, ds5, ds5-edge, hori-steam, keyboard, mouse, gamepad, touchpad, touchscreen, xb360, xbox-elite, xbox-series, unified-gamepad`.
- Options d'un composite (`schema/composite_device_v1.json`) : `auto_manage` (« If this is false, InputPlumber will not try to manage the device unless an external service enables management of the device. Defaults to 'false' ») et `persist` (« will not stop the CompositeDevice if all the SourceDevices have stopped … allow reconnecting a SourceDevice and resuming input »).
- Politique polkit livrée (`rootfs/usr/share/polkit-1/actions/org.shadowblip.InputPlumber.policy`) : `LoadProfilePath`, `LoadProfileFromYaml`, `SetTargetDevices`, `SetInterceptMode`, `SetInterceptActivation`, `SetGamepadOrder`, `SetManageAllDevices`, `Stop`, `SendEvent` sont `auth_admin` ; toutes les lectures sont `yes`. La règle actuelle (`os/gaming/controller.nix`) n'ouvre que `SetManageAllDevices` et `SetTargetDevices`.
- Profil (`schema/device_profile_v1.json`, `profiles/default.yaml`) : `mapping: [{name, source_event, target_events[]}]`, `target_devices` optionnel ; vocabulaire `gamepad.button` (South…Screenshot, DPad*, *Bumper, *Trigger, *Paddle1-3, *Stick, *Touchpad*), `gamepad.axis` (LeftStick, RightStick, Hat0-3), `gamepad.trigger`, `gamepad.gyro{name,axis,direction,deadzone}`, `gamepad.dial{name,direction}`, `keyboard: Key…`, `mouse.button/motion{direction,speed_pps}`, `touchpad`, `touchscreen`.
- OGUI (`core/global/launch_manager.gd`) : profil par jeu = réglage `game.<nom>.gamepad_profile` + `_target`, appliqué à l'entrée en jeu, remis au profil global à la sortie (`set_gamepad_profile("")`), via `load_target_modified_profile` qui réécrit le profil selon la cible (pads DualSense → `CenterPad`) et l'enregistre en `<profil>_<cible>` avant `LoadProfilePath`.

### Le bug « manage twice »

Aucune issue upstream ne le décrit (recherches `manage all`, `duplicate composite`, `timeout` sur le dépôt : rien de pertinent) ; le commentaire d'`emu-pad` est la seule trace. La lecture de `manager.rs:434-470` est **cohérente** avec l'observation, sans le prouver : `SetManageAllDevices(false)` appelle `stop()` sur chaque composite non `auto_manage`, `SetManageAllDevices(true)` relance `discover_all_devices` dans une tâche ; une deuxième découverte peut courir pendant que les sources de la première sont encore cachées/en arrêt. Le démon **ne doit pas activer la gestion « seulement pour un lancement »** : c'est exactement le cycle qui déclenche le bug. La sortie propre est de ne plus jamais toucher `ManageAllDevices`.

### Design

1. **Gestion permanente.** Dans chaque `devices.d/*.yaml` déjà surchargé, ajouter :
   ```yaml
   options:
     auto_manage: true
     persist: true      # la cible virtuelle survit à un changement de pile / une coupure BT
   ```
   Les manettes sont `ds5-edge` en permanence, pour les jeux, les émulateurs **et** l'UI (SDL voit une DualSense Edge). Le coût de 7-9 s de construction du composite n'est payé qu'au branchement, plus à chaque lancement. `emu-pad` et les trois `systemctl restart` disparaissent.
2. **Profil par jeu.** `launchers.input_profile` → `Input1.ApplyGameProfile(game_id)` fait, sur chaque composite : `LoadProfilePath("$XDG_DATA_HOME/reprise/input-profiles/<key>.yaml")` et, si `launchers.target_devices` est renseigné, `Set TargetDevices`. À `SessionEnded` : `LoadProfilePath(<défaut>)` et cibles par défaut. Le profil par défaut reprend le `paddle-swap` de `controller.nix` (aujourd'hui concaténé au `default.yaml` livré ; il devient un fichier à nous).
3. **Deuxième joueur.** `GamepadOrder` (rw) fixe l'ordre des pads virtuels : le cas `eden -u 1`/`--profile Dina` se règle en base (`sessions.player`) et par cette propriété, sans wrapper.
4. **Règle polkit** livrée par le module NixOS (prérequis système, section 6) : `LoadProfilePath`, `LoadProfileFromYaml`, `SetTargetDevices`, `SetInterceptMode`, `SetInterceptActivation`, `SetGamepadOrder` → `polkit.Result.YES` pour l'utilisateur de session. Plus `manage-units` pour `inputplumber.service`.
5. **Raccourcis pendant la partie** (capture, MangoHud, volume) : le démon lit en **non exclusif** le nœud evdev de la **cible virtuelle** : le `/dev/input/event*` dont le nom udev est celui de la DualSense Edge émulée et qui n'apparaît pas dans `/dev/inputplumber/by-hidden` (le dossier des nœuds physiques passés en mode 000, cf. `emu-pad`). `SourceDevicePaths` ne sert pas ici : il liste les sources physiques cachées, pas la cible. Plus jamais le pad physique : c'est ce qui faisait doubler les appuis, et ses nœuds en mode 000 sont de toute façon illisibles. `controller_evdev.py` se porte quasi tel quel (même `sanitize_game_name`, même détection de combos) ; `controller_sdl.py` et les règles udev qui les démarrent disparaissent, le démon voyant les pads arriver par `InterfacesAdded` de l'`ObjectManager` racine d'InputPlumber (vérifié sur le bus).
6. **Overlay Guide (plus tard, pas dans le lot 1)** : `InterceptMode = 1 (PASS)` + `SetInterceptActivation(["Gamepad:Button:Guide"], "Gamepad:Button:Guide")` ; l'UI reçoit tout par `DBusDevice.InputEvent` tant qu'elle est ouverte, puis remet `PASS`.

### Écran de remap en QML

Modèle = une ligne par `mapping` : `{name, source: {kind: 'gamepad.button'|'gamepad.axis'|'gamepad.trigger'|'gamepad.gyro'|'gamepad.dial'|'keyboard'|'mouse.button'|'mouse.motion', value, extra}, targets: [même forme]}`. Le vocabulaire proposé à gauche vient de `Capabilities` du composite (ce que le pad sait faire), à droite de `TargetCapabilities` ; on n'offre jamais un bouton que le pad n'a pas. « Appuyez sur la source » = `Input1.CaptureSource(timeout)` : le démon passe le composite en `InterceptMode 2`, attend le premier `InputEvent` non nul, remet le mode précédent, renvoie la chaîne (`Gamepad:Button:LeftPaddle1`). Pendant l'interception l'UI ne reçoit plus rien par SDL non plus : l'annulation vient du flux `InputEvent` (bouton East) ou du timeout, jamais de la manette « normale ». Sauvegarde = sérialisation YAML (`version: 1, kind: DeviceProfile, name, target_devices?, mapping`) dans `input-profiles/<key>.yaml` puis `LoadProfilePath`. La ligne `launchers.target_devices` est un simple choix parmi `SupportedTargetDeviceIds`.

### Ce que je ne ferais pas

- Redémarrer `inputplumber.service` depuis l'app (la raison du bug est le cycle on/off, pas le service).
- Basculer `ManageAllDevices` par lancement.
- Utiliser `InterceptMode` pour les raccourcis en jeu (ça coupe la manette au jeu).
- Réécrire des profils « modifiés par cible » à la OGUI : nos pads sont tous `ds5-edge`, un seul profil par jeu suffit.

---

## 4. Bluetooth depuis l'UI

### Faits

- BlueZ `org.bluez.AgentManager1` (`/org/bluez`) : `RegisterAgent(object agent, string capability)`, capacités `""` (= KeyboardDisplay), `DisplayOnly`, `DisplayYesNo`, `KeyboardOnly`, `NoInputNoOutput`, `KeyboardDisplay` ; `RequestDefaultAgent(agent)`.
- `org.bluez.Agent1` : `RequestAuthorization(device)` « Authorize an incoming pairing attempt which would trigger the just-works model », `AuthorizeService(device, uuid)`, `RequestConfirmation(device, passkey)`, `Cancel()`, `Release()`.
- `org.bluez.Device1.Pair()` : « If the application has registered its own agent, then that specific agent will be used. Otherwise it will use the default agent. […] In case there is no application agent and also no default agent present, this method will fail. » Propriétés utiles : `Address`, `Name`, `Alias` (rw), `Icon` (`input-gaming` pour les pads), `Class`, `Appearance`, `Paired`, `Bonded`, `Trusted` (rw), `Connected`, `RSSI`, `ServicesResolved`.
- `org.bluez.Adapter1` : `StartDiscovery`/`StopDiscovery`, `SetDiscoveryFilter({Transport:'auto'|'bredr'|'le', RSSI, Pattern, …})`, `RemoveDevice(path)`, propriétés `Powered`, `Discovering`, `Pairable`, `PowerState`.
- OGUI : son extension Rust (`extensions/core/src/bluetooth/bluez.rs`, `bluez/device.rs`, `dbus/bluez/{adapter1,device1}.rs`) découvre par `ObjectManager.GetManagedObjects` + `InterfacesAdded/Removed`, expose `pair()`/`connect_to()`/`set_trusted()` qui délèguent au proxy ; **aucun agent enregistré** (aucun `Agent1`/`RegisterAgent` dans le code, pas de `agent_manager1.rs`). Son menu (`bluetooth_settings_menu.gd`) n'appelle que `start_discovery`/`connect_to`. Sous GNOME ça marche parce que gnome-shell est l'agent par défaut ; dans une session gamescope sans shell, `Pair()` échouerait (« this method will fail »).

### Design

Module `bluetooth` in-process, `dbus-fast` sur le bus système :

1. À l'init : `RegisterAgent("/com/ilyasturki/Reprise/BtAgent", "NoInputNoOutput")` — les manettes sont en « just works », l'agent répond `RequestAuthorization` → OK, `AuthorizeService` → OK, `RequestConfirmation` → OK, `RequestPinCode`/`RequestPasskey` → `org.bluez.Error.Rejected` (aucune manette n'en demande). `RequestDefaultAgent` conditionné à la session, pas à une détection (BlueZ n'expose pas la liste des agents) : dans la session gamescope (variable posée par notre fichier de session) on le demande ; sous GNOME on ne le demande pas et gnome-shell garde les demandes entrantes. Dans les deux cas notre agent répond à **nos** `Pair()` (règle BlueZ ci-dessus).
2. Découverte : `SetDiscoveryFilter({"Transport": "auto"})` puis `StartDiscovery` ; liste = objets `Device1` filtrés sur `Icon == "input-gaming"` ou `Appearance` dans la plage HID gamepad (0x03C0-0x03CF), les autres derrière un bouton « tout afficher ».
3. Appairage : `Pair()` → `Trusted = true` (sinon la reconnexion au réveil du pad demande une autorisation) → `Connect()`. Oubli : `Adapter1.RemoveDevice(path)`.
4. Exposition à QML : par **notre** D-Bus de session (`Bluetooth1.Devices` = `aa{sv}` {address, name, icon, paired, connected, rssi}, signal `DevicesChanged` relayé depuis `PropertiesChanged`/`InterfacesAdded`). Pas de second protocole socket/JSON : un seul IPC pour tout, et PySide6 a `QtDBus` pour l'écouter.

### Ce que je ne ferais pas

- Copier OGUI sans agent : ça ne marche que parce que GNOME est derrière.
- Parler à BlueZ depuis l'UI : deux clients du bus système pour la même chose, et l'agent doit vivre dans un processus qui reste.

---

## 5. Journal : la frontière fournisseur

### Interface (le point, pas la surface)

```python
@dataclass
class SummaryRequest:
    images: list[Path]            # ≤ 40, déjà triées et dédupliquées par le démon
    context: str                  # entrée précédente, synopsis, méta du jeu, langue attendue
    schema: dict                  # JSON Schema : title, body, next, images, memory{synopsis, entities, language, profile}
    lang: str | None

@dataclass
class SummaryResult:
    data: dict                    # validé contre schema par le démon, pas par le fournisseur
    provider: str; model: str
    usage: dict | None            # tokens ou coût si connu
    retry_after: datetime | None  # mur de quota → le démon met la file en pause

class Provider(Protocol):
    name: str
    async def summarize(self, req: SummaryRequest) -> SummaryResult: ...
```

Extraction des images, hachage, dédup, curation depuis `attachments/`, écriture de la note, mémoire, notifications : **hors** fournisseur, dans `reprise-journal@`. Le fournisseur ne fait que « images + texte + schéma → JSON ».

### Fournisseurs

| Fournisseur | Appel | Vérifié | Coût / vie privée |
|---|---|---|---|
| **`codex` (défaut)** | `codex exec -i a.png,b.png … --output-schema schema.json -o out.json --json -m gpt-5.6-sol "<prompt>"` ; « reuses saved CLI authentication by default », « prints only the final agent message to stdout » | `codex --help` local (0.153.4) : `-i, --image <FILE>...`, `-m`, `--output-schema`, `--json` | Inclus dans l'abonnement ChatGPT ; murs d'usage de plusieurs jours (déjà gérés par `.codex-limit.json`, qui devient `retry_after`). Données : politique ChatGPT du compte. Phase 1 : le corps reste `game-session-summary.mjs`, appelé en sous-processus avec un JSON en entrée. |
| **Anthropic Messages API** | SDK `anthropic` 1.4.0, blocs `{"type":"image","source":{"type":"base64",…}}` avant le texte, sortie structurée `output_config.format` ; modèle `claude-opus-5` (défaut recommandé par la doc) ou `claude-sonnet-5` | Doc vision : JPEG/PNG/GIF/WebP, 10 Mo/image, 600 images par requête sur les modèles 1M, au-delà de 20 images chaque côté ≤ 2000 px ; coût `⌈w/28⌉×⌈h/28⌉` tokens, 1920×1080 = 2 691 tokens en haute résolution (Claude ≥ 4.7), 1 560 si réduite à 1456×819 | 40 images × 1 560 ≈ 62 k tokens d'entrée : ≈ 0,31 $/session sur `claude-opus-5` (5 $/M), ≈ 0,12 $ sur `claude-sonnet-5` (2 $/M), moitié prix en Batch API (asynchrone, ce qu'est un journal). « Anthropic does not use uploaded images to train models », images « not stored beyond the duration of the API request ». |
| **OpenAI Responses API** | `input_image` en data URL base64 ou `file_id`, `detail: low/high/original/auto` | Doc : PNG/JPEG/WebP/GIF non animé, 1 500 images, 512 Mo par requête ; combinaison avec `text.format` JSON Schema **non vérifiée** sur cette page | Clé API facturée ; utile surtout si l'on quitte l'abonnement ChatGPT. |
| **Ollama local** | `POST /api/chat` avec `images: [base64]` et `format: <schema>` (« Vision models accept the same format parameter ») | Modèles : `qwen3-vl` 2B/4B/8B/30B/32B/235B ; la doc vision d'Ollama cite `gemma4`. **Non vérifié** : qualité d'un 8B sur 40 captures de jeu et tenue d'un 30B dans les 16 Go de la RX 7900 GRE | Zéro coût marginal, zéro sortie réseau ; qualité à évaluer sur 5 sessions avant d'en faire une option affichée. |

Réglage : `settings.journal.provider` + `settings.journal.model` ; le démon valide le JSON contre le schéma quel que soit le fournisseur, et réduit lui-même les images (1456×819 est un bon plafond : lisible, et le moins cher partout).

### Ce que je ne ferais pas

- Un système de plugins pour ça : quatre classes dans un module, un `Protocol`, c'est la frontière.
- Envoyer le mkv ou plus de 40 images : le pipeline actuel a déjà trouvé la bonne taille.

---

## 6. Distribution « personnel maintenant, publiable plus tard »

### Ce qui ne peut pas vivre dans un Flatpak (doc Flatpak, `sandbox-permissions.rst`)

- Le sandbox ne voit `/etc` et `/usr` de l'hôte qu'en lecture, montés à part : « `host-etc` — Host's `/etc` is mounted at `/run/host/etc` », « `host-os` — Host's `/usr, /bin, …` Mounted at `/run/host` ». Donc **aucune règle udev, aucune règle polkit, aucune unité système, aucune politique D-Bus système, aucun binaire setuid** ne peut être installé par le paquet.
- Bus : « an app can only own its own name on the bus » ; « Access to the entire bus with `--socket=system-bus` […] stops the filtering and using them is a security risk. So they must be avoided ». Notre démon **pourrait** posséder `com.ilyasturki.Reprise.*` et parler à `org.shadowblip.InputPlumber`/`org.bluez` par `--system-talk-name` ; ce n'est pas le bus qui bloque.
- Périphériques : `--device=input` « Input devices as exposed in /dev/input. This includes game controllers » existe ; mais `/dev/uinput`, hidraw, KMS (`gsr-kms-server`) ne sont pas dans le modèle.

Concrètement, restent hors paquet : `game-devices-udev-rules`, modules noyau `xone`/`xpadneo`, `inputplumber.service` + sa politique D-Bus + notre règle polkit, `gpu-screen-recorder` avec `gsr-kms-server` setuid, la session GDM gamescope, la variable `SDL_JOYSTICK_HIDAPI_XBOX=0`. Un Flatpak ne pourrait offrir que « UI + démon en mode maison » avec capture par portail — ce qui contredit « jamais de souris ».

### Unité publiable

1. **Un paquet Python** (`pyproject.toml`, `uv`) qui contient l'UI (QML + hôte) et le démon : `reprise` (binaire UI), `reprise-daemon`, `reprise-cli` (import, journal à la demande, diagnostic).
2. **Un flake Nix** : paquet, `nixosModules.reprise` (InputPlumber + `devices.d` retargetés, règle polkit étendue, udev, gpu-screen-recorder, session GDM optionnelle), `homeManagerModules.reprise` (unités utilisateur, réglages, chemins). NixOS est la distribution de référence.
3. **AUR** puis **Fedora COPR** plus tard, avec `docs/prerequisites.md` : InputPlumber ≥ 0.79 avec nos `devices.d` et la règle polkit, gpu-screen-recorder + gsr-kms-server, règles udev manettes, optionnellement gamescope + fichier de session.

Layout du dépôt (monorepo) :

```
reprise/
  ui/            # le thème Reprise actuel (QML, MIT) + l'hôte
  daemon/        # paquet Python reprise_daemon : modules/{session,library,recording,journal,input,bluetooth,shortcuts,splash}
  providers/     # journal : codex.py, anthropic.py, openai.py, ollama.py + le script zx hérité
  nix/           # flake, modules NixOS/home-manager, devices.d, polkit
  packaging/     # PKGBUILD, spec COPR, fichiers .desktop/.service/session gamescope
  tools/         # shot, sample, sounds… (existants)
  docs/
```

### Licences

- Vérifiées : SortFilterProxyModel **MIT** (réutilisable dans l'UI), dbus-fast **MIT**, python-evdev **BSD-3**, InputPlumber et OpenGamepadUI **GPL-3.0**. Non confirmées par l'API GitHub (NOASSERTION) : zbus, py-sdl2, mpv, python-mpv — à relire dans leurs dépôts avant d'en dépendre juridiquement ; aucune n'est liée au démon, toutes sont appelées en processus séparé ou remplacées.
- Le thème reste **MIT** ; le démon en **MIT** aussi (même famille, pas de raison d'en changer). InputPlumber/OGUI ne contaminent pas : on ne fait que parler D-Bus. PySide6 est LGPL-3 : lien dynamique, OK. mpv et ffmpeg : processus séparés, OK.
- **Si le shell fork Pegasus (C++)**, ce fork est GPL-3 et vit dans **son propre dépôt** ; le démon, la base et le thème restent MIT parce que la frontière D-Bus/SQLite/processus est aussi la frontière de licence. Ne pas copier de code C++ de Pegasus dans `ui/`.

### Ce que je ne ferais pas

- Un Flatpak en premier : il ne peut porter aucun des prérequis, et en faire un plus tard reste possible pour la partie « maison ».
- Un dépôt par module : tout bouge ensemble pendant deux ans.

---

## 7. Nom et positionnement

Reprise reste le nom de l'UI (établi, déjà dans `theme.cfg`). Pour le produit entier, trois candidats, collisions relevées le 2026-09-09 (PyPI, crates.io, AUR, GitHub) :

1. **Coulisses** — « ce qui se passe derrière la scène de Reprise » : le démon et ses modules. PyPI libre, crates libre, AUR libre, 10 dépôts GitHub sans étoile. **Mon choix**, nom du démon (`coulisses`) et du produit « Reprise + Coulisses ».
2. **Entracte** — le journal, écrit entre deux sessions. PyPI et crates libres ; **collision AUR** (`entracte`, un pomodoro), 16 dépôts mineurs. Bon nom pour le seul module journal.
3. **Régie** (`regie`) — la cabine de contrôle : sessions, capture, manettes. PyPI/crates/AUR libres, mais 629 dépôts dont `dashersw/regie` (83★, lib JS) : le plus faible.

Rejeté pour le produit : « Reprise » seul (PyPI pris, `symfony/reprise` 82★, 595 dépôts).

---

## Coût (un dev expérimenté) et risques

| Lot | Jours |
|---|---|
| Squelette démon : asyncio, service dbus-fast, schéma + migrations, machine d'état, unité, module HM | 5 |
| Bibliothèque : importateurs Lutris/enregistrements/journal, port des scrapers de pegasus-sync | 4 |
| Orchestration de lancement (processus enfant, cgroup, hooks) — le runner lui-même est ailleurs | 3 |
| Capture : unité transitoire, watchdog, classement (port de `process-game-recording.mjs`) | 3 |
| Journal : frontière fournisseur + `codex` enveloppant le script zx ; Anthropic + Ollama | 2 + 2 |
| InputPlumber : gestion permanente, profils, cibles, `CaptureSource`, écran de remap | 4 |
| Bluetooth : agent, découverte, appairage, écran | 3 |
| Raccourcis sur la cible virtuelle (port de `controller_evdev.py`) | 2 |
| Lecture vidéo : Qt Multimedia + salle mpv + test focus gamescope | 2 |
| Packaging : flake, modules, pyproject, PKGBUILD, docs prérequis | 3 |
| **Total** | **≈ 33 jours** (7 semaines pleines ; 3-4 mois en soirées) |

Risques, par ordre : (1) le focus d'une deuxième fenêtre sous gamescope pour mpv — à tester la première semaine ; (2) la lecture Qt de l'AV1 4K sous GNOME Wayland sans texture matérielle (copies CPU) — mpv est le filet ; (3) `auto_manage: true` rend chaque pad une DualSense Edge pour GNOME aussi, y compris hors jeu — voulu ici, à documenter pour les autres ; (4) la règle polkit étendue est indispensable, sinon chaque `LoadProfilePath` demande un mot de passe ; (5) SQLite un seul écrivain : toute écriture de l'UI passe par D-Bus, sans exception ; (6) le fournisseur Ollama peut décevoir — le garder « expérimental » jusqu'à mesure ; (7) le sibling choisit le shell — si Pegasus reste, le pont JSON coûte 1-2 jours de plus et perd la lecture vidéo intégrée.

---

## Sources

- Local : `/home/yasso/NixOs/bin/lib/game-session.mjs`, `/home/yasso/NixOs/bin/process-game-recording.mjs`, `/home/yasso/NixOs/home/gaming/game-session-summary.nix`, `/home/yasso/NixOs/os/gaming/controller.nix`, `/home/yasso/NixOs/home/gaming/emulators.nix` ; store Nix (`qtmultimedia-6.11.2` → `ffmpeg-9.0.1-lib` → `dav1d-1.5.3`, `libaom-3.12.1`, `libva-2.24.1` ; `pyside6-6.11.0` avec `QtMultimedia`, `QtDBus`) ; `codex exec --help` (0.153.4) ; `busctl --system tree/introspect org.shadowblip.InputPlumber` sur la machine (0.77.1).
- InputPlumber (v0.79.2) : https://github.com/ShadowBlip/InputPlumber/tree/main/bindings/dbus-xml (`org.shadowblip.Input.Manager.xml`, `org.shadowblip.Input.CompositeDevice.xml`, `org.shadowblip.Input.DBusDevice.xml`), https://raw.githubusercontent.com/ShadowBlip/InputPlumber/main/src/constants.rs, https://raw.githubusercontent.com/ShadowBlip/InputPlumber/main/src/input/manager.rs (l. 434-470, 863-869), https://raw.githubusercontent.com/ShadowBlip/InputPlumber/main/src/input/composite_device/mod.rs (l. 1083-1103, 2026), https://raw.githubusercontent.com/ShadowBlip/InputPlumber/main/src/input/target/mod.rs, https://raw.githubusercontent.com/ShadowBlip/InputPlumber/main/rootfs/usr/share/polkit-1/actions/org.shadowblip.InputPlumber.policy, https://raw.githubusercontent.com/ShadowBlip/InputPlumber/main/rootfs/usr/share/inputplumber/schema/composite_device_v1.json, https://raw.githubusercontent.com/ShadowBlip/InputPlumber/main/rootfs/usr/share/inputplumber/schema/device_profile_v1.json, https://raw.githubusercontent.com/ShadowBlip/InputPlumber/main/rootfs/usr/share/inputplumber/profiles/default.yaml, https://shadowblip.github.io/InputPlumber/usage/, https://github.com/ShadowBlip/InputPlumber/tree/main/docs/dbus-interface (périmé).
- OpenGamepadUI : https://raw.githubusercontent.com/ShadowBlip/OpenGamepadUI/main/core/global/launch_manager.gd, https://raw.githubusercontent.com/ShadowBlip/OpenGamepadUI/main/core/systems/input/input_plumber.gd, https://raw.githubusercontent.com/ShadowBlip/OpenGamepadUI/main/extensions/core/src/bluetooth/bluez.rs, https://raw.githubusercontent.com/ShadowBlip/OpenGamepadUI/main/extensions/core/src/bluetooth/bluez/device.rs, https://raw.githubusercontent.com/ShadowBlip/OpenGamepadUI/main/core/ui/card_ui/settings/bluetooth_settings_menu.gd, `docs/class-reference/{BluetoothAdapter,BluetoothDevice,InputPlumberInstance}.md`.
- BlueZ : https://raw.githubusercontent.com/bluez/bluez/master/doc/org.bluez.AgentManager.rst, https://raw.githubusercontent.com/bluez/bluez/master/doc/org.bluez.Agent.rst, https://raw.githubusercontent.com/bluez/bluez/master/doc/org.bluez.Device.rst, https://raw.githubusercontent.com/bluez/bluez/master/doc/org.bluez.Adapter.rst.
- Vidéo : https://raw.githubusercontent.com/mpv-player/mpv/master/DOCS/man/options.rst (`--wid`, `--input-gamepad`, `--input-ipc-server`, `--hwdec`), https://doc.qt.io/qt-6/advanced-ffmpeg-configuration.html, https://doc.qt.io/qt-6/qtmultimedia-index.html, https://raw.githubusercontent.com/jaseg/python-mpv/main/README.rst.
- Fournisseurs : https://platform.claude.com/docs/en/build-with-claude/vision.md (limites, coût par token, formats), https://learn.chatgpt.com/docs/non-interactive-mode.md (`codex exec`), https://developers.openai.com/api/docs/guides/images-vision, https://docs.ollama.com/capabilities/structured-outputs, https://docs.ollama.com/capabilities/vision, https://ollama.com/library/qwen3-vl.
- Distribution : https://raw.githubusercontent.com/flatpak/flatpak-docs/master/docs/sandbox-permissions.rst (`host-etc`, `host-os`, `--device=input`, bus).
- Bibliothèques : https://pypi.org/project/dbus-fast/ (5.0.22), https://pypi.org/project/evdev/ (2.0.0), https://registry.npmjs.org/dbus-next (0.10.2, 2021), https://github.com/dbusjs/node-dbus-next ; licences via l'API GitHub (`oKcerG/SortFilterProxyModel` MIT, `Bluetooth-Devices/dbus-fast` MIT, `gvalkov/python-evdev` BSD-3, `ShadowBlip/*` GPL-3.0).
- Collisions de noms : `https://pypi.org/pypi/<nom>/json`, `https://crates.io/api/v1/crates/<nom>`, `https://aur.archlinux.org/rpc/v5/search/<nom>?by=name`, `gh api search/repositories?q=<nom>+in:name`.
