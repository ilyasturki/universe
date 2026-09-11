# Module `journal`

Hook `post-process` : après chaque partie, une entrée de journal (JSON, champs fixes) rédigée par un modèle à partir des captures de la session et d'images extraites de l'enregistrement, déposée au cœur par `universe journal-add`, puis, si `markdown_export`, la note Obsidian du jeu rendue dans `<journal_root>/<id>/`. Port de `~/NixOs/bin/game-session-summary.mjs` (le prompt, la sélection d'images et la mémoire par jeu sont repris tels quels ; la plomberie handoff/unités est abandonnée, voir la fin).

Python 3 stdlib + `ffmpeg`/`ffprobe` ; aucun code chargé par le cœur.

```
modules/journal/
  module.toml
  bin/process      hook post-process
  bin/render       note Markdown depuis les entrées JSON (idempotent)
  bin/migrate      note Markdown existante -> entrées JSON
  bin/_common.py   réglages, dates, libellés par langue, slug, sessions.jsonl, appel de la CLI
  bin/images.py    captures de la session, extraction et tri des images
  bin/prompt.py    SYSTEM_PROMPT, schéma de sortie, brief, acceptation, mémoire
  bin/providers.py codex | claude | stub
  bin/note.py      rendu et analyse de la note
  tests/test_journal.py
```

## Réglages

| Clé | Type | Portée | Défaut | Rôle |
|---|---|---|---|---|
| `enabled` | bool | jeu | `true` | écrire une entrée après la partie |
| `language` | enum `auto fr en es de it pt ja` | jeu | `auto` | `auto` = la langue du jeu à l'écran (règle du prompt) ; sinon imposée au modèle et à la note |
| `provider` | enum `codex claude stub` | global | `codex` | qui rédige |
| `model` | string | global | `gpt-5.6-sol` | modèle passé au fournisseur (`-sol` obligatoire pour l'auth ChatGPT de codex) |
| `markdown_export` | bool | global | `true` | rendre la note Obsidian |
| `journal_root` | — | — | `paths.journal_root` du cœur (`UNIVERSE_JOURNAL_ROOT`) | dossier des notes, `<journal_root>/<id>/<Titre>.md` ; surchargeable dans `[modules.journal]` |
| `max_images` | int | global | `40` | plafond d'images envoyées au modèle |

`requires.bins = ["ffmpeg", "codex"]` : avec `provider = "claude"` le cœur déclarera quand même le module indisponible sans `codex` (question ouverte ci-dessous).

## Déroulé de `bin/process`

Env lu : `GAME_ID` (repli `GAME_SLUG`, puis slug du titre), `GAME_TITLE`, `SESSION_ID`, `SESSION_STARTED_AT`/`ENDED_AT`/`DURATION_S` (repli : l'id de session et la durée), `RECORDING_PATH` (vide = captures seules), `JOURNAL_DIR`, `MODULE_DATA_DIR`, `MODULE_SETTINGS_JSON`, `UNIVERSE_BIN` (la CLI à rappeler), `UNIVERSE_GAME_JSON` (repli pour les statistiques).

1. `enabled = false` → sortie 0. `JOURNAL_DIR/<session>.json` déjà présent → sortie 0 (`--force` régénère).
2. Mur de quota codex (`MODULE_DATA_DIR/codex-limit.json`, posé par un run précédent) encore actif → sortie 75 sans toucher à ffmpeg.
3. Captures : `JOURNAL_DIR/attachments/AAAAMMJJ-HHMMSS.{png,jpg}` dont l'horodatage tombe dans `[début − 90 s, fin + 120 s]`.
4. Images de l'enregistrement, seulement pour compléter jusqu'à `min(20, max_images)` : les 45 dernières secondes (ou le quart de la durée) sont réservées à 3 images « derniers instants », le reste est réparti au prorata des intervalles entre captures ; 4 candidats par place, extraits en une passe `ffmpeg` chacun (PNG bord long 1568 px + un gris 9×8 dont on tire l'écart-type et un dHash 64 bits), images plates écartées (σ < 4), choix par tranche de temps puis distance de Hamming, doublons < 4 bits retirés. 4 extractions en parallèle.
5. Aucune image → entrée courte `provider = "none"` (le libellé « rien à résumer » dans la langue connue) ; pas d'appel au modèle.
6. Brief : `SYSTEM_PROMPT` + jeu, date/heures/durée, rang de la session et temps cumulé (depuis `games/<id>/sessions.jsonl`, sinon `UNIVERSE_GAME_JSON.stats`, sinon le nombre d'entrées), mémoire du jeu, entrée précédente (paragraphes, 1500 caractères), une ligne par image (capture / auto-extraite / derniers instants, heure), consigne sur le profil `narrative`/`arcade`. `language ≠ auto` ajoute une règle qui impose la langue.
7. Sortie du modèle validée : préambule et clôtures retirés, tirets longs → virgules, un « je vais vérifier… » rejeté comme non-entrée, `next` sans étiquette, titre sur une ligne ≤ 80 caractères. Verdict `images.gallery`/`unusable` normalisé (numéros hors bornes ou contradictoires ignorés, les images non citées restent utilisables en dernier).
8. Galerie : captures gardées (jamais toutes rejetées) + images extraites dans l'ordre du modèle jusqu'à 6 au total, une place tenue pour une image des derniers instants. Les images gardées sont copiées en `JOURNAL_DIR/attachments/<session>-<n>.png` (les anciennes `<session>-*.png` sont supprimées avant) ; le nom ne matche pas le motif des captures, un re-run ne se les réinjecte pas.
9. Entrée construite (ci-dessous) et déposée : `$UNIVERSE_BIN journal-add <session> <json>`. CLI absente ou en échec → `JOURNAL_DIR/<session>.json` écrit directement, avertissement, sortie 0 (le cœur réindexe les fichiers). Réponse `invalid:` → rien d'écrit, sortie 1.
10. Mémoire fusionnée et écrite (seulement après un dépôt réussi) dans `MODULE_DATA_DIR/memory/<id>.json` (amorcée depuis l'ancien `<journal_root>/<id>/.game-memory.json` s'il n'y a rien d'autre) : union des entités, synopsis conservé s'il rétrécit de plus de 20 %, profil figé à la première lecture, langue relue à chaque session.
11. `markdown_export` → rendu de la note (voir `bin/render`). Le dossier de travail `MODULE_DATA_DIR/work/<session>-*/` est supprimé en sortie.

Codes de retour : 0 entrée écrite ou rien à faire ; 1 modèle sans entrée exploitable ou entrée refusée par le cœur (l'entrée manquante est le signal, `process --force` sous le même env la relance) ; 75 quota codex atteint, entrée à reprendre plus tard.

## Fournisseurs (`providers.py`)

**codex** (défaut). Invocation identique au script zx :

```
codex exec --skip-git-repo-check --ignore-user-config --disable browser_use --disable computer_use --ephemeral
  -C <work_dir> -s read-only -c approval_policy=never -c model=<model> -c model_reasoning_effort=high
  -c model_verbosity=medium -c project_doc_max_bytes=0 -c tools.web_search=true -c mcp_servers={}
  -i <image>… --output-schema <work_dir>/schema.json -o <work_dir>/entry.json <SYSTEM_PROMPT + brief>
```

cwd et `-C` = le dossier de travail vide sous `MODULE_DATA_DIR` (codex remonte les `AGENTS.md` depuis le cwd ; `~/Documents/AGENTS.md` est une fiche personnelle). Délai 900 s, 2 tentatives. `hit your usage limit` → `codex-limit.json` avec l'instant de reprise lu dans le message (« try again at … », sinon +6 h) et sortie 75.

**claude**. `claude -p --output-format json --json-schema <schéma> --restricted --tools Read,WebSearch --allowedTools Read WebSearch --strict-mcp-config --no-session-persistence --add-dir <work_dir> --append-system-prompt <SYSTEM_PROMPT> [--model <model>] <brief>` ; les images sont listées par chemin dans le brief et lues par l'outil Read, l'objet validé est lu dans `structured_output`. Le script zx n'avait plus ce chemin ; écrit d'après `claude --help` 2.1, non exercé en réel.

**stub**. Entrée déterministe sans réseau (titre « Stub session of <jeu> », un paragraphe puis deux puces, toutes les images utilisables, mémoire `arcade`), pour les tests.

## Schéma de l'entrée

Exactement `Journal1.Entry` d'api.md, sans clé supplémentaire :

```json
{"session":"20260424-113452","game":"mario-kart-8-deluxe","written_at":"2026-09-11T16:20:16+02:00",
 "lang":"en","title":"DK's Snowboard Cross Trial","provider":"codex",
 "paragraphs":["After looking through the Star Cup…","- **Boss:** …"],
 "next_up":"Finish the lap and try to beat Nin★Fausti's 2:22.680 ghost.",
 "images":["attachments/20260424-113452-1.png","attachments/20260424-113452-2.png"]}
```

- `paragraphs` : le corps découpé sur les lignes vides ; chaque puce devient un paragraphe qui commence par `- ` (les étiquettes en gras restent dans le texte). L'interface affiche des chaînes nues.
- `provider` : `codex`, `claude`, `stub`, `none` (session sans image), `import` (entrée migrée).
- `images` : chemins relatifs à `JOURNAL_DIR` ; captures `attachments/AAAAMMJJ-HHMMSS.png`, images extraites `attachments/<session>-<n>.png` (anciennes notes : `attachments/frames/frame-<session>-NN.jpg`).
- `written_at` : RFC 3339 local.

L'entrée ne porte ni fin, ni durée, ni chemin d'enregistrement : le rendu les lit dans `games/<id>/sessions.jsonl` (à côté de `journal/`), puis dans `journal/.migrated-sessions.jsonl` pour ce qui vient d'une migration.

## Rendu Markdown (`bin/render`)

`render [<id>] [--journal-dir D] [--journal-root R] [--title T] [--sessions F]` ; sans argument, env `GAME_ID`, `JOURNAL_DIR`, `GAME_TITLE`, `MODULE_SETTINGS_JSON`. Titre : `--title`, `GAME_TITLE`, `games/<id>/game.toml`, frontmatter de la note existante, l'id. Fichier : `<journal_root>/<id>/<Titre>.md` (`gameNoteName` : `/ \ : # ^ [ ] |` → espace), ou l'unique `.md` du dossier qui porte un marqueur `<!-- session:` (les notes actuelles s'appellent souvent `<Titre> (journal).md`). Réécrit seulement si le contenu change ; le chemin est imprimé sur stdout.

Format, identique aux notes existantes (parse → rendu reproduit octet pour octet 12 des 34 notes réelles, les 22 autres ne diffèrent que par une ligne vide sous le titre ou les guillemets du frontmatter) :

```
---
game: "Dishonored: Definitive Edition"      (guillemets JSON seulement si YAML l'exige)
sessions: 3
first_played: 2024-03-09                     (plus ancien / plus récent marqueur)
last_played: 2026-04-25
cover: attachments/20260425-000349.png       (première image du document)
---

# Journal: <Titre>                           (libellé dans la langue de l'entrée la plus récente)

## #N · <Titre de session>                   (N = index NNN du nom de l'enregistrement, sinon rang parmi
*JJ/MM/AAAA · HHhMM à HHhMM · 1 h 23 min*     les sessions enregistrées ; pas de numéro sans enregistrement ;
<!-- session: AAAAMMJJ-HHMMSS -->             sans titre, la ligne de métadonnées monte dans le `## `)

<paragraphes ; les `- ` consécutifs forment une liste ; provider none → en italique>

**Next up:** <next_up>                       (Reprise :, Retomar:, Weiter:, Ripresa:, 次回：)

**Recording:** [<nom>.mkv](file:///…)       (Enregistrement :, …)

![](attachments/AAAAMMJJ-HHMMSS.png)         (captures)

*Frames from the recording*                  (Images extraites de l'enregistrement, …)

![](attachments/<session>-1.png)
```

Entrées de la plus récente à la plus ancienne. Les images référencées sont copiées de `JOURNAL_DIR/<rel>` vers `<dossier de la note>/<rel>` si elles n'y sont pas déjà (Obsidian ne suit pas un lien qui sort du coffre) ; celles des anciennes notes y sont déjà.

## Migration (`bin/migrate`)

`migrate <note.md> <journal-dir> [--game <id>] [--no-images] [--force]` découpe la note sur ses `## ` et marqueurs et écrit `<journal-dir>/<session>.json` par bloc (`--game` = id, sinon slug du titre). Formes reconnues : en-tête titré + ligne italique, en-tête ancien `## #9 · 22/12/2025 · 22h19 à 22h25 · 6 min`, bloc virtuel sans numéro, bloc sans corps (marqueur + lien d'enregistrement), texte italique « rien à résumer » (→ `provider none`), libellés fr/en/es/de/it/pt/ja mêlés dans une même note. `lang` est lu sur les libellés du bloc (enregistrement, puis légende des images, puis reprise), `written_at` = fin de session (la note n'a pas d'autre horloge), `provider = "import"`. Les images référencées sont copiées depuis le dossier de la note vers `<journal-dir>/<rel>`. Les spans (début, fin à la minute, durée, chemin du mkv) vont dans `<journal-dir>/.migrated-sessions.jsonl` (`source: "import-journal"`), que `render` lit en repli et que le cœur peut absorber. Fichiers existants ignorés sans `--force`.

Vérifié sur des copies de notes réelles dans le scratchpad : `migrate` puis `render` reproduit `Cuphead.md` (7 sessions, fr + en) à l'identique.

## Tests

```
nix shell --impure --expr 'let pkgs = import <nixpkgs> {}; in pkgs.python3.withPackages (ps: [ ps.pytest ])' -c python3 -m pytest modules/journal -q
```

Sélection des captures (PNG synthétiques, fenêtre et fichiers étrangers), extraction depuis un mkv `testsrc` (ordre, queue, PNG écrits, hachages distincts), dédoublonnage (`smptebars` → 1 image, noir → 0), stub de bout en bout via `bin/process` sous-processus avec un `universe` factice (entrée validée champ par champ, images copiées, mémoire, note, second run sans effet, remise au cœur sans fichier, refus `Error.Invalid`, `enabled=false`, langue imposée), aller-retour `render` → `migrate` → mêmes entrées et sessions, formes anciennes, composition exacte des arguments `codex exec` et mur de quota (subprocess remplacé).

## Ce qui a été abandonné du script zx, et pourquoi

- File d'attente de handoffs, verrous de run et de journal, unités `%i`, compteur de tentatives, notifications `NOTIFY_HELPER`, `pegasus-sync` : le cœur déclenche le hook une fois par session avec l'env qu'il faut, la ré-exécution est `process --force`, l'interface écoute `EntryWritten`.
- Liste `.no-journal` : c'est le réglage `enabled` par jeu.
- Seconde passe de tri des images de remplacement (`reviewImages`) et réserve de candidats : elle ne servait qu'à combler une galerie courte quand le modèle rejetait des images, au prix d'un second appel ; la galerie reste plus courte dans ce cas.
- ImageMagick : écart-type et hachage sortent de la même passe `ffmpeg` (gris 9×8), dHash à la place de l'aHash.
- Ancres `captureStart`/`sessionEnd` reconstruites depuis le mtime du mkv : le cœur donne début, fin et durée ; les décalages dans le mkv partent de `SESSION_STARTED_AT` (une session dont le portail a retardé la capture décalera l'heure affichée des images extraites, la durée sondée par `ffprobe` limite l'échantillonnage à ce que le fichier contient).
- Numérotation des entrées d'après l'index NNN du recorder : conservée quand le nom du mkv en porte un, rang parmi les sessions enregistrées sinon (les mkv classés par `universe recording-file` s'appellent `<session>.mkv`).
- Retrait de la ligne « **Reprise :** » des anciennes notes, renumérotation : hors périmètre, c'est la note rendue qui fait foi désormais.

## Questions ouvertes pour le cœur

1. `Journal1.AddEntry` « copie les pièces jointes listées » : depuis où ? Le module dépose déjà `images` dans `JOURNAL_DIR/attachments/` et passe des chemins relatifs à `JOURNAL_DIR` ; la copie doit être un no-op sur un chemin déjà en place.
2. `Journal1.Render(id)` : appelle-t-il `bin/render` du module (avec `GAME_ID`, `GAME_TITLE`, `JOURNAL_DIR`, `MODULE_SETTINGS_JSON`) ou réimplémente-t-il le rendu ? Le module suppose le premier.
3. `requires.bins` inclut `codex` ; un utilisateur en `provider = "claude"` sans codex verra le module indisponible. Soit le cœur accepte une liste de binaires alternative, soit `codex` sort de `bins` et le hook signale lui-même l'absence.
4. `sessions.jsonl` est lu à `dirname(JOURNAL_DIR)/sessions.jsonl` ; le hook injecte sa propre session depuis l'env si elle n'y est pas encore. Les spans migrés (`.migrated-sessions.jsonl`, `source: "import-journal"`) attendent d'être absorbés dans `sessions.jsonl` à la migration des notes.
5. Le hook rend 75 sur mur de quota et 1 sur échec du modèle sans rien écrire : le cœur voudra peut-être relancer `post-process` plus tard pour ces sessions (aujourd'hui : `process --force`).
6. Le cœur doit ignorer `journal/.migrated-sessions.jsonl` et `*.json.tmp` en réindexant `journal/*.json` (les entrées s'appellent strictement `AAAAMMJJ-HHMMSS.json`).
7. (v3) L'essai réel avait utilisé un nom de bus factice pour retomber sur l'écriture directe ; en v4 le module rappelle `UNIVERSE_BIN`, l'écriture directe reste le repli.

## Essai réel codex (frontière prouvée)

`/mnt/recordings/games/mario-kart-8-deluxe/089-20260424-113452-2m.mkv` (4K AV1, 173 s, aucune capture), `JOURNAL_DIR` et `MODULE_DATA_DIR` dans le scratchpad, bus absent : 20 images extraites (80 candidates, ~1 min), codex `gpt-5.6-sol` ~2 min, entrée `en` « DK's Snowboard Cross Trial », un paragraphe (Time Trials sur Wii DK's Snowboard Cross, fantôme Nin★Fausti 2:22.680, deux champignons, lap 3), `next_up` cohérent avec la dernière image, 6 images en galerie, mémoire `arcade` écrite, note rendue avec `## #89 · …` (index lu dans le nom du mkv). Durée totale 2 min 59 s.
