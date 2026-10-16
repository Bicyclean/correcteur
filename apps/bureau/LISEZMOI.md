# Application de bureau

Éditeur de texte avec correction en direct, pour macOS et Windows, construit
avec Tauri. Le moteur est la bibliothèque Rust native ; ses données
(`data/lexique`, `data/regles`) sont incluses dans l'application et lues au
démarrage. Aucun accès réseau : la politique de sécurité de la fenêtre
n'autorise que les échanges avec le processus de l'application.

## Utilisation

- Écrivez ou collez un texte : l'analyse se relance après une courte pause de
  frappe, les erreurs sont soulignées dans le texte et listées à droite.
- **Corriger le texte d'une autre application** : copiez-le, puis appuyez sur
  **⌘⇧E** (macOS) ou **Ctrl+Maj+E** (Windows), même quand l'application est en
  arrière-plan. La fenêtre s'ouvre avec le texte du presse-papiers.
  Corrigez, puis **⌘↩** / **Ctrl+Entrée** recopie le texte corrigé et ferme
  la fenêtre : il ne reste qu'à coller.
- **⌘J** / **Ctrl+J** : problème suivant (avec **Maj** : précédent).
  **Échap** : fermer la fenêtre.
- Fermer la fenêtre ne quitte pas l'application : elle reste dans la barre des
  menus (macOS) ou la zone de notification (Windows), pour que le raccourci
  fonctionne. Pour quitter : menu de l'icône, **Quitter**.

Si une autre application utilise déjà le raccourci, il n'est pas enregistré ;
l'icône de la barre des menus propose aussi « Corriger le presse-papiers ».

## Construire

Prérequis : Rust, Node 20 ou plus récent, et sous Windows le runtime WebView2
(présent sur Windows 10 et 11 à jour).

```bash
npm ci
```

Lancer en développement :

```bash
npm run dev
```

Produire l'application (`.app` et `.dmg` sur macOS, `.msi` et `.exe` sur
Windows, dans `src-tauri/target/release/bundle/`) :

```bash
npm run build
```

L'application n'est pas signée : au premier lancement, macOS demande de
l'ouvrir par clic droit > Ouvrir, et Windows SmartScreen affiche un
avertissement. La signature demande des certificats de développeur.

## Organisation

| Chemin | Rôle |
|---|---|
| `src-tauri/src/main.rs` | Commandes (`analyser`, presse-papiers), raccourci global, icône de la barre des menus. |
| `src/main.ts` | Éditeur, surlignage, raccourcis de la fenêtre. |
| `../../web/commun/` | Panneau partagé avec l'add-in Word (liste des problèmes, réglages, dictionnaire). |

`src-tauri` est exclu de l'espace de travail Cargo racine : ses dépendances
sont lourdes et propres à l'application.
