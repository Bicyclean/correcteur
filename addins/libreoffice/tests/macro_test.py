# Macro exécutée dans LibreOffice par tester.py : interroge le vérificateur
# et écrit le résultat en JSON. @SORTIE@ est remplacé par tester.py.
import json
import traceback

from com.sun.star.lang import Locale

SORTIE = "@SORTIE@"
PHRASES = [
    "Il à faim.",
    "Les enfants mange des pommes.",
    "Quelle belle journée!",
    "Le chat dort.",
    "😀 Elle est parti hier.",
]


def extrait(texte, debut, longueur):
    """Extrait selon des positions UTF-16 (celles de LibreOffice)."""
    u = texte.encode("utf-16-le")
    return u[2 * debut : 2 * (debut + longueur)].decode("utf-16-le")


def main(*args):
    res = {}
    try:
        ctx = XSCRIPTCONTEXT.getComponentContext()  # noqa: F821 — fourni par LibreOffice
        smgr = ctx.ServiceManager
        fr = Locale("fr", "FR", "")
        lsm = smgr.createInstanceWithContext("com.sun.star.linguistic2.LinguServiceManager", ctx)
        service = "com.sun.star.linguistic2.Proofreader"
        res["disponibles"] = list(lsm.getAvailableServices(service, fr))
        res["configures"] = list(lsm.getConfiguredServices(service, fr))
        p = smgr.createInstanceWithContext("org.correcteur.Correcteur", ctx)
        res["nom"] = p.getServiceDisplayName(fr)
        res["resultats"] = []
        for t in PHRASES:
            longueur = len(t.encode("utf-16-le")) // 2
            r = p.doProofreading("essai", t, fr, 0, longueur, ())
            res["resultats"].append(
                {
                    "texte": t,
                    "suivant": r.nStartOfNextSentencePosition,
                    "erreurs": [
                        {
                            "extrait": extrait(t, e.nErrorStart, e.nErrorLength),
                            "regle": e.aRuleIdentifier,
                            "suggestions": list(e.aSuggestions),
                        }
                        for e in r.aErrors
                    ],
                }
            )
    except Exception:  # noqa: BLE001
        res["erreur"] = traceback.format_exc()
    with open(SORTIE, "w", encoding="utf-8") as f:
        json.dump(res, f, ensure_ascii=False, indent=1)
    XSCRIPTCONTEXT.getDesktop().terminate()  # noqa: F821


g_exportedScripts = (main,)
