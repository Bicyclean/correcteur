# correcteur

Correcteur et assistant de rédaction libre et gratuit pour le français.
Licence : GPL-3.0 ou ultérieure (voir `LICENSE`). Le lexique dérivé de
Lexique 3.83 reste sous CC BY-SA 4.0 (voir `data/lexique/LISEZMOI.md`).

> Nom provisoire : le nom définitif du projet reste à choisir.

Tout le traitement est local : le texte ne quitte jamais l'appareil.

## État

| Étape | Contenu | État |
|---|---|---|
| 1 | Moteur Rust : typographie, orthographe de base, homophones, tests | **Fait** — lexique Lexique 3.83 importé (125 000 formes) |
| 2 | Add-in Word (Windows, macOS) | **Fonctionne dans Word pour le web** (Safari) ; essais dans Word pour Mac et Windows à faire |
| 3 | Accords et grammaire | **Fait** — accords sujet-verbe et groupe nominal, participes, négation, quelques constructions |
| 4 | Application de bureau Tauri | **Fait** — éditeur, raccourci global ⌘⇧E / Ctrl+Maj+E pour le presse-papiers ; construite sur macOS, raccourci global et Windows à vérifier |
| 5 | Extension LibreOffice | À faire |
| 6 | Conjugaison, synonymes, version 2 | À faire |

Ce que le moteur détecte aujourd'hui :

- **Orthographe** : mots inconnus, suggestions classées par proximité et fréquence.
- **Homophones** : a/à, ou/où, ce/se, c’/s’, et/est (règles prudentes, contexte immédiat).
- **Grammaire** : accord sujet-verbe, accords dans le groupe nominal,
  participe passé avec être et avoir, infinitif à la place du participe
  (« il a manger »), négation sans « ne », quelques constructions critiquées
  (« pallier à »). Règles prudentes : voisinage immédiat seulement.
- **Typographie** : espaces insécables avant `; : ! ?` et dans les guillemets,
  espaces manquantes ou en trop, guillemets français, apostrophes, tirets
  d'incise, majuscule en début de phrase, points de suspension.

Lexique : Lexique 3.83 (125 000 formes, sans noms propres), complété d'un
petit lexique rédigé à la main.

## Utilisation

```bash
cargo test
```

```bash
echo "Il à mangé , puis il ce lave les mains!" | cargo run -q -p correcteur-cli -- analyser
```

```bash
cargo run --release -p correcteur-cli -- evaluer
```

```bash
cargo run --release -p correcteur-cli -- mesurer
```

Add-in Word : voir [addins/word/LISEZMOI.md](addins/word/LISEZMOI.md).
Application de bureau : voir [apps/bureau/LISEZMOI.md](apps/bureau/LISEZMOI.md).

## Documentation

- [Architecture](docs/architecture.md)
- [Format des données et du corpus](docs/format-donnees.md)
- [Provenance et cadre clean-room](docs/provenance.md)
- [Résultats par version](docs/resultats/)

## Contribuer

Lisez d'abord [docs/provenance.md](docs/provenance.md). Chaque règle a au
moins un test positif et un test négatif dans `corpus/fr/regles/`. Avant
chaque fusion : `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`
et `cargo test` doivent passer.
