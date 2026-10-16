# Lexiques

| Fichier | Contenu | Source | Licence |
|---|---|---|---|
| `00-amorce.tsv` | Formes absentes de Lexique 3.83 : noms propres, formes élidées, mots figés. Rédigé à la main. | `CONTRIB-INIT` | GPL-3.0-or-later |
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
espaces et liaisons écartées.

## Régénérer

Télécharger `Lexique383.zip` depuis http://www.lexique.org, l'extraire hors du
dépôt, puis :

```bash
cargo run --release -p correcteur-cli -- importer-lexique383 chemin/vers/Lexique383.tsv
```

La sortie est triée : un nouvel import ne produit un écart que si la source a changé.
