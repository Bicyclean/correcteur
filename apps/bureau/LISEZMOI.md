# Application de bureau

Éditeur de texte avec correction en direct, pour macOS et Windows, construit
avec Tauri. Le moteur est la bibliothèque Rust native ; ses données
(`data/lexique`, `data/regles`) sont incluses dans l'application et lues au
démarrage. Aucun accès réseau : la politique de sécurité de la fenêtre
n'autorise que les échanges avec le processus de l'application.

## Utilisation

- Écrivez ou collez un texte : l'analyse se relance après une courte pause de
  frappe, les erreurs sont soulignées dans le texte et listées à droite. Seul
  le paragraphe modifié est réanalysé.
- **Corriger le texte d'une autre application** : copiez-le, puis appuyez sur
  **⌘⇧E** (macOS) ou **Ctrl+Maj+E** (Windows), même quand l'application est en
  arrière-plan. La fenêtre s'ouvre avec le texte du presse-papiers.
  Corrigez, puis **⌘↩** / **Ctrl+Entrée** recopie le texte corrigé et ferme
  la fenêtre : il ne reste qu'à coller.
- **Fichiers texte** (`.txt`, `.md`) : **Ouvrir** (⌘O / Ctrl+O), ou déposer le
  fichier sur la fenêtre ; **Enregistrer** (⌘S / Ctrl+S) et **Enregistrer
  sous** (⌘⇧S / Ctrl+Maj+S) ; **Nouveau** (⌘N / Ctrl+N). L'encodage (UTF-8 ou
  Windows-1252) et les fins de ligne du fichier sont conservés ; un texte que
  Windows-1252 ne peut pas représenter (espace fine insécable…) est enregistré
  en UTF-8, et l'application le signale. Les documents Word (`.docx`) ne
  s'ouvrent pas ici : copiez-collez leur texte, ou utilisez le complément Word.
- **⌘J** / **Ctrl+J** : problème suivant (avec **Maj** : précédent).
  **Échap** : fermer la fenêtre.
- Fermer la fenêtre ne quitte pas l'application : elle reste dans la barre des
  menus (macOS) ou la zone de notification (Windows), pour que le raccourci
  fonctionne. Pour quitter : menu de l'icône, **Quitter** (ou ⌘Q sur macOS).
  Relancer l'application ramène la fenêtre existante.

Si une autre application utilise déjà le raccourci, il n'est pas enregistré :
les Réglages le signalent, et l'icône de la barre des menus propose aussi
« Corriger le presse-papiers ».

## Rien ne se perd

Le texte en cours est enregistré en continu comme brouillon : après avoir
quitté l'application (ou un arrêt brutal), il est retrouvé tel quel, avec son
fichier et l'indication « Modifié » s'il n'avait pas été enregistré. Un
fichier enregistré est relu au lancement, au cas où il aurait changé.
Remplacer un texte modifié (ouverture, presse-papiers) demande confirmation ;
un texte sans fichier remplacé reste récupérable par **Annuler** dans la
notification.

## Réglages et fichiers

| Fichier | macOS | Windows |
|---|---|---|
| `reglages.json`, `dictionnaire.txt` | `~/Library/Application Support/org.correcteur.bureau/` | `%APPDATA%\org.correcteur.bureau\` |
| `brouillon.json` | même dossier | `%APPDATA%\org.correcteur.bureau\` |
| `journal.txt` (erreurs) | `~/Library/Logs/org.correcteur.bureau/` | `%LOCALAPPDATA%\org.correcteur.bureau\logs\` |

`dictionnaire.txt` contient un mot par ligne : il se modifie à la main, se
sauvegarde et se copie d'un poste à l'autre. **Réglages > Application >
Dossier des réglages** l'affiche dans le Finder ou l'Explorateur. Les
écritures sont atomiques : une coupure pendant l'enregistrement laisse
l'ancien fichier intact.

**Ouvrir à l'ouverture de session** (Réglages > Application) lance le
correcteur discrètement, sans fenêtre, pour que le raccourci soit toujours
disponible. La taille et la position de la fenêtre sont retenues.

## Outils

La section **Outils** du panneau conjugue un verbe (infinitif, forme
conjuguée ou pronominal) et cherche les synonymes d'un mot : laissé vide, le
champ prend le mot sélectionné, et un clic sur un synonyme remplace la
sélection. Le style (répétitions, phrases longues) s'active dans les
**Réglages** ; la lisibilité du texte s'affiche après chaque analyse.

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
| `src-tauri/src/main.rs` | Commandes (analyse, presse-papiers, documents, réglages), raccourci global, menus, icône de la barre des menus. |
| `src-tauri/src/stockage.rs` | Fichiers : réglages, dictionnaire, brouillon, journal, encodages des documents (testé par `cargo test`). |
| `src/main.ts` | Éditeur, surlignage, document courant et brouillon, raccourcis de la fenêtre. |
| `../../web/commun/` | Panneau partagé avec l'add-in Word (liste des problèmes, réglages, dictionnaire). |

`src-tauri` est exclu de l'espace de travail Cargo racine : ses dépendances
sont lourdes et propres à l'application.
