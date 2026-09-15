// Mise a jour automatique — plugin updater officiel de Tauri v2.
//
// Le manifeste `latest.json` est publie sur la release GitHub par
// .github/workflows/release.yml, signe avec la cle minisign dont la partie
// publique est dans tauri.conf.json. Un binaire non signe par cette cle est
// refuse par le plugin : rien a verifier nous-memes.

import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { ask, message } from "@tauri-apps/plugin-dialog";
import { logger } from "@/utils/logger";
import { invoke } from "@/utils/invoke";

/**
 * Cherche une mise a jour, la propose, l'installe et redemarre.
 * @param silencieux true = ne rien afficher s'il n'y a rien ou si le reseau tombe
 *                   (verification au demarrage) ; false = bouton « Verifier ».
 * @returns true si une installation a ete lancee.
 */
export async function checkForUpdate(silencieux = true): Promise<boolean> {
  // En dev le binaire n'est pas installe : `check()` compare contre une version
  // de travail et l'installeur n'aurait rien a remplacer.
  if (import.meta.env.DEV) return false;

  try {
    // Copie portable / SFX complet : l'installeur NSIS que le programme de mise
    // a jour lance ne contient QUE l'executable. Il poserait une seconde copie
    // dans Program Files, sans `logiciel\` ni les pilotes, pendant que celle-ci
    // resterait a son ancienne version. Ces copies-la se mettent a jour en
    // retelechargeant `Nitrite_vX_full.exe`.
    if (await invoke<boolean>("is_portable_install")) {
      if (!silencieux) {
        await message(
          "Cette copie est la version portable complete (logiciels, pilotes et scripts inclus).\n" +
            "Elle se met a jour en retelechargeant Nitrite_vX_full.exe depuis la page des versions.",
          { title: "Mise a jour" },
        );
      }
      return false;
    }

    const update = await check();
    if (!update) {
      if (!silencieux) await message("NiTriTe est a jour.", { title: "Mise a jour" });
      return false;
    }

    logger.info("SYSTEM", `Mise a jour disponible : ${update.version}`);
    const notes = update.body ? `\n\n${update.body}` : "";
    const ok = await ask(
      `NiTriTe ${update.version} est disponible (version actuelle ${update.currentVersion}).${notes}\n\n` +
        "Installer maintenant ? L'application redemarrera.",
      { title: "Mise a jour disponible", kind: "info" },
    );
    if (!ok) return false;

    await update.downloadAndInstall();
    await relaunch();
    return true;
  } catch (e) {
    logger.error("SYSTEM", "Verification de mise a jour impossible", e);
    if (!silencieux) {
      await message(`Verification impossible : ${e}`, { title: "Mise a jour", kind: "error" });
    }
    return false;
  }
}
