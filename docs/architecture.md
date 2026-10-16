# Architecture

## Vue d'ensemble

```
             ┌──────────────────────────── data/ (TSV, chargés à l'exécution)
             │
  texte ──► correcteur-core (Rust, sans réseau) ──► liste de problèmes
             ▲            ▲             ▲              ▲
     add-in Word     extension      application     correcteur-cli
     (Office.js +    LibreOffice    Tauri           (tests, évaluation,
      WebAssembly)   (UNO + lib     (lib native)     mesures)
                      native)
```

Tout le traitement est local. Le moteur ne fait aucun appel réseau et ne lit
que les fichiers qu'on lui donne. Le texte de l'utilisateur ne quitte jamais
l'appareil.

## Dépôt

| Chemin | Contenu |
|---|---|
| `crates/correcteur-core` | Moteur : découpage, lexique, règles, suggestions, corpus. |
| `crates/correcteur-cli` | Commande `correcteur` : `analyser`, `evaluer`, `mesurer`, `importer-lexique383`. |
| `crates/correcteur-wasm` | Module WebAssembly : interface C minimale, échanges en JSON. |
| `addins/word` | Add-in Word (TypeScript, Office.js) ; voir `addins/word/LISEZMOI.md`. |
| `data/lexique/*.tsv` | Lexiques de formes fléchies (chargés dans l'ordre alphabétique). |
| `data/regles/` | Description des règles, locutions, listes (élisions, abréviations…). |
| `data/SOURCES.tsv` | Ressources utilisées et leurs licences. |
| `corpus/fr/regles/` | Tests de règles : au moins un positif et un négatif par règle. |
| `corpus/fr/evaluation/` | Phrases libres pour mesurer précision et rappel. |
| `docs/` | Architecture, format des données, provenance, résultats. |

À venir (ordre de livraison) : règles d'accord
(étape 3), `apps/bureau` Tauri (étape 4), `addins/libreoffice` (étape 5),
conjugaison et synonymes (étape 6).

## Interface du moteur

```rust
let engine = Engine::from_dir("data")?;          // ou Engine::new(EngineData { … }) en WebAssembly
let issues: Vec<Issue> = engine.check(texte, &Options::default());
```

Chaque `Issue` contient :

- `start`, `end` : positions en octets UTF-8 ; `start_utf16`, `end_utf16` :
  positions en unités UTF-16, directement utilisables par Office.js et UNO ;
- `category` : `orthographe`, `homophone`, `grammaire` ou `typographie` ;
- `rule` : identifiant stable (voir `data/regles/regles.tsv`) ;
- `message` et `explanation` : textes en français ;
- `suggestions` : remplacements du texte `start..end`, du plus probable au
  moins probable. Une suggestion vide signifie « supprimer ».

`Options` permet de désactiver des catégories ou des règles, de choisir
l'espace fine ou normale avant `; ! ?`, d'activer ou non les vérifications
d'apostrophe, de guillemets et de points de suspension, et de fournir un
dictionnaire personnel.

## Déroulement d'une analyse

1. **Zones ignorées** : adresses web et courriels (`text.rs`).
2. **Découpage en mots** (`analysis.rs`) : suites de lettres et chiffres avec
   apostrophes et traits d'union internes. Un mot absent du lexique est
   découpé : élision (`l’arbre` → `l’` + `arbre`), puis composé
   (`dit-il` → `dit` + `il`). Chaque mot sait s'il commence une phrase et
   s'il touche le mot précédent (seulement des blancs entre eux).
3. **Règles** (`rules/`), indépendantes les unes des autres :
   - `typo.rs` travaille sur les caractères ;
   - `spelling.rs` cherche chaque mot dans le lexique, puis appelle
     `suggest.rs` ;
   - `homophones.rs` examine le mot précédent et le mot suivant et leurs
     lectures dans le lexique (catégorie, traits) ;
   - `grammar.rs` vérifie les accords (sujet-verbe, groupe nominal,
     participes avec être et avoir), l'infinitif employé à la place du
     participe et la négation sans « ne » ;
   - `locutions.rs` reconnaît les suites de mots de `locutions.tsv`, y compris
     les constructions prépositionnelles critiquées.
4. **Mise en forme** (`lib.rs`) : tri, dédoublonnage, filtrage selon les
   options, textes tirés de `regles.tsv`, positions UTF-16.

## Grammaire : principe de prudence

Les règles de grammaire n'utilisent que le voisinage immédiat du verbe ou du
nom, sans analyse complète de la phrase. Le sujet n'est retenu que s'il est
sûr : pronom personnel juste avant le verbe (avec d'éventuels pronoms
compléments ou « ne » entre les deux), ou groupe nominal simple
(déterminant, adjectif, nom, adjectif) en début de proposition. Un sujet
coordonné (« lui et elle ») est écarté. L'accord du participe avec avoir
n'est vérifié qu'en l'absence de complément placé avant (pronom objet, « que »,
« combien »). Dans le doute, rien n'est signalé : les erreurs plus lointaines
(« les enfants de mon frère joue ») ne sont pas encore détectées.

Les corrections proposées sont générées à partir du lexique : forme du même
lemme avec la personne, le genre ou le nombre attendus.

## Suggestions orthographiques

Candidats : formes de même *squelette* (minuscules, sans accents, lettres
doublées réduites), formes à une modification du mot, formes à une
modification du squelette, coupure en deux mots. Classement par une distance
d'édition pondérée (accent : 0,25 ; lettre doublée : 0,5 ; inversion : 0,75 ;
autre : 1), diminuée de 0,08 × ln(1 + fréquence).

## Données séparées du code

Le code implémente la *détection* ; les textes affichés, les listes et les
locutions sont dans `data/`. Au chargement, le moteur vérifie que chaque règle
du code est décrite dans `regles.tsv` et que chaque locution renvoie à une
règle connue : une incohérence empêche le démarrage au lieu de produire un
message vide.

## Cibles de compilation

- Bibliothèque Rust (`rlib`) aujourd'hui.
- WebAssembly pour l'add-in Word (`crates/correcteur-wasm`), sans
  `wasm-bindgen` ni dépendance : le JavaScript réserve de la mémoire
  (`cr_alloc`), y écrit des chaînes UTF-8, appelle `cr_add_file` pour chaque
  fichier de données, `cr_build`, puis `cr_check(texte, options)`, et lit le
  résultat JSON (`cr_output_ptr`, `cr_output_len`). Les positions renvoyées
  sont en UTF-16. Module : 220 Ko ; avec le lexique complet, construction en
  130 ms et 40 Mo de mémoire (mesures Node, Apple Silicon).
- Bibliothèque native avec interface C pour LibreOffice et Tauri (étapes 4 et 5).

## Performance

Objectif : 500 mots en moins de 100 ms. Mesure : `correcteur mesurer`
(code de sortie non nul si la médiane dépasse 100 ms), exécutée par
l'intégration continue en compilation release.
