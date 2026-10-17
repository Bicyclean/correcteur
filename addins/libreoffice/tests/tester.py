#!/usr/bin/env python3
"""Teste l'extension dans LibreOffice, sans interface et avec un profil temporaire.

    python3 tests/tester.py dist/correcteur-….oxt [--soffice CHEMIN]

Le profil de l'utilisateur n'est pas touché : l'extension est installée dans
un dossier temporaire supprimé à la fin, par le gestionnaire d'extensions.

Deux modes, mêmes contrôles (controles.py) :
- socket, si ce Python sait importer « uno » (Linux : /usr/bin/python3 avec
  python3-uno) : LibreOffice est piloté à distance ;
- macro sinon (macOS : le Python de LibreOffice ne peut pas être lancé seul) :
  les contrôles s'exécutent dans LibreOffice, comme macro Python.
"""

import glob
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time
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
        # unopkg d'abord ; il échoue sur certaines installations (LibreOfficeDev 26.8 sur
        # macOS), le gestionnaire d'extensions prend alors le relais.
        unopkg = str(Path(soffice).with_name("unopkg.exe" if soffice.endswith(".exe") else "unopkg"))
        installe = subprocess.run([unopkg, "add", "-s", env, str(Path(oxt).resolve())], timeout=300).returncode == 0
        print(f"unopkg add : {'réussi' if installe else 'échec, installation par le gestionnaire'}")
        try:
            import uno  # noqa: F401
        except ImportError:
            res = mode_macro(soffice, env, profil, Path(oxt).resolve(), installe)
        else:
            res = mode_socket(soffice, env, Path(oxt).resolve(), installe)
        print(json.dumps(res, ensure_ascii=False, indent=1))
        verifier(res)
        print("extension LibreOffice : tous les contrôles passent")
    finally:
        shutil.rmtree(profil, ignore_errors=True)


ENVELOPPE = """

def main(*args):
    resultat = {appel}
    with open({sortie!r}, "w", encoding="utf-8") as f:
        f.write(resultat if isinstance(resultat, str) else json.dumps(resultat, ensure_ascii=False))
    XSCRIPTCONTEXT.getDesktop().terminate()


import json  # noqa: E402
g_exportedScripts = (main,)
"""


def mode_macro(soffice, env, profil, oxt, deja_installe):
    scripts = profil / "user" / "Scripts" / "python"
    scripts.mkdir(parents=True, exist_ok=True)
    controles = (ICI / "controles.py").read_text(encoding="utf-8")

    def macro(nom, appel, sortie):
        texte = controles + ENVELOPPE.format(appel=appel, sortie=sortie.as_posix())
        (scripts / f"{nom}.py").write_text(texte, encoding="utf-8")
        subprocess.run(
            [soffice, "--headless", "--norestore", "--nologo", env,
             f"vnd.sun.star.script:{nom}.py$main?language=Python&location=user"],
            timeout=300,
        )
        return sortie.read_text(encoding="utf-8") if sortie.exists() else None

    # 1. Installation, dans le processus de LibreOffice.
    etat = "ok" if deja_installe else macro(
        "installer", f"installer(XSCRIPTCONTEXT.getComponentContext(), {oxt.as_posix()!r})",
        profil / "installation.txt")
    if etat != "ok":
        sys.exit(f"installation de l'extension impossible :\n{etat or 'aucune réponse'}")
    # 2. Contrôles, après redémarrage (les services de l'extension sont alors enregistrés).
    sortie = macro("controler", "mesurer(XSCRIPTCONTEXT.getComponentContext())", profil / "resultat.json")
    if sortie is None:
        sys.exit("la macro n'a rien écrit : LibreOffice n'a pas pu l'exécuter")
    return json.loads(sortie)


def mode_socket(soffice, env, oxt, deja_installe):
    import uno

    sys.path.insert(0, str(ICI))
    import controles

    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        port = s.getsockname()[1]
    adresse = f"socket,host=127.0.0.1,port={port};urp"

    def demarrer():
        proc = subprocess.Popen([soffice, "--headless", "--norestore", "--nologo", env, f"--accept={adresse};"])
        local = uno.getComponentContext()
        resolveur = local.ServiceManager.createInstanceWithContext("com.sun.star.bridge.UnoUrlResolver", local)
        for _ in range(120):
            try:
                return proc, resolveur.resolve(f"uno:{adresse};StarOffice.ComponentContext")
            except Exception:  # noqa: BLE001 — LibreOffice démarre encore
                time.sleep(0.5)
        proc.kill()
        sys.exit("LibreOffice n'a pas accepté la connexion")

    def arreter(proc, ctx):
        try:
            ctx.ServiceManager.createInstanceWithContext("com.sun.star.frame.Desktop", ctx).terminate()
        except Exception:  # noqa: BLE001 — la connexion se ferme pendant l'arrêt
            pass
        try:
            proc.wait(timeout=60)
        except subprocess.TimeoutExpired:
            proc.kill()

    if not deja_installe:
        proc, ctx = demarrer()
        etat = controles.installer(ctx, str(oxt))
        arreter(proc, ctx)
    else:
        etat = "ok"
    if etat != "ok":
        sys.exit(f"installation de l'extension impossible :\n{etat}")
    proc, ctx = demarrer()
    res = controles.mesurer(ctx)
    arreter(proc, ctx)
    return res


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
