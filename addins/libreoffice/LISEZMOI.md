# Extension LibreOffice

Vérificateur grammatical pour LibreOffice Writer : homophones, grammaire et
typographie. LibreOffice soumet chaque paragraphe à l'extension, souligne les
erreurs pendant la frappe et propose les corrections au clic droit et dans
*Outils > Orthographe et grammaire*. L'analyse est faite sur l'ordinateur par
la bibliothèque native du projet ; aucun texte n'est envoyé.

L'orthographe d'usage (mots inconnus) n'est pas signalée par l'extension :
LibreOffice la vérifie déjà avec son propre dictionnaire, et les deux
soulignements feraient doublon.

## Installer

1. *Outils > Gestionnaire des extensions > Ajouter*, puis choisir le fichier
   `.oxt` correspondant au système (`dist/correcteur-…-macos.oxt`,
   `…-windows_x86_64.oxt`, `…-linux_x86_64.oxt`).
2. Redémarrer LibreOffice.
3. Vérifier que la langue du texte est le français (*Outils > Langue*) et que
   *Outils > Options > Paramètres linguistiques > Aides à l'écriture* coche
   « Vérifier la grammaire en cours de frappe » et le module « Correcteur
   (français) ».

L'extension macOS contient une bibliothèque universelle : elle fonctionne avec
LibreOffice pour Intel comme pour Apple Silicon.

## Construire

Rust (avec, sur macOS, les cibles `x86_64-apple-darwin` et
`aarch64-apple-darwin`) et Python 3 :

```bash
python3 construire.py
```

## Tester

Le test installe l'extension dans un **profil LibreOffice temporaire** (le
profil de l'utilisateur n'est pas touché), lance LibreOffice sans interface et
interroge le vérificateur avec quelques phrases :

```bash
python3 tests/tester.py dist/correcteur-0.2.0-macos.oxt
```

Sous Debian ou Ubuntu, le test exige les paquets `python3-uno` et
`libreoffice-script-provider-python` (exécution des macros Python).

L'installation passe par le gestionnaire d'extensions, et non par
`unopkg add`, qui échoue sur certaines installations (LibreOfficeDev 26.8
bêta sur macOS). Deux modes, mêmes contrôles (`tests/controles.py`) :
LibreOffice piloté par socket quand le Python utilisé sait importer `uno`
(Linux : `/usr/bin/python3`), macro Python exécutée dans LibreOffice sinon
(macOS, dont le Python embarqué ne peut pas être lancé seul).

Sous Linux, ne lancez pas LibreOffice avec un `LD_LIBRARY_PATH` qui pointe
vers un autre Python (c'est le cas après `actions/setup-python`) : il chargerait
cette bibliothèque Python à la place de la sienne et planterait
(`std::bad_alloc`).

## Organisation

| Fichier | Rôle |
|---|---|
| `oxt/correcteur_lo.py` | Composant UNO (service `Proofreader`) ; charge la bibliothèque avec ctypes. |
| `oxt/Linguistic.xcu` | Enregistre le vérificateur pour fr-FR, fr-BE, fr-CA, fr-CH, fr-LU, fr-MC. |
| `oxt/description.xml.modele` | Description de l'extension (version et plateforme remplies à la construction). |
| `construire.py` | Compile `crates/correcteur-ffi` et assemble le `.oxt` avec les données. |
| `tests/` | Test dans LibreOffice, sans interface. |

Positions : LibreOffice compte en unités UTF-16 ; le moteur renvoie des
positions dans cette unité, et la longueur des paragraphes est calculée de
même (un émoji compte pour deux).
