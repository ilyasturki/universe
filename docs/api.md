# Universe — API D-Bus et protocole des modules

Version figée pour le MVP (api = 1). Tout client (interface, CLI, module daemon, script shell) passe par ici. Les données lourdes voyagent en chaînes JSON (`s`) : `busctl`, QtDBus et la CLI les lisent de la même façon sans démarshaler d'`a{sv}`.

## Bus

- Nom : `io.github.ilyasturki.Universe` (activable : fichier de service D-Bus ou unité `universed.service` de `systemd --user`).
- Objet : `/io/github/ilyasturki/Universe`.
- Un module daemon qui expose quelque chose prend `io.github.ilyasturki.Universe.Module.<id>` ; l'interface le découvre par préfixe de nom (comme MPRIS).
- Erreurs : `io.github.ilyasturki.Universe.Error.<Kind>` avec `Kind ∈ NotFound, Ambiguous, Busy, Invalid, Unavailable, Io`. Le message est lisible.
- Identifiants : `id` = slug (`sanitizeGameName(title)`), nom du dossier `games/<id>/`. `session_id` = `AAAAMMJJ-HHMMSS`. `job_id` = `job-<n>`.

## Library1 — `io.github.ilyasturki.Universe.Library1`

| Membre | Signature | Rôle |
|---|---|---|
| `List()` | `→ s` | JSON `[Game]` ; jeux non cachés d'abord, dernière partie en premier |
| `Get(id)` | `s → s` | JSON `Game` résolu (défauts globaux fusionnés, stats de sessions, médias, modules actifs) |
| `Resolve(query)` | `s → as` | ids candidats : exact › mot entier › sous-chaîne › chemin. Vide = inconnu, >1 = ambigu |
| `Set(id, key, value)` | `sss → ()` | écrit une clé du `game.toml`. `key` pointée : `launch.proton`, `launch.env.FOO`, `desktop.hide_cursor`, `hidden`, `favorite`, `tags`, `sort_title`, `metadata.sgdb_id`, `capture.cursor` (raccourci de `modules.capture.cursor`, validé contre le schéma du module). `value` : chaîne ; booléens `true/false`, listes séparées par des virgules, `""` supprime la clé |
| `Remove(id, purge)` | `sb → ()` | corbeille (`trash`) du préfixe si `purge`, parcage des enregistrements et du journal dans `.archive/`, `game.toml` marqué `removed_at` |
| `Rescan()` | `→ ()` | relit `games/*/game.toml`, reconstruit l'index, déclenche `scan` des sources |
| `ImportLutris(apply)` | `b → s` | JSON rapport : jeux importés, diff d'environnement par jeu (`{id, lutris_env, universe_env, added, removed, changed}`), heures importées. `apply=false` = simulation |
| **signal** `LibraryChanged(ids)` | `as` | ids modifiés (vide = tout) |

`Game` (JSON) = contenu du `game.toml` + `{"stats": {"hours": f, "play_count": n, "last_played": "RFC3339"|null}, "media": {"box_front": path|null, "tile", "background", "logo", "screenshots": [path]}, "modules": {"capture": {enabled, cursor}, "journal": {...}}, "removed": b}`.

## Session1 — `io.github.ilyasturki.Universe.Session1`

| Membre | Signature | Rôle |
|---|---|---|
| `Launch(id, screen)` | `ss → s` | lance ; `screen` = nom du connecteur (`DP-1`) ou `""` (profil). Retourne `session_id`. Erreur `Busy` si une partie tourne |
| `Stop(session_id)` | `s → ()` | `systemctl --user stop` du scope |
| `Screenshot()` | `→ s` | hook `screenshot` du module qui le déclare ; chemin du PNG |
| `Sessions(id)` | `s → s` | JSON `[Session]` du `sessions.jsonl`, dernière en premier |
| **propriété** `Current` | `s` | JSON `{session_id, id, title, unit, screen, started_at}` ou `""`. `PropertiesChanged` émis |
| **signal** `SessionStarted(session_id, id)` | `ss` | après les hooks pre-launch, scope créé |
| **signal** `SessionEnded(session_id, id, duration_s)` | `ssu` | après `sessions.jsonl` écrit et hooks session-end |

`Session` (JSONL, une ligne) : `{"session":"20260910-213045","game":"the-technomancer","started_at":"RFC3339","ended_at":"RFC3339","duration_s":1234,"source":"daemon"|"import-recording"|"import-lutris","unit":"universe-game-<id>-<session>.scope","screen":"DP-1","exit":0,"recording":"path"|null}`.

## Sources1 — `io.github.ilyasturki.Universe.Sources1`

| Membre | Signature | Rôle |
|---|---|---|
| `List()` | `→ s` | JSON `[{id, name, available, logged_in, games_dir}]` |
| `LoginUrl(source)` | `s → s` | URL à ouvrir (l'interface l'affiche avec un QR) |
| `Login(source, code)` | `ss → s` | job_id ; verbe `login <code>` |
| `Library(source)` | `s → s` | JSON `[SourceGame]` (cache hors ligne, rafraîchi par `library`) |
| `Search(source, query)` | `ss → s` | JSON `[SourceGame]` |
| `Info(source, game_id)` | `ss → s` | JSON libre du verbe `info` |
| `Install(source, game_id)` | `ss → s` | job_id |
| `Update(source, game_id)` | `ss → s` | job_id ; `game_id=""` = tout ce qui est en attente |
| `Updates()` | `→ s` | JSON `[{id, title, local_build, remote_build, version, date}]` |
| `Scan(source)` | `s → s` | job_id ; `source=""` = toutes |
| `Jobs()` | `→ s` | JSON `[Job]` |
| **signal** `Progress(job_id, done, total, message)` | `stts` | |
| **signal** `JobFinished(job_id, ok, message)` | `sbs` | |

`SourceGame` = `{"id": "1434554947", "title": "Mini Metro", "owned": true, "installed": true, "dir": path|null, "build": s|null, "remote_build": s|null}`.

## Media1 — `io.github.ilyasturki.Universe.Media1`

| Membre | Signature | Rôle |
|---|---|---|
| `Refresh(id, force)` | `sb → s` | job_id ; SteamGridDB, RAWG, captures Steam, overrides ; `id=""` = tous |
| `SetSlot(id, slot, path)` | `sss → ()` | copie dans `media/<slot>.<ext>` ; `slot ∈ box_front, tile, background, logo, screenshot` |
| `Unset(id, slot)` | `ss → ()` | |
| `Candidates(id, slot)` | `ss → s` | JSON `[{provider, url|path, score}]` (cache `.sync.json`) |
| `Pin(id, provider, provider_id)` | `sss → ()` | `provider ∈ sgdb, rawg, steam` → `metadata.<provider>_id` |
| **signal** `MediaChanged(id)` | `s` | |

## Recording1 — `io.github.ilyasturki.Universe.Recording1`

| Membre | Signature | Rôle |
|---|---|---|
| `File(session_id, path)` | `ss → s` | classe le mkv : `<recordings_root>/<id>/<session>.mkv` (déplacement même FS, sinon copie), écrit `recording` dans la session, retourne le chemin final |
| `List(id)` | `s → s` | JSON `[{session, path, size, duration_s, created_at}]` |
| **signal** `RecordingFiled(session_id, id, path)` | `sss` | déclenche les hooks post-process |

## Journal1 — `io.github.ilyasturki.Universe.Journal1`

| Membre | Signature | Rôle |
|---|---|---|
| `AddEntry(session_id, json)` | `ss → ()` | valide le schéma (§7), écrit `journal/<session>.json`, copie les pièces jointes listées |
| `List(id)` | `s → s` | JSON `[Entry]`, dernière en premier |
| `Render(id)` | `s → s` | rend la note Markdown (`<journal_root>/<id>/<Titre>.md`), retourne le chemin |
| **signal** `EntryWritten(session_id, id)` | `ss` | |

`Entry` = `{"session","game","written_at","lang","title","provider","paragraphs":[s],"next_up":s,"images":[relpath]}`.

## Modules1 — `io.github.ilyasturki.Universe.Modules1`

| Membre | Signature | Rôle |
|---|---|---|
| `List()` | `→ s` | JSON `[{id, name, kind: [..], version, dir, enabled, available, missing: [bin], hooks: {..}, verbs: [..], settings: [Setting], frontend_qml: path|null}]` |
| `Enable(id, enabled)` | `sb → ()` | écrit `[modules] enabled` de `config.toml` |
| `GetSettings(module, game_id)` | `ss → s` | JSON fusionné (global puis jeu) ; `game_id=""` = global seul |
| `SetSetting(module, game_id, key, value)` | `ssss → ()` | validé contre `[[settings]]` ; `game_id=""` écrit `config.toml [modules.<id>]`, sinon `game.toml [modules.<id>]` |
| `Doctor()` | `→ s` | JSON `[{check, ok, detail, module}]` : binaires requis, gsr-kms-server, Proton, extension curseur, jetons |
| **signal** `ModulesChanged()` | | |

`Setting` = `{"key","type": "bool"|"string"|"int"|"enum"|"path","default","label","scope": "global"|"game","choices": [..]}`.

## Settings1 — `io.github.ilyasturki.Universe.Settings1`

| Membre | Signature | Rôle |
|---|---|---|
| `Get()` | `→ s` | JSON de `config.toml` résolu (chemins absolus, défauts appliqués) |
| `Set(key, value)` | `ss → ()` | clé pointée de `config.toml` (`launch.proton`, `paths.recordings_root`, `desktop.profile`) |
| **propriété** `Version` | `s` | version du cœur |

## config.toml (défauts)

```toml
schema = 1
[paths]
games_root = "/mnt/games/PC"         # où les sources installent
prefixes_root = "/mnt/games/prefixes"
recordings_root = "/mnt/recordings/games"
journal_root = "~/Documents/notes/games/journal"
overrides = "~/Dotfiles/home/config/pegasus-art"
[launch]
proton = "proton-ge"                 # nom sous proton/ ou chemin
esync = true
fsync = true
mangohud = true
[desktop]
profile = "auto"                     # auto | gnome | none
hide_cursor = true
cursor_extension = "hide-cursor@elcste.com"
[proton]                             # nom → chemin
proton-ge = "~/.local/share/lutris/runners/wine/proton-ge"
[modules]
enabled = ["gog", "capture", "journal", "tracker-md"]
[modules.capture]
codec = "av1_10bit"
[keys]
sgdb = ""                            # ou fichier keys/sgdb
rawg = ""
```

## Protocole des modules

Un module est un dossier `modules/<id>/` (système : `$out/share/universe/modules/<id>` ; utilisateur : `$XDG_CONFIG_HOME/universe/modules/<id>`, qui écrase le système sur le même id) contenant `module.toml` et ses exécutables. Le cœur n'en charge jamais le code.

```toml
api = 1
id = "capture"
name = "Capture vidéo"
kind = ["hooks"]                  # hooks | daemon | source ; cumulables
version = "0.1.0"

[requires]
core = ">=0.1"
bins = ["gpu-screen-recorder"]    # un binaire absent = module « indisponible », jamais lancé

[hooks]                           # chemins relatifs au dossier du module
pre-launch   = "bin/pre"          # bloquant, avant le scope ; peut écrire UNIVERSE_ENV_FILE
post-launch  = "bin/start"        # asynchrone (unité transitoire), après SessionStarted
session-end  = "bin/stop"         # bloquant court, après la fin du scope, avant SessionEnded
post-process = "bin/process"      # asynchrone, après RecordingFiled (ou après session-end si aucun module capture)
screenshot   = "bin/shot"         # à la demande : Session1.Screenshot()
daemon       = "bin/daemon"       # long-vécu, lancé avec le cœur
timeout_s    = 20                 # pour les hooks bloquants

[limits]                          # unités transitoires des hooks asynchrones
cpu_weight   = 100                # défaut 20
memory_high  = "4G"               # défaut 2G

[source]                          # kind source
exe = "bin/source"                # lancé : bin/source <verbe> [args]

[frontend]
qml = "ui/Page.qml"               # facultatif : écran ajouté à l'interface

[[settings]]
key = "enabled"                   # réservé : toujours présent, scope game
type = "bool"
default = true
label = "Enregistrer la partie"
scope = "game"                    # global (config.toml [modules.<id>]) | game (game.toml [modules.<id>])
```

### Environnement des hooks

| Variable | Valeur | Hooks |
|---|---|---|
| `GAME_ID`, `GAME_SLUG`, `GAME_TITLE`, `GAME_DIR`, `GAME_EXE`, `GAME_TOML` | identité et chemins (TOML en lecture seule) | tous |
| `SESSION_ID`, `SESSION_UNIT`, `SESSION_SCREEN`, `SESSION_STARTED_AT`, `SESSION_ENDED_AT`, `SESSION_DURATION_S` | la partie ; `SESSION_SCREEN` = connecteur DRM | selon le hook |
| `RECORDING_PATH`, `JOURNAL_DIR` | mkv classé (vide si aucun), `games/<id>/journal` | post-process, tous |
| `MODULE_SETTINGS_JSON` | réglages globaux fusionnés avec ceux du jeu | tous |
| `UNIVERSE_ENV_FILE` | lignes `CLÉ=VALEUR` ajoutées à l'env du jeu, avant `launch.env` | pre-launch |
| `MODULE_DIR`, `MODULE_DATA_DIR`, `UNIVERSE_BUS`, `UNIVERSE_OBJECT` | dossier du module, `$XDG_DATA_HOME/universe/modules/<id>`, nom du bus, chemin d'objet | tous |
| `UNIVERSE_GAME_JSON` | `Library1.Get(id)` sérialisé | tous |

Codes de retour : 0 ok ; autre = erreur journalisée, la partie continue (pre-launch ≠ 0 annule le lancement).

### Protocole des sources

`bin/source <verbe> [args]`, env `MODULE_SETTINGS_JSON` + `MODULE_DATA_DIR` ; une ligne JSON par événement sur stdout, erreurs sur stderr, code de retour.

| Verbe | Argument | Événements |
|---|---|---|
| `login` | `[code]` | sans code : `{"event":"login_url","url":…}` ; avec : `{"event":"logged_in","user":…}` |
| `library` | | `{"event":"game", …}` par titre possédé |
| `search` | `<texte>` | `{"event":"game", …}` |
| `info` | `<id>` | `{"event":"info","data":{…}}` |
| `install` | `<id>` | `progress` puis `game` (installé) |
| `update` | `[id]` | sans id : `{"event":"update","id","title","local_build","remote_build","version","date"}` par mise à jour en attente ; avec id : `progress` puis `game` |
| `scan` | | `{"event":"game", …}` par installation trouvée (`owned` croisé avec la bibliothèque en cache) |

`{"event":"game","id":"1434554947","title":"Mini Metro","dir":"/mnt/games/PC/Mini Metro","exe":"MiniMetro.exe","build":"5904…","owned":true,"installed":true,"release_year":2015,"dlcs":[]}`, `{"event":"progress","done":123,"total":456,"message":"…"}`, `{"event":"done"}`.

## Contrat de l'interface (hôte PySide6)

L'hôte expose à QML un objet `api` : `api.keys.is{Accept,Cancel,Filters,Details,PageUp,PageDown,PrevPage,NextPage,Menu}(event)`, `api.allGames` (modèle, rôles `title, sortTitle, favorite, playTime, playCount, lastPlayed, releaseYear, developerList, publisherList, genreList, players, description, summary, assets{boxFront,tile,background,logo,screenshotList}, id, hidden, tags, source`), `api.collections`, `api.memory.{get,set,has}`, `game.launch()` → `Session1.Launch`, et `api.universe` (client D-Bus : `sessions(id)`, `recordings(id)`, `journal(id)`, `modules()`, `settings(id)`, `set(id,key,value)`, `sources`, `install`, `update`, `login`, `doctor`). Signaux `SessionStarted/Ended`, `LibraryChanged`, `RecordingFiled`, `EntryWritten`, `Progress`, `JobFinished` relayés comme signaux Qt.
