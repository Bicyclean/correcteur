# Lexiques

| Fichier | Contenu | Source | Licence |
|---|---|---|---|
| `00-amorce.tsv` | Formes absentes de Lexique 3.83 : noms propres, formes élidées, mots figés. Rédigé à la main. | `CONTRIB-INIT` | GPL-3.0-or-later |
| `05-pays.tsv` | Noms de pays, territoires, continents et régions du monde, avec genre et nombre. Rédigé à la main. | `CONTRIB-INIT` | GPL-3.0-or-later |
| `10-lexique383.tsv` | 125 000 formes, 154 000 lectures. | `LEXIQUE383` | CC BY-SA 4.0 |

## Attribution

`10-lexique383.tsv` est dérivé de **Lexique 3.83**, base de données lexicales
du français de Boris New, Christophe Pallier et coll. (http://www.lexique.org),
distribuée sous licence Creative Commons Attribution – Partage dans les mêmes
conditions 4.0 International (https://creativecommons.org/licenses/by-sa/4.0/).
Le fichier dérivé est distribué sous la même licence.

Référence : New, B., Pallier, C., Brysbaert, M., Ferrand, L. (2004).
*Lexique 2: A New French Lexical Database*. Behavior Research Methods,
Instruments, & Computers, 36(3), 516-524.

Modifications apportées : colonnes réduites à forme, lemme, catégorie, traits,
fréquence et source ; catégories et analyses verbales converties au format du
projet ; une lecture par analyse verbale ; apostrophes typographiques ;
ligature œ rétablie pour les radicaux de `data/regles/ligatures.txt` ;
fréquence égale à la moyenne de `freqfilms2` et `freqlivres` ; formes à
espaces et liaisons écartées ; lectures verbales erronées retirées (voir
ci-dessous).

### Lectures verbales écartées

Lexique 3.83 contient des erreurs d'étiquetage qui produisaient des fausses
alertes : « lui » noté présent de *luire* (« il lui dit » signalé), « n’ »
noté présent de *avoir*, infinitifs notés « vous » au présent (« abandonner »).
`correcteur ecarter-lectures` retire de `10-lexique383.tsv` les formes
conjuguées et infinitifs que le modèle de conjugaison du verbe contredit,
quand la forme garde une autre lecture. Les lignes retirées (177) sont
consignées dans `lectures-ecartees.txt` (extension `.txt` : tous les `.tsv`
de ce dossier sont chargés comme lexiques). Les participes ne sont jamais
retirés : un participe contredit est souvent une variante correcte que le
modèle ne produit pas (« inclus », « frit »).

## Régénérer

Télécharger `Lexique383.zip` depuis http://www.lexique.org, l'extraire hors du
dépôt, puis :

```bash
cargo run --release -p correcteur-cli -- importer-lexique383 chemin/vers/Lexique383.tsv
cargo run --release -p correcteur-cli -- ecarter-lectures
cargo run --release -p correcteur-cli -- attribuer-conjugaisons
```

La sortie est triée : un nouvel import ne produit un écart que si la source a changé.
