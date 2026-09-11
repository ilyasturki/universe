# Module `gog` — source GOG

`modules/gog/` : `module.toml` (kind `source`, requiert `gogdl`), `bin/source` (Python 3 stdlib), `tests/test_source.py` (pytest, sans réseau ni gogdl réel). Remplace `~/Dotfiles/home/scripts/game` pour tout ce qui touche GOG.

## Verbes et événements

`bin/source <verbe> [args]` — une ligne JSON par événement sur stdout, logs humains sur stderr (préfixe `gog:`, lignes de gogdl relayées telles quelles), code de retour 0/1 (2 = usage). **Chaque verbe finit par `{"event":"done"}`**, `login` compris.

| Verbe | Événements | Notes |
|---|---|---|
| `login` | `login_url{url}` | URL fixe (client id de Galaxy, `layout=client2`) ; le code est dans l'URL de redirection après connexion |
| `login <code>` | `logged_in{user}` | `gogdl auth --code` écrit les jetons dans `auth_path` ; `user` = `username` de `embed.gog.com/userData.json`. gogdl imprime `{"error":true}` avec exit 0 sur un mauvais code : le module le détecte et échoue |
| `library` | `game`×N | Bearer via `gogdl auth` (rafraîchit `auth_path` en place si expiré, sans rien dire sur stderr) ; toutes les pages de `getFilteredProducts` ; cache `$MODULE_DATA_DIR/library.json`. Hors ligne ou non connecté : rejoue le cache avec un avertissement, échoue s'il n'y en a pas |
| `search <texte>` | `game`×≤20 | Catalogue public, `query=like:<texte>&productType=in:game&limit=20`. `owned` croisé avec le cache (`null` sans cache) |
| `info <id>` | `info{data}` | JSON brut de `gogdl info <id> --platform <p> --with-dlcs|--skip-dlcs` (`size`, `buildId`, `versionName`, `dlcs`, `builds`…) |
| `install <id>` | `progress`×n, `game` | `gogdl download <id> … --path <games_dir>` (gogdl ajoute lui-même le dossier du jeu). Après coup, relit `goggame-<id>.info` sous `games_dir` ; absent = échec même si gogdl a rendu 0 |
| `update` | `update`×n | Voir « Décision de mise à jour » |
| `update <id>` | `progress`×n, `game` — ou `done` seul | `gogdl update <id> … --path "<dossier exact>"` uniquement si les builds diffèrent. Après coup, le `.info` doit porter le build distant, sinon échec |
| `scan` | `game`×N | Parcours de `scan_dirs` ∪ `{games_dir}` |

`game` : `{"id","title","owned":bool|null,"installed":bool,"dir","exe","build","dlcs":[ids],"release_year","image"}`. `exe` = `path` de la playTask primaire (`isPrimary && type=="FileTask"`), relatif à `dir`. `image` = jaquette (`https://images-N.gog-statics.com/<hash>.jpg` pour la bibliothèque, `coverVertical` pour le catalogue). `title` vient de la boutique pour `library`/`search` et du `.info` (`name`) pour `scan`/`install`/`update` — ils diffèrent parfois (« Batman™: Arkham Origins » vs « Batman Arkham Origins »).

`progress` : `{"done","total","message":"37.81%"}` en octets écrits, au plus une ligne par seconde, première et dernière (100 %) toujours émises.

`update` : `{"id","title","local_build","remote_build","version","date"}` ; `local_build` peut être `null` (`.info` sans `buildId`, ex. de Blob, INSIDE) — c'est offert quand même, une mise à jour le répare.

## Réglages (`[[settings]]`, scope global)

| Clé | Type | Défaut | Rôle |
|---|---|---|---|
| `games_dir` | path | `/mnt/games/PC` | `--path` de `download` |
| `scan_dirs` | string | `/mnt/games/PC,/mnt/games/gog` | dossiers scannés, séparés par des virgules |
| `platform` | enum windows/linux | `windows` | `--platform` et segment `os/<p>` de l'endpoint builds |
| `with_dlcs` | bool | `true` | `--with-dlcs` sinon `--skip-dlcs` (toujours explicite, le défaut gogdl est « sans ») |
| `auth_path` | path | `~/.config/gogdl/auth.json` | jetons existants de l'utilisateur ; créé avec `{}` s'il manque ou est vide (gogdl `json.loads` le fichier dès qu'il existe) |
| `install_timeout_s` | int | `7200` | délai maximal d'un `download`/`update` |

`MODULE_SETTINGS_JSON` écrase les défauts lus dans `module.toml` (`tomllib`) ; `MODULE_DIR` localise le TOML (sinon le dossier parent du script) ; `MODULE_DATA_DIR` (sinon `$XDG_DATA_HOME/universe/modules/gog`) reçoit `library.json` et `gogdl/`.

## Endpoints et commandes

| Usage | Appel | Auth |
|---|---|---|
| jeton | `gogdl --auth-config-path <auth_path> auth` → `.access_token` | fichier |
| bibliothèque | `GET https://embed.gog.com/account/getFilteredProducts?mediaType=1&page=N` (`.products[]`, `.totalPages`) | Bearer |
| identité | `GET https://embed.gog.com/userData.json` (`.username`) | Bearer |
| recherche | `GET https://catalog.gog.com/v1/catalog?query=like:<q>&productType=in:game&limit=20` (`.products[]`, `id` string, `releaseDate` `AAAA.MM.JJ`) | aucune |
| build public courant | `GET https://content-system.gog.com/products/<id>/os/<platform>/builds?generation=2` → `.items[]` avec `branch==null`, trié par `date_published`, dernier | aucune |
| info / install / update | `gogdl info|download|update <id> --platform <p> --with-dlcs|--skip-dlcs [--path …]` | fichier |

Tous les appels HTTP passent par `fetch(url, token)` (urllib, 30 s, `User-Agent: universe-gog/0.1`) — c'est ce que les tests remplacent.

## Règles T5 et leur application

1. **Cache gogdl dédié** : chaque invocation reçoit `GOGDL_CONFIG_PATH=$MODULE_DATA_DIR/gogdl` ; les manifestes vont dans `…/gogdl/heroic_gogdl/manifests/<id>`, jamais dans `~/.config/heroic_gogdl` (celui de Heroic / du script `game`). Conséquence : pour un jeu installé avant Universe, le premier `update` n'a pas de manifeste de référence et retélécharge tout (`Deleted: 0 New: N`) — c'est le chemin sûr sans xdelta, pas un bug.
2. **Décision de mise à jour par le module** : `update` compare toujours `goggame-<id>.info.buildId` au dernier build public de l'endpoint builds. Builds égaux → `done` sans appeler gogdl (vérifié par un test qui inspecte les appels du faux gogdl). `update` sans id vérifie les 13 installations en parallèle (8 threads) ; un endpoint muet = jeu ignoré avec un log.
3. **Timeout et CRITICAL** : `download`/`update` tournent dans leur propre groupe de processus (`start_new_session`) ; le module lit stdout+stderr fusionnés via un thread et une file ; si la file reste vide jusqu'à l'échéance (`install_timeout_s`) ou si une ligne contient `[TASK_EXEC] CRITICAL`, `killpg(SIGTERM)` puis `SIGKILL` après 10 s, et le verbe échoue. SIGTERM/SIGINT reçus par le module tuent aussi le groupe. gogdl utilise `multiprocessing` : tuer seulement le pid laisserait les workers.
4. **stdout de download/update n'est pas du JSON** : tout est relayé sur stderr sauf les lignes `Progress: x d/t`. Seuls `auth`, `info` impriment du JSON (parsé) ; `import` n'est pas utilisé (le scan lit les `.info` directement).
5. **gogdl rend 0 même quand il abandonne** (`cancelled`, `fatal_error`) : le succès est vérifié sur disque (présence du `.info` après `download`, `buildId == build distant` après `update`).

## Scan

Parcours de chaque dossier de `scan_dirs` jusqu'à 5 niveaux (le script `game` fait `-maxdepth 5` ; Dishonored est à `/mnt/games/gog/dishonored/drive_c/GOG Games/Dishonored/`, profondeur 4 — la consigne « ≤ 2 » l'aurait manqué), sans suivre les liens ni entrer dans les dossiers cachés, et sans descendre sous un dossier qui contient déjà des `goggame-*.info`. Dans un dossier : jeu de base = `gameId == rootGameId` ; `dlcs` = les autres `.info` dont `rootGameId` est ce jeu. Un dossier sans jeu de base est ignoré (log). Un id vu deux fois garde la première occurrence. `owned` = id présent dans `library.json` (`null` sans cache) ; `release_year`/`image` viennent du cache quand il connaît l'id.

Sur cette machine : 13 jeux de base, dont 5 hors bibliothèque (Cuphead, Cyberpunk 2077, INSIDE, Mount & Blade II, de Blob) — `update` les vérifie mais ne les propose pas (log « not in the library ») et `update <id>` les refuse, `gogdl update` échouerait.

## Ce que le cœur doit faire des `game`

- La clé est `id` (string). `library` et `scan` décrivent le même jeu sous deux titres possibles ; le cœur garde `id` et choisit le titre de la boutique quand il l'a.
- **Règle d'identité** : un `game.toml` prend `source = "gog"` seulement si le manifeste (`scan` : `.info` sur disque) **et** la bibliothèque (`library` : id possédé) sont d'accord, c'est-à-dire `installed == true` et `owned == true`. `owned == false` (installé, pas possédé — les 5 ci-dessus) = jeu local à traiter comme un dossier sans source ; `owned == null` = pas encore de cache, réessayer après un `library`.
- `library` avec `installed:false` = titre possédé installable (`Sources1.Install`). `search` idem mais `owned` peut être `false`.
- `install`/`update` réussis renvoient un `game` complet (`dir`, `exe`, `build`) : de quoi créer ou rafraîchir le `game.toml`. Le `dir` de `install` est `games_dir/<installDirectory du manifeste>`, connu seulement après coup.
- `exe` est relatif à `dir` ; le cwd de lancement doit être le dossier de l'exe (T1). Un `.lnk` peut apparaître (de Blob : `Launch de Blob.lnk`) — pas lançable tel quel.
- `progress.done/total` sont des octets ; `Progress(job_id, done, total, message)` peut les relayer sans conversion.
- Le module ne prend pas de verrou : ne pas lancer deux `install`/`update` gogdl en même temps sur le même id.

## Essais réels (11 septembre)

`scan` : 13 jeux, DLC comptés (Dead Cells 5, Cyberpunk 2, Bannerlord 2, Batman AO 1, Cuphead 1), 80 ms. `library` : 17 titres, 8 installés, 0,8 s. `search witcher` : 20 résultats, 2 possédés. `update` : rien en attente pour les 8 possédés. `install 1136126792` (Absolute Drift, 365 Mo, le plus petit possédé non installé) : 4 événements `progress` (0 %, 49,76 %, 62,41 %, 100 %), `game{dir:"/mnt/games/PC/Absolute Drift", exe:"absolutedrift.exe", build:"56631974714622235"}`, 4,3 s, 238 fichiers ; le manifeste gogdl est apparu dans `$MODULE_DATA_DIR/gogdl/heroic_gogdl/manifests/1136126792` et pas dans `~/.config/heroic_gogdl` ; `update 1136126792` juste après → « already current » sans appel gogdl.

## Questions ouvertes pour le cœur

- `done` terminal sur `login` : conservé pour l'uniformité ; à retirer si le cœur préfère la lettre de api.md.
- `title` de `scan` vs `library` : le module ne réconcilie pas ; le cœur tranche (proposition ci-dessus).
- `platform = linux` : les builds Linux natifs passent par un autre gestionnaire gogdl (installateurs) et l'endpoint builds peut répondre 404 → `update` ignore tout. Non testé, hors cible.
- Une mise à jour interrompue laisse le dossier dans un état intermédiaire (T5 §4) ; le cœur pourrait proposer un `repair` — non exposé par le protocole aujourd'hui.
