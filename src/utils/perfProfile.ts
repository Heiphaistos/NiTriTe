/**
 * Profil de performance de l'interface, adapte a la machine.
 *
 * Sur un PC modeste (2 coeurs, 4 Go, iGPU ancien), ce qui coute le plus n'est
 * pas le JS mais le rendu : flous `backdrop-filter`, ombres lumineuses et
 * animations infinies repeintes a chaque image, plus une dizaine de sondes
 * PowerShell lancees en parallele au demarrage. Le profil « leger » coupe tout
 * cela ; « equilibre » ne garde que les transitions courtes.
 */

export type PerfTier = "full" | "balanced" | "light";
export type PerfMode = "auto" | PerfTier;

export interface HardwareHints {
  cores: number;          // navigator.hardwareConcurrency
  memoryGb: number | null; // navigator.deviceMemory (plafonne a 8 par Chromium)
  reducedMotion: boolean; // preference OS « reduire les animations »
}

export const PERF_MODE_LABELS: Record<PerfMode, string> = {
  auto: "Automatique",
  full: "Complet",
  balanced: "Équilibré",
  light: "Léger",
};

export const PERF_MODE_DESCRIPTIONS: Record<PerfMode, string> = {
  auto: "Choisi selon le processeur et la mémoire de ce PC",
  full: "Tous les effets : flous, lueurs, animations",
  balanced: "Sans flous ni animations en boucle, transitions courtes conservées",
  light: "Aucun effet ni animation, démarrage et rafraîchissements allégés — pour PC modestes",
};

export function detectTier(h: HardwareHints): PerfTier {
  const mem = h.memoryGb ?? 8;
  if (h.cores <= 2 || mem <= 4) return "light";
  if (h.cores <= 4 || mem < 8 || h.reducedMotion) return "balanced";
  return "full";
}

export function resolveTier(mode: PerfMode, h: HardwareHints): PerfTier {
  return mode === "auto" ? detectTier(h) : mode;
}

/** Nombre de sondes systeme lancees en parallele au demarrage. */
export function startupConcurrency(tier: PerfTier): number {
  return tier === "light" ? 2 : tier === "balanced" ? 3 : 8;
}

/** Multiplicateur des intervalles de rafraichissement (monitoring, capteurs…). */
export function pollMultiplier(tier: PerfTier): number {
  return tier === "light" ? 2 : tier === "balanced" ? 1.5 : 1;
}

export function readHardwareHints(): HardwareHints {
  const nav = typeof navigator !== "undefined" ? navigator : undefined;
  const cores = Math.max(1, nav?.hardwareConcurrency ?? 4);
  const mem = (nav as (Navigator & { deviceMemory?: number }) | undefined)?.deviceMemory;
  let reducedMotion = false;
  try { reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches; } catch { /* hors navigateur */ }
  return { cores, memoryGb: typeof mem === "number" ? mem : null, reducedMotion };
}

/**
 * Execute des taches asynchrones avec au plus `limit` en vol. Utilise au
 * demarrage : 23 sondes PowerShell/WMI simultanees saturent un double coeur
 * et figent l'interface pendant de longues secondes.
 */
export async function runLimited<T>(tasks: (() => Promise<T>)[], limit: number): Promise<PromiseSettledResult<T>[]> {
  const results: PromiseSettledResult<T>[] = new Array(tasks.length);
  let next = 0;
  const lane = async () => {
    while (next < tasks.length) {
      const i = next++;
      try { results[i] = { status: "fulfilled", value: await tasks[i]() }; }
      catch (reason) { results[i] = { status: "rejected", reason }; }
    }
  };
  await Promise.all(Array.from({ length: Math.max(1, Math.min(limit, tasks.length)) }, lane));
  return results;
}

/** Intervalle de sondage ajuste au profil actif (lu sur <html data-perf>). */
export function scaledInterval(baseMs: number): number {
  let tier: PerfTier = "full";
  try {
    const v = document.documentElement.getAttribute("data-perf");
    if (v === "balanced" || v === "light") tier = v;
  } catch { /* hors navigateur */ }
  return Math.round(baseMs * pollMultiplier(tier));
}
