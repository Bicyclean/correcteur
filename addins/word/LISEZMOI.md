# Add-in Word

Panneau latéral qui analyse le document avec le moteur compilé en
WebAssembly. L'analyse se fait dans le panneau, sur l'ordinateur : le texte du
document n'est jamais envoyé. La politique de sécurité de la page
(`connect-src 'self'`) interdit d'ailleurs toute requête vers un autre site.

## Prérequis

- Node 20 ou plus récent.
- Rust avec la cible `wasm32-unknown-unknown` (`rustup target add wasm32-unknown-unknown`).
  Avec le Rust de Homebrew, installez `rustup` à côté : `build.mjs` le trouve tout seul.
- Word **Microsoft 365** ou Word 2021 et plus récent, sous Windows ou macOS, ou
  Word pour le web. Les versions de Word sous Windows qui affichent les
  add-ins avec Internet Explorer (Word 2016 et 2019 en licence perpétuelle)
  ne savent pas exécuter de WebAssembly.

## Construire et tester

```bash
npm ci
```

```bash
npm test
```

```bash
npm run typecheck
```

`npm test` compile le module WebAssembly, construit `dist/` et lance les
tests : interface du module, calcul des plages, réglages, et une simulation
qui applique chaque suggestion du corpus de règles par la même recherche que
dans Word.

## Essayer sans Word

```bash
npm start
```

Ouvrez http://localhost:3000/taskpane.html : hors de Word, le panneau passe
en mode démonstration, avec une zone de texte à la place du document.

## Essayer dans Word

Word exige HTTPS. Une fois, installez les certificats de développement
Office. Cette commande ajoute une autorité de certification locale au
trousseau ou au magasin de certificats : c'est à vous de la lancer.

```bash
npx office-addin-dev-certs install
```

Puis lancez le serveur (il passe en HTTPS dès que les certificats existent) :

```bash
npm start
```

**macOS** : copiez `dist/manifest.xml` dans
`~/Library/Containers/com.microsoft.Word/Data/Documents/wef/` (créez le
dossier s'il n'existe pas), redémarrez Word, puis *Insertion > Mes
compléments* : le correcteur apparaît dans la section des compléments de
développement.

**Windows** : partagez un dossier contenant `manifest.xml`, ajoutez son
chemin réseau dans *Fichier > Options > Centre de gestion de la confidentialité
> Paramètres > Catalogues de compléments approuvés* en cochant « Afficher dans
le menu », redémarrez Word, puis *Insertion > Mes compléments > Dossier partagé*.

**Word pour le web** (essai réussi avec Safari) : *Accueil > Compléments >
Plus de compléments > Mes compléments > Charger mon complément*, puis
choisissez `dist/manifest.xml`. Le complément chargé n'est retenu que par le
navigateur utilisé : il faut le recharger dans un autre navigateur. Firefox
n'utilise pas forcément le trousseau de macOS et refuse alors le certificat
de développement (panneau vide) ; Safari et Edge l'acceptent.

Pour vérifier le manifeste avant de le charger (envoie le manifeste au
service de validation de Microsoft) :

```bash
npx office-addin-manifest validate dist/manifest.xml
```

Le bouton **Corriger** apparaît dans l'onglet Accueil.

## Publier

```bash
node build.mjs --url https://exemple.org/correcteur
```

Le contenu de `dist/` est statique : il peut être servi par n'importe quel
hébergement HTTPS. Le manifeste produit pointe vers cette adresse. Les
données (environ 7 Mo, dont le lexique) gagnent à être servies compressées.

## Fonctionnement

| Fichier | Rôle |
|---|---|
| `src/engine.ts` | Charge le module WebAssembly et les données, appelle l'analyse. |
| `src/word.ts` | Lit les paragraphes, sélectionne et remplace dans Word. |
| `src/taskpane.ts` | Point d'entrée : moteur WebAssembly, Word ou mode démonstration. |
| `../../web/commun/panneau.ts` | Interface du panneau, partagée avec l'application de bureau. |
| `../../web/commun/mapping.ts` | Transforme une position dans un paragraphe en extrait à chercher dans Word. |
| `../../web/commun/settings.ts` | Réglages et dictionnaire personnel, dans le stockage local du panneau. |

Le document est analysé paragraphe par paragraphe. Office.js ne donne pas de
plage à partir d'une position : pour sélectionner ou corriger une erreur, le
panneau cherche dans le paragraphe un extrait qui la contient, élargi jusqu'à
des caractères visibles quand l'erreur est une espace, et retient la bonne
occurrence. Avant chaque modification, il vérifie que le paragraphe n'a pas
changé depuis l'analyse ; après, il relance l'analyse.

## Points à vérifier dans Word

Ces points ne peuvent pas être testés hors de Word. Ils sont à confirmer à
la première installation.

1. Recherche d'un extrait contenant une espace insécable (U+00A0) ou fine (U+202F).
2. Confusion possible, par la recherche de Word, entre guillemets droits et
   courbes : le panneau ne garde que les correspondances exactes, ce qui devrait suffire.
3. Office.js tente d'ouvrir un cadre de télémétrie Microsoft, bloqué par la
   politique de sécurité du panneau ; vérifier que rien d'autre n'en dépend.
4. Remplacement d'un extrait qui chevauche deux mises en forme : le texte
   remplacé prend la mise en forme du début de l'extrait.
