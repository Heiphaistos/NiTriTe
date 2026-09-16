// Mise a jour automatique — les DEUX versions, par deux chemins differents.
//
//   version installee : plugin officiel de Tauri. Il telecharge l'installeur
//                       NSIS leger, le lance, et arrete l'application lui-meme
//                       (`process::exit(0)` cote Rust) en passant /R a NSIS pour
//                       qu'elle redemarre. Rien a relancer nous-memes.
//
//   version portable  : `updater_portable` cote Rust. Le plugin ne sait pas
//                       remplacer un executable sur place sous Windows, donc ce
//                       chemin le fait : verification de signature, ecartement
//                       de l'ancien binaire, mise en place du neuf, relance.
//
// Les deux lisent un manifeste signe sur nitrite.heiphaistos.org. Ni l'un ni
// l'autre ne touche a `logiciel\`, `Drivers\` ou `Script Windows\` : la charge
// telechargee ne contient que l'application.

import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { ask, message } from "@tauri-apps/plugin-dialog";
import { logger } from "@/utils/logger";
import { invoke } from "@/utils/invoke";

interface MajPortable {
  version: string;
  version_actuelle: string;
  notes: string;
}

function proposer(version: string, actuelle: string, notes: string): Promise<boolean> {
  return ask(
    `Nitrite ${version} est disponible (vous avez la ${actuelle}).` +
      (notes ? `\n\n${notes}` : "") +
      "\n\nVoulez-vous la mettre a jour maintenant ? L'application redemarrera.",
    { title: "Une nouvelle version est sortie", kind: "info" },
  );
}

/** Version posee par l'installeur : chemin Tauri standard. */
async function majInstallee(silencieux: boolean): Promise<boolean> {
  const update = await check();
  if (!update) {
    if (!silencieux) await message("Nitrite est a jour.", { title: "Mise a jour" });
    return false;
  }

  logger.info("SYSTEM", `Mise a jour disponible : ${update.version}`);
  if (!(await proposer(update.version, update.currentVersion, update.body ?? ""))) return false;

  // Ne rend jamais la main : le plugin arrete l'application pour laisser
  // l'installeur remplacer les fichiers, puis NSIS la relance.
  await update.downloadAndInstall();
  return true;
}

/** Copie portable : remplacement de l'executable sur place. */
async function majPortable(silencieux: boolean): Promise<boolean> {
  const maj = await invoke<MajPortable | null>("portable_maj_verifier");
  if (!maj) {
    if (!silencieux) await message("Nitrite est a jour.", { title: "Mise a jour" });
    return false;
  }

  logger.info("SYSTEM", `Mise a jour disponible : ${maj.version}`);
  if (!(await proposer(maj.version, maj.version_actuelle, maj.notes))) return false;

  await invoke("portable_maj_appliquer", { maj }, 300_000);
  logger.info("SYSTEM", `Mise a jour portable installee : ${maj.version}`);
  await relaunch();
  return true;
}

/**
 * Cherche une mise a jour, la propose, l'installe et redemarre.
 * @param silencieux true = ne rien afficher s'il n'y a rien ou si le canal est
 *                   injoignable (verification au demarrage) ; false = bouton
 *                   « Verifier les mises a jour ».
 * @returns true si une mise a jour a ete lancee.
 */
export async function checkForUpdate(silencieux = true): Promise<boolean> {
  // En dev le binaire n'est pas installe : rien a remplacer.
  if (import.meta.env.DEV) return false;

  try {
    return (await invoke<boolean>("est_installee"))
      ? await majInstallee(silencieux)
      : await majPortable(silencieux);
  } catch (e) {
    // Canal injoignable, serveur en panne, signature refusee : l'utilisateur
    // continue de travailler avec la version qu'il a.
    logger.error("SYSTEM", "Verification de mise a jour impossible", e);
    if (!silencieux) {
      await message(`Verification impossible : ${e}`, { title: "Mise a jour", kind: "error" });
    }
    return false;
  }
}
