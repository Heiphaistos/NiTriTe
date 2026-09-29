/**
 * Logique pure de Master Install : categories, recherche, profils, file
 * d'installation/desinstallation. Sortie de la page pour etre testable.
 */

export interface CatalogApp {
  id: string;
  name: string;
  description: string;
  category: string;
  winget_id?: string | null;
  choco_id?: string | null;
  url?: string | null;
  icon?: string;
}

/** Ordre d'affichage des categories connues. Toute autre categorie presente
 *  dans le catalogue est ajoutee a la suite (auparavant elle etait
 *  silencieusement masquee : 29 apps invisibles dans « Tout »). */
export const CATEGORY_ORDER: { id: string; label: string }[] = [
  { id: "Outils Essentiels", label: "Outils Essentiels" },
  { id: "Navigateurs", label: "Navigateurs" },
  { id: "Securite", label: "Sécurité" },
  { id: "Antivirus", label: "Antivirus" },
  { id: "Desinstallateurs Antivirus", label: "Désinstallateurs Antivirus" },
  { id: "Developpement", label: "Développement" },
  { id: "Multimedia", label: "Multimédia" },
  { id: "Streaming Video", label: "Streaming Vidéo" },
  { id: "Streaming Audio", label: "Streaming Audio" },
  { id: "Communication", label: "Communication" },
  { id: "Reseaux Sociaux", label: "Réseaux Sociaux" },
  { id: "Bureautique", label: "Bureautique" },
  { id: "PDF et Documents", label: "PDF & Documents" },
  { id: "Suites Professionnelles", label: "Suites Pro" },
  { id: "Productivite", label: "Productivité" },
  { id: "IA & Assistants", label: "IA & Assistants" },
  { id: "Utilitaires", label: "Utilitaires" },
  { id: "Utilitaires Systeme", label: "Utilitaires Système" },
  { id: "Benchmarks et Tests", label: "Benchmarks & Tests" },
  { id: "Partition et Disque", label: "Partition & Disque" },
  { id: "Récupération de Données", label: "Récupération de Données" },
  { id: "Désinstallateurs Propres", label: "Désinstallateurs" },
  { id: "Virtualisation", label: "Virtualisation" },
  { id: "Stockage Cloud", label: "Stockage Cloud" },
  { id: "Compression", label: "Compression" },
  { id: "Internet", label: "Internet" },
  { id: "Jeux", label: "Jeux" },
  { id: "Imprimantes & Scan", label: "Imprimantes & Scan" },
  { id: "Services Apple", label: "Services Apple" },
];

export function normalizeStr(s: string): string {
  return s.toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, "");
}

/** Categories a afficher, dans l'ordre, avec toutes celles du catalogue. */
export function buildCategories(apps: CatalogApp[]): { id: string; label: string }[] {
  const present = new Map<string, string>();
  for (const a of apps) {
    const key = normalizeStr(a.category);
    if (!present.has(key)) present.set(key, a.category);
  }
  const out: { id: string; label: string }[] = [];
  for (const c of CATEGORY_ORDER) {
    const key = normalizeStr(c.id);
    if (present.has(key)) {
      out.push({ id: present.get(key)!, label: c.label });
      present.delete(key);
    }
  }
  for (const raw of [...present.values()].sort((a, b) => a.localeCompare(b, "fr"))) {
    out.push({ id: raw, label: raw });
  }
  return out;
}

/** Recherche insensible a la casse ET aux accents, sur nom, description,
 *  categorie et identifiant winget. */
export function matchesSearch(app: CatalogApp, query: string): boolean {
  const q = normalizeStr(query.trim());
  if (!q) return true;
  return [app.name, app.description, app.category, app.winget_id ?? ""]
    .some((f) => normalizeStr(f).includes(q));
}

export type InstallMethod = "winget" | "choco" | "direct" | "auto";

/** Methode principale par laquelle le backend tentera l'installation
 *  (la cascade complete est winget -> Chocolatey -> Scoop -> URL directe). */
export function primaryMethod(app: CatalogApp): InstallMethod {
  if (app.winget_id) return "winget";
  if (app.choco_id) return "choco";
  if (app.url) return "direct";
  return "auto";
}

export const METHOD_LABEL: Record<InstallMethod, string> = {
  winget: "WinGet",
  choco: "Chocolatey",
  direct: "Téléchargement direct",
  auto: "Recherche Chocolatey/Scoop",
};

export interface InstallProfile {
  id: string;
  label: string;
  color: string;
  wingetIds: string[];
}

/** Profils de selection rapide. Chaque identifiant doit exister dans le
 *  catalogue (verifie par un test) : un identifiant absent etait
 *  auparavant ignore en silence, le profil affichait « 6 apps » pour en
 *  selectionner 4. */
export const PROFILES: InstallProfile[] = [
  { id: "essential", label: "Essentiels", color: "#f97316",
    wingetIds: ["7zip.7zip", "Google.Chrome", "Mozilla.Firefox", "Notepad++.Notepad++", "VideoLAN.VLC", "Microsoft.VCRedist.2015+.x64"] },
  { id: "technician", label: "Technicien", color: "#06b6d4",
    wingetIds: ["Microsoft.Sysinternals.Suite", "CrystalDewWorld.CrystalDiskInfo", "REALiX.HWiNFO", "voidtools.Everything", "Klocman.BulkCrapUninstaller", "CPUID.CPU-Z", "Piriform.Recuva"] },
  { id: "office", label: "Bureau", color: "#3b82f6",
    wingetIds: ["Adobe.Acrobat.Reader.64-bit", "TheDocumentFoundation.LibreOffice", "Zoom.Zoom", "Microsoft.Teams", "Mozilla.Thunderbird"] },
  { id: "dev", label: "Dev", color: "#22c55e",
    wingetIds: ["Microsoft.VisualStudioCode", "Git.Git", "Python.Python.3.13", "OpenJS.NodeJS.LTS", "Microsoft.PowerShell", "Microsoft.WindowsTerminal"] },
  { id: "gaming", label: "Gaming", color: "#a855f7",
    wingetIds: ["Valve.Steam", "Discord.Discord", "EpicGames.EpicGamesLauncher", "Microsoft.DirectX", "Parsec.Parsec"] },
  { id: "security", label: "Sécurité", color: "#ef4444",
    wingetIds: ["Malwarebytes.Malwarebytes", "KeePassXCTeam.KeePassXC", "Bitwarden.Bitwarden", "OO-Software.ShutUp10"] },
  { id: "creative", label: "Créatif", color: "#ec4899",
    wingetIds: ["Inkscape.Inkscape", "GIMP.GIMP", "HandBrake.HandBrake", "OBSProject.OBSStudio", "Audacity.Audacity", "BlenderFoundation.Blender"] },
];

/** Apps du catalogue correspondant a un profil (comparaison insensible a la casse). */
export function profileApps<T extends CatalogApp>(profile: InstallProfile, apps: T[]): T[] {
  const wanted = new Set(profile.wingetIds.map((w) => w.toLowerCase()));
  return apps.filter((a) => a.winget_id && wanted.has(a.winget_id.toLowerCase()));
}

export interface QueueProgress<T> {
  item: T;
  index: number; // 1-based
  total: number;
  etaSeconds: number | null;
}

export interface QueueOutcome<T, R> {
  results: { item: T; result: R }[];
  cancelled: boolean;
}

/**
 * Execute `worker` sur chaque element, un par un. Sequentiel volontairement :
 * winget/msiexec refusent deux installations simultanees (verrou MSI global,
 * erreur 1618), une file parallele echouerait au hasard.
 * `shouldStop` est consulte avant chaque element : annuler laisse finir
 * l'installation en cours (l'interrompre corromprait l'app) et saute la suite.
 */
export async function runQueue<T, R>(
  items: T[],
  worker: (item: T) => Promise<R>,
  opts: {
    onProgress?: (p: QueueProgress<T>) => void;
    shouldStop?: () => boolean;
    now?: () => number;
  } = {},
): Promise<QueueOutcome<T, R>> {
  const now = opts.now ?? (() => Date.now());
  const start = now();
  const results: { item: T; result: R }[] = [];
  for (let i = 0; i < items.length; i++) {
    if (opts.shouldStop?.()) return { results, cancelled: true };
    const done = i;
    const etaSeconds = done > 0 ? ((now() - start) / 1000 / done) * (items.length - done) : null;
    opts.onProgress?.({ item: items[i], index: i + 1, total: items.length, etaSeconds });
    results.push({ item: items[i], result: await worker(items[i]) });
  }
  return { results, cancelled: false };
}

export function formatEta(seconds: number | null): string {
  if (seconds === null || !Number.isFinite(seconds) || seconds < 0) return "";
  const m = Math.floor(seconds / 60);
  const s = Math.round(seconds % 60);
  return m > 0 ? `~${m} min ${s}s restantes` : `~${s}s restantes`;
}
