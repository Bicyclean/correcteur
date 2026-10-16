# Extension LibreOffice : vérificateur grammatical (service Proofreader).
#
# LibreOffice transmet chaque paragraphe à doProofreading et souligne les
# erreurs renvoyées ; les suggestions apparaissent au clic droit et dans
# Outils > Orthographe et grammaire. L'analyse est faite par la bibliothèque
# native du projet (lib/), chargée avec ctypes. Aucun accès réseau.
#
# Licence : GPL-3.0-or-later.

import ctypes
import json
import os
import platform
import sys

import uno  # noqa: F401 — active les imports « com.sun.star »
import unohelper
from com.sun.star.beans import PropertyValue
from com.sun.star.lang import Locale, XServiceDisplayName, XServiceInfo
from com.sun.star.linguistic2 import (
    ProofreadingResult,
    SingleProofreadingError,
    XProofreader,
    XSupportedLocales,
)
from com.sun.star.text.TextMarkupType import PROOFREADING

IMPLEMENTATION = "org.correcteur.Correcteur"
SERVICES = ("com.sun.star.linguistic2.Proofreader",)
COUNTRIES = ("FR", "BE", "CA", "CH", "LU", "MC")

# LibreOffice a son propre correcteur orthographique (Hunspell) : on ne
# double pas ses signalements. Les autres catégories sont vérifiées.
OPTIONS = "categorie_off=orthographe"

# Couleur du soulignement par catégorie (0xRRGGBB).
LINE_COLORS = {
    "homophone": 0x7C3AED,
    "grammaire": 0x0F766E,
    "typographie": 0x2563EB,
    "orthographe": 0xC2410C,
}

HERE = os.path.dirname(os.path.abspath(__file__))


def utf16_len(text):
    """Longueur en unités UTF-16, l'unité des positions dans LibreOffice
    (un émoji compte pour deux, alors que Python le compte pour un)."""
    return len(text.encode("utf-16-le")) // 2


def library_path():
    """Bibliothèque native adaptée au système et au processeur de LibreOffice."""
    machine = platform.machine().lower()
    arch = "aarch64" if machine in ("arm64", "aarch64") else "x86_64"
    if sys.platform == "darwin":
        # Bibliothèque universelle (Intel et Apple Silicon).
        return os.path.join(HERE, "lib", "macos", "libcorrecteur.dylib")
    if sys.platform.startswith("win"):
        return os.path.join(HERE, "lib", "windows-" + arch, "correcteur.dll")
    return os.path.join(HERE, "lib", "linux-" + arch, "libcorrecteur.so")


class Engine:
    """Accès à la bibliothèque native (voir crates/correcteur-ffi)."""

    def __init__(self, lib_path, data_dir):
        lib = ctypes.CDLL(lib_path)
        lib.cr_engine_new.argtypes = [ctypes.c_char_p, ctypes.c_size_t]
        lib.cr_engine_new.restype = ctypes.c_void_p
        lib.cr_check.argtypes = [
            ctypes.c_void_p,
            ctypes.c_char_p,
            ctypes.c_size_t,
            ctypes.c_char_p,
            ctypes.c_size_t,
        ]
        # c_void_p et non c_char_p : il faut garder le pointeur pour le libérer.
        lib.cr_check.restype = ctypes.c_void_p
        lib.cr_string_free.argtypes = [ctypes.c_void_p]
        lib.cr_engine_free.argtypes = [ctypes.c_void_p]
        lib.cr_last_error.restype = ctypes.c_char_p
        self.lib = lib
        path = data_dir.encode("utf-8")
        self.handle = lib.cr_engine_new(path, len(path))
        if not self.handle:
            raise RuntimeError(self.last_error())

    def last_error(self):
        return (self.lib.cr_last_error() or b"").decode("utf-8", "replace")

    def check(self, text, options=OPTIONS):
        data = text.encode("utf-8")
        opts = options.encode("utf-8")
        ptr = self.lib.cr_check(self.handle, data, len(data), opts, len(opts))
        if not ptr:
            raise RuntimeError(self.last_error())
        try:
            return json.loads(ctypes.string_at(ptr).decode("utf-8"))
        finally:
            self.lib.cr_string_free(ptr)


class Correcteur(
    unohelper.Base, XProofreader, XServiceInfo, XServiceDisplayName, XSupportedLocales
):
    def __init__(self, ctx, *args):
        self.ctx = ctx
        self.locales = tuple(Locale("fr", c, "") for c in COUNTRIES)
        self.ignored_rules = set()
        self.engine = None
        self.error = None

    def _engine(self):
        # Chargement à la première vérification, pour ne pas ralentir le démarrage.
        if self.engine is None and self.error is None:
            try:
                self.engine = Engine(library_path(), os.path.join(HERE, "data"))
            except Exception as e:  # noqa: BLE001 — l'erreur est journalisée une fois
                self.error = str(e)
                print("Correcteur : moteur non chargé :", e, file=sys.stderr)
        return self.engine

    # XServiceInfo
    def getImplementationName(self):
        return IMPLEMENTATION

    def supportsService(self, name):
        return name in SERVICES

    def getSupportedServiceNames(self):
        return SERVICES

    # XServiceDisplayName
    def getServiceDisplayName(self, locale):
        return "Correcteur (français)"

    # XSupportedLocales
    def getLocales(self):
        return self.locales

    def hasLocale(self, locale):
        return locale.Language == "fr"

    # XProofreader
    def isSpellChecker(self):
        return False

    def doProofreading(self, doc_id, text, locale, start, suggested_end, properties):
        result = ProofreadingResult()
        result.aDocumentIdentifier = doc_id
        result.xFlatParagraph = None
        result.aText = text
        result.aLocale = locale
        result.nStartOfSentencePosition = start
        # Le paragraphe entier est analysé d'un coup : LibreOffice n'a pas à
        # rappeler pour les phrases suivantes.
        result.nBehindEndOfSentencePosition = utf16_len(text)
        result.nStartOfNextSentencePosition = utf16_len(text)
        result.aProperties = ()
        result.xProofreader = self
        result.aErrors = ()
        engine = self._engine()
        if engine is None or start > 0 or not text.strip():
            return result
        try:
            issues = engine.check(text)
        except Exception as e:  # noqa: BLE001
            print("Correcteur : analyse impossible :", e, file=sys.stderr)
            return result
        result.aErrors = tuple(
            self._error(i) for i in issues if i["rule"] not in self.ignored_rules
        )
        return result

    def _error(self, issue):
        e = SingleProofreadingError()
        e.nErrorStart = issue["start"]
        e.nErrorLength = max(1, issue["end"] - issue["start"])
        e.nErrorType = PROOFREADING
        e.aRuleIdentifier = issue["rule"]
        e.aShortComment = issue["message"]
        e.aFullComment = issue["explanation"]
        e.aSuggestions = tuple(issue["suggestions"])
        color = PropertyValue()
        color.Name = "LineColor"
        color.Value = LINE_COLORS.get(issue["category"], 0x2563EB)
        e.aProperties = (color,)
        return e

    def ignoreRule(self, rule, locale):
        self.ignored_rules.add(rule)

    def resetIgnoreRules(self):
        self.ignored_rules.clear()


g_ImplementationHelper = unohelper.ImplementationHelper()
g_ImplementationHelper.addImplementation(Correcteur, IMPLEMENTATION, SERVICES)
