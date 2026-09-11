# Module `tracker-md` — tracker Markdown

`modules/tracker-md/` : `module.toml` (kind `hooks`, aucun binaire requis), `bin/sync` (Python 3 stdlib, hook `session-end`, aussi utilisable en `--all`), `tests/test_sync.py` (pytest, sur une copie de tracker en `tmp_path`). Alimente le tracker Markdown en texte brut de `~/Documents/notes/games` : la colonne Hours et le lien Journal des six tables décrites dans `README.md` du tracker. Réplique la règle `max(existing, Lutris)` et le slug de `~/Documents/notes/games/.claude/skills/game-tracker/scripts/sync_lutris.py` et `_lib.py`, sans en dépendre (le module tourne sans `uv`).

## `bin/sync` (hook `session-end`, bloquant, `timeout_s = 10`)

Env : `GAME_ID` (= slug), `GAME_TITLE`, `UNIVERSE_GAME_JSON` (lu : `.stats.hours`, `.title`, `.metadata.*`, `.release_year` ; `.journal_count` est fourni par le cœur mais pas lu ici — le signal qui fait foi pour le lien Journal est l'existence du dossier, pas ce compteur), `UNIVERSE_JOURNAL_ROOT` (racine du journal Markdown, `<root>/journal` si absent), `MODULE_SETTINGS_JSON`. `JOURNAL_DIR` (le dossier `games/<id>/journal` du cœur, pour les pièces jointes JSON) est reçu comme tous les hooks mais ne sert à rien ici — ce n'est pas le journal Markdown.

1. Cherche, dans `playing.md`, `hold.md`, `backlog.md`, `played.md`, `dropped.md` (dans cet ordre ; `wishlist.md` est une liste à puces, jamais lu), la ligne dont la cellule Game correspond — texte insensible à la casse, ou `slugify(cellule) == GAME_ID`.
2. **Hours** : `max(cellule existante, stats.hours)`, une décimale. Une cellule vide et des heures calculées à 0 ne produisent rien (pas de `0.0` fabriqué). Une valeur égale à l'existant n'écrit rien.
3. **Journal** : `[↗](journal/<slug>/)` si `<root>/journal/<slug>/` existe, sinon `[↗](journal/.archive/<slug>/)` si seul l'archivé existe — seulement quand la cellule est vide. `journal_link = false` désactive complètement l'écriture.
4. Réécrit uniquement la ligne trouvée, uniquement les cellules qui changent : les largeurs de colonnes sont lues sur la ligne de séparation du tableau (celles que `_lib.py` y a écrites) et réappliquées telles quelles — le reste du fichier reste identique au caractère près. Écriture atomique (`.tmp` + `os.replace`) et seulement si quelque chose a changé.
5. Ligne absente des cinq tables : `create_missing = false` (défaut) → rien n'est écrit, note sur stderr. `create_missing = true` → ligne ajoutée à `playing.md`, à sa place alphabétique (convention des six tables).
6. `update_metadata = true` (défaut) : complète `.data/games.json[GAME_ID]` — `developer`, `publisher` (jointure `, ` de `metadata.developers`/`publishers`), `genre` (`metadata.genres`), `release_year` (racine du JSON, pas sous `metadata`), `summary`, `metacritic`, `rawg_id`, `sgdb_id`, `steam_appid` (`metadata.*`) — uniquement les champs absents ou vides (`None`/`""`/`[]`, et `0` pour les identifiants et `metacritic`, qui est la valeur « inconnu » côté Rust). Un champ déjà renseigné n'est jamais touché. **Si le slug n'a aucune entrée dans `games.json`, le module n'en crée pas** — seul `add_game.py` du tracker le fait.
7. Sort toujours 0 ; toute erreur est journalisée sur stderr avec le préfixe `[tracker-md]` et n'interrompt jamais la fin de partie.

## `bin/sync --all` (manuel, synchronisation en masse)

Sans D-Bus : parcourt `$UNIVERSE_DATA_HOME/games/*/game.toml` (défaut `~/.local/share/universe/games`), ignore les jeux avec `removed_at`, additionne `duration_s` de chaque `sessions.jsonl` voisin (÷ 3600) et applique la même logique par jeu que le hook (une seule fonction, deux points d'entrée). Utile pour un premier remplissage avant que le cœur ne tourne en continu.

## Réglages (`[[settings]]`, scope global)

| Clé | Type | Défaut | Rôle |
|---|---|---|---|
| `root` | path | `~/Documents/notes/games` | racine du tracker (tables + `.data/` + `journal/`) |
| `create_missing` | bool | `false` | ajoute une ligne dans `playing.md` quand le jeu n'a de ligne dans aucune table |
| `update_metadata` | bool | `true` | complète les champs manquants de `.data/games.json[slug]` |
| `journal_link` | bool | `true` | écrit le lien Journal quand le dossier existe |

`MODULE_SETTINGS_JSON` écrase les défauts ; `root` n'est jamais expansé par le cœur (type `path`), le module fait son propre `os.path.expanduser`.

## Ordre des hooks — pourquoi le lien Journal arrive parfois en retard

`session-end` (ce module) tourne **avant** `SessionEnded` et avant `post-process` (module `journal`, qui écrit `journal/<slug>/*.md`). Sur la toute première partie enregistrée d'un jeu, le dossier `journal/<slug>/` n'existe donc pas encore au moment où `bin/sync` s'exécute : le lien atterrit à la session suivante, ou via `bin/sync --all` une fois le pipeline journal passé. Les Hours, eux, sont toujours à jour dès la fin de la partie (`sessions.jsonl` est écrit avant que les hooks `session-end` ne tournent).

## Ce que le module ne fait jamais

- Ne réordonne, ne resort et ne reformate pas une table existante : une ligne trouvée dans un mauvais ordre y reste, seules ses cellules Hours/Journal bougent.
- Ne touche jamais Vibes, Rating, ni la liste `wishlist.md`.
- Ne fait jamais baisser Hours.
- Ne crée jamais d'entrée dans `.data/games.json` : seuls les champs d'une entrée déjà présente sont complétés.
- N'écrit rien si rien n'a changé (pas de réécriture « no-op », mtime stable).
- N'échoue jamais la fin de partie : `bin/sync` sort toujours 0.
- Ne gère pas un `\|` échappé dans une cellule (le parsing coupe naïvement sur `|`) : aucune des six tables n'en contient aujourd'hui (vérifié), mais une ligne qui en aurait un serait ignorée silencieusement plutôt que corrompue.
