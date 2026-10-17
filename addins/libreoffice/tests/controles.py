# Contrôles de l'extension, communs aux deux modes de tester.py :
# exécutés dans LibreOffice (macro) ou depuis un Python qui le pilote (socket).
# Toutes les positions sont en unités UTF-16, comme dans LibreOffice.
import traceback

import uno
import unohelper
from com.sun.star.lang import Locale
from com.sun.star.task import XInteractionHandler
from com.sun.star.ucb import XCommandEnvironment

PHRASES = [
    "Il à faim.",
    "Les enfants mange des pommes.",
    "Quelle belle journée!",
    "Le chat dort.",
    "😀 Elle est parti hier.",
]


class Accepter(unohelper.Base, XInteractionHandler):
    """Répond « oui » aux demandes de confirmation (profil de test seulement)."""

    def handle(self, requete):
        for suite in requete.getContinuations():
            if suite.queryInterface(uno.getTypeByName("com.sun.star.task.XInteractionApprove")):
                suite.select()
                return


class Environnement(unohelper.Base, XCommandEnvironment):
    def getInteractionHandler(self):
        return Accepter()

    def getProgressHandler(self):
        return None


def installer(ctx, oxt):
    """Installe l'extension avec le gestionnaire d'extensions ; « ok » ou le détail de l'erreur."""
    try:
        gestionnaire = ctx.getValueByName("/singletons/com.sun.star.deployment.ExtensionManager")
        gestionnaire.addExtension(unohelper.systemPathToFileUrl(oxt), (), "user", None, Environnement())
        return "ok"
    except Exception as e:  # noqa: BLE001
        message = traceback.format_exc()
        cause = getattr(e, "Cause", None)
        while cause is not None:
            message += f"\ncause : {getattr(cause, 'Message', cause)}"
            cause = getattr(cause, "Cause", None)
        return message


def extrait(texte, debut, longueur):
    u = texte.encode("utf-16-le")
    return u[2 * debut : 2 * (debut + longueur)].decode("utf-16-le")


def mesurer(ctx):
    """Interroge le vérificateur et renvoie un dictionnaire sérialisable en JSON."""
    res = {}
    try:
        smgr = ctx.ServiceManager
        fr = Locale("fr", "FR", "")
        lsm = smgr.createInstanceWithContext("com.sun.star.linguistic2.LinguServiceManager", ctx)
        service = "com.sun.star.linguistic2.Proofreader"
        res["disponibles"] = list(lsm.getAvailableServices(service, fr))
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
    return res
