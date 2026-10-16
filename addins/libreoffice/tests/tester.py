#!/usr/bin/env python3
"""Teste l'extension dans LibreOffice, sans interface et avec un profil temporaire.

    python3 tests/tester.py dist/correcteur-….oxt [--soffice CHEMIN]

Le profil de l'utilisateur n'est pas touché : l'extension est installée dans
un dossier temporaire supprimé à la fin. L'installation passe par le
gestionnaire d'extensions, depuis une macro (voir macro_installer.py).
"""

import glob
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ICI = Path(__file__).resolve().parent


def soffice_par_defaut():
    for c in (
        "/Applications/LibreOffice.app/Contents/MacOS/soffice",
        "/Applications/LibreOfficeDev.app/Contents/MacOS/soffice",
        r"C:\Program Files\LibreOffice\program\soffice.exe",
        "/usr/bin/soffice",
        "/usr/lib/libreoffice/program/soffice",
    ):
        if os.path.exists(c):
            return c
    sys.exit("soffice introuvable : précisez --soffice")


def main():
    args = sys.argv[1:]
    soffice = args[args.index("--soffice") + 1] if "--soffice" in args else soffice_par_defaut()
    oxt = next(a for a in args if a.endswith(".oxt"))
    if "*" in oxt:
        oxt = sorted(glob.glob(oxt))[-1]
    profil = Path(tempfile.mkdtemp(prefix="correcteur-lo-"))
    try:
        env = f"-env:UserInstallation={profil.as_uri()}"
        subprocess.run([soffice, "--headless", "--norestore", "--terminate_after_init", env], timeout=300)
        scripts = profil / "user" / "Scripts" / "python"
        scripts.mkdir(parents=True, exist_ok=True)

        def macro(nom, **valeurs):
            texte = (ICI / f"{nom}.py").read_text(encoding="utf-8")
            for cle, val in valeurs.items():
                texte = texte.replace(f"@{cle}@", val)
            (scripts / f"{nom}.py").write_text(texte, encoding="utf-8")
            subprocess.run(
                [soffice, "--headless", "--norestore", "--nologo", env,
                 f"vnd.sun.star.script:{nom}.py$main?language=Python&location=user"],
                timeout=300,
            )

        # 1. Installation, dans le processus de LibreOffice.
        installe = profil / "installation.txt"
        macro("macro_installer", OXT=Path(oxt).resolve().as_posix(), SORTIE=installe.as_posix())
        etat = installe.read_text(encoding="utf-8") if installe.exists() else "aucune réponse"
        if etat != "ok":
            sys.exit(f"installation de l'extension impossible :\n{etat}")
        # 2. Vérifications, après redémarrage (les services de l'extension sont alors enregistrés).
        sortie = profil / "resultat.json"
        macro("macro_test", SORTIE=sortie.as_posix())
        if not sortie.exists():
            sys.exit("la macro n'a rien écrit : LibreOffice n'a pas pu l'exécuter")
        res = json.loads(sortie.read_text(encoding="utf-8"))
        print(json.dumps(res, ensure_ascii=False, indent=1))
        verifier(res)
        print("extension LibreOffice : tous les contrôles passent")
    finally:
        shutil.rmtree(profil, ignore_errors=True)


def verifier(res):
    assert "erreur" not in res, res.get("erreur")
    assert "org.correcteur.Correcteur" in res["disponibles"], "vérificateur non enregistré pour fr-FR"
    par_texte = {r["texte"]: r for r in res["resultats"]}
    attendu = {
        "Il à faim.": ("à", "HOMO_A_VERBE", "a"),
        "Les enfants mange des pommes.": ("mange", "GRAM_ACCORD_SUJET_VERBE", "mangent"),
        "Quelle belle journée!": ("!", "TYPO_ESPACE_AVANT_MANQUANTE", "\u202f!"),
        # Après un émoji : positions en UTF-16, comme dans LibreOffice.
        "😀 Elle est parti hier.": ("parti", "GRAM_PARTICIPE_ETRE", "partie"),
    }
    for texte, (extrait, regle, suggestion) in attendu.items():
        erreurs = par_texte[texte]["erreurs"]
        assert any(
            e["extrait"] == extrait and e["regle"] == regle and e["suggestions"][:1] == [suggestion]
            for e in erreurs
        ), f"{texte} : {erreurs}"
    assert par_texte["Le chat dort."]["erreurs"] == [], "faux positif sur une phrase correcte"
    for r in res["resultats"]:
        assert r["suivant"] == len(r["texte"].encode("utf-16-le")) // 2, "paragraphe entier non couvert"


if __name__ == "__main__":
    main()
