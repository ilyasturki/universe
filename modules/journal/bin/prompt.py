"""Brief, output schema and acceptance, ported from ~/NixOs/bin/game-session-summary.mjs.
The brief is French because it addresses the model; the entry comes back in the game's language."""
import re

from _common import JOURNAL_LANGUAGES, LABELS, LANG_NAMES, date_fr, duree_fr, heure_fr, journal_lang

CODEX_EFFORT = "high"
CODEX_VERBOSITY = "medium"

SYSTEM_PROMPT = "\n".join([
    "Tu tiens le journal de bord de jeu de l'utilisateur.",
    'Tu renvoies un objet structuré avec cinq champs : "title" (le titre de la session), "body" (le texte du journal), "next" (où le joueur reprendra), "images" (ton tri des images fournies) et "memory" (la mémoire du jeu).',

    "LANGUE, la règle qui prime sur toutes les autres : title, body, next et toute la mémoire s'écrivent DANS LA LANGUE DU JEU, celle affichée à l'écran sur les images. Un jeu en anglais donne une entrée entièrement en anglais, un jeu en français une entrée entièrement en français. Une entrée qui mélange deux langues est ratée, même si tout le reste est juste. Déduis la langue des images, et de la mémoire quand elle la précise ; si tu n'arrives pas à trancher, écris en anglais.",
    "Noms propres : personnages, lieux, boss, chapitres, quêtes, objets, attaques et équipes s'écrivent EXACTEMENT comme le jeu les affiche. Ne les traduis jamais, n'ajoute pas de traduction entre parenthèses, et ne les isole ni par des guillemets ni par de l'italique : le récit est déjà dans leur langue, ils s'y fondent. Si la mémoire contient un nom écrit dans une autre langue que celle du jeu, corrige-le vers l'orthographe affichée à l'écran et n'utilise plus l'ancienne forme.",

    "BUT : le joueur doit pouvoir relire l'entrée dans six mois et savoir ce qu'il a fait pendant cette session, sans relancer le jeu ni rouvrir l'enregistrement. Couvre tout ce qu'il a fait : quête principale, quêtes secondaires, contenu optionnel (exploration, collectibles, défis, achats, améliorations, mini-jeux). Chaque chose faite apparaît une fois, en une ligne, sans remplissage.",

    "CHAMP title : un titre court et concret pour CETTE session, de 3 a 6 mots, sans point final, sans guillemets, sans numéro ni date. Il nomme ce qui a défini la session (le lieu atteint, le boss battu, l'objectif accompli, la bascule de l'histoire) plutôt que le jeu lui-même, il ne dévoile rien de la suite, et il ne reprend pas le titre de la session précédente.",

    "CHAMP body, forme : deux formes possibles. Le profil du jeu, donné dans la mémoire (ou que tu établis toi-même à la première session), fixe la forme par défaut ; le contenu de CETTE session peut la faire basculer vers l'autre.",
    "Forme A, prose seule : un paragraphe de 2 a 5 phrases, aucune liste. C'est la forme par défaut d'un jeu \"arcade\", celle vers laquelle un jeu \"narrative\" bascule quand la session n'a fait avancer qu'une seule chose (farming, allers-retours, tentatives répétées sur un même boss, réglages, une seule course), et celle qu'impose une session faite d'un seul enchaînement.",
    "Test de la forme : demande-toi si tes puces pourraient être lues dans le désordre sans rien perdre. Quatre exploits, cinq circuits, trois quêtes menées en parallèle passent ce test, ce sont des choses séparables : forme B. Un procès, un donjon, une enquête dont chaque étape découle de la précédente ne le passe pas : la découper en puces supprime les \"donc\" et les \"ce qui\" qui la rendent lisible, alors raconte-la en prose, forme A.",
    "Forme B, prose puis liste : 1 a 3 phrases qui posent le déroulé et l'ambiance, puis une liste à puces. C'est la forme par défaut d'un jeu \"narrative\", et celle vers laquelle un jeu \"arcade\" bascule quand la session contient plusieurs choses distinctes à retenir (plusieurs coupes, plusieurs niveaux terminés, plusieurs déblocages).",
    "Liste de la forme B : une puce par chose faite, une seule ligne chacune, de la plus importante à la moins importante. Pas de sous-listes, entre 2 et 8 puces. Une catégorie sans rien dedans ne s'écrit PAS : on ne mentionne jamais qu'il ne s'est rien passé quelque part. S'il n'y a qu'une chose à dire, c'est la forme A.",
    "Étiquettes de la forme B : une puce s'ouvre par une étiquette en gras qui dit de quel TYPE d'activité elle relève, dans la langue du jeu : quête principale, quête secondaire, contenu optionnel, boss, exploration, ou la catégorie propre au jeu quand elle est plus juste (une arène, une course, un contrat, un donjon). Une étiquette ne nomme JAMAIS le sujet de la puce (l'objet trouvé, la personne interrogée, l'indice présenté, le circuit couru) : le sujet, c'est le travail de la puce.",
    "Étiquettes, quand ne pas en mettre : elles servent à distinguer les puces les unes des autres. Si toutes les puces de la liste porteraient la même étiquette, n'en mets aucune et laisse les puces nues ; une étiquette identique partout n'apprend rien.",
    "CHAMP body, interdits : pas de tiret long (— –) en guise de ponctuation, pas de titre (#), pas de bloc de code, pas de phrase d'introduction ni de méta-commentaire (\"Voici\", \"Dans cette session\"). Commence directement par le récit. N'écris pas la ligne de reprise dans le body : elle a son propre champ.",

    "CHAMP next : une seule phrase, sans étiquette ni gras (le journal ajoute l'étiquette lui-même), qui dit où le joueur en est et la prochaine action concrète à faire. Aucun spoiler : ni coupable, ni rebondissement, ni issue, ni événement que le joueur n'a pas encore atteint.",
    "CHAMP next, interdits : la phrase parle de la partie, jamais de la machine ni de l'enregistrement. Ne mentionne ni les images, ni la vidéo, ni un écran-titre, ni un menu de pause, ni le fait d'avoir quitté, sauvegardé, rechargé ou relancé le jeu. Le joueur sait rallumer sa console : pars directement de l'endroit du jeu où il en est resté, et donne la prochaine action à faire DANS le jeu.",
    "Quand des images sont marquées DERNIERS INSTANTS, c'est là que la session s'arrête vraiment : lis-y où le joueur en est resté, plutôt que de t'arrêter à la dernière action que tu identifies confortablement ailleurs. Une fin de niveau, un écran de résultats ou un retour à la carte s'y trouvent souvent et changent le next comme le body. L'une d'elles peut aussi être un fondu, un menu ou un écran-titre : ces écrans appartiennent à l'enregistrement et non à la partie, alors lis l'image d'avant et n'en parle nulle part dans l'entrée. Sans ces marques, déduis la fin de la session des images que tu as.",

    "STYLE : deuxième personne, ton décontracté, comme un joueur qui raconte et non comme une notice.",
    "Style, à bannir : les envolées et les superlatifs (\"un moment inoubliable\", \"une étape majeure de ton aventure\"), les formules creuses qui n'apprennent rien (\"tu continues ta progression\", \"l'aventure se poursuit\", \"une session riche en émotions\"), les tournures toutes faites (\"non seulement... mais aussi\", \"il convient de noter\", \"c'est là que tout bascule\") et les adjectifs passe-partout (riche, vibrant, emblématique, iconique, crucial, mythique). Un fait concret vaut mieux qu'un adjectif.",
    "Style, à faire : appuie chaque phrase sur quelque chose de vérifiable (un nom, un lieu, un score, un niveau, un objet obtenu). Varie les débuts de phrases et de puces. Une puce dit une seule chose : deux faits sans rapport reliés par \"et\" font deux puces, ou une puce et une coupe. Ne redis pas dans la liste ce que le paragraphe vient de dire, ni dans la puce ce que son étiquette dit déjà. Si une phrase n'ajoute rien à la précédente, supprime-la : court et complet vaut mieux que long.",

    "But du récit : raconte LA SESSION (ce qui a été fait, où le joueur en est, la progression), pas le contenu littéral de chaque image. Les captures ne sont que les instants que le joueur a jugés marquants, pas un journal exhaustif : relie-les par les actions, combats et étapes intermédiaires qui se sont logiquement déroulés entre elles, même si aucune image ne les montre. Reste confiant sur le déroulé général et ne nuance que les détails vraiment incertains.",
    "Inférence : nomme personnages, lieux, boss et chapitres dès que les images, la mémoire et ta recherche web te permettent de les identifier avec une confiance raisonnable ; ne te rabats sur une description générique que si tu n'y arrives vraiment pas. N'invente jamais un personnage ou un événement absent de cette session, et ne dévoile rien de la suite.",
    "Exception pour les comparses : les personnages secondaires et adversaires occasionnels (concurrents d'une épreuve, passants, marchands, PNJ de service) ne se nomment QUE si leur nom est lisiblement affiché à l'écran sur une image, ou déjà présent dans la mémoire fournie. Sinon reste générique. Un nom propre plausible mais non lu est une invention : ne le produis jamais.",
    "Le reste du texte affiché (dialogues, menus, sous-titres) : sers-t'en pour comprendre la scène et remplir la mémoire, mais ne le cite JAMAIS mot pour mot ; reformule-le au niveau de la session.",
    "Images fournies : les \"captures\" sont les moments que le joueur a jugés importants, les images \"auto-extraites de l'enregistrement\" sont échantillonnées automatiquement. Lis-les TOUTES ; en cas de doute sur ce qu'il faut raconter, privilégie les captures du joueur. Elles sont ton ancrage : appuie ton récit sur ce qu'elles montrent, puis comble par déduction raisonnable les transitions entre elles, sans jamais contredire ce qu'elles montrent.",
    "Images hors jeu : certaines images ne montrent pas le jeu (bureau, navigateur, lanceur, messagerie, autre application) ou ne montrent rien d'exploitable (écran de chargement, écran noir, fondu, transition floue, menu vide, message d'erreur). Elles ne font PAS partie de la session : ne les raconte pas, ne les mentionne pas, et ne t'en sers pas pour deviner ce qui se passait.",
    "CHAMP images : tu tries les images pour la galerie du journal, en te servant des numéros qu'on te donne. \"gallery\" liste les numéros des images exploitables, classées de la meilleure à la moins bonne pour illustrer cette session : une bonne image montre le jeu dans une scène lisible et représentative (un lieu, un personnage, un combat, une cinématique, un moment fort). \"unusable\" liste les numéros des images hors jeu ou inexploitables décrites ci-dessus. Chaque numéro fourni apparaît dans exactement une des deux listes.",
    "Repérage : si le jeu a une progression identifiable, sers-toi de la recherche web (soluce, wiki, walkthrough) pour situer précisément où en est le joueur, nommer correctement lieux, personnages, boss et chapitres, et distinguer une quête principale d'une quête secondaire. Pour un jeu sans progression linéaire (bac à sable, jeu musical, abstrait) ou sans soluce trouvable, ignore cette étape. N'y puise jamais un événement que le joueur n'a pas encore atteint et ne spoile pas la suite.",
    f"CHAMP memory : tiens à jour la continuité, dans la langue du jeu. \"synopsis\" est un résumé courant de l'histoire jusqu'ici (quelques phrases, en intégrant cette session). \"entities\" recense personnages, lieux et boss rencontrés avec leur orthographe canonique, celle du jeu. \"language\" est le CODE à deux lettres de la langue du jeu, parmi {', '.join(JOURNAL_LANGUAGES)} ; prends le plus proche si le jeu est dans une autre langue. \"profile\" vaut \"narrative\" ou \"arcade\" et décrit LE JEU, pas cette session : une fois établi il ne change plus, sauf erreur manifeste. Réutilise et complète les noms déjà connus qu'on te fournit, sans les réécrire autrement (à la seule exception d'un nom à corriger vers la langue du jeu).",
    "CHAMP memory, règle de non régression : le synopsis ne se réécrit pas plus court. Reprends TOUS les éléments du synopsis qu'on te fournit (étapes, badges, lieux traversés, personnages clés) et contente-toi d'y intégrer la session courante. Tu peux reformuler, jamais résumer ni élaguer : un synopsis plus court que celui reçu est une perte de mémoire.",
])

PROFILE_SHAPES = {
    "narrative": "forme B (1 a 3 phrases puis une liste à puces)",
    "arcade": "forme A (prose seule, 2 a 5 phrases)",
}

# Every property is required: codex's --output-schema runs in strict mode.
OUTPUT_SCHEMA = {
    "type": "object",
    "additionalProperties": False,
    "required": ["title", "body", "next", "images", "memory"],
    "properties": {
        "title": {"type": "string", "description": "Titre court de la session, 3 a 6 mots, sans point final."},
        "body": {"type": "string", "description": "Le corps du journal, dans la langue du jeu : forme A (prose seule) ou forme B (prose puis liste à puces). Sans la ligne de reprise."},
        "next": {"type": "string", "description": "Où le joueur en est et sa prochaine action concrète, une phrase, sans étiquette ni gras."},
        "images": {
            "type": "object",
            "additionalProperties": False,
            "required": ["gallery", "unusable"],
            "properties": {
                "gallery": {"type": "array", "items": {"type": "integer"}, "description": "Numéros des images exploitables, de la meilleure à la moins bonne."},
                "unusable": {"type": "array", "items": {"type": "integer"}, "description": "Numéros des images hors jeu ou inexploitables."},
            },
        },
        "memory": {
            "type": "object",
            "additionalProperties": False,
            "required": ["synopsis", "entities", "language", "profile"],
            "properties": {
                "synopsis": {"type": "string"},
                "entities": {
                    "type": "object",
                    "additionalProperties": False,
                    "required": ["characters", "places", "bosses"],
                    "properties": {
                        "characters": {"type": "array", "items": {"type": "string"}},
                        "places": {"type": "array", "items": {"type": "string"}},
                        "bosses": {"type": "array", "items": {"type": "string"}},
                    },
                },
                "language": {"type": "string", "enum": JOURNAL_LANGUAGES, "description": "Code à deux lettres de la langue du jeu à l'écran."},
                "profile": {"type": "string", "enum": ["narrative", "arcade"]},
            },
        },
    },
}


def system_prompt(forced_lang=None):
    if not forced_lang:
        return SYSTEM_PROMPT
    code = journal_lang(forced_lang)
    name = LANG_NAMES[code][0]
    return SYSTEM_PROMPT + "\n" + (
        f"LANGUE IMPOSÉE par l'utilisateur, qui remplace la règle LANGUE ci-dessus : title, body, next et toute la mémoire "
        f"s'écrivent en {name} ({code}), quelle que soit la langue affichée à l'écran ; memory.language vaut \"{code}\". "
        "Les noms propres gardent l'orthographe affichée par le jeu."
    )


def image_label(im):
    if im.kind != "frame":
        return "capture (moment que tu as marqué)"
    if im.tail:
        return "image auto-extraite de l'enregistrement, DERNIERS INSTANTS de la session"
    return "image auto-extraite de l'enregistrement"


# Binds by position: same order as the -i flags.
def image_lines(images, with_paths=False):
    lines = []
    for i, im in enumerate(images, 1):
        line = f"- image {i} : {image_label(im)}, vers {heure_fr(im.t)}"
        if with_paths:
            line += f", fichier {im.file}"
        lines.append(line)
    return lines


def memory_context(memory):
    if not memory:
        return None
    e = memory.get("entities") or {}

    def line(label, arr):
        return f"{label} : {', '.join(arr)}" if arr else None

    shape = PROFILE_SHAPES.get(memory.get("profile"))
    parts = [
        "Mémoire du jeu (réutilise et complète, ne te contredis pas) :",
        f"Synopsis : {memory['synopsis']}" if memory.get("synopsis") else None,
        f"Langue du jeu, dans laquelle tu écris TOUT : {memory['language']}" if memory.get("language") else None,
        f"Profil du jeu : {memory['profile']}, forme par défaut {shape}." if shape else None,
        line("Personnages", e.get("characters")),
        line("Lieux", e.get("places")),
        line("Boss", e.get("bosses")),
    ]
    return "\n".join(p for p in parts if p)


def build_user_prompt(title, start, end, duration_s, session_number, total_sec, images, prev, memory, image_intro, lines):
    mem_ctx = memory_context(memory)
    ordinal = "1re" if session_number == 1 else f"{session_number}e"
    no_memory_note = (
        "C'est ta première entrée pour ce jeu : recherche brièvement le jeu sur le web pour établir la prémisse et l'orthographe des noms principaux."
        if session_number == 1 else
        "Aucune mémoire de jeu n'a encore été enregistrée, mais ce n'est PAS ta première session sur ce jeu (vois l'historique ci-dessus et l'entrée précédente) : ne présente donc pas cette session comme un début ou un redémarrage, et recherche brièvement le jeu sur le web pour la prémisse et l'orthographe des noms."
    )
    profile = (memory or {}).get("profile")
    return "\n".join([
        f"Jeu : {title}",
        f"Session : {date_fr(start)}, de {heure_fr(start)} a {heure_fr(end)} ({duree_fr(duration_s)})",
        f"Historique : {ordinal} session sur ce jeu, temps de jeu cumulé d'environ {duree_fr(total_sec)}.",
        "",
        mem_ctx or no_memory_note,
        "",
        f'Entrée précédente du journal (pour la continuité, ne la répète pas) :\n"""\n{prev}\n"""' if prev else "",
        "",
        image_intro,
        *lines,
        "",
        f"Profil déjà établi pour ce jeu : {profile} (forme par défaut {PROFILE_SHAPES.get(profile, PROFILE_SHAPES['arcade'])}). Garde-le dans la mémoire, et ne bascule vers l'autre forme que si le contenu de CETTE session le justifie."
        if profile else
        "Aucun profil n'est encore enregistré pour ce jeu : détermine s'il est \"narrative\" ou \"arcade\", écris-le dans la mémoire, et écris cette entrée dans la forme correspondante.",
        "",
        "Situe d'abord cette session dans la progression du jeu (sers-toi du web et d'une soluce si le jeu en a une), puis trie les images, donne un titre à la session, rédige l'entrée du journal et mets à jour la mémoire.",
    ])


# --- acceptance of the model's fields --------------------------------------------

COLONS = r"[ \u00a0\u202f]?[:\uff1a]"


def _any_label(key):
    return "|".join(re.escape(v) for v in dict.fromkeys(l[key] for l in LABELS.values()))


NEXT_UP_RE = re.compile(rf"^\*\*(?:{_any_label('next')}){COLONS}\*\*\s*(.+?)\s*$", re.M)
STALL_OPENER = re.compile(
    r"^\s*(?:je\s+(?:vais|commence|dois|vérifie|verifie)|i(?:'|’)?(?:ll|m going to|m about to| will| am going to| need to)|i\s+will|let me|first,? let me)\b",
    re.I,
)
PREAMBLE_RE = re.compile(
    r"^\s*(?:voici|voil[àa]|here(?:'s| is)|this is)[^\n]*(?:journal|entrée|entree|résumé|resume|session|entry|summary)[^\n]*[ \u00a0\u202f]?[:\uff1a]\s*\n+",
    re.I,
)


def dedash(text):
    t = re.sub(r"[ \t]*[—–―][ \t]*", ", ", text)
    t = re.sub(r",\s*,", ",", t)
    return re.sub(r"[ \t]+$", "", t, flags=re.M)


def sanitize_body(text):
    t = str(text or "").strip()
    t = re.sub(r"^```[a-zA-Z]*\n?", "", t)
    t = re.sub(r"\n?```$", "", t).strip()
    t = PREAMBLE_RE.sub("", t, count=1).strip()
    return dedash(t).strip()


def accept_body(raw):
    body = sanitize_body(raw)
    if STALL_OPENER.match(body):
        return None
    body = NEXT_UP_RE.sub("", body).rstrip()
    return body or None


def accept_next(raw):
    t = re.sub(r"\s+", " ", dedash(str(raw or "").strip())).strip()
    return NEXT_UP_RE.sub(r"\1", t).strip()


def accept_title(raw):
    t = str(raw or "").split("\n")[0].strip()
    t = re.sub(r"^#+\s*", "", t)
    t = re.sub(r"^\*+|\*+$", "", t).strip()
    t = re.sub(r"^[\"“«‹']+|[\"”»›']+$", "", t).strip()
    t = re.sub(r"\s+", " ", dedash(t))
    t = re.sub(r"\s*[.…]+$", "", t).strip()
    return t[:80]


def accept_result(out):
    """-> {title, body, next, images, memory} | None when the model narrated its work instead."""
    if not isinstance(out, dict) or not out.get("body"):
        return None
    body = accept_body(out.get("body"))
    nxt = accept_next(out.get("next"))
    if not body or not nxt:
        return None
    return {
        "title": accept_title(out.get("title")),
        "body": body,
        "next": nxt,
        "images": out.get("images") if isinstance(out.get("images"), dict) else None,
        "memory": out.get("memory") if isinstance(out.get("memory"), dict) else None,
    }


def paragraphs_from_body(body):
    paras, cur = [], []

    def flush():
        if cur:
            paras.append(" ".join(cur).strip())
            cur.clear()

    for line in str(body or "").splitlines():
        s = line.strip()
        if not s:
            flush()
            continue
        m = re.match(r"^(?:[-*•]|\d+[.)])\s+(.*)$", s)
        if m:
            flush()
            paras.append("- " + m.group(1).strip())
        else:
            cur.append(s)
    flush()
    return [p for p in paras if p]


# --- per-game memory -------------------------------------------------------------

# The prompt forbids shortening the synopsis; this catches the rewrites that do anyway.
SYNOPSIS_SHRINK_FLOOR = 0.8


def pick_synopsis(old, new, warn=None):
    o = (old or "").strip()
    n = (new or "").strip()
    if not n:
        return o
    if o and len(n) < len(o) * SYNOPSIS_SHRINK_FLOOR:
        if warn:
            warn(f"synopsis rewrite shrank {len(o)} -> {len(n)} chars; keeping the previous one")
        return o
    return n


def merge_memory(old, new, warn=None):
    if not new:
        return old

    def union(a, b):
        seen = {}
        for x in list(a or []) + list(b or []):
            v = str(x).strip()
            if v and v.lower() not in seen:
                seen[v.lower()] = v
        return list(seen.values())

    oe = (old or {}).get("entities") or {}
    ne = new.get("entities") or {}
    old_profile = (old or {}).get("profile")
    profile = old_profile if old_profile in PROFILE_SHAPES else (new.get("profile") if new.get("profile") in PROFILE_SHAPES else None)
    if old_profile and new.get("profile") and old_profile != new.get("profile") and warn:
        warn(f"model proposed profile {new['profile']!r} over the stored {old_profile!r}; keeping the stored one")
    mem = {
        "synopsis": pick_synopsis((old or {}).get("synopsis"), new.get("synopsis"), warn),
        "entities": {
            "characters": union(oe.get("characters"), ne.get("characters")),
            "places": union(oe.get("places"), ne.get("places")),
            "bosses": union(oe.get("bosses"), ne.get("bosses")),
        },
        "language": journal_lang(new.get("language") or (old or {}).get("language")),
    }
    if profile:
        mem["profile"] = profile
    return mem
