# Publier et déployer l'application de bureau

## Publier une version

1. Mettre le même numéro de version dans `apps/bureau/package.json`,
   `apps/bureau/src-tauri/Cargo.toml` et `apps/bureau/src-tauri/tauri.conf.json`
   (puis `npm install --package-lock-only` dans `apps/bureau`).
2. Vérifier que la CI est verte sur la branche principale.
3. Créer et pousser l'étiquette :

   ```bash
   git tag v0.4.0
   git push origin v0.4.0
   ```

4. Le workflow **Publication** construit les installateurs et les joint à une
   publication GitHub **en brouillon**. La relire (notes, fichiers), puis la
   publier depuis l'onglet *Releases*.

Pour essayer les installateurs sans rien publier : onglet *Actions* >
*Publication* > *Run workflow*. Ils sont alors disponibles en artefacts.

| Système | Fichier | Usage |
|---|---|---|
| macOS 11 ou plus récent | `Correcteur_<version>_universal.dmg` | Mac Intel et Apple Silicon. |
| Windows 10 et 11 | `Correcteur_<version>_x64_fr-FR.msi` | Installation pour tous les utilisateurs, déploiement en entreprise. |
| Windows 10 et 11 | `Correcteur_<version>_x64-setup.exe` | Installation pour l'utilisateur courant, sans droits d'administrateur. |

## Signature

Sans certificat de développeur, les systèmes avertissent au premier lancement.

- **macOS** : l'application est signée « ad hoc » (sans identité). Au premier
  lancement : Réglages Système > Confidentialité et sécurité > « Ouvrir quand
  même ». Pour supprimer l'avertissement, il faut un compte Apple Developer
  (99 $ par an), un certificat *Developer ID Application* et la notarisation.
  Le workflow les prend en charge en ajoutant à l'étape `tauri-action` les
  variables `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`,
  `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD` et `APPLE_TEAM_ID`,
  définies comme secrets du dépôt, et en retirant `"signingIdentity": "-"` de
  `tauri.conf.json`.
- **Windows** : SmartScreen affiche « Windows a protégé votre ordinateur »
  jusqu'à ce que l'installateur soit signé par un certificat de signature de
  code (par exemple Azure Trusted Signing, ou un certificat OV/EV d'une
  autorité de certification) et ait acquis une réputation.

Sur un parc géré, l'administrateur peut aussi autoriser l'application
explicitement (profil de configuration macOS, règle AppLocker ou Intune).

## Déployer en entreprise

**Windows**, installation silencieuse pour tous les utilisateurs :

```bat
msiexec /i Correcteur_0.4.0_x64_fr-FR.msi /qn
```

Le MSI convient aux outils de déploiement (Intune, SCCM, GPO). WebView2,
nécessaire à l'application, est présent sur Windows 10 et 11 à jour ; à
défaut, l'installateur le télécharge.

**macOS** : copier `Correcteur.app` dans `/Applications` (depuis le `.dmg`,
ou par un outil de gestion de parc).

**Réglages et dictionnaire** : chaque utilisateur a les siens (voir
`apps/bureau/LISEZMOI.md`). Pour fournir un dictionnaire commun, déposer un
`dictionnaire.txt` dans le dossier de réglages de chaque utilisateur avant le
premier lancement.

**Confidentialité** : le texte, les réglages et le brouillon restent sur le
poste. La seule connexion réseau est la recherche des mises à jour, faite à la
demande de l'utilisateur ou, s'il l'active, au démarrage.

## Mises à jour

L'application cherche les mises à jour dans le fichier `latest.json` de la
dernière publication :
`https://github.com/Bicyclean/correcteur/releases/latest/download/latest.json`.
Il donne la version, les notes et, par système, l'adresse et la signature du
paquet (`.app.tar.gz` sur macOS, installateur sur Windows : le `.msi` pour un
poste installé par le `.msi`, le `-setup.exe` sinon). La recherche est faite à
la demande (Réglages > Application) ou, si l'utilisateur l'active, au
démarrage une fois par jour ; l'installation demande toujours son accord.

**Signature.** Chaque paquet est signé par une clé privée (minisign) ; la clé
publique est dans `apps/bureau/src-tauri/tauri.conf.json`, et l'application
refuse tout paquet dont la signature ne correspond pas. La clé privée et son
mot de passe sont :

- dans les secrets du dépôt GitHub `TAURI_SIGNING_PRIVATE_KEY` et
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, utilisés par le workflow Publication ;
- sur le poste du mainteneur, dans `~/.tauri/correcteur.key` et
  `~/.tauri/correcteur.key.motdepasse`, **à sauvegarder** (gestionnaire de mots
  de passe, coffre chiffré).

Si la clé est perdue, les postes installés ne pourront plus recevoir de mise à
jour : il faudra leur faire installer une nouvelle version à la main, avec une
nouvelle clé publique.

**Publier une mise à jour** : rien de plus qu'une version normale (étiquette
`v…`). Le workflow signe les paquets et joint `latest.json` à la publication ;
les postes ne la voient qu'une fois la publication rendue publique.

**En entreprise** : laisser la recherche automatique coupée (valeur par défaut)
et déployer chaque version par l'outil de gestion du parc. Une mise à jour
lancée depuis l'application sur un poste installé par le `.msi` demande les
droits d'administrateur.
