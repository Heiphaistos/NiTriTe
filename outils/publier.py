# -*- coding: utf-8 -*-
"""Fabrique et publie une version de Nitrite.

Ce que produit ce script, a partir d'un `npx tauri build` deja fait :

  Pour le client, sur la page GitHub -- deux fichiers, rien d'autre :
    Nitrite_<v>_portable_complet.exe       application + contenu, sans installation
    Nitrite_<v>_installateur_complet.exe   application + contenu, installe

  Pour l'application elle-meme, sur nitrite.heiphaistos.org/maj/ -- invisible
  pour le client, jamais telecharge a la main :
    latest.json / latest-portable.json     manifestes
    nitrite-<v>-installee.exe  + .sig      installeur leger (chemin Tauri)
    nitrite-<v>-portable.exe   + .sig      binaire nu (chemin portable)

La charge de mise a jour ne contient JAMAIS logiciel, Drivers ni Script Windows :
ces dossiers appartiennent a Momo, une mise a jour ne les touche pas.

`build.bat` fait un travail voisin mais s'auto-eleve et attend une saisie au
clavier : inutilisable depuis une session automatisee.

Usage :
    python outils/publier.py --etape sfx     # fabrique les deux SFX clients
    python outils/publier.py --etape canal   # prepare et televerse le canal
    python outils/publier.py --etape tout
"""

import argparse
import hashlib
import io
import json
import os
import shutil
import subprocess
import sys
from datetime import datetime, timezone

RACINE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SEPTZIP = r"C:\Program Files\7-Zip\7z.exe"
STUB = os.path.join(RACINE, "tools", "7zSD.sfx")
SORTIE = os.path.join(RACINE, "release")
CONTENU = ["logiciel", "Drivers", "Script Windows"]
VPS = "root@212.227.140.45"
VPS_DIR = "/var/www/nitrite-maj"
BASE_URL = "https://nitrite.heiphaistos.org/maj"


def version():
    with io.open(os.path.join(RACINE, "package.json"), encoding="utf-8") as f:
        return json.load(f)["version"]


def executer(cmd, **kw):
    print("  $", os.path.basename(str(cmd[0])), " ".join(str(c) for c in cmd[1:4]))
    r = subprocess.run(cmd, cwd=RACINE, **kw)
    if r.returncode != 0:
        raise SystemExit("echec : %s (code %d)" % (cmd[0], r.returncode))
    return r


def outil(nom):
    """Chemin complet d'un outil en ligne de commande, sans passer par un shell."""
    for candidat in (nom + ".cmd", nom + ".exe", nom):
        trouve = shutil.which(candidat)
        if trouve:
            return trouve
    raise SystemExit("%s introuvable dans le PATH" % nom)


def chemins(v):
    nsis = os.path.join(RACINE, "src-tauri", "target", "release", "bundle", "nsis")
    return {
        "exe": os.path.join(RACINE, "src-tauri", "target", "release", "nitrite.exe"),
        "setup": os.path.join(nsis, "Nitrite_%s_x64-setup.exe" % v),
        "setup_sig": os.path.join(nsis, "Nitrite_%s_x64-setup.exe.sig" % v),
    }


def verifier_build(v):
    c = chemins(v)
    for cle, p in c.items():
        if not os.path.exists(p):
            raise SystemExit(
                "%s manquant : lancer d'abord `npx tauri build` avec "
                "TAURI_SIGNING_PRIVATE_KEY dans l'environnement (%s)" % (cle, p)
            )
    return c


def assembler_sfx(destination, config, contenu_7z):
    """stub + configuration (en CRLF) + archive = executable auto-extractible."""
    with io.open(destination, "wb") as sortie:
        with io.open(STUB, "rb") as f:
            sortie.write(f.read())
        sortie.write(config.encode("utf-8"))
        with io.open(contenu_7z, "rb") as f:
            while True:
                bloc = f.read(8 << 20)
                if not bloc:
                    break
                sortie.write(bloc)
    taille = os.path.getsize(destination)
    print("  -> %s : %.1f Mio" % (os.path.basename(destination), taille / 1048576.0))


def config_sfx(titre, invite, programme):
    return (
        ";!@Install@!UTF-8!\r\n"
        'Title="%s"\r\n'
        'BeginPrompt="%s"\r\n'
        'RunProgram="%s"\r\n'
        ";!@InstallEnd@!\r\n" % (titre, invite, programme)
    )


def archiver_contenu(archive):
    """Les trois dossiers, compresses directement depuis le depot.

    Ne jamais recopier ces 733 Mo dans un dossier de preparation comme le fait
    build.bat : 7-Zip sait lire les dossiers la ou ils sont.
    """
    if os.path.exists(archive):
        os.remove(archive)
    cmd = [SEPTZIP, "a", "-t7z", "-m0=lzma2", "-mx=5", "-mmt=on", "-ms=on", archive]
    cmd += ["./" + d for d in CONTENU]
    executer(cmd + ["-y"], stdout=subprocess.DEVNULL)


def etape_sfx(v):
    c = verifier_build(v)
    if not os.path.exists(STUB):
        raise SystemExit(
            "tools/7zSD.sfx manquant. 7-Zip ne le distribue plus : le reprendre "
            "dans les 215040 premiers octets d'un SFX Nitrite deja publie."
        )
    os.makedirs(SORTIE, exist_ok=True)
    archive = os.path.join(SORTIE, "_contenu.7z")

    print("[1/3] Compression du contenu (logiciel, Drivers, Script Windows)...")
    archiver_contenu(archive)

    print("[2/3] Version portable complete...")
    portable_7z = os.path.join(SORTIE, "_portable.7z")
    shutil.copyfile(archive, portable_7z)
    tmp_exe = os.path.join(SORTIE, "Nitrite.exe")
    shutil.copyfile(c["exe"], tmp_exe)
    executer([SEPTZIP, "a", "-t7z", "-m0=lzma2", "-mx=5", "-mmt=on", portable_7z, tmp_exe, "-y"],
             stdout=subprocess.DEVNULL)
    os.remove(tmp_exe)
    assembler_sfx(
        os.path.join(SORTIE, "Nitrite_%s_portable_complet.exe" % v),
        config_sfx("Nitrite %s (portable)" % v,
                   "Extraire Nitrite %s ? Aucune installation, aucune trace." % v,
                   "Nitrite.exe"),
        portable_7z,
    )
    os.remove(portable_7z)

    print("[3/3] Installateur complet...")
    inst_7z = os.path.join(SORTIE, "_installateur.7z")
    shutil.copyfile(archive, inst_7z)
    a_joindre = [c["setup"], os.path.join(RACINE, "outils", "install.cmd")]
    executer([SEPTZIP, "a", "-t7z", "-m0=lzma2", "-mx=5", "-mmt=on", inst_7z] + a_joindre + ["-y"],
             stdout=subprocess.DEVNULL)
    assembler_sfx(
        os.path.join(SORTIE, "Nitrite_%s_installateur_complet.exe" % v),
        config_sfx("Nitrite %s (installation)" % v,
                   "Installer Nitrite %s sur cet ordinateur ?" % v,
                   "install.cmd"),
        inst_7z,
    )
    os.remove(inst_7z)
    os.remove(archive)


def lire_sig(chemin):
    with io.open(chemin, encoding="utf-8") as f:
        return f.read().strip()


def signer(chemin):
    """Signe un fichier avec la cle du projet et rend la signature en base64."""
    cle = os.path.expanduser(os.path.join("~", ".tauri", "nitrite-updater.key"))
    if not os.path.exists(cle):
        raise SystemExit("cle privee introuvable : %s" % cle)
    executer([outil("npx"), "tauri", "signer", "sign", "-f", cle, "-p", "", chemin],
             stdout=subprocess.DEVNULL)
    return lire_sig(chemin + ".sig")


def manifeste(v, url, signature, notes):
    # `notes` s'affiche DANS la fenetre de proposition, juste sous la ligne qui
    # annonce deja la version. Y remettre le numero (« Nitrite 8.218.0 »)
    # n'apprenait rien a personne et faisait doublon a l'ecran. On n'ecrit ici
    # que ce qui a une valeur pour l'utilisateur, ou rien.
    return json.dumps(
        {
            "version": v,
            "notes": notes,
            "pub_date": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "platforms": {"windows-x86_64": {"signature": signature, "url": url}},
        },
        indent=2,
    )


def sha256(chemin):
    h = hashlib.sha256()
    with io.open(chemin, "rb") as f:
        for bloc in iter(lambda: f.read(1 << 20), b""):
            h.update(bloc)
    return h.hexdigest()


def etape_canal(v, notes):
    c = verifier_build(v)
    prep = os.path.join(SORTIE, "_canal")
    if os.path.exists(prep):
        shutil.rmtree(prep)
    os.makedirs(prep)

    nom_inst = "nitrite-%s-installee.exe" % v
    nom_port = "nitrite-%s-portable.exe" % v

    # Installeur leger : deja signe par le bundler pendant `tauri build`.
    shutil.copyfile(c["setup"], os.path.join(prep, nom_inst))
    shutil.copyfile(c["setup_sig"], os.path.join(prep, nom_inst + ".sig"))
    sig_inst = lire_sig(c["setup_sig"])

    # Binaire nu : c'est ce que la version portable remplace chez elle.
    cible_port = os.path.join(prep, nom_port)
    shutil.copyfile(c["exe"], cible_port)
    sig_port = signer(cible_port)

    with io.open(os.path.join(prep, "latest.json"), "w", encoding="utf-8", newline="\n") as f:
        f.write(manifeste(v, "%s/%s" % (BASE_URL, nom_inst), sig_inst, notes))
    with io.open(os.path.join(prep, "latest-portable.json"), "w", encoding="utf-8", newline="\n") as f:
        f.write(manifeste(v, "%s/%s" % (BASE_URL, nom_port), sig_port, notes))

    print("  empreinte du binaire portable : %s..." % sha256(cible_port)[:16])
    print("[canal] televersement vers %s:%s" % (VPS, VPS_DIR))
    fichiers = [os.path.join(prep, n) for n in sorted(os.listdir(prep))]
    executer([outil("scp"), "-o", "BatchMode=yes"] + fichiers + ["%s:%s/" % (VPS, VPS_DIR)])
    print("  -> %s/latest.json" % BASE_URL)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--etape", choices=["sfx", "canal", "tout"], default="tout")
    p.add_argument(
        "--notes",
        default="",
        help="une phrase affichee dans la fenetre de proposition de mise a jour ; "
        "vide par defaut, le numero de version y figure deja",
    )
    a = p.parse_args()
    v = version()
    print("Nitrite %s" % v)
    if a.etape in ("sfx", "tout"):
        etape_sfx(v)
    if a.etape in ("canal", "tout"):
        etape_canal(v, a.notes)
    return 0


if __name__ == "__main__":
    sys.exit(main())
