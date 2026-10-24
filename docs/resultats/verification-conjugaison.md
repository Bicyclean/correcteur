# Résultats — vérification de la conjugaison (tous les temps et tous les modes)

Deux volets : les **tables** (le logiciel conjugue-t-il juste ?) et la
**détection** (repère-t-il les fautes de conjugaison dans un texte ?).

## Tables de conjugaison

1 225 formes attendues, rédigées d'après la conjugaison usuelle, couvrent une
centaine de verbes (réguliers, irréguliers, à changement de radical, en -cer,
-ger, -yer, -eler, -eter, défectifs) à tous les temps simples, plus les
participes et l'impératif : `corpus/fr/conjugaison/formes.tsv`, vérifié par
`crates/correcteur-core/tests/conjugaison.rs`.

Écarts trouvés et corrigés :

- *pourvoir* faisait « pourvis » au passé simple (« pourvus ») ;
- *mouvoir* donnait « mu » (« mû ») ;
- les verbes impersonnels n'avaient ni imparfait, ni subjonctif, ni participe
  présent (« il pleuvait », « qu'il pleuve », « pleuvant ») ;
- « s'en aller » devenait « je m'en alle » (le pronom *en* n'était pas reconnu) ;
- dix verbes de Lexique n'avaient aucun modèle : *croître*, *accroître*,
  *décroître*, *clore*, *éclore*, *enclore*, *foutre*… (« j'accroissais » était
  signalé comme faute) ;
- *asseoir* (« assoyait », « que j'assoie »), *frire* (« frit »), *choir*,
  *déchoir*, *échoir*, *absoudre* (« absoute ») : formes manquantes ou fausses.

Les temps composés, l'auxiliaire (être ou avoir), l'accord du participe et les
verbes pronominaux sont affichés dans les tables : revus sur *aller*, *venir*,
*arriver*, *se lever*, *se souvenir*, *s'apercevoir*, *s'en aller*.

## Détection des fautes

Trois batteries de phrases fautives, chacune avec sa correction, par temps et
par mode (présent, imparfait, passé simple, futur, conditionnel, subjonctif
présent et imparfait, impératif, temps composés, infinitif, participe).

| Batterie | Cas | Détection | 1re suggestion juste | Rôle |
|---|---:|---:|---:|---|
| 1 (réglage) | 176 | 65 % → **88 %** | 80 % → **89 %** | a servi à écrire les règles |
| 2 (neuve) | 90 | 73 % → **87 %** | 95 % → **97 %** | écrite avant les corrections de la fin |
| 3 (neuve) | 53 | 81 % → **88 %** | 93 % → **91 %** | écrite après les corrections de la batterie 2 |

Les premiers chiffres sont ceux d'avant le travail du lot, les seconds ceux
d'après. Les batteries 2 et 3 ont été écrites sans regarder le code : leur
première mesure (73 % et 81 %) est la plus honnête ; le second chiffre inclut
les corrections qu'elles ont suscitées.

**Fausses alertes** : 168 phrases justes, dont soixante bâties sur les temps
rares (passé simple, passé antérieur, subjonctif imparfait et plus-que-parfait,
conditionnel passé 2e forme, impératifs pronominaux, *vaincre*, *moudre*,
*clore*, accords de collectifs) : **aucune** alerte. Sur 820 000 mots de romans et
d'articles de Wikipédia : aucune fausse alerte nouvelle ; les seules alertes
nouvelles sont de vraies fautes des textes (« sans qu'il s'en rendit compte »,
« il est bien possible que cette allégresse fut vaine »). 59 fausses alertes
d'orthographe disparaissent (participes féminins et pluriels que Lexique
n'atteste pas : « archivée », « réutilisées »…).

## Ce qui change

- accord sujet-verbe : la proposition qui suit « et » est enfin vérifiée
  (« … et tu viendra ») ; sujets en nom propre (« Marie mangent »), sujet après
  un adverbe en incise (« Hier, le chat mangent »), « les rues est inondées »,
  « les voisins se plaint » ;
- règles nouvelles : impératif (`GRAM_IMPERATIF` : « ouvres la fenêtre », « dit-lui »),
  concordance des temps (`GRAM_CONCORDANCE` : « elle disait qu'elle viendra »),
  « si » + imparfait + futur (`GRAM_SI_FUTUR`), participe sans auxiliaire
  (`GRAM_PARTICIPE_SEUL` : « il voulu partir »), infinitif seul après le
  pronom sujet (`GRAM_INFINITIF_SEUL` : « il manger »), accord après « étant »
  (`GRAM_ACCORD_PARTICIPE`) ;
- subjonctif : « que » repris après « et » ou une virgule, après un adverbe
  (« il faut absolument que »), après *content*, *heureux*, *ravi*…, passé simple
  à la place de l'imparfait du subjonctif (« qu'il eut » → « eût ») ;
- infinitif après *pour*, *sans*, *avant de*, *venir de*, *commencer à*…
  (« pour réussi », « vient de partit ») ;
- suggestions : une forme mal écrite après un pronom sujet reçoit d'abord les
  formes conjuguées de la bonne personne (« tu nettoye » → « nettoies »,
  « nous savont » → « savons »), même quand Lexique n'a pas la forme voulue
  (« tu rentras »).

## Ce qui reste hors de portée

- les verbes coordonnés dont le sujet est loin (« … se serrèrent la main et se
  sépara ») ;
- le participe seul après un nom, avec ou sans « par » (« les mesures prise par
  le gouvernement ») : le nom qui précède n'est pas toujours celui que le
  participe qualifie, l'essai donnait trop de fausses alertes et a été retiré ;
- le gérondif mal formé (« en marchent »), indiscernable de « en » pronom ;
- « nous » et « vous » devant un infinitif (« vous aller »), qui peuvent être
  compléments ou réfléchis ;
- les homophones qui demandent de comprendre la phrase (« est » / « et ») ;
- un verbe à une forme valide mais d'un autre temps que celui du contexte
  (« je mangeai souvent là quand j'étais étudiant »).
