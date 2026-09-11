# Interface — `ui/` (piste H)

Hôte PySide6 (Qt 6.11) autour du thème Reprise (`~/Projects/pegasus-ui`), porté en Qt 6 et
branché sur le démon par QtDBus. Tout ce que QML voit passe par l'objet `api` ; rien du cœur
n'est chargé en Python.

```
ui/
  pyproject.toml            paquet universe_ui, script universe-ui
  universe_ui/
    host.py                 QGuiApplication + QQmlApplicationEngine, options, capture, script de touches
    api.py                  api.keys / memory / allGames / collections / universe / screens
    models.py               Game, GameListModel et les proxys QML (`import Universe`)
    universe_client.py      UniverseClient (bus de session) et FakeClient (fixtures/library.json)
    gamepad.py              SDL2 → QKeyEvent (Mapper pur + GamepadThread), KeyScript
    screens/                données des écrans ajoutés : settings.py, sources.py, media.py
    fixtures/               library.json (8 jeux + 1 caché, sessions, clips, journal, modules, sources) et art.py
    qml/                    theme.qml, core/, ui/, pages/, sound/, assets/ (thème porté + écrans ajoutés)
  tests/                    pytest (offscreen)
```

## Lancer

Sans le flake, tout passe par un shell nix :

```sh
cd ui
nix shell --impure --expr 'let pkgs = import <nixpkgs> {}; in pkgs.python3.withPackages (ps: [ ps.pyside6 ps.pytest ps.pysdl2 ps.qrcode ])' \
  -c python3 -m universe_ui --fake            # bibliothèque factice, aucun démon
  -c python3 -m universe_ui                   # démon réel sur le bus de session
```

Options : `--fullscreen`, `--size WxH` (1920x1080 par défaut), `--no-gamepad`,
`--fake-launch` (la session factice lance `sleep 2` dans un QProcess, implique `--fake`),
`--screenshot PATH --after MS` (capture `grabWindow()` puis quitte), `--quit-after MS`,
`--keys "Right Right Return Wait I"` avec `--key-gap MS`/`--key-delay MS` (touches postées à la
fenêtre : noms `A B X Y LB RB LT RT Start Up Down Left Right Return Esc`, `Wait` marque une pause,
`Wait:N` en marque N, `Hold:A`/`Release:A` séparent un appui, `Shot:chemin.png` capture la fenêtre).
Les touches postées pendant qu'un jeu a le focus sont ignorées par Qt (pas d'item actif) : le
script ne peut pas piloter l'hôte derrière le jeu, la manette non plus (`focusWindow()` est nul),
ce qui est le comportement voulu tant qu'il n'y a pas d'overlay.

Chemins Qt : le shell nix ne pose ni `QML2_IMPORT_PATH` ni chemins de greffons ; `host.py` les
déduit alors de `ldd` sur les `.so` de PySide6 (qtbase, qtdeclarative, qtmultimedia) et y ajoute
les `qt5compat`/`qtsvg` du store qui pointent sur le même qtbase (plusieurs coexistent, un greffon
lié à un autre qtbase est refusé). Dès que `QML2_IMPORT_PATH` ou `QML_IMPORT_PATH` est posé
(`wrapQtAppsHook` du flake, `nix flake check`), il ne touche à rien.

À l'écran (Wayland) : `QT_QPA_PLATFORM=wayland` et `LD_LIBRARY_PATH` complété de
`/run/current-system/sw/lib` pour que QtMultimedia trouve pipewire (sons et lecteur vidéo). Sur
un écran en DPR 2 la capture fait 3840×2160.

Tests, commande exacte (`ps.qrcode` ajouté à celle du plan ; la config pytest est dans
`ui/pyproject.toml`) :

```sh
cd ui && nix shell --impure --expr 'let pkgs = import <nixpkgs> {}; in pkgs.python3.withPackages (ps: [ ps.pyside6 ps.pytest ps.pysdl2 ps.qrcode ])' -c python3 -m pytest -q
```

`conftest.py` force
`QT_QPA_PLATFORM=offscreen` et détourne `XDG_STATE_HOME`/`XDG_CACHE_HOME`/`XDG_DATA_HOME`
vers un dossier temporaire : rien n'est écrit dans l'état de l'utilisateur. La suite passe aussi
avec `QML2_IMPORT_PATH` posé (mode du `nix flake check`). Le seul test qui touche le bus
(`test_bus_subscriptions_bind`) est sauté quand `io.github.ilyasturki.Universe` n'y est pas
déjà enregistré : un appel de méthode activerait `universed` via son fichier `.service` sur
les données réelles de l'utilisateur.

## Manette

SDL2 (pysdl2) dans un QThread, `SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS=1`, branchement à
chaud. Chaque bouton devient la touche clavier que Pegasus associait déjà à l'action, le thème
n'a donc pas changé de logique de touches. Rien n'est posté quand aucune fenêtre n'a le focus
(un jeu tient l'écran).

| Manette | Touche | Action dans le thème |
|---|---|---|
| A | Entrée | Accept : lancer, ouvrir, valider |
| B | Échap | Cancel : retour, fermer |
| X | I | Details : fiche du jeu |
| Y | F | Filters : favori, tri, sections |
| LB / RB | Q / E | PrevPage / NextPage : onglets |
| LT / RT | Page↑ / Page↓ | collection (Library), section (Settings), pages du clavier |
| Start, Guide | F1 | Menu : menu contextuel du jeu ciblé, sinon bascule sur l'onglet Settings |
| croix, stick gauche | flèches | navigation, répétition après 350 ms toutes les 90 ms (flèches seulement) |

Axes : seuil d'appui 0,5, de relâchement 0,3 (hystérésis) ; les gâchettes ne vont que vers le
positif. `Mapper` est pur (testé sans matériel), `GamepadThread` l'alimente.

## Ce que le thème a gagné

- Onglet **Settings** avec cinq sections (LT/RT ou la barre de puces) : *Modules* (chaque module
  de `Modules1.List`, sa bascule et ses réglages globaux rendus par type : bool → interrupteur,
  enum → sélecteur de puces, string/path/int → clavier virtuel), *Install* (bibliothèque de la
  source par `Sources1.Library`, recherche, statut Installed / Update available / Owned,
  A = Install / Update, barre de progression sur `Progress`/`JobFinished`), *Updates*
  (`Sources1.Updates`, tout mettre à jour), *Login* (`LoginUrl` affiché en QR + URL, saisie du
  code au clavier virtuel → `Login`), *Doctor* (`Modules1.Doctor`, pastille verte/rouge).
- Menu contextuel du jeu (Start, ou « More » sur la fiche) : Play/Continue, Details, favori,
  **Game settings** (clés cœur de `Library1.Set` + réglages `scope = game` des modules actifs,
  même rendu générique), **Recordings** (`Recording1.List`, vignette ffmpeg mise en cache dans
  `$XDG_CACHE_HOME/universe/thumbnails/`, lecteur QtMultimedia, A = lecture/pause),
  **Journal** (`Journal1.List`, lecture entrée par entrée), et **Stop** quand une session tourne.
- Flux de lancement : fondu vers le fond → `Session1.Launch(id, écran)` en asynchrone (le démon
  bloque sur les hooks pré-lancement) ; l'interface reste ouverte ; `SessionStarted` confirme,
  `SessionEnded` recharge le jeu (`Library1.Get`), restaure la page et affiche « Titre · N min » ;
  badge de session dans la barre d'onglets depuis la propriété `Current` (relue sur
  `PropertiesChanged`) ; un lancement pendant une session est refusé avec un toast ; l'échec
  (`launchFailed`) rend la page et affiche le message du démon.
- Toasts pour toutes les erreurs D-Bus (`error(kind, message)`).

## Décisions

- **Proxys en Python.** `QtQml.Models.SortFilterProxyModel` de Qt 6 n'a ni `get()`, ni
  `ExpressionFilter`, et son `FunctionFilter` plante (phase 0, T2). `RecentGames`, `SortedGames`,
  `LimitedGames`, `FavouriteGames`, `SearchGames`, `LibraryGames` sont des
  `QSortFilterProxyModel` enregistrés sous `import Universe` ; `get(i)` rend l'objet `Game`,
  `sourceRow(i)` remplace `mapToSource(i)` (le nom C++ est virtuel, le masquer casse le tri).
  `lastPlayed` absent trie en dernier même en décroissant.
- **Jeux cachés hors de tous les modèles.** Le thème n'a pas de notion de jeu caché ;
  `hidden = true` se règle dans Game settings et retire le jeu partout. `removed` idem.
- **Collections = plateformes.** Le thème étiquette une collection par son `shortName`
  (`assets/platforms/<shortName>.svg`). Le démon donne `platform` (« windows »,
  « Nintendo Switch »…) : `api.py` le traduit en `windows`, `switch`, `wii`, `gamecube`, `nds`,
  `ps3`… (table `PLATFORM_SHORT`), à défaut la source (`gog`, `lutris`) ; tri par nom.
  `game.source` est le `kind` du dictionnaire `source` du démon.
- **Extras.** Le thème lit les extras Pegasus en listes (`extra["metacritic"][0]`). Toute clé de
  `metadata` inconnue (hors `*_id`, `*_appid`, valeurs nulles) devient un extra en liste, `_`
  → `-` (`hltb_main` → `hltb-main`), puis `metadata.extra` s'y superpose.
- **Décodage tolérant.** Chaque clé manquante a un défaut ; `developer`/`developers`,
  `genre`/`genres`, listes ou chaînes à virgules acceptées ; `stats.hours` → secondes ;
  `players: 0` → 1 ; dates ISO 8601 avec décalage.
- **Erreurs.** `io.github.ilyasturki.Universe.Error.<Kind>` en nom d'erreur, ou, comme le fait
  `universed` 0.1, un `org.freedesktop.DBus.Error.Failed` dont le message commence par ce
  préfixe : dans les deux cas `UniverseError(kind, message)` sans le préfixe.
- **Signaux.** `bus.connect(service, path, iface, "Sig", self, SLOT("slot(QString,uint)"))`
  avec des `@Slot` typés (T4) ; `PropertiesChanged` reçu en `QDBusMessage` et `Current` relue
  (l'`a{sv}` n'est pas lisible depuis PySide6). `UniverseClient.connected` garde le résultat de
  chaque abonnement, une signature fausse perd le signal en silence.
- **Réglages par jeu.** Valeur du `game.toml`, sinon le bloc `effective` du démon, sinon la
  config globale. Clés cœur : `launch.proton` (choix = clés de `config.proton`), `esync`,
  `fsync`, `mangohud`, `args`, `working_dir`, `desktop.hide_cursor`, `favorite`, `hidden`,
  `sort_title`, `tags`, `metadata.sgdb_id`, `metadata.rawg_id`. Envoi en chaînes : `true/false`,
  listes à virgules, `""` supprime. Réglages de module par `Modules1.SetSetting`.
- **Menu (Start).** Ouvre le menu contextuel du jeu ciblé (rail, grille, fiche) ; sans jeu
  ciblé, bascule Settings ↔ onglet précédent.
- **Rendu logiciel.** Le QPA `offscreen` charge le scenegraph logiciel, où aucun ShaderEffect
  ne rend (OpacityMask, FastBlur, ColorOverlay). `CoverCard`, `BackgroundStage`, `LaunchFrame`,
  `PlatformIcon` testent `GraphicsInfo.api === GraphicsInfo.Software` et dégradent (coins droits,
  pas de flou, icône non teintée). À l'écran (GL) rien ne change.
- **`api.memory`** : `$XDG_STATE_HOME/universe/ui-memory.json`.
- **Journal** : les images relatives d'une entrée sont résolues sous `<game.dir>/journal/`
  (le `dir` que le démon renvoie dans le jeu, donc valable aussi sous `UNIVERSE_DATA_HOME`) ;
  repli sur `$XDG_DATA_HOME/universe/games/<id>/journal/` si le jeu n'a pas de `dir`.
- **Clavier virtuel** : celui du thème, étendu d'une rangée (majuscule, `-_./:`, espace, effacer,
  terminer) pour saisir chemins, codes et valeurs.
- **Pas de `git commit`**, rien touché hors `ui/` et ce fichier.

## Ce qui est simulé (`--fake`)

`FakeClient` sert `fixtures/library.json` avec la même surface que `UniverseClient` : jaquettes,
fonds, logos et captures peints au démarrage (dégradé + titre, dossier temporaire) ; `Launch`
crée une session de 2 s (`--fake-launch` : un vrai `sleep 2`), met à jour stats et sessions,
émet `SessionStarted`/`SessionEnded`/`LibraryChanged` ; `Install`/`Update`/`Scan`/`Login`/
`Media1.Refresh` sont des jobs à pas de 150 ms sur un seul QTimer, avec `Progress` puis
`JobFinished` ; `Recording1.List` fabrique un clip ffmpeg de 3 s (testsrc) si ffmpeg est là ;
`SetSetting` valide contre le schéma (enum, int, bool). `Settings1.Version` = `0.0.0-fake`.

## Vérifié contre le démon

`universed` 0.1.0 (piste C, env isolé sous `scratchpad/uni`, 55 jeux migrés de Lutris, médias
SGDB/RAWG/Steam) : les dix abonnements se lient (`connected` tout à `True`) ; `List`, `Get`,
`Resolve`, `Set` (favori aller-retour avec `LibraryChanged` reçu en `QStringList`), `Sources1.
List/Library/LoginUrl/Updates/Jobs`, `Modules1.List/Doctor`, `settings(id)`, `Sessions`,
`Recording1.List` (clip réel de 3,8 Go : vignette et lecteur), `Journal1.List`, propriétés
`Current` et `Version` ; erreurs `NotFound`/`Unavailable` décodées (`launchFailed` sur un jeu
au backend `emulator`, toast) ; captures offscreen des pages Home, Library (49 visibles, 6
cachés exclus), fiche (Metacritic 56 depuis `metadata`), Settings/Modules (schémas réels, libellés
français), Install (cache GOG, statuts), Doctor, Game settings, Recordings.

## À vérifier avec le démon

- `Session1.Launch` d'un vrai jeu depuis l'interface : durée du blocage sur les hooks, fondu,
  `SessionStarted` puis `PropertiesChanged`/`Current`, badge, `Stop` depuis le menu, retour
  de page et toast à `SessionEnded`, `RecordingFiled`/`EntryWritten` en fin de partie (seule la
  voie d'erreur a été exercée).
- `Sources1.Login` réel (le démon ne renvoie pas `logged_in` dans `List`, contrairement à
  `api.md` : la section Login affiche donc « Signed in: no » tant qu'il ne l'expose pas),
  `Install`/`Update`/`Scan` avec `Progress` (jobs réels).
- `Modules1.Enable/SetSetting` et `Library1.Set` sur toutes les clés (seul `favorite` a été
  écrit), rejet d'une valeur hors schéma.
- `Media1.MediaChanged` après un `Refresh`, `ModulesChanged` après `Enable`.
- `Session1.Screenshot` (non exposé dans l'interface, présent dans le client).
- Manette physique (SDL2 sous Wayland/GNOME, répétition, hot-plug) et sons à l'écran.
- `QScreen.name()` (`DP-1`) comme argument `screen` de `Launch` sur un poste multi-écrans.
- Paquet `.#universe-ui` du flake (wrapQtAppsHook avec qtsvg, qt5compat, qtmultimedia,
  qtwayland ; SDL2 dans `LD_LIBRARY_PATH`).
