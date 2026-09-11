# 02 — Session, capture, captures d'écran, fin de session

Périmètre : modèle de session (gamescope dédié vs GNOME), capture vidéo sans sélecteur de portail, captures d'écran sans GNOME Shell, appairage Bluetooth et notifications sans GNOME, détection de fin de partie et temps de jeu. Contrainte discriminante : aujourd'hui le sélecteur de portail n'existe que pour qu'un enregistrement suive le jeu du moniteur 4K (DP-1) vers la TV (HDMI) en pleine session. Chaque choix ci-dessous répond à ce cas, dans les deux sessions.

Versions vérifiées sur la machine : gamescope 3.16.23, gpu-screen-recorder 5.10.2, systemd 260, noyau 7.2.3, connecteurs DRM `card1-DP-1` (connecté), `card1-HDMI-A-1`, `card1-HDMI-A-2`, `card1-DP-2` (déconnectés). `mkvmerge`, `mangohudctl`, `mangoapp`, `umu-run`, `wpctl` sont installés ; `gamescopectl` est dans le store (`/nix/store/…-gamescope-3.16.23/bin/gamescopectl`) mais pas dans le PATH.

## 1. Verdict

**Gamescope d'abord.** La session GDM dédiée (`gamescope --backend drm`) devient le mode de jeu ; GNOME reste un mode complet et natif (le jeu tourne directement sous Mutter, sans gamescope imbriqué). Le cas TV se dissout dans la session gamescope : gamescope écoute le hotplug DRM et re-choisit la sortie selon `--prefer-output` sans redémarrage, et la capture passe par le flux PipeWire du compositeur via `xdg-desktop-portal-gamescope`, qui n'a aucun dialogue et suit tout changement de sortie. Sous GNOME, la capture est KMS par connecteur (`-w DP-1`) et c'est le démon qui pilote la bascule moniteur→TV (`org.gnome.Mutter.DisplayConfig.ApplyMonitorsConfig`), redémarre l'encodeur sur le nouveau connecteur et recolle les segments (`mkvmerge`). Les captures d'écran passent par `gamescopectl screenshot` (gamescope) et `gpu-screen-recorder -w DP-1 -o x.png` (GNOME) : l'extension GameShot, `await-game-window.js`, `resolve-capture-target.js` et `-s 3840x2160` partent ensemble. La fin de partie et le temps de jeu appartiennent au démon via un scope systemd par lancement et `cgroup.events`.

## 2. Les faits, avec sources

### 2.1 Gamescope embarqué : hotplug et choix de sortie

- **Gamescope réagit au hotplug DRM.** wlroots émet `wlr_device.events.change` sur un événement udev `change` de la carte KMS (`backend/session/session.c`, https://gitlab.freedesktop.org/wlroots/wlroots/-/raw/master/backend/session/session.c : `} else if (strcmp(action, "change") == 0) { … wl_signal_emit_mutable(&dev->events.change, &event);`). Gamescope s'y abonne dans `src/wlserver.cpp` (`wlserver.wlr.device_change_listener.notify = kms_device_handle_change; wl_signal_add( &wlserver.wlr.device->events.change, … )`) et le handler fait `GetBackend()->DirtyState(); wl_log.infof( "Got change event for KMS device" ); nudge_steamcompmgr();`. La boucle principale de `src/steamcompmgr.cpp` (« If our DRM state is out-of-date, refresh it. This might update the output size. `if ( GetBackend()->PollState() )` ») appelle `drm_poll_state` → `refresh_state` → `setup_best_connector(drm, out_of_date >= 2, false)` (`src/Backends/DRMBackend.cpp`).
- **`setup_best_connector` bascule vers le meilleur connecteur.** Source : https://raw.githubusercontent.com/ValveSoftware/gamescope/master/src/Backends/DRMBackend.cpp. Il commence par « `if (drm->pConnector && …connection != DRM_MODE_CONNECTED) { drm_log.infof("current connector '%s' disconnected"); drm->pConnector = nullptr; }` », parcourt les connecteurs connectés, prend la priorité minimale (`get_connector_priority`), puis : « `if (!force) { if ((!best && drm->pConnector) || (best && best == drm->pConnector)) { // Let's keep our current connector return true; } }` » — donc si un connecteur mieux classé apparaît, ou si le courant disparaît, il change de sortie en session, sans redémarrage. C'est exactement ce qui fait qu'un Steam Deck docké passe sur la TV.
- **Sémantique de `--prefer-output`** : « `-O, --prefer-output  list of connectors in order of preference (ex: DP-1,DP-2,DP-3,HDMI-A-1)` » (`gamescope --help` local). `get_connector_priority` : priorité du nom s'il est listé, sinon celle de `"*"` s'il est listé, sinon `connector_priorities.size()` (le pire). Valve utilise `-O '*',eDP-1` pour préférer n'importe quel écran externe au panneau interne (issue #2296, https://github.com/ValveSoftware/gamescope/issues/2296).
- **Pas de bouton runtime.** `drm->connector_priorities = parse_connector_priorities( g_sOutputName );` est exécuté une fois dans `init_drm` (DRMBackend.cpp:1358) ; aucune `ConVar` ne touche au choix de sortie (les `cv_drm_*` déclarées lignes 63-95 sont des chicken bits couleur/planes). `gamescopectl` ne peut donc pas changer la sortie. Issue #645 (« Select monitor for gamescope to appear on », ouverte, https://github.com/ValveSoftware/gamescope/issues/645) le confirme côté utilisateurs ; `--display-index` n'existe qu'en mode imbriqué (« forces gamescope to use a specific display in nested mode ») et « does nothing on the Wayland backend » (misyltoad, même issue).
- **Un hotplug synthétique existe côté noyau.** `drivers/gpu/drm/drm_sysfs.c` `status_store` (https://raw.githubusercontent.com/torvalds/linux/master/drivers/gpu/drm/drm_sysfs.c) : écrire `off` / `on` / `detect` dans `/sys/class/drm/card1-DP-1/status` fixe `connector->force` puis appelle `connector->funcs->fill_modes(...)`. Dans `drm_probe_helper.c` (https://raw.githubusercontent.com/torvalds/linux/master/drivers/gpu/drm/drm_probe_helper.c) : « `if (old_status != connector->status) { … dev->mode_config.delayed_event = true; if (dev->mode_config.poll_enabled) mod_delayed_work(…, &dev->mode_config.output_poll_work, 0); }` », `poll_enabled = true` est posé inconditionnellement dans `drm_kms_helper_poll_init` (l. 939), et `output_poll_execute` finit par `drm_kms_helper_hotplug_event(dev)` quand `delayed_event` est vrai → uevent → wlroots → gamescope. **Chaîne vérifiée dans les sources, non testée ici** (DP-1 est la seule sortie branchée ; forcer `off` aurait éteint l'écran jusqu'à `detect`). Le fichier `status` appartient à root (mode 0644) : il faut une règle udev qui le passe en `g+w` pour le groupe `video`, comme on le fait pour `/sys/class/backlight/*/brightness`.
- **Hotplug : bugs connus** uniquement NVIDIA (#2333 « Display output corrupts permanently after a DRM connector hotplug (TV power-cycle) on NVIDIA », fermé, https://github.com/ValveSoftware/gamescope/issues/2333, commentaire mainteneur : « NVIDIA needs to investigate their internal bug »). Rien d'équivalent sur AMD.

### 2.2 Gamescope : captures d'écran et flux de capture

- **`gamescopectl screenshot`.** `src/steamcompmgr.cpp` : « `static ConCommand cc_screenshot( "screenshot", "Take a screenshot to a given path.", …` » avec `args[1]` = chemin (défaut `/tmp/gamescope.png`) et `args[2]` = type. Types (`protocol/gamescope-control.xml`, https://raw.githubusercontent.com/ValveSoftware/gamescope/master/protocol/gamescope-control.xml) : `base_plane_only = 1` « Just the game w/ no display color mgmt », `all_real_layers = 2` « Just the game + overlays », `full_composition = 3`, `screen_buffer = 4` « The buffer displayed on-screen - 1:1 » ; « Extension of the file determines the format » (`.png`, `.avif` pour le HDR). Le type 1 est rendu **à la résolution de rendu du jeu** (« Repaint base-plane-only screenshots at the game's render resolution to preserve supersampling », steamcompmgr.cpp:3414). `gamescopectl` parle à gamescope par le socket Wayland `GAMESCOPE_WAYLAND_DISPLAY` (`src/Apps/gamescopectl.cpp:86`) et reçoit l'événement `screenshot_taken` avec le chemin. Le paquet nix `gamescope` livre `gamescopectl` mais seul `/run/wrappers/bin/gamescope` est dans le PATH.
- **Flux PipeWire toujours créé.** `src/main.cpp:1105` : « `#if HAVE_PIPEWIRE if ( !init_pipewire() ) { fprintf( stderr, "Warning: failed to setup PipeWire, screen capture won't be available\n" ); }` », quel que soit le backend. `src/pipewire.cpp` : nœud `pw_stream_new(state->core, "gamescope", PW_KEY_MEDIA_CLASS "Video/Source")`, `pwr_log.infof("stream available on node ID: %u")`. Le contenu est peint par `paint_pipewire()` (steamcompmgr.cpp:2776) en SDR (`outputEncodingEOTF = EOTF_Gamma22`) ; le HDR sur ce flux est une demande ouverte (#2126). Le choix de la fenêtre exposée suit la logique « SteamControlled » (l. 2837) ; #1898 signale que l'UI Steam n'y apparaît pas toujours.
- **gpu-screen-recorder ne consomme un nœud PipeWire qu'à travers le portail** : le README (https://git.dec05eba.com/gpu-screen-recorder/about/) ne cite PipeWire que comme dépendance de `-Dportal=true` ; `-w` accepte « `window_id|monitor|focused|portal|region|v4l2_device_path` » (`gpu-screen-recorder --help` local), pas de nœud.
- **`xdg-desktop-portal-gamescope` (Igalia pour Valve, fork Jovian)** implémente `org.freedesktop.impl.portal.Access`, `.ScreenCast` et `.Screenshot` (README, https://raw.githubusercontent.com/Jovian-Experiments/xdg-desktop-portal-gamescope/main/README.md). `src/screencast.rs` : `select_sources` renvoie `SelectSourcesResponse {}` sans dialogue, `start_cast` fait `get_gamescope_pipewire_node_id()` puis `StreamBuilder::new(node_id).source_type(SourceType::Monitor)` ; `src/access.rs` répond automatiquement. **Aucun sélecteur, jamais.** C'est le mécanisme de l'enregistrement Steam sur SteamOS. Pas dans nixpkgs (404 sur `pkgs/by-name/xd/xdg-desktop-portal-gamescope`) ; Jovian a la dérivation (`pkgs/xdg-desktop-portal-gamescope/default.nix`, Rust + meson) et la câble par `systemctl --user set-environment XDG_DESKTOP_PORTAL_DIR="@gamescope-portals@/share/xdg-desktop-portal/gamescope-portals"` (`pkgs/gamescope-session/portals.patch`) avec un `symlinkJoin` de `xdg-desktop-portal-gamescope` et `xdg-desktop-portal-holo`, plus un `gamescope-portals.conf` installé dans `share/xdg-desktop-portal/` (`default.nix`, « # portals »). Le fichier `gamescope.portal.in` n'a pas de `UseIn=`, d'où le `portals.conf`.
- **KMS sous gamescope.** `gsr-kms-server` (https://git.dec05eba.com/gpu-screen-recorder/plain/kms/server/kms_server.c) ouvre son propre fd, énumère `drmModeGetPlaneResources`, lit chaque plane par `drmModeGetFB2`, exporte les handles en fd (`drm_prime_handles_to_fds`) et typpe les planes (`PLANE_PROPERTY_IS_PRIMARY | IS_CURSOR | IS_OVERLAY`, « Not all drm drivers support zpos. In that case assume that the planes are stacked in the order primary, overlay, cursor »). Il ne dépend pas de qui est DRM master (GNOME et KDE le sont déjà). Ce que gsr fait des planes overlay de liftoff (mangoapp sur un overlay, jeu sur le primary) : **non vérifié** (source de `capture/kms.c` inaccessible depuis ici, cgit renvoie une page HTML). Aucune issue « gamescope » dans le tracker gsr à part #60 (HDR trop sombre).
- **`-w focused` est X11 seulement.** Man local : « `focused - Record the currently focused window (X11 only) (use with -s option)` », « `-s WxH … Required for -w focused` ». Sous XWayland-dans-gamescope : non testé, pas retenu comme option de design.

### 2.3 gpu-screen-recorder : capture d'image et KMS

- Man local : « `-c container_format  Container format (mp4, mkv, flv, webm). Defaults to extension from -o filename.` », exemple « Take screenshot: `gpu-screen-recorder -w screen -o screenshot.jpg` », et `-sc` reçoit le type `"screenshot"`. Formats image : « JPEG, PNG » (README). Donc `gpu-screen-recorder -w DP-1 -o shot.png` = capture KMS d'une image, sans shell.
- KMS sans mot de passe : « If GPU Screen Recorder is installed with -Dcapabilities=true (which is the default option) then `gsr-kms-server` is installed with admin capabilities. This removes a password prompt when recording a monitor with the `-w monitor` option » (README). `/run/wrappers/bin/gsr-kms-server` est présent.
- `-w screen` = « Record the first monitor found » ; `-w DP-1` cible un connecteur DRM. Sous GNOME, l'encodeur est dimensionné par le mode du CRTC (plus de fenêtre de démarrage à attendre : `-s 3840x2160`, `await-game-window.js` n'ont plus d'objet).

### 2.4 Structure d'une session gamescope (SteamOS / ChimeraOS / OGUI / Jovian / Bazzite)

- **ChimeraOS `gamescope-session`** (https://github.com/ChimeraOS/gamescope-session) : `usr/bin/gamescope-session-plus` (l'`Exec=` du `.desktop`) fait `export XDG_SESSION_TYPE=x11`, `dbus-update-activation-environment --systemd DESKTOP_SESSION $(env | grep ^XDG_ …)`, `systemctl --user set-environment XDG_DESKTOP_PORTAL_DIR=""` (« so that xdg-desktop-portal doesn't find any portal implementations »), `systemctl --user unset-environment DISPLAY XAUTHORITY`, puis `systemctl --user --wait start gamescope-session-plus@${CLIENT}.service`. L'unité : `BindsTo=graphical-session.target`, `Before=graphical-session.target`, `Wants=graphical-session-pre.target`, `KillMode=mixed`. Le script `usr/share/gamescope-session-plus/gamescope-session-plus` crée `startup.socket` et `stats.pipe` (mkfifo), lance `gamescope … --prefer-output $OUTPUT_CONNECTOR --xwayland-count … --steam -R $socket -T $stats &`, lit `DISPLAY` et `GAMESCOPE_WAYLAND_DISPLAY` sur le socket (« `read -r -t 5 response_x_display response_wl_display <>"$socket"` »), boucle `mangoapp`, lance le client, et « When the client exits, kill gamescope nicely » ; un compteur de sessions courtes (`/tmp/chimeraos-short-session-tracker`, 5 échecs en 60 s) rebascule sur le bureau.
- **`steamos-session-select`** (ChimeraOS) : appelle `/usr/lib/os-session-select $@` s'il existe, sinon `steam -shutdown`. **Bazzite** (`system_files/deck/shared/usr/libexec/os-session-select`) : « This script is now deprecated, please use steamosctl » → `steamosctl set-default-login-mode desktop|game`, `steamosctl switch-to-desktop-mode gnome.desktop` ; SDDM `holo.conf` : `[Autologin] Relogin=true Session=gamescope-session-ogui-steam.desktop`. **Jovian** (`modules/steam/autostart.nix`) : `services.displayManager.autoLogin.enable = true; sddm.autoLogin.relogin = true; defaultSession = "gamescope-wayland"`, `steamosctl set-default-desktop-session ${cfg.desktopSession}.desktop`, et « Traditional Display Managers cannot be enabled in conjunction with this option ». Toutes les distributions font la bascule **par re-login du display manager**, jamais en vivant dans la session.
- **nixpkgs `programs.opengamepadui.gamescopeSession`** (https://raw.githubusercontent.com/NixOS/nixpkgs/nixos-unstable/nixos/modules/programs/opengamepadui.nix) reprend ce script en ligne (« Based on gamescope-session-plus from ChimeraOS ») : `MANGOHUD_CONFIGFILE` temporaire avec `no_display`, `GAMESCOPE_MODE_SAVE_FILE`, `mkfifo` socket/stats, `gamescope … -R $socket -T $stats`, lecture de `DISPLAY`/`GAMESCOPE_WAYLAND_DISPLAY`, boucle `mangoapp`, client, `kill $gamescope_pid`. `programs.steam.gamescopeSession` est plus simple (`gamescope --steam ${args} -- steam …` dans un `.desktop` `wayland-sessions/steam.desktop`). Le `pegasus-session` actuel (`~/NixOs/os/gaming/pegasus.nix`) ne fait ni `dbus-update-activation-environment`, ni `import-environment`, ni le lien avec `graphical-session.target` : `game-recording.service` (`After=/Wants=graphical-session.target`) ne s'y comporte pas comme sous GNOME.
- **GDM sur NixOS.** `nixos/modules/services/display-managers/gdm.nix` : « Use AutomaticLogin if delay is zero … Otherwise with TimedLogin » → `autoLogin.delay != 0` produit `TimedLoginEnable = true; TimedLogin = user; TimedLoginDelay = delay` ; et `preStart` (à chaque démarrage de `display-manager.service`) : « Set default session in session chooser to a specified values – basically ignore session history. `set-session ${sessionData.autologinSession}` », qui écrit dans AccountsService (« setSessionScript wants AccountsService », `wants = [ "accounts-daemon.service" ]`). Le worker GDM lit la session sauvegardée depuis AccountsService avant d'ouvrir la session (`daemon/gdm-session-worker.c`:3022 « Load settings from accounts daemon before continuing », `on_saved_session_name_read`). AccountsService expose `org.freedesktop.Accounts.User.SetSession(s session)` et l'action polkit `org.freedesktop.accounts.change-own-user-data` est `allow_active=yes` (https://gitlab.freedesktop.org/accountsservice/accountsservice/-/raw/main/data/org.freedesktop.accounts.policy.in). Le greeter gnome-shell affiche l'indicateur de timed login (`js/gdm/loginDialog.js`, `showTimedLoginIndicator`). **Non testé ici** : que GDM 50 relance bien le timed login après une déconnexion (pas seulement au boot).

### 2.5 GNOME : bascule d'écran et sa notification

- `org.gnome.Mutter.DisplayConfig.ApplyMonitorsConfig(u serial, u method, a(iiduba(ssa{sv})) logical_monitors, a{sv} properties)` avec « Possible methods: 0: verify 1: temporary 2: persistent », et le signal `MonitorsChanged` (« The client should then call GetResources() to read the new layout ») — https://gitlab.gnome.org/GNOME/mutter/-/raw/main/data/dbus-interfaces/org.gnome.Mutter.DisplayConfig.xml. Les scripts existants (`~/NixOs/bin/resolve-capture-target.js`) parlent déjà à cette interface ; ils documentent le piège de nommage : « Mutter calls it "HDMI-1" where DRM — and so gpu-screen-recorder's -w — wants "HDMI-A-1"; DP happens to coincide ».
- `mkvmerge` local : « `+` A single '+' causes the next file to be appended instead of added … `$ mkvmerge -o full.mkv file1.mkv + file2.mkv` ».

### 2.6 Fin de session, temps de jeu, umu, MangoHud, volume

- **Lutris** (`lutris/util/process_watcher.py`, https://raw.githubusercontent.com/lutris/lutris/master/lutris/util/process_watcher.py) : surveille les enfants du processus lutris moins `SYSTEM_PROCESSES = { "wineserver", "services.exe", "winedevice.exe", "plugplay.exe", "explorer.exe", … }` et avoue : « This is not accurate since not all processes are started by lutris but are started by Systemd instead ». Le watchdog `pgrep` de `game-recording.nix` (`gameProcessNames`, 4 ratés × 15 s après 60 s de grâce) a la même faiblesse et se mêle des jeux lancés à côté.
- **cgroup v2** (https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html) : `cgroup.events` est « A read-only flat-keyed file which exists on non-root cgroups … a value change in this file generates a file modified event » ; `populated` = « 1 if the cgroup or its descendants contains any live processes; otherwise, 0 » ; « poll and [id]notify events are triggered when the value changes ».
- **`systemd-run --scope`** (man local, systemd 260) : « If a command is run as transient scope unit, it will be executed by systemd-run itself as parent process and will thus inherit the execution environment of the caller. However, the processes of the command are managed by the service manager similarly to normal services … Execution in this case is synchronous, and will return only when the command finishes. » `--collect` = `CollectMode=inactive-or-failed`. `systemctl kill PATTERN…` : « Send a UNIX process signal to one or more processes of the unit ».
- **umu** : `UMU_NO_RUNTIME` est lu et propagé (`umu/umu_run.py`:321 `env["UMU_NO_RUNTIME"] = os.environ.get("UMU_NO_RUNTIME") or ""`) mais absent du man ; l'issue #531 montre l'effet : « `UMU_NO_RUNTIME=1 ./umu-run` … `WARNING: Runtime Platform disabled` » tout en téléchargeant quand même le SLR (bug ouvert, https://github.com/Open-Wine-Components/umu-launcher/issues/531). `UMU_NO_PROTON=1` (documenté) « Runs the executable natively within the Steam Linux Runtime ». `PROTONPATH` = « Path to a Proton directory, version name (e.g., GE-Proton9-5) or codename (e.g., GE-Proton) » (`docs/umu.1.scd`). `umu` est « a copy paste of `SteamLinuxRuntime_sniper` » avec `_v2-entry-point` renommé `umu` (README). Le `disable_runtime: true` de Lutris concerne le runtime Lutris, pas le SLR. Issue #661 : sous game mode, la propriété X `STEAM_GAME` mal posée fait perdre la fenêtre à gamescope (réutilisation d'ID X non gérée par umu).
- **MangoHud** : « To enable mangohud with gamescope you need to install mangoapp. `gamescope --mangoapp -- %command%` … Using normal mangohud with gamescope is not supported. » (README). mangoapp reçoit un message System V (`src/app/main.cpp`: `key = ftok("mangoapp", 65); msgid = msgget(key, 0666 | IPC_CREAT)`, `ctrl_thread` : `no_display` 1 = cacher, 2 = montrer, 3 = basculer) ; le client `mangohudctl` (`src/app/control.c`, binaire ELF du paquet nix `mangohud-0.8.3`) : « Usage: mangohudctl [set|toggle] attribute [value] … no_display hides or shows hud ». Pour la couche Vulkan (jeu sous GNOME) : option `control=` « Sets up a unix socket with a specific name » (socket abstrait `\0mangohud` d'après `control/src/control/__init__.py`), protocole `:cmd=param;`, commande `hud` → `no_display = !no_display` (`src/control.cpp`). Issue #2334 : « `--expose-wayland` breaks `--mangoapp` ».
- **Volume** : `wpctl set-volume ID VOL[%][-/+]`, `wpctl set-mute ID 1|0|toggle` (`wpctl --help` local) ; `@DEFAULT_AUDIO_SINK@` comme ID.

### 2.7 Bluetooth et notifications

- **BlueZ** (`doc/org.bluez.AgentManager.rst`, `doc/org.bluez.Device.rst`, https://github.com/bluez/bluez/tree/master/doc) : `RegisterAgent(object agent, string capability)` avec capacités `""`, `DisplayOnly`, `DisplayYesNo`, `KeyboardOnly`, `NoInputNoOutput`, `KeyboardDisplay` ; « Every application can register its own agent and for all actions triggered by that application its agent is used » ; `Device1.Pair()` : « In case there is no application agent and also no default agent present, this method will fail » ; `Agent1.RequestAuthorization(object device)` « gets called to request the user to authorize an incoming pairing attempt which would in other circumstances trigger the just-works model ».
- **OpenGamepadUI** (`extensions/core/src/bluetooth/bluez/{adapter,device}.rs`) : `Adapter1.StartDiscovery/StopDiscovery`, `Discoverable`, `Pairable`, `Device1.Pair/CancelPairing/Connect/Disconnect`, `Trusted`, `Blocked`, `WakeAllowed` — **aucun `RegisterAgent`** dans l'arbre. Il compte sur un agent par défaut fourni par le système.
- **Qt** (`qtconnectivity/src/bluetooth/qbluetoothlocaldevice_bluez.cpp`) : `requestPairing` fait `pairingTarget->Pair()` et n'enregistre aucun agent (aucune occurrence de `Agent` dans le fichier). QtBluetooth n'apporte donc rien de plus que D-Bus direct, et échoue pareil sans agent.
- **Notifications** : ChimeraOS lance `steam_notif_daemon` dans la session ; OGUI n'implémente pas `org.freedesktop.Notifications` (son `NotificationManager` est interne). Sans serveur sur le bus, `notify-send` (utilisé par `game-notify`, `process-game-recording.mjs`) échoue en silence.

## 3. Le design recommandé

Responsabilités du démon (`reprise-daemon`, un seul processus, présent dans les deux sessions) :

1. Lancer chaque jeu dans un scope systemd, surveiller `cgroup.events`, tenir temps de jeu et dernière partie, exécuter l'arrêt forcé.
2. Démarrer et arrêter gpu-screen-recorder sur `populated 1 → 0`, puis finaliser, nommer, miniaturiser et transmettre au journal (l'actuel `process-game-recording.mjs`).
3. Posséder la bascule d'écran : `ApplyMonitorsConfig` sous GNOME, hotplug synthétique sous gamescope ; redémarrer la capture sur le bon connecteur et recoller les segments.
4. Prendre les captures d'écran (`gamescopectl` / gsr) sous `attachments/<ts>.png`.
5. Tenir l'agent BlueZ et l'appairage manette.
6. Servir `org.freedesktop.Notifications`, le volume et les bascules HUD dans la session gamescope.

### 3.1 Sessions

**Session GDM « Reprise »** (`wayland-sessions/reprise.desktop`, `DesktopNames=reprise`, `Exec=reprise-session`), construite sur le patron ChimeraOS/OGUI, en trois pièces :

```
reprise-session                       (script, Exec du .desktop)
  ulimit -n 524288
  export XDG_SESSION_TYPE=x11 XDG_CURRENT_DESKTOP=reprise
  dbus-update-activation-environment --systemd DESKTOP_SESSION XDG_CURRENT_DESKTOP XDG_SESSION_TYPE …
  systemctl --user unset-environment DISPLAY XAUTHORITY WAYLAND_DISPLAY
  systemctl --user --wait start reprise-session.service

reprise-session.service               (user) BindsTo=graphical-session.target Before=graphical-session.target
                                      Wants=graphical-session-pre.target KillMode=mixed
  ExecStart=reprise-gamescope-session  :
    mkfifo $XDG_RUNTIME_DIR/reprise/startup.socket
    gamescope --backend drm --prefer-vk-device 1002:744c -O DP-1,HDMI-A-1 \
      --rt --adaptive-sync --hide-cursor-delay 3000 -e -f --xwayland-count 2 --mangoapp \
      -R $socket -T $stats -- reprise-ui &
    read -r -t 5 DISPLAY GAMESCOPE_WAYLAND_DISPLAY <> $socket
    dbus-update-activation-environment --systemd DISPLAY GAMESCOPE_WAYLAND_DISPLAY
    systemctl --user start reprise-daemon.service
    wait gamescope ; systemctl --user stop reprise-daemon.service
  ExecStopPost=reprise-connector DP-1 detect   (un connecteur forcé « off » ne doit pas survivre à un re-login GNOME)

reprise-daemon.service                (user) PartOf=graphical-session.target ; WantedBy=graphical-session.target
                                      (démarré par gnome-session sous GNOME, par le script ci-dessus sous gamescope)
```

Points fixes : `-O DP-1,HDMI-A-1` (le moniteur gagne ; une TV qui garde HPD haut en veille ne peut pas voler la session au boot) ; pas de `--expose-wayland` (casse mangoapp, #2334 ; les jeux restent X11 dans XWayland, et `PROTON_ENABLE_WAYLAND=1` doit être retiré de l'env commun pour cette session : sans socket Wayland exposé, au mieux il ne fait rien, au pire il laisse le jeu sans pilote d'affichage utilisable) ; `reprise-ui` = Pegasus/Reprise avec `QT_QPA_PLATFORM=xcb` comme aujourd'hui ; `pkgs.gamescope` dans `environment.systemPackages` pour avoir `gamescopectl`.

Portails : la variable `GAMESCOPE_WAYLAND_DISPLAY` doit être dans l'environnement d'activation D-Bus **avant** le premier appel de gsr, parce que `xdg-desktop-portal-gamescope` obtient le nœud par le protocole Wayland `gamescope_pipewire` (`src/gamescope_pipewire.rs`, `Event::StreamNode { node_id }`), donc en se connectant au socket de gamescope — d'où le `dbus-update-activation-environment` ci-dessus plutôt qu'un simple `import-environment`. Le backend se sélectionne par `<XDG_CURRENT_DESKTOP>-portals.conf`, donc avec `XDG_CURRENT_DESKTOP=reprise` on ne reprend pas le `XDG_DESKTOP_PORTAL_DIR` de SteamOS (il ne sert qu'à empêcher les backends gtk/kde de planter sans `DISPLAY`, ce que le choix explicite évite déjà) mais le câblage natif NixOS :

```nix
xdg.portal.extraPortals = [ xdg-desktop-portal-gamescope ];   # dérivation copiée de Jovian
xdg.portal.config.reprise.default = "gamescope";               # → reprise-portals.conf
```

**Bascule vers GNOME et retour**, par re-login GDM (le seul mécanisme que SteamOS, Bazzite et Jovian utilisent, ici traduit pour GDM) :

```nix
services.displayManager.autoLogin = { enable = true; user = "yasso"; };
services.displayManager.gdm.autoLogin.delay = 2;      # TimedLogin, rejoué à chaque greeter
services.displayManager.defaultSession = "reprise";   # preStart set-session : chaque boot = mode jeu
```

- Reprise → bureau : `busctl --system call org.freedesktop.Accounts /org/freedesktop/Accounts/User$(id -u) org.freedesktop.Accounts.User SetSession s gnome` puis `systemctl --user stop reprise-session.service` (gamescope se ferme, GDM revient, timed login de 2 s dans GNOME).
- GNOME → Reprise : `reprise-session-select game` = `SetSession s reprise` + `gnome-session-quit --logout --no-prompt`. Le reboot ramène toujours en mode jeu (preStart réécrit AccountsService). Sous GNOME, Reprise reste lançable en fenêtre plein écran comme aujourd'hui.

### 3.2 Capture vidéo — une conception par session

**Session gamescope : `-w portal` via `xdg-desktop-portal-gamescope`.**

```
gpu-screen-recorder -w portal -restore-portal-session no \
  -f 60 -fm vfr -c mkv -k av1_10bit -ac opus -a default_output -a default_input \
  -tune quality -bm cbr -q 20000 -ffmpeg-video-opts 'rc_mode=QVBR;global_quality=95;b=16000000;maxrate=32000000;bufsize=64000000' \
  -o "$OUT"
```

Sans `-s`, sans attente de fenêtre, sans résolution de connecteur : le portail répond immédiatement avec le nœud du compositeur, à la taille de sortie de gamescope, cadre = ce que l'écran montre (jeu + mangoapp + toasts de Reprise). Une bascule DP-1 → HDMI-A-1 est invisible pour l'enregistreur : le nœud est le même, le CRTC change derrière. Contenu SDR (le flux ne fait pas de HDR) — comme aujourd'hui.

Repli si le flux se révèle capricieux (#1898) : KMS `-w <connecteur actif>`, connecteur lu dans `/sys/class/drm/card1-*/enabled`, redémarré par le démon sur événement udev `drm` (pyudev) ; segments recollés comme sous GNOME.

**Session GNOME : KMS par connecteur, bascule pilotée par le démon, segments.**

1. `gpu-screen-recorder -w DP-1 … -o seg-001.mkv` dès que le scope du jeu est peuplé (§3.4) ; plus de `-w portal`, plus de `-restore-portal-session`, plus de `-s`.
2. Bascule TV (action « TV » dans Reprise, ou raccourci manette) : le démon envoie `SIGINT` à gsr et attend la fin du fichier, appelle `ApplyMonitorsConfig(serial, 1 /*temporary*/, [(0, 0, 1.0, 0, true, [("HDMI-1", "<mode-id 3840x2160@60>", {})])], {})` (nom Mutter, pas DRM), puis relance `gpu-screen-recorder -w HDMI-A-1 … -o seg-002.mkv`. Retour : même séquence avec `DP-1`. Si la bascule est faite ailleurs, le signal `MonitorsChanged` sert de déclencheur et `GetCurrentState` donne le connecteur.
3. En fin de session : `mkvmerge -o <NNN-date-dur>.mkv seg-001.mkv + seg-002.mkv` (mêmes paramètres d'encodage, chaque segment commence sur une keyframe puisque c'est un encodeur neuf ; `ffmpeg -f concat -c copy` marche aussi mais gère moins bien les timestamps VFR). Le trou de 1 à 3 s pendant la bascule est le prix ; l'écran est noir à ce moment-là de toute façon.

Ce qui disparaît des deux côtés : le sélecteur de portail et son délai de 60 s, `await-game-window.js`, `resolve-capture-target.js`, `-s 3840x2160`, l'extension GameShot, `GI_TYPELIB_PATH` à démonter.

### 3.3 Captures d'écran

- gamescope : `GAMESCOPE_WAYLAND_DISPLAY=$GAMESCOPE_WAYLAND_DISPLAY gamescopectl screenshot "$JOURNAL/<gameKey>/attachments/<ts>.png" 1` (jeu seul, résolution de rendu, sans HUD — ce que le journal veut) ; `2` si l'on veut le HUD. Le type 1 est déjà le défaut de la commande (`eScreenshotType = GAMESCOPE_CONTROL_SCREENSHOT_TYPE_BASE_PLANE_ONLY` avant lecture des arguments), donc le journal reçoit la bonne image même si `gamescopectl` ne transmettait pas le troisième mot — ce que je n'ai pas vérifié.
- GNOME : `gpu-screen-recorder -w <connecteur courant> -o "$…/<ts>.png"` (KMS, sans shell, sans polkit grâce à `gsr-kms-server` setcap). Le connecteur courant est celui que le démon a choisi en §3.2.
- Le démon joue son propre son d'obturateur (Reprise a déjà QtMultimedia) et écrit sous le même `attachments/<ts>.png` que `controller_sdl.py` aujourd'hui, donc `game-session-summary.mjs` ne change pas.

### 3.4 Lancement, fin de session, temps de jeu (démon)

```
systemd-run --user --scope --collect --unit="game-${slug}-${ts}" \
  --property=TimeoutStopSec=15 \
  -- env WINEPREFIX=/mnt/games/prefixes/${slug} GAMEID=umu-${slug} PROTONPATH=GE-Proton \
     umu-run "${exe}" ${args}
```

- Fin de partie = `populated 0` dans `/sys/fs/cgroup/user.slice/user-$UID.slice/user@$UID.service/app.slice/game-${slug}-${ts}.scope/cgroup.events`, surveillé par inotify (`IN_MODIFY`). Couvre wineserver, `services.exe`, pressure-vessel/bwrap et tout ce que Lutris exclut à la main : les namespaces de pressure-vessel ne changent pas l'appartenance cgroup, et `populated` compte les descendants.
- Temps de jeu = `populated 1 → 0`, stocké par le démon (SQLite ou JSON, hors de ce périmètre) ; `lastPlayed` idem. Plus de lecture de `pga.db`.
- Quitter de force depuis la manette : `systemctl --user kill --signal=SIGTERM game-…scope`, puis `systemctl --user stop game-…scope` (SIGKILL après `TimeoutStopSec`).
- Runtime : garder le SLR (ce pour quoi GE-Proton est construit) ; `UMU_NO_RUNTIME=1` par jeu seulement, en sachant qu'il télécharge quand même le SLR (#531). AC Odyssey (wine `system`) tourne par `wine` directement dans le même scope. Les cinq `script:` d'installation restent des one-shots hors démon.
- Le hook de capture n'existe plus : le démon démarre gsr lui-même quand le scope passe à `populated 1` (après un délai court sous GNOME, immédiatement sous gamescope), et l'arrête sur `populated 0` ; `process-game-recording.mjs` devient une étape du démon (même sortie, même nommage `NNN-YYYYMMDD-HHMMSS-<dur>.mkv`).

### 3.5 Manette, volume, HUD, Bluetooth, notifications — sans GNOME

- Volume : `wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%+` / `5%-`, `wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle`. Plus d'uinput.
- HUD : gamescope → `mangohudctl toggle no_display` (même IPC que Steam) ; GNOME → `MANGOHUD_CONFIG=…,control=mangohud` sur le jeu et le démon envoie `:hud;` sur le socket abstrait `\0mangohud` (`socket(AF_UNIX).connect('\0mangohud').send(b':hud;')`). Plus de `Super+F12` synthétique.
- Bluetooth (bus système, D-Bus direct, pas de QtBluetooth) :
  1. `org.bluez.AgentManager1.RegisterAgent('/org/reprise/agent', 'NoInputNoOutput')` + `RequestDefaultAgent` (session gamescope seulement ; sous GNOME l'agent de gnome-shell reste le défaut mais l'agent de l'app sert quand même à ses propres `Pair`). L'objet implémente `org.bluez.Agent1` : `Release`, `RequestAuthorization` → retourne sans erreur, `AuthorizeService` → idem, `Cancel`.
  2. `Adapter1.SetDiscoveryFilter({'Transport': 'auto'})`, `StartDiscovery` ; écouter `org.freedesktop.DBus.ObjectManager.InterfacesAdded` / `PropertiesChanged` sur `org.bluez.Device1` (`Name`, `Icon == 'input-gaming'`, `RSSI`, `Paired`, `Connected`).
  3. Sélection manette : `Device1.Pair()` → `Device1.Trusted = true` → `Device1.Connect()` ; `StopDiscovery`. Oubli : `Adapter1.RemoveDevice(path)`.
- Notifications : dans la session gamescope, le démon (ou l'UI) possède `org.freedesktop.Notifications` sur le bus de session (`Notify(susssasa{sv}i) → u`, `CloseNotification`, `GetCapabilities` → `['body','actions']`, `GetServerInformation`, signaux `NotificationClosed`, `ActionInvoked`) et rend les toasts dans Reprise. `game-notify` et le reste marchent alors sans modification. Sous GNOME, gnome-shell garde le nom.
- Connecteur (gamescope, bascule volontaire) : règle udev `SUBSYSTEM=="drm", KERNEL=="card[0-9]-*", ACTION=="add|change", RUN+="chgrp video /sys%p/status", RUN+="chmod g+w /sys%p/status"`, puis `reprise-connector DP-1 off` = `echo off > /sys/class/drm/card1-DP-1/status` (gamescope passe sur HDMI-A-1 au uevent suivant) et `reprise-connector DP-1 detect` pour revenir. À tester TV branchée avant d'y câbler un bouton.

## 4. Coût et risques

Estimation pour un dev expérimenté, les deux sessions comprises : **≈ 10 jours + 2,5 jours de marge**.

| Chantier | Jours |
|---|---|
| Session Reprise (script, unités, env, portails, module NixOS) + bascule GDM/AccountsService | 2 (+1 si le timed login GDM après logout ne se comporte pas comme prévu) |
| Empaquetage `xdg-desktop-portal-gamescope` + capture portail + validation du flux hors Steam | 1,5 (+1 si le flux ne suit pas la fenêtre du jeu : repli KMS) |
| Capture GNOME (KMS, `ApplyMonitorsConfig`, `MonitorsChanged`, segments, `mkvmerge`) | 1,5 |
| Règle udev + `reprise-connector` + tests avec la TV | 0,5 (+0,5) |
| Captures d'écran (`gamescopectl`, gsr png), retrait de GameShot et des scripts gjs | 0,5 |
| Scope systemd, inotify `cgroup.events`, temps de jeu, arrêt forcé, intégration umu | 1,5 |
| Agent BlueZ + liste d'appairage dans l'UI | 1,5 |
| Serveur `org.freedesktop.Notifications`, volume, bascules HUD | 1 |

Risques, par ordre d'importance :

1. **Flux portail hors Steam** : gamescope choisit la fenêtre exposée avec la stratégie Steam (`SteamControlled`) ; #1898 rapporte des trous. Test d'une heure avant d'engager : dans la session actuelle, `gst-launch-1.0 pipewiresrc target-object=<id du nœud "gamescope"> ! videoconvert ! autovideosink` pendant un lancement de jeu. Repli KMS prêt.
2. **Changement de mode pendant le flux** : `pipewire.cpp` calcule la taille de capture depuis `g_nOutputWidth` à l'init ; le comportement quand la sortie change de résolution n'est pas vérifié. Sans conséquence ici (moniteur et TV en 3840×2160), à retester si une TV 1080p arrive.
3. **Timed login GDM après déconnexion** : vérifié dans nixpkgs et dans les sources GDM, pas exercé sur GDM 50. Si ça ne marche pas : le greeter apparaît et demande un clic — ou on remplace GDM par SDDM `Relogin=true` comme Jovian/Bazzite (0,5 j).
4. **KMS sous gamescope** : composition des planes overlay par gsr non vérifiée ; n'est que le repli.
5. **Hotplug synthétique** : chaîne noyau vérifiée, non exécutée ; le pire cas est un écran noir jusqu'à `detect`, à tester avec la TV branchée et un `sleep 10 && echo detect` de sécurité.
6. **`STEAM_GAME` sous gamescope** (umu #661) : une fenêtre de jeu peut échapper au focus de gamescope ; la session Pegasus actuelle n'a pas montré ce symptôme avec Lutris, à surveiller avec umu.
7. **Segments GNOME** : 1 à 3 s manquantes à chaque bascule, et une bascule pendant une cinématique HDR laisse deux fichiers de gamut identique mais de métadonnées connecteur différentes (`HDR_OUTPUT_METADATA` par CRTC) — sans effet en SDR.

## 5. Ce que je ne ferais pas

- **Gamescope imbriqué sous GNOME** pour unifier la capture : `--expose-wayland` casse mangoapp (#2334), les backends imbriqués traînent des bugs ouverts (#1132, #2138, #2140), et cela ajoute une composition et de la latence sur un 4K à haute fréquence. Le mode GNOME reste natif.
- **`-w focused` / `-w <xid>` de gsr sur XWayland-dans-gamescope** : le man dit X11 seulement, rien ne documente le cas XWayland, et le portail fait déjà mieux (suit la sortie, sans `-s`).
- **`org.gnome.Mutter.ScreenCast.RecordWindow` + pipeline GStreamer** sous GNOME : donnerait un flux de fenêtre sans sélecteur, mais remplace l'encodeur gsr (QVBR AV1 10 bits réglé au poil) par une chaîne `pipewiresrc ! vaav1enc` à requalifier. Pas pour gagner 2 s par bascule.
- **Le jeton de restauration du portail** (`-restore-portal-session yes`) : un jeton de fenêtre référence une fenêtre morte à la partie suivante, le dialogue revient. C'est le problème qu'on quitte.
- **Le watchdog `pgrep` et la liste `gameProcessNames`** : approximatif (Lutris le dit lui-même), aveugle aux jeux non listés, bavard avec deux jeux ouverts ; le scope le remplace intégralement.
- **Garder GameShot** : elle n'existait que parce que `org.gnome.Shell.Screenshot` refuse les appelants de fond ; gsr et gamescopectl n'ont pas ce problème.
- **QtBluetooth** : sur BlueZ il appelle `Device1.Pair()` sans agent, exactement comme OGUI, donc échoue pareil sans agent par défaut ; D-Bus direct + `Agent1` fait tout le travail.
- **`-O HDMI-A-1,DP-1`** (TV prioritaire) : une TV qui garde HPD haut en veille prendrait la session au boot. Le moniteur reste prioritaire ; la TV s'obtient par le hotplug réel ou synthétique.
- **Le HDR sur la capture** : le flux gamescope est SDR (#2126) et gsr HDR n'a de sens qu'en KMS ; l'utilisateur enregistre en `av1_10bit` SDR aujourd'hui, on ne change pas ça.
- **Construire sur la session OpenGamepadUI** : son script est celui de ChimeraOS ; la valeur est dans le patron (socket `-R`, `import-environment`, `BindsTo=graphical-session.target`), qu'on recopie, pas dans Godot.
