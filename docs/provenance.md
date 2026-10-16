# Provenance et cadre clean-room

Ce projet est une réimplémentation originale. Il s'inspire des *fonctionnalités*
décrites publiquement par des correcteurs existants, dont Antidote, mais
n'en reprend **aucun élément** : ni code, ni binaire, ni donnée, ni lexique,
ni règle, ni message, ni libellé d'interface, ni documentation.

## Règles de contribution

1. Une règle s'écrit à partir d'une grammaire de référence ou d'une ressource
   libre, **décrite avec vos propres mots**. Les messages et explications sont
   rédigés pour le projet.
2. Toute donnée (entrée de lexique, règle, locution, phrase de corpus) porte un
   code de source, défini dans `data/SOURCES.tsv`.
3. Une ressource externe n'est intégrée qu'après vérification de sa licence,
   consignée dans `data/SOURCES.tsv` avec ses mentions d'attribution.
4. Une personne qui a examiné en détail le fonctionnement interne du logiciel
   de référence (code, données, règles internes, rétro-ingénierie) **ne rédige
   pas** les règles correspondantes. Elle le déclare dans le tableau ci-dessous.
5. En cas de règle ambiguë ou de sources contradictoires, on ouvre une
   question plutôt que de trancher seul.

## Codes de source

| Code | Signification |
|---|---|
| `CONTRIB-INIT` | Rédigé par le contributeur initial pour ce projet (lexique d'amorce, règles, messages, corpus). |
| `REF-GRAM-FR` | Règle de grammaire générale, décrite dans les grammaires de référence du français (par exemple *Le Bon Usage* de Grevisse, la *Grammaire méthodique du français* de Riegel, Pellat et Rioul). Les ouvrages servent à vérifier la règle ; aucun texte n'en est repris. |
| `LEXIQUE383` | Lexique 3.83 (lexique.org), CC BY-SA 4.0 ; voir `data/lexique/LISEZMOI.md`. |
| `REF-TYPO-FR` | Usage typographique français courant, décrit notamment dans le *Lexique des règles typographiques en usage à l'Imprimerie nationale*. Même principe. |

Les sources sont indiquées dans la dernière colonne de chaque fichier de
`data/` et de `corpus/`.

## Ressources externes envisagées

Les licences des ressources « candidates » sont celles annoncées publiquement ;
elles doivent être **vérifiées sur la source** (fichier de licence de la
version téléchargée) avant toute intégration. Une ressource intégrée est
consignée dans `data/SOURCES.tsv` avec l'empreinte de l'archive utilisée.

| Ressource | Usage prévu | Licence annoncée | Compatibilité GPL-3.0 | Statut |
|---|---|---|---|---|
| Lexique 3.83 (lexique.org) | Formes, lemmes, catégories, fréquences | CC BY-SA 4.0 (vérifiée dans `README-Lexique.txt` de l'archive) | Oui, à sens unique (CC BY-SA 4.0 → GPLv3). Le fichier dérivé reste sous CC BY-SA 4.0. | **Intégré** (`data/lexique/10-lexique383.tsv`) |
| Dictionnaire de Grammalecte (Dicollecte) | Formes fléchies, couverture | MPL 2.0 | Oui (licence secondaire GPL), sauf mention contraire | Candidat |
| Règles de Grammalecte | Inspiration et réutilisation de règles | GPL-3.0 | Oui ; conserver les mentions d'attribution | Candidat |
| Morphalou 3 (ATILF) | Formes fléchies | LGPL-LR | **Incertaine** : à examiner | À étudier |
| Wiktionnaire | Compléments, thésaurus | CC BY-SA 4.0 (contenu récent) | Oui, à sens unique | Candidat |
| Thésaurus français de LibreOffice / Dicollecte | Synonymes (étape 6) | À vérifier | À vérifier | À étudier |

## Déclarations des contributeurs

| Contributeur | Accès au fonctionnement interne du logiciel de référence | Peut rédiger les règles correspondantes |
|---|---|---|
| Claude (assistant IA d'Anthropic), contributeur initial, sous la direction du porteur du projet | Aucun : pas d'examen du code, des binaires, des données ni des règles internes. | Oui |
| *Porteur du projet — à compléter* | | |

Remarque sur le contributeur initial : un modèle de langage a été entraîné sur
de grands volumes de textes publics. Il n'a pas consulté le logiciel de
référence pour ce travail, et tous les messages ont été rédigés pour le projet.
Une relecture humaine des messages et des règles est recommandée avant chaque
publication.
