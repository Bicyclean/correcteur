# Interface

Un seul panneau, écrit une fois (`web/commun/panneau.ts` et `panneau.css`),
sert l'add-in Word et l'application de bureau. L'hôte ne fournit que l'accès
au document et le moteur.

## Organisation

- **Corrections** : résumé (nombre de suggestions, mots, lisibilité), filtres
  par catégorie, cartes. Une carte montre la catégorie, le message et les
  corrections, la première mise en avant. La carte active se déplie :
  contexte, explication, « Ignorer », « Ajouter au dictionnaire » ou
  « Désactiver cette règle ». En typographie, « Tout appliquer » corrige
  d'un coup les problèmes sans ambiguïté.
- **Outils** : conjugaison (tableau par mode) et synonymes ou antonymes ; un
  mot cliqué remplace la sélection du document.
- **Réglages** : catégories, options typographiques, seuils de style,
  dictionnaire personnel, règles désactivées (réactivables une à une).

Chaque action réversible affiche une notification avec « Annuler ».
L'application de bureau relie texte et cartes : cliquer un mot souligné
ouvre sa carte, activer une carte surligne le passage. Les corrections passent
par l'édition native de la zone de texte : ⌘Z / Ctrl+Z les annule.

## Clavier

| Touche | Effet |
|---|---|
| ↑ / ↓ dans la liste | Problème précédent / suivant |
| Entrée sur une carte | Appliquer la première correction |
| Suppr / Retour arrière | Ignorer |
| Échap | Replier la carte |
| ← / → sur les onglets | Changer d'onglet |
| ⌘J / Ctrl+J (bureau) | Problème suivant (Maj : précédent) |
| ⌘↩ / Ctrl+Entrée (bureau) | Copier le texte et fermer |
| ⌘⇧E / Ctrl+Maj+E (bureau, partout) | Corriger le presse-papiers |

## Accessibilité

- Onglets, groupes de filtres, interrupteurs et listes exposent leurs rôles
  ARIA ; chaque contrôle a un nom explicite.
- Les suggestions sont annoncées en clair (« Remplacer « ! » par « espace fine
  insécable ! » »), les espaces spéciales étant aussi rendues visibles à
  l'écran (« fine », « insécable »).
- Contraste des textes d'au moins 4,5:1 en thème clair comme sombre ; focus
  toujours visible ; animations supprimées si le système le demande
  (`prefers-reduced-motion`).

## Système de design

Jetons CSS dans `:root` (couleurs claires et sombres, espacements sur une
grille de 4 px, rayons, tailles de texte), une couleur par catégorie, police
du système (San Francisco, Segoe UI). Les icônes (`web/commun/icones.ts`) sont
dessinées pour le projet sur une grille de 24.
