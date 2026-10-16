# Macro exécutée par tester.py : installe l'extension @OXT@ dans le profil
# courant avec le gestionnaire d'extensions, dans le processus de LibreOffice
# (unopkg passe par un processus séparé, qui échoue sur certaines installations).
import traceback

import uno

import unohelper
from com.sun.star.task import XInteractionHandler
from com.sun.star.ucb import XCommandEnvironment

OXT = "@OXT@"
SORTIE = "@SORTIE@"


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


def main(*args):
    message = "ok"
    try:
        ctx = XSCRIPTCONTEXT.getComponentContext()  # noqa: F821 — fourni par LibreOffice
        gestionnaire = ctx.getValueByName("/singletons/com.sun.star.deployment.ExtensionManager")
        gestionnaire.addExtension(unohelper.systemPathToFileUrl(OXT), (), "user", None, Environnement())
    except Exception as e:  # noqa: BLE001
        message = traceback.format_exc()
        cause = getattr(e, "Cause", None)
        while cause is not None:
            message += f"\ncause : {getattr(cause, 'Message', cause)}"
            cause = getattr(cause, "Cause", None)
    with open(SORTIE, "w", encoding="utf-8") as f:
        f.write(message)
    XSCRIPTCONTEXT.getDesktop().terminate()  # noqa: F821


g_exportedScripts = (main,)
