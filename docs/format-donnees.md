# Format des données

Tous les fichiers sont en UTF-8. Les lignes vides et celles qui commencent par
`#` sont ignorées. Les tables sont en TSV (tabulation entre les colonnes). Les
apostrophes droites sont converties en apostrophes typographiques (’) au
chargement.

## `data/lexique/*.tsv` — lexiques

| Colonne | Exemple | Remarques |
|---|---|---|
| forme | `mangé` | Telle qu'elle s'écrit, casse comprise (`Paris`). |
| lemme | `manger` | |
| catégorie | `VER` | `NOM`, `NPR`, `ADJ`, `VER`, `AUX`, `ADV`, `PRE`, `CON`, `DET`, `PRO`, `INT`, `ABR`. |
| traits | `ppas.m.s` | Voir ci-dessous ; vide si sans objet. |
| fréquence | `80` | Occurrences par million de mots. |
| source | `CONTRIB-INIT` | Code défini dans `data/SOURCES.tsv`. |

Une forme homographe a une ligne par lecture (`fait` : verbe conjugué,
participe, nom). Plusieurs fichiers peuvent coexister ; ils sont fusionnés et
les fréquences d'une même forme s'additionnent. Les fichiers importés (voir
`data/lexique/LISEZMOI.md`) ne se modifient pas à la main.

**Traits.** Genre et nombre pour les noms, adjectifs, déterminants :
`m`, `f`, `e` (épicène) puis `s`, `p`, `e` (invariable), par exemple `f.p`.
Verbes : `inf` (infinitif), `ppre` (participe présent), `ppas[.genre.nombre]`
(participe passé), `mode.temps.personne` sinon, avec les modes `ind`, `sub`,
`imp`, `cond` et les temps `pres`, `impf`, `ps`, `fut`, par exemple
`ind.pres.3s`. Pronoms personnels : personne et nombre (`1s`, `3`).

## `data/regles/regles.tsv` — description des règles

`identifiant`, `catégorie`, `message`, `explication`, `source`.

Les marqueurs `{0}`, `{1}`… sont remplacés par les valeurs fournies par la
règle (en général le texte trouvé, puis la correction). Tout identifiant
utilisé par le code ou par `locutions.tsv` doit figurer ici.

## `data/regles/locutions.tsv`

`locution fautive`, `correction`, `identifiant de règle`, `source`.

Les mots de la locution sont séparés par des espaces ; une élision est un mot
à part : `d’ ou` reconnaît « d’ou ». `=lemme` reconnaît toutes les formes d'un
lemme (`=pallier à` reconnaît « palliait à »), et `$1`, `$2`… reprennent dans
la correction le texte du 1er, 2e… mot trouvé (`$1 chez le docteur`). La
comparaison ignore la casse ; la majuscule initiale du texte est reportée sur
la correction.

## Tables de grammaire

- `determinants.tsv` : `forme`, `genre.nombre`, `autre genre`, `autre nombre`,
  `source`. Classe fermée décrite pour le projet, plutôt que tirée du lexique
  (Lexique 3.83 donne « l’ » comme masculin seulement). `-` : pas de contrepartie.
- `pronoms_sujets.tsv` : `pronom`, `personne` (`1s`…`3p`), `genre` et `nombre`
  imposés au participe ou à l'attribut (`e` : indifférent), `source`.
- `mots_grammaticaux.tsv` : `mot`, `classe`, `source`. Classes : `clitique`
  (peut se placer entre sujet et verbe), `objet` (complément d'objet),
  `charniere` (peut ouvrir une proposition), `adverbe` (peut s'intercaler
  entre auxiliaire et participe), `negation` (pas, jamais…), `antecedent`
  (que, combien : complément placé avant le verbe).
- `h_aspire.txt` : mots ou lemmes à h aspiré, devant lesquels on n'élide pas.
- `noms_deux_genres.txt` : noms employés aux deux genres, dont le genre n'est
  pas vérifié (épicènes et homographes que Lexique décrit mal).

## Listes simples

- `elisions.txt` : formes élidées qui se soudent au mot suivant (`qu’`).
- `abreviations.txt` : abréviations dont le point ne termine pas la phrase
  (sensible à la casse : `M`, `etc`).
- `verbes_a.txt` : lemmes de verbes construits avec « à » + infinitif.
- `ligatures.txt` : radicaux écrits avec « œ » (cœur, sœur, œuvre…). Sert à
  l'import de Lexique 3.83, qui écrit « oe ». À la recherche, « oe » saisi à la
  place de « œ » est accepté (« soeur » est reconnu).

## Corpus — `corpus/fr/**/*.tsv`

`étiquette`, `phrase annotée`, `source`.

- L'étiquette est un identifiant de règle (`corpus/fr/regles`) ou une
  catégorie (`corpus/fr/evaluation`).
- Une erreur attendue s'écrit `⟦texte fautif→correction⟧`, ou
  `⟦texte fautif⟧` si la correction n'est pas vérifiée. Une correction vide
  (`⟦ →⟧`) signifie « supprimer ».
- `{nbsp}` : espace insécable U+00A0 ; `{fine}` : espace fine insécable U+202F.
- Une ligne sans annotation est une phrase qui ne doit produire **aucun**
  signalement.

Dans `corpus/fr/regles`, une ligne annotée doit produire exactement les
signalements annotés, avec la bonne règle et, si elle est donnée, la bonne
première suggestion (test `cargo test --test corpus`). Chaque règle doit avoir
au moins une ligne annotée et une ligne sans annotation.

Le corpus d'évaluation n'est pas un test : il peut contenir des erreurs que le
moteur ne détecte pas encore, pour que le rappel mesuré reste honnête.
