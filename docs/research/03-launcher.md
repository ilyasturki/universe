# 03 — La couche lanceur : remplacer Lutris, par quoi, à quel prix

## 1. Verdict

Remplacer Lutris par un noyau de lancement maison, mince (Python, ~800 lignes), posé sur `umu-run` pour les jeux Proton, sur `wine` système pour l'exception AC Odyssey, et sur des commandes directes pour les émulateurs — **en phase 1**, avant tout le reste, parce que sessions, enregistrement et journal s'accrochent à ce que le lanceur produit (un scope systemd par partie).

Lutris n'est pas « inutile » : il porte aujourd'hui une dizaine de faits par jeu et le contrat des hooks. Mais depuis 0.5.17 il n'est plus qu'un traducteur YAML → variables d'environnement devant `umu-run`, plus un chronomètre et une base pga.db. On garde les faits, on jette le programme. Bascule jeu par jeu grâce à un pont `lutris lutris:rungame/<slug>` qui reste valable deux semaines, puis suppression.

## 2. Les faits

### 2.1 Ce que Lutris fait réellement au lancement (source installée, 0.5.22)

Chemin des sources : `/nix/store/flcswl51rib4p79a439bk8vx2a6hqxw0-lutris-0.5.22-fhsenv-rootfs/usr/lib64/python3.14/site-packages/lutris/` (noté `$L` ci-dessous).

- **Tout runner Proton passe par umu.** `$L/runners/wine.py:847-856` : `if proton.is_proton_path(command[0]) and not proton.is_umu_path(command[0]): command[0] = proton.get_umu_path()`. Les trois runners « wine » du profil (`~/.config/lutris/runners/wine/{proton-ge,proton-em,proton-cachyos}`) sont des liens vers le store Nix et contiennent un `proton` : Lutris les reconnaît comme Proton et lance `umu-run`. La pile actuelle est donc **Pegasus → lutris → umu-run → pressure-vessel (steamrt4) → proton → wine**.
- **L'environnement que Lutris fabrique** (`$L/util/wine/proton.py:157-190`, `$L/runners/wine.py:1174-1270`) : `PROTONPATH=<dossier proton>`, `GAMEID=umu-default` (sauf correspondance store+appid dans `~/.local/share/lutris/runtime/umu-games/umu-games.json`, 314 entrées — aucun des trois ids GOG de la bibliothèque n'y figure), `PROTON_VERB=waitforexitandrun` (ou `runinprefix` si un autre jeu tourne déjà dans le même préfixe), `WINEARCH=win64`, `WINEPREFIX`, `WINEESYNC/WINEFSYNC` et leurs inverses `PROTON_NO_ESYNC/PROTON_NO_FSYNC` (`wine.py:1218-1230`), `WINEDLLOVERRIDES` construit depuis `overrides:` (`wine.py:1269`), `HOST_LC_ALL` copié de `LC_ALL`, puis le bloc `env:` du jeu par-dessus (`$L/runners/runner.py:293`).
- **`disable_runtime: true` ne désactive que le runtime Lutris** (le bundle LD_LIBRARY_PATH Ubuntu-18.04) : `$L/runners/runner.py:284-289`. Le conteneur Steam Linux Runtime, lui, est décidé par umu, pas par Lutris.
- **mangohud** = préfixe `mangohud` + `MANGOHUD=1 MANGOHUD_DLSYM=1` (`$L/runner_interpreter.py:18-19`). **prefix_command** (RE4 : `WINEDLLOVERRIDES="amd_ags_x64.dll=n,b"`) est découpé par shlex et préfixé à la commande (`runner_interpreter.py:40-42`).
- **fps_limit est mort.** `$L/sysoptions.py` ne connaît plus que `gamescope_fps_limiter` ; `strangle` n'est pas installé. Le cap 60 fps de Bannerlord (`mount-blade-bannerlord-*.yml`) n'est pas appliqué aujourd'hui.
- **Hooks** : `prelaunch_command` lancé en arrière-plan sauf `prelaunch_wait` (`$L/game.py:566-582`), `postexit_command` à l'arrêt (`game.py:975-982`), avec `GAME_NAME` et `GAME_DIRECTORY` dans l'env (`game.py:697-699`). Le bloc `system:` d'un jeu remplace le global clé par clé (seul `env` fusionne), ce que `~/Dotfiles/home/scripts/ac-odyssey-prelaunch` contourne déjà en chaînant `start-game-recording` à la main.
- **Playtime** = un `Timer` démarré dans `start_game` (`game.py:804`) et ajouté en heures à `games.playtime` à l'arrêt (`game.py:888-902`), `lastplayed = int(time.time())` (`game.py:990`). L'arrêt est détecté par la fin du processus `umu-run` : « Tracking PIDs of Proton games is not possible at the moment » (`wine.py:1300-1303`). Un exécutable-lanceur qui se termine pendant que le jeu continue clôt donc la session Lutris et déclenche `postexit_command` trop tôt.
- **`lutris lutris:rungame/<slug>` en CLI** met `quit_on_game_exit = True` et convertit SIGTERM/SIGINT en `game.stop()` (`$L/gui/application.py:684-694, 766-773`) : Lutris se termine quand le jeu s'arrête. C'est ce qui rend le pont de transition sûr.
- **Runners émulateurs** : `yuzu.py` → `-f -g <rom>` (`fullscreen` par défaut `True`), `dolphin.py` → `[-b] [-u dir] -e <iso>`, `rpcs3.py` → `[--no-gui] <EBOOT.BIN>`, `melonds` et `mgba` sont des runners JSON (`/usr/share/lutris/json/melonds.json` : `--fullscreen` par défaut), `runner_executable` est remplacé par les wrappers `<emu>-pad` de `~/NixOs/home/gaming/emulators.nix`. Les AppImages de `~/.config/lutris/runners/{melonds,mgba}` sont du poids mort.
- **Base pga.db** (`~/.local/share/lutris/pga.db`) : 53 jeux (30 wine, 14 yuzu, 6 dolphin, 2 rpcs3, 1 melonds), `playtime REAL` en heures, `lastplayed INTEGER` epoch, `hidden` en colonne + catégorie `.hidden` (6), favoris = catégorie `favorite` (4), autres catégories (Poadcast 20, Nintendo 17, Multiplayer 14, Finished 11, Platformer 11…). **Deux autres consommateurs** que Lutris : `~/Dotfiles/home/scripts/game` (`DB="$LUTRIS_DIR/pga.db"` en tête) et `~/Dotfiles/home/scripts/pegasus-sync.py` (lecture l.277-300, **écriture** l.2160 `UPDATE games SET {column} = 1`, ligne de lancement l.502 `launch: lutris lutris:rungame/{slug}`).
- **`lutris -d`** journalise chaque variable d'environnement du processus lancé, une par ligne, `KEY="VALUE"` derrière le préfixe du logger (`$L/monitored_command.py:160`, formats dans `$L/util/log.py:18-20`). C'est l'outil de migration : on capture l'env réel de chaque jeu et on le diffe contre celui du nouveau lanceur. (Que la ligne de commande elle-même soit journalisée : non vérifié.)

### 2.2 Inventaire exact des réglages Lutris en service (55 fichiers `~/.config/lutris/games/*.yml`)

| Réglage | Occurrences | Remplacement |
|---|---|---|
| `game.exe`, `game.prefix`, `game.arch` (tous win64) | 35 / 39 / 8 | colonnes `exe`, `prefix` ; `WINEARCH=win64` fixe (umu refuse win32, `wine.py:853`) |
| `game.main_file` (émulateurs) | 23 | colonne `rom` + gabarit par émulateur |
| `game.args` (`-nolauncher`, Spider-Man) | 1 | `args` JSON |
| `game.working_dir` (GTA V, 2× KH) | 3 | `working_dir` |
| `system.env` : `PROTON_SONY_HIDRAW_XINPUT=1` (8), `LC_ALL=''` (5 installeurs), `PROTON_SONY_DUALSENSE_AS_DUALSHOCK4=1` (Dead Cells, de Blob), `WINE_CPU_TOPOLOGY=4:0,1,2,3` (Technomancer), `RADV_DEBUG=nodcc` (Skyward Sword) | 22 | `env` JSON, appliqué en dernier |
| `wine.version: proton-em` (GTA V, Mini Metro), `system` (AC Odyssey) | 3 | `proton` = nom dans le dossier de runners, ou `backend=wine` |
| `wine.overrides` (GTA V : `socialclub=n,b`, `version=n,b`) | 1 | `dll_overrides` → `WINEDLLOVERRIDES=socialclub=n,b;version=n,b` |
| `wine.esync/fsync: false`, `dxvk_version: manual` (AC Odyssey) | 1 | `WINEESYNC=0 WINEFSYNC=0` ; le préfixe contient déjà les DLL DXVK, ne pas y toucher |
| `system.prefix_command` (RE4) | 1 | variable dans `env` |
| `system.prelaunch_command` (AC Odyssey ; Bannerlord = vide) | 2 | `prelaunch` (le hook global reste chaîné par le lanceur, pas par le script) |
| `system.fps_limit: '60'` (Bannerlord) | 1 | rien : déjà sans effet |
| `rpcs3.nogui: false` (Simpsons) | 1 | flag par jeu |
| `dolphin.platform: '1'` (Mario Kart Wii) | 1 | rien : informatif |
| `script:` (5 installeurs one-shot : Cyberpunk, Hollow Knight, RE4, Dishonored, Technomancer) | 5 | archivés dans l'import ; un repack se réinstalle avec `umu-run Setup.exe` (§3.6) |
| global : `mangohud: true`, `PROTON_ENABLE_WAYLAND=1`, `disable_runtime`, `game_path`, hooks record | 1 | défauts du lanceur |

### 2.3 umu-launcher 1.4.0 (installé) — sémantique vérifiée dans la source

Source : `/nix/store/vacwhvxpl3vga75by7b97q61zar1yz9n-python3.13-umu-launcher-unwrapped-1.4.0/lib/python3.13/site-packages/umu/` (noté `$U`). Pages de manuel `umu(1)` et `umu(5)` au même endroit (`share/man`). `umu-run` sur PATH est un wrapper FHS bubblewrap (`/nix/store/…-umu-launcher-1.4.0-bwrap`) ; nixpkgs-unstable a 1.4.4 (dérivation présente dans le store). Amont : 1.4.0 le 25 mars 2026 (« non-Proton compatibility tools », `UMU_NO_PROTON`, purge de `LD_PRELOAD` sous gamescope), 1.4.1 le 6 juillet (retrait arm64 sniper, `UMU_CONTAINER_NSENTER`), 1.4.4 le 25 juillet 2026 — https://github.com/Open-Wine-Components/umu-launcher/releases.

- **Variables** (`umu(1)`, `$U/umu_run.py:120-330`) : `GAMEID` (défaut `umu-default`, aucun fix), `PROTONPATH` (chemin absolu, nom sous `~/.local/share/Steam/compatibilitytools.d`, ou jeton `GE-Proton`/`GE-Latest`/`UMU-Latest` → téléchargement GitHub ; `$U/umu_proton.py:54-57`), `WINEPREFIX` (défaut `~/Games/umu/$GAMEID`), `STORE`, `PROTON_VERB` (défaut `waitforexitandrun`, seuls les verbes de `$U/umu_consts.py:52` acceptés), `UMU_LOG`, `UMU_RUNTIME_UPDATE=0` (`$U/umu_runtime.py:290`), `UMU_NO_PROTON`, `PROTONFIXES_DISABLE=1`. umu dérive `STEAM_COMPAT_DATA_PATH=$WINEPREFIX`, `STEAM_COMPAT_INSTALL_PATH=dirname(exe)`, `STEAM_COMPAT_TOOL_PATHS`, `STEAM_COMPAT_APP_ID=md5(prefix)`, `SteamAppId`, `UMU_ID` (`$U/umu_run.py:214-330`).
- **Le conteneur n'est pas contournable par variable.** `UMU_NO_RUNTIME` est recopié dans l'env (`umu_run.py:320`) et jamais consulté. Le runtime est choisi uniquement par `require_tool_appid` du `toolmanifest.vdf` de l'outil (`$U/umu_run.py:745-802`, `$U/umu_runtime.py:493-503, 522-660`) ; sans cette clé, runtime « host ». GE-Proton11-5 déclare `"require_tool_appid" "4183110"` = steamrt4 (`/nix/store/wwmg06lh2x6vs5vmplz9d6x5flx2q2fq-proton-ge-steamcompattool/toolmanifest.vdf`), et `~/.local/share/umu/steamrt4/` date du 29 août : c'est ce qui tourne aujourd'hui.
- **Proton en direct sans umu** : le script `proton` exige `STEAM_COMPAT_DATA_PATH` (`…/proton:2718-2720`), accepte `run | waitforexitandrun | runinprefix | destroyprefix | getcompatpath | getnativepath` (`proton:2761-2783`) et lance l'exe via `c:\windows\system32\umu.exe` quand `UMU_ID` est posé (`proton:2671-2681` ; le stub existe dans `files/lib/wine/x86_64-windows/umu.exe`). Sur NixOS ses binaires wine sont liés FHS : il faut un `steam-run`/bwrap. Et GloriousEggroll : « Running non-Steam games with GE-Proton outside of Steam is only supported using umu » (https://github.com/GloriousEggroll/proton-ge-custom). Verdict : on ne contourne pas.
- **Builds acceptés** : n'importe quel dossier avec `toolmanifest.vdf` (+ `compatibilitytool.vdf` facultatif). Les trois arbres Nix en sont : `GE-Proton11-5` (proton-ge-bin.steamcompattool), `EM-10.0-33+` (proton-em), `cachyos-11.0-20260703-slr` (overlay chaotic, `~/NixOs/overlays/default.nix`). `PROTONPATH` absolu vers le store fonctionne (résolution `toolmanifest.vdf`, `umu_run.py:788-800`).
- **Processus et sortie** (`$U/umu_run.py:700-745`) : umu se déclare `PR_SET_CHILD_SUBREAPER`, `Popen(command, start_new_session=True)` puis `proc.wait()` ; SIGTERM/SIGINT sont propagés à tout l'arbre. Le code de retour est celui de pressure-vessel → proton → `umu.exe`. Que `umu.exe` attende la descendance du jeu (comme le stub `steam.exe`) : non vérifié dans sa source ; le scope systemd (§3.2) rend la question sans objet.
- **Latence** : à chaque lancement umu sonde `1.1.1.1:53` (timeout 5 s, `umu_run.py:891-900`) puis **bloque** sur la mise à jour du runtime (`future.result()`, `umu_run.py:955-960`). `UMU_RUNTIME_UPDATE=0` supprime le second point, pas le premier hors-ligne.
- **`--config game.toml`** (`umu(5)`, `$U/umu_plugins.py`) : clés `[umu] prefix, proton, exe` obligatoires, `game_id, store, launch_args` facultatives — et rien d'autre : ni `PROTON_VERB`, ni env arbitraire, et la docstring prévient « some features are lost in this usage, such as running winetricks verbs and automatic updates to Proton ». Inutile pour un lanceur.
- **Gamescope** : `check_env` vide `LD_PRELOAD` sous session gamescope (FIXME, `umu_run.py:123-129`). Le préfixe `mangohud` de Lutris (qui pose LD_PRELOAD) ne survit donc pas à la session Pegasus/gamescope.

### 2.4 protonfixes et umu-database

- `umu-protonfixes` « is the heart of UMU-Proton and GE-Proton » (https://github.com/Open-Wine-Components/umu-protonfixes), embarqué dans les trois arbres (`protonfixes/` : 311 fixes steam, 78 gog, 36 umu, 8 zoomplatform). Base locale `protonfixes/umu-database.csv` (1 200 lignes) ; API https://umu.openwinecomponents.org (`?store=…&codename=…`) — https://github.com/Open-Wine-Components/umu-database.
- **Résolution** (`protonfixes/fix.py:96-117`) : id numérique → `gamefixes-steam/<id>.py` ; `umu-<id>` avec `STORE` reconnu → `gamefixes-<store>/umu-<id>.py` ; sinon → `gamefixes-umu/umu-<id>.py`. **Aucun repli vers le fix Steam.** Avec `GAMEID=umu-default`, seul `default.py` s'exécute : la bibliothèque n'a aujourd'hui aucun fix.
- Candidats réels : GTA V `umu-271590` (`gamefixes-umu/umu-271590.py` : `winebus DisableHidraw=1` — à tester, ça contrarie l'usage hidraw DualSense), Cyberpunk GOG `umu-1091500`, Bannerlord GOG `umu-261550`, KH 1.5+2.5 (EGS) `umu-2552430`, HZD Complete `umu-1151640` (pas Remastered). Les fixes GOG correspondants n'existent pas dans l'arbre ; KH n'a qu'un fix Steam (`SteamDeck=1`), inatteignable via `umu-2552430`. Bilan : protonfixes est un bonus opt-in par jeu, pas une raison de garder Lutris.

### 2.5 Alternatives à un noyau maison

- **Lutris headless** (`lutris lutris:rungame/<slug>`, `lutris --list-games -j` : 53 entrées, champs `id, slug, name, runner, platform, year, playtimeSeconds, lastplayed, coverPath`) : fonctionne, mais garde pga.db comme vérité du playtime et une détection de fin fondée sur le PID d'umu-run. Bon pont, mauvaise fondation.
- **faugus-launcher** 2.2.2 (Python/GTK4, umu en backend, préfixes sous `~/Faugus`, navigation manette, **pas de CLI**) — https://github.com/Faugus/faugus-launcher ; présent dans nixpkgs (1.22.6 sur le pin du registre, `nix eval nixpkgs#faugus-launcher.version`).
- **Nero-umu** (C++/Qt, umu en backend, CLI « a prefix to run in alongside an executable », sans contrôle d'env ni de verbe) — https://github.com/SeongGino/Nero-umu.
- **Bottles** : `bottles-cli run -b BOTTLE -e EXE -a ARGS` (https://docs.usebottles.com/advanced/cli), mais pas d'umu, et Proton-GE « only in special cases » (https://docs.usebottles.com/components/runners).
- **Heroic** : pas de lancement CLI silencieux (issue #1368, https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/issues/1368) ; ses `GamesConfig/*.json` locaux pointent encore sur `wine-ge-8-26` (mort).
- **OpenGamepadUI-lutris** : shelle `lutris --list-games -j` — un plugin OGUI re-emballerait Lutris.

Aucune n'expose le contrat dont on a besoin : env par jeu, verbe Proton, hooks, scope systemd, sessions dans notre base. Le noyau maison est la seule option qui ne nous remet pas un GUI et un stockage tiers sur les bras.

### 2.6 Émulateurs : flags des versions installées (vérifiés en local, sept. 2026)

| Émulateur | Version | Commande | Source |
|---|---|---|---|
| Dolphin | `2606a` | `dolphin-emu-pad -e "<iso>"` — aujourd'hui `batch: false` dans `dolphin.yml`, donc UI visible, on garde ; `-b` (sans UI, exige `-e`) en option par jeu ; `-u DIR` profil ; `-C Sys.Sec.Key=Val` | `dolphin-emu --help` |
| RPCS3 | `0.0.42-unstable-2026-08-15` | `rpcs3-pad --no-gui [--fullscreen] "<…/USRDIR/EBOOT.BIN>"` (`--fullscreen` n'agit qu'avec `--no-gui` ; `--game-screen N`) | `rpcs3 --help` |
| Eden | `0.2.1-unstable-2026-09-07` (module NixOS) | `eden-pad -f -g "<xci>"` ; second profil `eden -u 1` | `-f -g` prouvés par l'invocation Lutris `yuzu.py`, `-u` par `emulators.nix` ; amont 403, `--help` n'imprime rien |
| Ryujinx canary | `1.3.351` | `ryujinx-pad --fullscreen --profile "Yasso" "<xci>"` ; aussi `--application-id`, `--docked-mode`/`--handheld-mode`, `--hide-cursor`, `--root-data-dir`, `--system-language` | chaînes du binaire (`strings -el`), `--help` lance l'émulateur |
| melonDS | `1.1` | `melonds-pad -f "<nds>"` (`-b auto|always|never`) | `melonDS --help` |
| mGBA | Qt 0.10.x | `mgba-pad -f "<gba>"` (`-C OPTION=VALUE`) | `mgba-qt --help` |
| Cemu | 2.x | `cemu-pad -f -g "<rpx>"` (`-a <persistent id>` compte) | `cemu --help` |
| PCSX2 | `2.6.3` | `pcsx2-pad -nogui -fullscreen -- "<iso>"` (**tiret simple** ; `-nogui` implique `-batch` ; `-bigpicture` sinon) | `pcsx2-qt -help` |
| shadPS4 | 2026 | `shadps4-pad -f true -g "<eboot|id>"` (`-b` Big Picture, `-p` patch) | `shadps4 --help` |

## 3. Le design recommandé

### 3.1 Un binaire : `reprise-launch`

Python 3, sans dépendance hors stdlib (sqlite3, json, subprocess, dbus par `gdbus`/`busctl` en sous-processus ou `sdbus` si dispo). Installé par home-manager, MIT, aucun chemin Nix codé en dur : les emplacements viennent d'un fichier `~/.config/reprise/launcher.toml` (dossier runners Proton, dossier préfixes, hooks).

```
reprise-launch <slug>            # bloque jusqu'à la fin du scope, code de retour du jeu
reprise-launch --dry-run <slug>  # imprime argv + env triés (format KEY="VALUE"), sans lancer
reprise-launch --stop [<slug>]   # systemctl --user stop game-<slug>-*.scope
reprise-launch --list [--json]   # remplace lutris --list-games -j
reprise-launch --setup <slug>    # crée/répare le préfixe (umu createprefix + isolate_home)
```

Résolution : `games ⨝ launch` → backend → argv + env → hooks prelaunch → scope → attente → hooks postexit → session.

### 3.2 Le scope systemd, cœur de la sémantique

```
systemd-run --user --scope --collect \
  --unit="game-${slug}-$(date +%s).scope" \
  -p KillMode=control-group -p TimeoutStopSec=20 \
  -- <argv>
```

- `--scope` exécute la commande **dans** le processus `systemd-run` avec l'env hérité : pas de `--setenv`, l'env est celui du lanceur.
- `--wait` est refusé avec `--scope` (vérifié : « --wait may not be combined with --scope »). Le lanceur attend donc la disparition du scope : `busctl --user monitor org.freedesktop.systemd1` sur `UnitRemoved`, ou sondage `systemctl --user is-active <unit>` toutes les 2 s.
- Vérifié : un scope dont le processus principal a quitté reste `active` tant qu'un descendant vit (`sh -c 'sleep 25 & exit 0'` → `ActiveState=active`), et `systemctl --user stop` le rend `inactive` en tuant l'arbre. C'est exactement le comportement que Lutris n'a pas : PlayGTAV.exe → GTA5.exe, wineserver, pressure-vessel, tout est dans le cgroup. Les propriétés `-p KillMode=control-group -p TimeoutStopSec=20` sont acceptées sur un scope (vérifié : `KillMode=control-group TimeoutStopUSec=20s`).
- **Playtime = durée de vie du scope.** **Arrêt forcé = `systemctl --user stop`.** **Interface pour les autres modules** : `systemctl --user is-active 'game-*.scope'` remplace le `pgrep` sur des radicaux de noms de processus dans `game-recording.service` (ExecStartPre et chien de garde), et `ControlGroup=` donne la liste exacte des PID à qui veut la fenêtre.

### 3.3 Backends

**`proton`** (30 jeux moins AC Odyssey) :

```
# env hérité du lanceur, plus :
  WINEPREFIX=<prefix> PROTONPATH=$(readlink -f ~/.local/share/reprise/proton/<proton>)
  GAMEID=<game_id_umu|umu-default> [STORE=<store>]
  PROTON_VERB=waitforexitandrun          # runinprefix si une session ouverte a le même prefix
  WINEARCH=win64 PROTON_ENABLE_WAYLAND=1 UMU_RUNTIME_UPDATE=0
  WINEESYNC=1 WINEFSYNC=1 | PROTON_NO_ESYNC=1 PROTON_NO_FSYNC=1 selon launch.esync/fsync
  WINEDEBUG=-all DXVK_LOG_LEVEL=error PROTON_DXVK_D3D8=1   # posés par Lutris à chaque lancement wine (wine.py:1180-1183, 1250-1253)
  [WINEDLLOVERRIDES=<dll_overrides>] MANGOHUD=1 MANGOHUD_DLSYM=1
  + launch.env (dernier mot)
umu-run "<exe>" <args…>          # cwd = working_dir ou dirname(exe)
```

Deux entrées Kingdom Hearts partagent `/mnt/games/prefixes/kingdom-hearts-final-mix` : le préfixe est une colonne, pas une fonction du slug, et la règle `runinprefix` de Lutris est reprise (une ligne `sessions` ouverte sur le même `prefix`).

**`wine`** (AC Odyssey) : `wine` du PATH (`wine-11.8 (Staging)`, `~/NixOs/home/gaming/wine.nix`), `WINEPREFIX`, `WINEESYNC=0 WINEFSYNC=0`, env du jeu, `prelaunch = ac-odyssey-prelaunch` réduit à sa moitié « effacer `1.save` » puisque le hook global n'est plus remplacé. Ce que Lutris ajoute exactement à `WINEDLLOVERRIDES` avec `dxvk_version: manual` n'est **pas vérifié** : capture `lutris -d` avant bascule (§3.6).

**`emulator`** : gabarits de §2.6, `rom` en colonne, flags par jeu (`rpcs3.nogui`, profil Eden/Ryujinx) dans `args`. `native` pour un jeu Linux (aucun aujourd'hui).

**mangohud sous gamescope** : umu efface `LD_PRELOAD` ; on passe par l'env (`MANGOHUD=1`, couche Vulkan implicite importée dans le conteneur) et, dans la session Pegasus, par `gamescope --mangoapp`. À confirmer par un lancement ; l'import de la couche hôte par pressure-vessel n'a pas été vérifié ici.

**`PROTON_ENABLE_WAYLAND=1` sous la session gamescope** : `~/NixOs/os/gaming/pegasus.nix` rappelle que gamescope n'expose pas de socket Wayland sans `--expose-wayland` ; dans cette session le jeu est un client XWayland. Soit vérifier que Proton retombe sur X11 quand `WAYLAND_DISPLAY` est absent, soit rendre le drapeau conditionnel à la session (`launcher.toml`, table `[session.gamescope]`). Sous GNOME il reste un défaut.

### 3.4 La base : `~/.local/share/reprise/library.db`

```sql
CREATE TABLE games(
  id INTEGER PRIMARY KEY, slug TEXT UNIQUE NOT NULL, title TEXT NOT NULL, sort_title TEXT,
  platform TEXT NOT NULL, year INTEGER, hidden INTEGER NOT NULL DEFAULT 0,
  favorite INTEGER NOT NULL DEFAULT 0, added_at INTEGER NOT NULL,
  legacy_playtime_s INTEGER NOT NULL DEFAULT 0, legacy_last_played INTEGER);
CREATE TABLE launch(
  game_id INTEGER PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
  backend TEXT NOT NULL CHECK (backend IN ('proton','wine','emulator','native','lutris')),
  exe TEXT, rom TEXT, args TEXT NOT NULL DEFAULT '[]', working_dir TEXT,
  prefix TEXT, proton TEXT, emulator TEXT,
  env TEXT NOT NULL DEFAULT '{}', dll_overrides TEXT,
  esync INTEGER NOT NULL DEFAULT 1, fsync INTEGER NOT NULL DEFAULT 1,
  game_id_umu TEXT, store TEXT, prelaunch TEXT, postexit TEXT,
  mangohud INTEGER NOT NULL DEFAULT 1, record INTEGER NOT NULL DEFAULT 1);
CREATE TABLE sessions(
  id INTEGER PRIMARY KEY, game_id INTEGER NOT NULL REFERENCES games(id),
  started_at INTEGER NOT NULL, ended_at INTEGER, duration_s INTEGER, exit_code INTEGER,
  scope_unit TEXT NOT NULL, recording_id TEXT, journal_entry TEXT);
CREATE TABLE tags(game_id INTEGER REFERENCES games(id) ON DELETE CASCADE,
  tag TEXT NOT NULL, PRIMARY KEY (game_id, tag));
CREATE TABLE installers(game_id INTEGER, kind TEXT, payload TEXT); -- les 5 blocs script:, archivés
```

`backend='lutris'` est le pont : `reprise-launch` exécute `lutris lutris:rungame/<slug>` dans le scope, tant que le jeu n'a pas été validé. Dans ce mode le lanceur n'exécute **aucun** hook global : le `system.yml` de Lutris lance déjà `start-game-recording`, et le chaîner deux fois frapperait deux chemins d'enregistrement par partie. Playtime affiché = `legacy_playtime_s + SUM(sessions.duration_s)`. `pegasus-sync.py` lit cette base à la place de pga.db (l.277-300), écrit ses `plays` depuis `sessions` (l.578-662, plus besoin de répartir des heures Lutris sur des comptes d'enregistrements), et bascule sa ligne l.502 en `launch: reprise-launch {slug}`. Le script `game` change son `DB=`. Deux jours de travail à eux deux, comptés en §4.

### 3.5 Contrat des hooks (ce que les autres modules peuvent lire)

Exporté avant `prelaunch` et pendant toute la partie : `GAME_SLUG`, `GAME_NAME`, `GAME_PLATFORM`, `GAME_SESSION_ID`, `GAME_SCOPE` (nom d'unité), `GAME_ROOT` (dossier du jeu), `WINEPREFIX` (backends wine/proton uniquement), `GAME_RECORDING_SKIP` (passthrough). `start-game-recording` / `stop-game-recording` restent les hooks globaux par défaut, chaînés par le lanceur avant le hook du jeu — la faiblesse « le bloc système du jeu écrase le global » disparaît.

Le correctif `winetricks isolate_home` quitte le hook d'enregistrement (`~/NixOs/home/gaming/game-recording.nix:190-195`) : il s'exécutait via le `WINE` de Lutris dans le bwrap de Lutris. Il devient une étape de `reprise-launch --setup` :
`WINEPREFIX=<prefix> PROTONPATH=<proton> umu-run winetricks isolate_home && touch "$WINEPREFIX/.sandboxed"` (`umu(1)` exemple 11 ; exige GE-Proton ou UMU-Proton, `$U/umu_run.py:240-247`).

### 3.6 Migration, dans l'ordre

1. **Runners Proton sans Lutris** : dans `~/NixOs/home/gaming/lutris.nix`, remplacer `programs.lutris.winePackages` par `home.file.".local/share/reprise/proton/proton-ge".source = …` (idem `proton-em`, `proton-cachyos`). Noms sans version, comme l'overlay le veut déjà (« bumps keep the runner directory … valid »).
2. **Import one-shot** `import-lutris.py` : `sqlite3 'file:~/.local/share/lutris/pga.db?mode=ro'` (games, categories, games_categories) + `~/.config/lutris/games/<configpath>.yml` + `system.yml` → `library.db` selon la table §2.2 ; `playtime*3600 → legacy_playtime_s`, `lastplayed → legacy_last_played`, catégorie `favorite` → `favorite=1`, `.hidden`/`hidden` → `hidden=1`, le reste → `tags`, blocs `script:` → `installers`. Tous les jeux naissent avec `backend='lutris'`.
3. **Validation jeu par jeu** — chaque capture est un vrai lancement, donc sans enregistrement ni journal, et borné dans le temps : `GAME_RECORDING_SKIP=1 timeout 20 lutris -d lutris:rungame/<slug> 2>&1 | grep -oE '[A-Za-z_][A-Za-z0-9_]*=".*"$' | sort > /tmp/env-<slug>`. Le logger préfixe chaque ligne (`$L/util/log.py:19-20`), d'où le `grep -o` ancré en fin de ligne ; SIGTERM devient `game.stop()` (`application.py:766-773`) et les 20 s parasites tombent dans pga.db après l'import, donc à la poubelle. Puis `reprise-launch --dry-run <slug> | sort | diff - /tmp/env-<slug>`. Écarts acceptés : `UMU_LOG`, `WINE`, `WINE_MONO_CACHE_DIR`, `WINE_GECKO_CACHE_DIR`, `__GL_SHADER_DISK_CACHE*`, `GAME_DIRECTORY`. Écart inattendu = on corrige avant de passer `backend` à `proton`/`wine`/`emulator`. Ordre : les 23 émulateurs (un jour), puis les 29 Proton, AC Odyssey et GTA V en dernier.
4. **Pegasus** : `pegasus-sync.py` (§3.4), régénérer `metadata.pegasus.txt` ; le thème et les hooks `game-start`/`game-end` de Pegasus ne changent pas.
5. **Enregistrement** : `game-recording.service` passe de `pgrep` sur radicaux à `systemctl --user is-active 'game-*.scope'` (à coordonner avec la section enregistrement).
6. **Installeurs** : un repack ou un Setup GOG s'installe désormais par `WINEPREFIX=/mnt/games/prefixes/<slug> PROTONPATH=<proton-ge> GAMEID=umu-default LC_ALL='' umu-run "/mnt/games/downloads/…/Setup.exe"` ; sous-commande `game install-repack` plus tard.
7. **Retrait** : après deux semaines sans `backend='lutris'` : `programs.lutris.enable = false`, `trash ~/.config/lutris ~/.local/share/lutris` (garder une copie de `pga.db` dans `~/Documents/notes/games/.data/`).

### 3.7 Mises à jour du runtime hors lancement

`UMU_RUNTIME_UPDATE=0` à chaque partie ; un `systemd.user.timers` hebdomadaire `umu-runtime-update.service` exécute `WINEPREFIX=$XDG_CACHE_HOME/reprise/umu-warm PROTONPATH=<proton-ge> umu-run ""` (création de préfixe = passage complet par `setup_umu`). Mécanisme plausible d'après `umu_run.py:947-965`, **non testé**.

### 3.8 Imports ultérieurs (hors v1) : Lutris, Heroic, Steam

- **Lutris** : `lutris --list-games -j` → `[{id, slug, name, runner, platform, year, directory, playtime, playtimeSeconds, lastplayed, coverPath}]` (vérifié, 53 entrées) ; la config de lancement n'y est pas, elle reste dans `games/*.yml` — l'import §3.6.2 couvre les deux.
- **Heroic** (formes vérifiées sur la machine) : `~/.config/heroic/sideload_apps/library.json` → `{games: [{runner: "sideload", app_name, title, install: {executable, platform, is_dlc}, folder_name, art_cover, art_square, is_installed, canRunOffline}]}` ; `store_cache/{gog,legendary,nile}_library.json` (vides ici) pour les jeux de boutique ; `GamesConfig/<app_name>.json` porte `winePrefix`, `wineVersion.{bin,name,type}`, `enviromentOptions`, `wrapperOptions`, `eacRuntime`, `showMangohud` (les entrées présentes pointent sur `wine-ge-8-26`, mort). Un importateur mappe `install.executable → exe`, `winePrefix → prefix`, `wineVersion → proton` (ou `wine`), `enviromentOptions → env`.
- **Steam** : `steamapps/*.acf` et `compatibilitytools.d` (standard, non vérifié ici). Hors périmètre tant que la bibliothèque n'a aucun jeu Steam.

## 4. Coût et risques

**Coût** (un dev expérimenté, Python) : noyau `reprise-launch` (résolution, 4 backends, scope, sessions, hooks, CLI) 2 j ; import + `pegasus-sync.py` + `game` 1 j ; validation des 53 jeux par diff d'env 1,5 j ; module home-manager (paquet, liens Proton, timer) 0,5 j. **≈ 5 jours**, plus 1 jour de marge pour AC Odyssey, GTA V (proton-em + overrides) et RE4.

**Risques**
- *Parité d'environnement* : le seul vrai risque, borné par le diff §3.6.3. Cas particuliers : `dxvk_version: manual` (AC Odyssey), `prefix_command` RE4 (devient une variable ordinaire : Proton fait `append_to_env_str` sur `WINEDLLOVERRIDES`, `proton:1082, 2598`), GTA V sous proton-em avec `socialclub=n,b;version=n,b`.
- *Fin de session* : le scope change la définition du playtime (arbre entier au lieu du PID umu-run) — les durées vont **augmenter** un peu (wineserver, écrans de lanceur). Assumé.
- *protonfixes* : rien ne se perd (rien n'est appliqué aujourd'hui) ; l'opt-in GTA V `umu-271590` désactive hidraw, à tester manette en main.
- *Anticheat* : aucun jeu concerné. Le jour où : `PROTON_EAC_RUNTIME`/`PROTON_BATTLEYE_RUNTIME` pointent vers des dossiers que Lutris télécharge (`wine.py:1246-1249`) ; il faudrait les récupérer soi-même.
- *Réseau au lancement* : sonde 5 s hors-ligne, incompressible sans patch umu.
- *Imbrication FHS sur NixOS* : `umu-run` (bwrap) → pressure-vessel (bwrap). Aujourd'hui ça tourne depuis le bwrap de Lutris (quel `umu-run` il résout, celui du PATH ou un autre, n'a pas été vérifié) ; en direct, profondeur équivalente ou moindre. `umu-run` seul n'a jamais été prouvé sur cette machine : à confirmer par le premier lancement `backend='proton'`, ou avant par `WINEPREFIX=/tmp/umu-test PROTONPATH=<store proton-ge> UMU_RUNTIME_UPDATE=0 umu-run ""` (crée un préfixe, ~30 s, aucune fenêtre).
- *mangohud dans la session gamescope* : LD_PRELOAD purgé par umu ; repli `--mangoapp`.
- *Double comptage* : l'import est atomique (Lutris n'écrit plus rien une fois `backend` basculé), et `legacy_*` gèle l'historique.

## 5. Ce que je ne ferais pas

- **Garder Lutris headless au-delà du pont.** Sa fin de session est le PID d'umu-run, ses hooks se font écraser par le bloc `system:` d'un jeu, son playtime vit dans une base que trois scripts se disputent. Chaque module suivant se construirait sur ces trois défauts.
- **Faugus, Nero, Bottles, Heroic ou un plugin OpenGamepadUI comme backend** : un GUI et un stockage de plus, pas de contrat d'env, pas de scope ; Heroic n'a même pas de lancement CLI silencieux.
- **`umu-run --config`** : perd winetricks et la mise à jour Proton, ne porte ni verbe ni env.
- **Contourner le Steam Linux Runtime** : impossible par variable en 1.4.x, non supporté par GE-Proton hors umu, et il faudrait un bwrap maison sur NixOS pour un gain non mesuré.
- **protonup-qt / ProtonPlus** : Nix livre déjà proton-ge-bin, proton-em et proton-cachyos ; des noms sans version dans `launch.proton` gardent la base stable à chaque bump.
- **Un TOML par jeu comme base** : les sessions exigent SQLite de toute façon ; un export TOML n'a de sens que pour déboguer une commande umu à la main.
- **Un runtime bypass « pour la latence »** : les 5 s hors-ligne viennent de la sonde réseau, pas du conteneur.
