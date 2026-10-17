#!/usr/bin/env python3
"""Construit l'extension LibreOffice (.oxt) pour le système courant.

    python3 construire.py                 compile la bibliothèque native et assemble dist/*.oxt
    python3 construire.py --sans-compiler réutilise les bibliothèques déjà compilées

Sur macOS, la bibliothèque est universelle (Intel et Apple Silicon) : il faut
les cibles Rust x86_64-apple-darwin et aarch64-apple-darwin.
"""

import os
import platform
import re
import shutil
import subprocess
import sys
import zipfile

ICI = os.path.dirname(os.path.abspath(__file__))
RACINE = os.path.abspath(os.path.join(ICI, "..", ".."))
OXT = os.path.join(ICI, "oxt")
DIST = os.path.join(ICI, "dist")


def version():
    with open(os.path.join(RACINE, "Cargo.toml"), encoding="utf-8") as f:
        return re.search(r'^version = "([^"]+)"', f.read(), re.M).group(1)


def environnement_rust():
    """Préfère la chaîne rustup, qui a les cibles croisées (le Rust de Homebrew ne les a pas)."""
    env = dict(os.environ)
    for rustup in ("rustup", "/opt/homebrew/opt/rustup/bin/rustup", "/usr/local/opt/rustup/bin/rustup"):
        for extra in ([], ["--toolchain", "stable"]):
            try:
                rustc = subprocess.run(
                    [rustup, "which", "rustc", *extra], capture_output=True, text=True, check=True
                ).stdout.strip()
            except (OSError, subprocess.CalledProcessError):
                continue
            env["PATH"] = os.path.dirname(rustc) + os.pathsep + env.get("PATH", "")
            return env
    return env


def compiler(cible=None):
    commande = ["cargo", "build", "--release", "-p", "correcteur-ffi"]
    if cible:
        commande += ["--target", cible]
    subprocess.run(commande, cwd=RACINE, check=True, env=environnement_rust())
    return os.path.join(RACINE, "target", *([cible] if cible else []), "release")


def bibliotheques(compiler_aussi):
    """(fichier à inclure, chemin dans l'extension), et valeur de <platform>."""
    machine = platform.machine().lower()
    arch = "aarch64" if machine in ("arm64", "aarch64") else "x86_64"
    if sys.platform == "darwin":
        cibles = ["x86_64-apple-darwin", "aarch64-apple-darwin"]
        dossiers = [compiler(c) if compiler_aussi else os.path.join(RACINE, "target", c, "release") for c in cibles]
        universelle = os.path.join(RACINE, "target", "libcorrecteur-universelle.dylib")
        subprocess.run(
            ["lipo", "-create", *[os.path.join(d, "libcorrecteur.dylib") for d in dossiers], "-output", universelle],
            check=True,
        )
        return [(universelle, "lib/macos/libcorrecteur.dylib")], "macosx_x86_64,macosx_aarch64"
    dossier = compiler() if compiler_aussi else os.path.join(RACINE, "target", "release")
    if sys.platform.startswith("win"):
        return [(os.path.join(dossier, "correcteur.dll"), f"lib/windows-{arch}/correcteur.dll")], f"windows_{arch}"
    return [(os.path.join(dossier, "libcorrecteur.so"), f"lib/linux-{arch}/libcorrecteur.so")], f"linux_{arch}"


def main():
    compiler_aussi = "--sans-compiler" not in sys.argv
    v = version()
    libs, plateformes = bibliotheques(compiler_aussi)
    os.makedirs(DIST, exist_ok=True)
    nom = os.path.join(DIST, f"correcteur-{v}-{plateformes.split(',')[0].replace('macosx_x86_64', 'macos')}.oxt")

    with open(os.path.join(OXT, "description.xml.modele"), encoding="utf-8") as f:
        description = f.read().replace("@VERSION@", v).replace("@PLATEFORMES@", plateformes)

    with zipfile.ZipFile(nom, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("description.xml", description)
        for dossier, _, fichiers in os.walk(OXT):
            for f in sorted(fichiers):
                if f.endswith(".modele") or f.endswith(".pyc"):
                    continue
                chemin = os.path.join(dossier, f)
                z.write(chemin, os.path.relpath(chemin, OXT))
        for source, cible in libs:
            z.write(source, cible)
        z.write(os.path.join(RACINE, "LICENSE"), "LICENSE.txt")
        # Données lues par le moteur, et mentions de licence du lexique.
        # Pas de thésaurus : LibreOffice a le sien.
        for sous in ("lexique", "regles", "conjugaison"):
            base = os.path.join(RACINE, "data", sous)
            for f in sorted(os.listdir(base)):
                if f.endswith((".tsv", ".txt", ".md")):
                    z.write(os.path.join(base, f), f"data/{sous}/{f}")
    print(f"extension construite : {os.path.relpath(nom, os.getcwd())} ({os.path.getsize(nom) // 1024} Ko)")
    return nom


if __name__ == "__main__":
    main()
