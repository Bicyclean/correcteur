# Résultats — vérification par catégories (orthographe, grammaire, syntaxe, vocabulaire)

Méthode : des phrases fautives rédigées pour le projet, chacune avec sa
correction attendue. Pour chaque cas, on regarde si une alerte recouvre la
faute (détection) et si la première suggestion, appliquée, redonne la phrase
correcte (première suggestion juste). Les phrases correctes servent de test
négatif.

Deux batteries distinctes :

- **batterie 1** (209 cas) : sert à écrire et régler les règles ;
- **batterie 2** (126 cas) : phrases nouvelles, écrites après les règles, pour
  mesurer ce que le logiciel détecte vraiment.

## Avant et après

| Catégorie (batterie 1) | Cas | Détection avant | Détection après | 1re suggestion juste |
|---|---:|---:|---:|---:|
| Orthographe | 65 | 98 % | 98 % | 95 % |
| Accords et conjugaison | 69 | 59 % | 97 % | 97 % |
| Subjonctif | 19 | 0 % | 100 % | 100 % |
| Syntaxe | 39 | 87 % (après un premier lot : 16 %) | 97 % | 94 % |
| Vocabulaire | 17 | 21 % | 94 % | 100 % |
| **Total** | **209** | **53 %** | **97 %** | **96 %** |

| Catégorie (batterie 2, phrases nouvelles) | Cas | Détection | 1re suggestion juste |
|---|---:|---:|---:|
| Orthographe | 44 | 100 % | 90 % |
| Grammaire | 40 | 95 % | 97 % |
| Syntaxe | 25 | 100 % | 100 % |
| Vocabulaire | 17 | 100 % | 100 % |
| **Total** | **126** | **98 %** | **95 %** |

Les premières suggestions fausses sont presque toutes des fautes de frappe sur
des mots tronqués (« scientifiq », « harmonieu ») ou très altérés.

## Fausses alertes

Mesurées sur du texte correct :

- quatre romans du domaine public (363 000 mots) : les seules alertes
  nouvelles sont de vraies fautes de ces textes (« il eût fallut », « tu as
  jamais fais », « tout les objets », « il dut remettra ») et « à priori » ;
- 54 articles de qualité de Wikipédia (457 000 mots) : 6 alertes nouvelles,
  dont 4 vraies fautes de ces articles (« les trois théâtre », « de Alice »,
  « voire même », « à priori ») ;
- dix textes d'essai (cinq avec fautes, cinq sans) : toutes les fautes visées
  détectées, aucune alerte sur les textes sans faute.

Les premières mesures sur Wikipédia révélaient une quarantaine de fausses
alertes que les romans cachaient : mots à h aspiré (« de hauts postes »), noms
propres étrangers (« de Holland »), « avant tout la », « la ligne A relie »,
« bel et bien qu’ils ont », « il ne fait aucun doute qu’elle a », « la bande
son contient ». Elles sont corrigées.

## Corpus d'évaluation

| Catégorie | VP | FP | FN | Précision | Rappel |
|---|---:|---:|---:|---:|---:|
| orthographe | 4 | 0 | 0 | 100 % | 100 % |
| homophone | 5 | 0 | 0 | 100 % | 100 % |
| grammaire | 11 | 0 | 1 | 100 % | 91,7 % |
| typographie | 7 | 0 | 0 | 100 % | 100 % |

Une phrase du corpus lui-même contenait une faute (« bien que encourageants »),
relevée par la nouvelle règle d'élision et corrigée.

## Performances

500 mots : 0,76 ms (médiane), contre 0,42 ms au départ. Un index des locutions
par premier mot et des pré-filtres évitent de chercher un sujet pour la plupart
des mots.

## Ce qui reste hors de portée

- les homophones qui demandent de comprendre la phrase (« est/et », « son/sont ») ;
- les paronymes sans mot déclencheur voisin ;
- les verbes coordonnés (« travaillent dur et réussi ») ;
- les participes passés absents de Lexique (« schématisé » est accepté quand le
  féminin est attesté, pas sinon).
