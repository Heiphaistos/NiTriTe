// Port de TokaISO 7.9 (MIT, Cédrick Roy)
// État partagé des onglets « Téléchargement OS » : catalogue, file, favoris.
import { ref, computed } from "vue";
import { invoke } from "@/utils/invoke";

export type SourceKind = "official" | "community" | "portal" | "kit";

export interface IsoSource {
  label: string;
  url: string;
  kind: SourceKind;
  size?: number;
  format?: string;
  verified?: string;
  language?: string;
  arch?: string;
  filename?: string;
  sample?: string;
  sha256?: string;
  note?: string;
  mirror_count?: number;
  kit?: string;
  platforms?: string[];
}

export interface IsoItem {
  id: string;
  family: string;
  section?: string;
  group?: string;
  section_group?: string;
  name: string;
  desc: string;
  key: string;
  image: string;
  sources: IsoSource[];
  tags?: string[];
  install?: string;
  personal?: boolean;
  new?: boolean;
  vm?: { ram: number; cpu: number; disk: number; steps: string[] };
}

export interface IsoJob {
  id: string;
  item: string;
  name: string;
  source: string;
  source_index: number;
  kind: string;
  format: string;
  file: string;
  state: "queued" | "running" | "paused" | "done" | "failed" | "cancelled";
  busy: boolean;
  received: number;
  total: number;
  speed: number;
  error: string;
  folder: string;
  group: string;
  mirror_used: boolean;
  mirror_count: number;
  priority: boolean;
  retry_at: number;
  deferred: boolean;
  sha256: string;
  expected_sha256: string;
  verification: "" | "verified" | "mismatch";
}

export interface Schedule { enabled: boolean; start: string; end: string; retry: boolean }

export interface Snapshot {
  folder: string;
  favorites: string[];
  limit_kib: number;
  schedule: Schedule;
  schedule_open: boolean;
  eta: number | null;
  jobs: IsoJob[];
}

export interface MediaFile {
  id: string;
  path: string;
  name: string;
  size: number;
  sha256: string;
  format: string;
  boot: string[];
  windows: { index: string; edition: string; label: string; architecture: string; languages: string[] }[];
  contents: string[];
  detail: string;
  state: "ready" | "absent" | "changed";
  duplicates: number;
  known: string[];
}

/** Familles du catalogue (libellés de TokaISO), dans l'ordre de sa navigation. */
export const FAMILIES: [string, string][] = [
  ["Windows", "Windows"], ["Live PE et récupération", "Live PE et récupération"], ["Pilotes et déploiement", "Pilotes et déploiement"],
  ["Linux", "Linux"], ["Linux Gaming", "Linux Gaming"], ["Rétrogaming et streaming", "Rétrogaming et streaming"],
  ["macOS", "macOS"], ["Office", "Microsoft Office"], ["Navigateur", "Navigateurs"],
  ["Installation d’applications", "Applications"], ["Machines Virtuelles", "Virtualisation"], ["Hyperviseur", "Hyperviseur"],
  ["Secours et diagnostic", "Secours et diagnostic"], ["Serveurs et stockage", "Serveurs et stockage"],
  ["Sauvegarde et migration", "Sauvegarde et migration"], ["USB, démarrage et déploiement", "USB et démarrage"],
  ["Serveurs de jeux", "Serveurs de jeux"], ["Outils portables", "Outils portables"],
  ["Images de machines virtuelles", "VM prêtes à l’emploi"], ["Sécurité et surveillance", "Sécurité et surveillance"],
  ["Création, développement et IA", "Création, dev et IA"], ["Assistance et systèmes à découvrir", "Assistance et découverte"],
  ["Pare-feu", "Pare-feu"], ["ERP/CRM", "ERP et CRM"], ["Self-Hosting", "Auto-hébergement"],
  ["Utilitaires hors ligne", "Utilitaires hors ligne"], ["Démarrage réseau PXE", "Démarrage réseau PXE"],
  ["Archives et systèmes légers", "Archives et systèmes légers"], ["Conteneurs et outils de labo", "Conteneurs et outils de labo"],
  ["Serveurs de sauvegarde", "Serveurs de sauvegarde"], ["Raspberry Pi et cartes ARM", "Raspberry Pi et cartes ARM"],
  ["Diagnostic matériel", "Diagnostic matériel"], ["Mon catalogue", "Mon catalogue"],
];

export const KIND_LABELS: Record<SourceKind, string> = {
  official: "Officiel", community: "Communauté", portal: "Portail éditeur", kit: "Kit Microsoft",
};
export const KIND_VARIANTS: Record<SourceKind, "success" | "warning" | "info" | "accent"> = {
  official: "success", community: "warning", portal: "info", kit: "accent",
};
export const STATES: Record<IsoJob["state"], string> = {
  queued: "En attente", running: "En cours", paused: "En pause", done: "Terminé", failed: "Échec", cancelled: "Annulé",
};

export const COMMUNITY_WARNING =
  "Cette source est communautaire (archive.org ou lien personnel) et non publiée par l’éditeur.\n\n" +
  "• L’image peut avoir été modifiée par rapport à l’original.\n" +
  "• Vérifie l’empreinte SHA-256 quand elle est connue avant de t’en servir.\n" +
  "• Tu dois disposer d’une licence valide pour le logiciel téléchargé.\n\nContinuer le téléchargement ?";

export function displayBytes(value?: number | null): string {
  if (!value) return "";
  const units = ["o", "Ko", "Mo", "Go", "To"];
  let n = value;
  let i = 0;
  while (n >= 1024 && i < units.length - 1) { n /= 1024; i++; }
  return `${n.toFixed(i >= 3 ? 1 : 0)} ${units[i]}`;
}

export const normalize = (value: string) => value.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();

export function familyLabel(family: string): string {
  return FAMILIES.find(([key]) => key === family)?.[1] ?? family;
}

/** Appel unique au moteur Rust (`iso_catalog_action`). */
export function api<T = Record<string, unknown>>(action: string, payload: Record<string, unknown> = {}, ms = 30_000): Promise<T> {
  return invoke<T>("iso_catalog_action", { action, payload }, ms);
}

export function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

const items = ref<IsoItem[]>([]);
const snapshot = ref<Snapshot | null>(null);
const loading = ref(false);
const loadError = ref("");
let listening = false;

const favorites = computed(() => new Set(snapshot.value?.favorites ?? []));
const activeCount = computed(() => snapshot.value?.jobs.filter(j => ["running", "queued", "paused"].includes(j.state)).length ?? 0);

async function loadCatalogue() {
  if (items.value.length || loading.value) return;
  loading.value = true;
  try {
    const data = await api<{ items: IsoItem[] }>("catalog-data");
    items.value = data.items;
    snapshot.value = await api<Snapshot>("snapshot");
    loadError.value = "";
  } catch (error) {
    loadError.value = errorText(error);
  } finally {
    loading.value = false;
  }
  if (!listening) {
    listening = true;
    const { listen } = await import("@tauri-apps/api/event");
    // Écoute pour toute la durée de vie de l'app : la file continue en arrière-plan.
    await listen<Snapshot>("iso-downloads", event => { snapshot.value = event.payload; });
  }
}

/** Avertissement obligatoire avant toute source communautaire (décision de Momo). */
export async function confirmCommunity(sources: IsoSource[]): Promise<boolean> {
  if (!sources.some(s => s.kind === "community")) return true;
  const { confirm } = await import("@tauri-apps/plugin-dialog");
  return confirm(COMMUNITY_WARNING, { title: "Source communautaire", kind: "warning" });
}

/** Ajoute une source à la file : seuls l'identifiant de fiche et l'index partent vers Rust. */
async function queueDownload(item: IsoItem, index: number): Promise<boolean> {
  if (!(await confirmCommunity([item.sources[index]]))) return false;
  snapshot.value = await api<Snapshot>("add", { item: item.id, source: index });
  return true;
}

async function queueAction(action: string, payload: Record<string, unknown> = {}) {
  snapshot.value = await api<Snapshot>(action, payload);
}

export async function openPortal(url: string) {
  await invoke("open_url", { url });
}

export function useIsoCatalog() {
  return { items, snapshot, favorites, activeCount, loading, loadError, loadCatalogue, queueDownload, queueAction };
}
