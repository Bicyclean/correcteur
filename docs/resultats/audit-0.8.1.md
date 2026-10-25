# Résultats — audit après la version 0.8.1

Un audit complet : homophones, orthographe, grammaire, syntaxe, vocabulaire,
typographie et conjugaison à tous les temps, par des batteries neuves (écrites
sans regarder le code) puis par des tests systématiques sur les verbes les plus
fréquents.

## Batteries de phrases fautives

| Batterie | Cas | Détection avant | Détection après | 1re suggestion juste |
|---|---:|---:|---:|---:|
| Audit 1 : homophones, orthographe, grammaire, syntaxe, vocabulaire | 91 | 70 % | **86 %** | 92 % |
| Audit 2 : tous les temps, accords, infinitif | 54 | 68 % | **92 %** | 90 % |
| Audit 3 : locutions et erreurs fréquentes | 41 | 48 % | **65 %** | 77 % |

Fausses alertes : **aucune** sur 324 phrases justes (dont 64 sur les temps rares)
et sur 820 000 mots de romans et d'articles de Wikipédia (les alertes
nouvelles sont de vraies fautes des textes ou de l'oral transcrit : « je crains
personne », « nous ne croyons pas que vous exagérez »). Les typographies (espaces, guillemets,
points de suspension, tirets, majuscules) : 27 cas contrôlés, tous justes.

## Tests systématiques de conjugaison

Pour 300 verbes courants, chaque forme avec son pronom, comparée à la forme d'une
autre personne :

| Mesure | Résultat |
|---|---:|
| Formes justes de l'indicatif et du conditionnel (10 temps, 17 940 formes) | **0 fausse alerte** |
| Formes d'une autre personne relevées (3 000 tirées) | **99,9 %** |
| Subjonctif présent (« il faut que… ») | 97 % (le reste : « vérifiiez », volontairement tu) |
| Subjonctif imparfait | 99 % |
| Impératif sans « s » à la 2e personne (« passes ici ») | 98 % |
| « être » + participe accordé (30 verbes) | 100 % |

`crates/correcteur-core/tests/conjugaison_systematique.rs` rejoue une version
réduite à chaque exécution des tests.

## Défauts trouvés et corrigés

- **Formes conjuguées absentes de Lexique** (« vous vîtes », « vous allâtes »,
  « nous tuâmes » signalés comme inconnus ; « ils élevâtes » jamais vérifié) :
  les modèles de conjugaison fournissent désormais leurs lectures au reste du
  moteur.
- **Nom ou verbe** : « Le voiture », « à le centre », « Le table » échappaient à
  la vérification parce que « voiture » est aussi un verbe rarissime ; la fréquence
  verbale de Lexique tranche.
- **Homophones** : « tu à fait » (as), « je peu » (peux), « il mange peut car »
  (peu), « Quant il pleut » (quand), « ces dommage » (c'est), « les enfants on
  faim » (ont), « il et grand » (est), « il est peut être » (peut-être).
- **Négation** : « je vois personne ».
- **Concordance** : « après qu’il soit » (indicatif), « deux nuit » (pluriel
  après un nombre), « si j’avais… je serai ».
- **Sujets** : après une virgule qui suit une proposition complète (« Il y a vingt
  ans, les gens écrivait »), après un adverbe (« Hier, Marie mangent »), « qui »
  relatif à antécédent sûr (« ce sont mes amis qui me soutienne »).
- **Participes** : « elle est passé me voir », « les lettres que j'ai fait »,
  « et les a envoyé » (sujet sous-entendu), « il a rentré dans la maison »,
  « je l'ai entendu chanté », « il a fini par accepté », « elle sait cuisiné ».
- **Vocabulaire** : « un homme censé », « il habite prêt de la gare »,
  « palier ce manque », « acquis de conscience », « amande » pour « amende »,
  trente paronymes de plus, 120 locutions (traits d'union, « où », « à »).
- **Impératif** : « ne bouges pas », « aies de la patience » dans une phrase
  commencée par un impératif.
- **Infinitif coordonné** : « mangèrent, burent et s’endormir ».
- **Orthographe rectifiée** : « tu connait » (graphie de 1990) n'était pas
  vérifiée par les règles d'accord.
- **Participes féminins** : « provenue » et ses semblables, absents de Lexique.

## Ce qui reste hors de portée

- les homophones qui demandent de comprendre la phrase (« ces » / « ses »,
  « est » / « et » entre deux noms) ;
- un participe seul après un nom (« les mesures prise par le gouvernement ») ;
- « nous » et « vous » devant un infinitif (« vous aller ») ;
- un verbe à une forme valide mais d'un autre temps que celui du contexte ;
- la proposition suivante d'un sujet éloigné (« … se serrèrent la main et se
  sépara »).
