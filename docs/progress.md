# Universe — progrès

Source de vérité de la session de livraison du MVP (docs/plan-v3.html). Relire après chaque compaction.

## État des pistes

| Piste | État | Notes |
|---|---|---|
| V · Vérifier | en cours | 5 tests phase 0 lancés en parallèle (sonnet), résultats attendus dans `scratchpad/phase0/test-N.md` |
| C · Cœur | à faire | crate Rust ; API D-Bus et protocole modules à figer dans docs/api.md |
| H · Hôte | à faire | PySide6 + port Reprise ; démarre dès que api.md est figé (faux `api` en attendant le démon) |
| M · Modules | à faire | capture, journal, gog, métadonnées (Media1 dans le cœur) |
| L · Livrer | à faire | flake, module home-manager, module NixOS, doctor, tracker-md, README |

## Décisions

- Cible = les 7 conditions du /goal, pas chaque ligne de §9/§10. Différé : complétions fish, man page, profils Hyprland/niri/KDE, extrait vidéo, galerie, --gallery.
- Tests 1/2/3 partagent l'écran : chaque partie à l'écran est sérialisée par `flock scratchpad/display.lock`.
- H et M démarrent en parallèle de C dès que docs/api.md est figé (hooks = env + `busctl call`, hôte = faux `api`).

## Faits utiles (coûteux à redécouvrir)

- PySide6 6.11.0 / Qt 6.11.1 : `nix shell --impure --expr 'let pkgs = import <nixpkgs> {}; in pkgs.python3.withPackages (ps: [ ps.pyside6 ])' -c python3 …` (le chemin `nixpkgs#python3Packages.pyside6` seul ne met pas le module sur le PATH). `<nixpkgs>` = /nix/store/xns837pvam002diia4wv25crg9k0mfa2-source. Qt nixpkgs loggue vers journald hors tty : `QT_FORCE_STDERR_LOGGING=1`.
- Outils présents : umu-run 1.4.0, gpu-screen-recorder + /run/wrappers/bin/gsr-kms-server, gogdl, codex, cargo 1.97, uv, zx, ffmpeg, jq, mangohud, gamescope 3.16.23. GNOME Wayland, XDG_CURRENT_DESKTOP=GNOME, DISPLAY=:0, WAYLAND_DISPLAY=wayland-0.
- proton-ge (Lutris runner) → /nix/store/wwmg06lh2x6vs5vmplz9d6x5flx2q2fq-proton-ge-steamcompattool ; aussi proton-em, proton-cachyos dans ~/.local/share/lutris/runners/wine/ ; UMU-Proton-10.0-4 dans ~/.local/share/Steam/compatibilitytools.d.
- The Technomancer : exe `/mnt/games/PC/The Technomancer/TheTechnomancer.exe`, préfixe `/mnt/games/gog/the-technomancer`, env `WINE_CPU_TOPOLOGY=4:0,1,2,3`, gog_id 1972906591, build 52654527801265271, GAMEID=umu-default (pas dans umu-database).
- Mini Metro : id 1434554947, build 59049085453457155, 337M, plus petit titre possédé (cible test 5 et `universe install`).
- gsr : flags dans ~/NixOs/home/gaming/game-recording.nix:241-290 (`-f 60 -fm vfr -c mkv -k av1_10bit -ac opus -tune quality -bm cbr -q 20000 -a default_output -a default_input -ffmpeg-video-opts rc_mode=QVBR;global_quality=95;b=16000000;maxrate=32000000;bufsize=64000000`).
- Slug : `sanitizeGameName` ~/NixOs/bin/lib/game-session.mjs:121 (lower, NFKD sans diacritiques, sans apostrophes, [^a-z0-9-]→-, squeeze, trim).
- game script : ~/Dotfiles/home/scripts/game ; gogdl via `gogdl --auth-config-path ~/.config/gogdl/auth.json` ; install = `download <id> --platform windows --with-dlcs --path /mnt/games/PC` (ajoute le dossier), update = `update <id> --platform windows --with-dlcs --path <dir>` ; builds distants = `https://content-system.gog.com/products/<id>/os/windows/builds?generation=2` sans auth ; bibliothèque = `https://embed.gog.com/account/getFilteredProducts?mediaType=1&page=N` avec bearer.
- Reprise (thème) : ~/Projects/pegasus-ui (theme.qml, core/, ui/, pages/, sound/, 5.4k lignes, QtQuick 2.15 + QtGraphicalEffects + SortFilterProxyModel 0.2 + QtMultimedia 5).
- Lutris : 55 yml dans ~/.config/lutris/games/, pga.db ~/.local/share/lutris/pga.db. Enregistrements /mnt/recordings/games/<slug>/ (48 dossiers). Journal ~/Documents/notes/games/journal/<slug>/.
- Scripts à absorber : ~/NixOs/bin/game-session-summary.mjs (journal codex), game-journal.mjs, process-game-recording.mjs, resolve-capture-target.js ; ~/Dotfiles/home/scripts/pegasus-sync.py (SGDB/RAWG/Steam).

## Phase 0 — résultats

(en attente)

## Journal des tours

- T1–T22 : lecture du plan, orientation, vérification des outils. Rien de codé.
