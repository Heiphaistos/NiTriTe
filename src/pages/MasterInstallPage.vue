<script setup lang="ts">
defineOptions({ name: "MasterInstallPage" });
import { ref, computed, onMounted, onUnmounted, type Component } from "vue";
import { invoke, invokeRaw } from "@/utils/invoke";
import { confirm } from "@tauri-apps/plugin-dialog";
import { cachedInvoke } from "@/composables/useCachedInvoke";
import NCard from "@/components/ui/NCard.vue";
import NButton from "@/components/ui/NButton.vue";
import NProgress from "@/components/ui/NProgress.vue";
import NSearchBar from "@/components/ui/NSearchBar.vue";
import NBadge from "@/components/ui/NBadge.vue";
import NTabs from "@/components/ui/NTabs.vue";
import NModal from "@/components/ui/NModal.vue";
import NSpinner from "@/components/ui/NSpinner.vue";
import { useNotificationStore } from "@/stores/notifications";
import {
  buildCategories, matchesSearch, normalizeStr, primaryMethod, METHOD_LABEL,
  PROFILES, profileApps, runQueue, formatEta, type CatalogApp, type InstallProfile,
} from "@/utils/installCatalog";
import {
  Download, CheckSquare, Square, Package,
  Globe, Shield, Code, Image, MessageSquare,
  FileText, Music, Video, Wrench,
  Cpu, Monitor, Printer, Archive,
  Bot, Users, Cloud, Star, Lock, Play,
  ChevronDown, ChevronRight, Layers, FileCode, Eye,
  RotateCcw, Trash2, Gauge, HardDrive, Database, Server, XCircle, RefreshCw, Stethoscope,
} from "lucide-vue-next";

const notifications = useNotificationStore();
const search = ref("");
const exportingScript = ref(false);

interface AppItem extends CatalogApp {
  checked: boolean;
  installed: boolean;
}

const apps = ref<AppItem[]>([]);

// ── Détection des apps déjà installées ─────────────────────────
// Sans elle, « Installé » et le bouton Désinstaller n'apparaissaient qu'après
// avoir installé l'app dans la session : impossible de désinstaller en un
// clic une app déjà présente sur le poste.
const detecting = ref(false);
async function detectInstalled() {
  detecting.value = true;
  try {
    const ids = new Set(await invokeRaw<string[]>("detect_installed_catalog_apps"));
    for (const a of apps.value) {
      a.installed = ids.has(a.id);
      if (a.installed && busyMode.value !== "uninstall") a.checked = false;
    }
  } catch {
    notifications.warning("Détection des applications installées impossible", "Les statuts « Installé » peuvent être incomplets.");
  } finally {
    detecting.value = false;
  }
}

// ── Filtres ────────────────────────────────────────────────────
type StatusFilter = "all" | "installed" | "missing";
const statusFilter = ref<StatusFilter>("all");
const activeCategory = ref("all");

const ICONS: Record<string, Component> = {
  "Outils Essentiels": Star, "Navigateurs": Globe, "Securite": Shield, "Antivirus": Shield,
  "Desinstallateurs Antivirus": Lock, "Developpement": Code, "Multimedia": Video,
  "Streaming Video": Play, "Streaming Audio": Music, "Communication": MessageSquare,
  "Reseaux Sociaux": Users, "Bureautique": FileText, "PDF et Documents": FileText,
  "Suites Professionnelles": Cpu, "Productivite": CheckSquare, "IA & Assistants": Bot,
  "Utilitaires": Wrench, "Utilitaires Systeme": Monitor, "Stockage Cloud": Cloud,
  "Compression": Archive, "Internet": Globe, "Jeux": Play, "Imprimantes & Scan": Printer,
  "Services Apple": Package, "Benchmarks et Tests": Gauge, "Partition et Disque": HardDrive,
  "Récupération de Données": Database, "Désinstallateurs Propres": Trash2, "Virtualisation": Server,
};
const PROFILE_ICONS: Record<string, Component> = {
  essential: Star, technician: Stethoscope, office: FileText, dev: Code,
  gaming: Play, security: Shield, creative: Image,
};

const categories = computed(() => buildCategories(apps.value));
const categoryTabs = computed(() => [
  { id: "all", label: "Tout" },
  ...categories.value.map((c) => ({ id: c.id, label: c.label })),
]);

const filteredApps = computed(() => apps.value.filter((a) => {
  if (!matchesSearch(a, search.value)) return false;
  if (statusFilter.value === "installed" && !a.installed) return false;
  if (statusFilter.value === "missing" && a.installed) return false;
  return activeCategory.value === "all" || normalizeStr(a.category) === normalizeStr(activeCategory.value);
}));

const groupedApps = computed(() => {
  const groups: Record<string, AppItem[]> = {};
  const byCat = new Map<string, AppItem[]>();
  for (const a of filteredApps.value) {
    const k = normalizeStr(a.category);
    if (!byCat.has(k)) byCat.set(k, []);
    byCat.get(k)!.push(a);
  }
  for (const c of categories.value) {
    const items = byCat.get(normalizeStr(c.id));
    if (items?.length) groups[c.id] = items;
  }
  return groups;
});

const totalCount = computed(() => apps.value.length);
const installedCount = computed(() => apps.value.filter((a) => a.installed).length);
const toInstall = computed(() => apps.value.filter((a) => a.checked && !a.installed));
const toUninstall = computed(() => apps.value.filter((a) => a.checked && a.installed));
const selectedCount = computed(() => toInstall.value.length + toUninstall.value.length);

function selectAllVisible() {
  filteredApps.value.forEach((a) => { if (!a.installed) a.checked = true; });
}
function deselectAll() {
  apps.value.forEach((a) => (a.checked = false));
}
function toggleApp(app: AppItem) {
  if (busy.value) return;
  app.checked = !app.checked;
}

const collapsedCategories = ref<Set<string>>(new Set());
function toggleCategory(catId: string) {
  const next = new Set(collapsedCategories.value);
  if (next.has(catId)) next.delete(catId); else next.add(catId);
  collapsedCategories.value = next;
}
function getCategoryInfo(id: string) {
  const c = categories.value.find((x) => x.id === id);
  return { label: c?.label ?? id, icon: ICONS[id] ?? Package };
}
function allCatChecked(catId: string): boolean {
  const catApps = (groupedApps.value[catId] ?? []).filter((a) => !a.installed);
  return catApps.length > 0 && catApps.every((a) => a.checked);
}
function toggleSelectCategory(catId: string) {
  const catApps = (groupedApps.value[catId] ?? []).filter((a) => !a.installed);
  const allChecked = catApps.every((a) => a.checked);
  catApps.forEach((a) => (a.checked = !allChecked));
}

function applyProfile(profile: InstallProfile) {
  const matched = profileApps(profile, apps.value);
  const missing = matched.filter((a) => !a.installed);
  missing.forEach((a) => (a.checked = true));
  if (matched.length === 0) notifications.warning(`Profil "${profile.label}" : aucune app correspondante`);
  else if (missing.length === 0) notifications.info(`Profil "${profile.label}" : tout est déjà installé`);
  else notifications.success(`Profil "${profile.label}" : ${missing.length} app(s) sélectionnée(s)`);
}

// ── File d'installation / désinstallation ──────────────────────
interface OpResult { id: string; name: string; success: boolean; message: string; manualUrl?: string }
type BusyMode = "install" | "uninstall" | null;

const busyMode = ref<BusyMode>(null);
const busy = computed(() => busyMode.value !== null);
const cancelRequested = ref(false);
const currentApp = ref("");
const queueIndex = ref(0);
const queueTotal = ref(0);
const etaLabel = ref("");
const lastLogLine = ref("");
const results = ref<OpResult[]>([]);
const resultsMode = ref<BusyMode>(null);
const showSummary = ref(false);
const showDryRun = ref(false);
const queueProgress = computed(() => queueTotal.value ? Math.round(((queueIndex.value - 1) / queueTotal.value) * 100) : 0);

async function installOne(app: AppItem): Promise<OpResult> {
  try {
    // install_app tente en cascade winget -> Chocolatey -> Scoop ->
    // téléchargement direct (backend), chaque étape vérifiée pour de vrai.
    // invokeRaw (pas de timeout 30s) : le bootstrap Chocolatey/Scoop seul
    // peut prendre plus d'une minute la première fois.
    const r = await invokeRaw<{ success: boolean; message: string }>("install_app", {
      appId: app.id, wingetId: app.winget_id ?? undefined,
    });
    if (r.success) { app.installed = true; app.checked = false; }
    return { id: app.id, name: app.name, success: r.success, message: r.message, manualUrl: r.success ? undefined : (app.url ?? undefined) };
  } catch (e: unknown) {
    return { id: app.id, name: app.name, success: false, message: (e instanceof Error ? e.message : String(e)).slice(0, 200), manualUrl: app.url ?? undefined };
  }
}

async function uninstallOne(app: AppItem): Promise<OpResult> {
  try {
    const r = await invokeRaw<{ success: boolean; message: string }>("uninstall_app", {
      appId: app.id, wingetId: app.winget_id ?? undefined,
    });
    if (r.success) { app.installed = false; app.checked = false; }
    return { id: app.id, name: app.name, success: r.success, message: r.message };
  } catch (e: unknown) {
    return { id: app.id, name: app.name, success: false, message: (e instanceof Error ? e.message : String(e)).slice(0, 200) };
  }
}

async function runBatch(mode: "install" | "uninstall", items: AppItem[]) {
  if (!items.length || busy.value) return;
  busyMode.value = mode;
  cancelRequested.value = false;
  results.value = [];
  resultsMode.value = mode;
  lastLogLine.value = "";
  const worker = mode === "install" ? installOne : uninstallOne;
  const outcome = await runQueue(items, worker, {
    shouldStop: () => cancelRequested.value,
    onProgress: (p) => {
      currentApp.value = p.item.name;
      queueIndex.value = p.index;
      queueTotal.value = p.total;
      etaLabel.value = formatEta(p.etaSeconds);
    },
  });
  results.value = outcome.results.map((r) => r.result);
  const ok = results.value.filter((r) => r.success).length;
  const ko = results.value.length - ok;
  const verb = mode === "install" ? "installée(s)" : "désinstallée(s)";
  if (outcome.cancelled) notifications.warning(`File annulée — ${ok} app(s) ${verb}, ${items.length - results.value.length} ignorée(s)`);
  else if (ko === 0) notifications.success(`${ok} app(s) ${verb}`);
  else notifications.warning(`${ok} app(s) ${verb}, ${ko} échec(s)`);
  busyMode.value = null;
  currentApp.value = "";
  etaLabel.value = "";
  queueIndex.value = 0;
  queueTotal.value = 0;
  showSummary.value = true;
}

async function installSelection() {
  if (!toInstall.value.length) { notifications.warning("Aucune application à installer dans la sélection"); return; }
  await runBatch("install", [...toInstall.value]);
}

async function uninstallSelection() {
  const list = [...toUninstall.value];
  if (!list.length) { notifications.warning("Aucune application installée dans la sélection"); return; }
  const names = list.slice(0, 12).map((a) => `• ${a.name}`).join("\n") + (list.length > 12 ? `\n… et ${list.length - 12} autre(s)` : "");
  const ok = await confirm(`Désinstaller ${list.length} application(s) ?\n\n${names}`, { title: "Nitrite", kind: "warning" });
  if (ok) await runBatch("uninstall", list);
}

async function uninstallApp(app: AppItem) {
  const ok = await confirm(`Désinstaller ${app.name} ?\n\nCette action supprimera l'application de votre système.`, { title: "Nitrite", kind: "warning" });
  if (ok) await runBatch("uninstall", [app]);
}

async function installApp(app: AppItem) {
  await runBatch("install", [app]);
}

function retryFailed() {
  const failedIds = new Set(results.value.filter((r) => !r.success).map((r) => r.id));
  const items = apps.value.filter((a) => failedIds.has(a.id));
  showSummary.value = false;
  if (resultsMode.value === "uninstall") void runBatch("uninstall", items);
  else void runBatch("install", items);
}

// ── Vérifier MAJ (par app) ─────────────────────────────────────
const checkingUpdateIds = ref<Set<string>>(new Set());

async function checkAppUpdate(app: AppItem) {
  if (!app.winget_id) { notifications.warning(`Vérification indisponible pour ${app.name} (pas d'ID WinGet)`); return; }
  checkingUpdateIds.value = new Set([...checkingUpdateIds.value, app.id]);
  try {
    const result = await invoke<{ stdout?: string }>("run_system_command", {
      cmd: "winget",
      args: ["upgrade", "--id", app.winget_id],
    });
    const out = result?.stdout ?? "";
    const low = out.toLowerCase();
    // winget renvoie un exit code non-zéro même quand l'app est déjà à jour
    // (quirk winget : "upgrade --id Git.Git" à jour → exit -1978335189) —
    // seul le texte distingue les 3 issues réelles. Sur Windows FR, winget
    // répond "Mise à niveau disponible introuvable." / "Aucune version de
    // package plus récente n'est disponible...".
    const upToDate = out.includes("No applicable upgrade")
      || low.includes("disponible introuvable")
      || low.includes("n'est disponible à partir des sources");
    const notFound = low.includes("ne correspond aux critères") || low.includes("no installed package found");
    if (upToDate) notifications.success(`${app.name} est à jour`);
    else if (notFound) notifications.error(`${app.name} introuvable via WinGet`, "L'ID WinGet ne correspond à aucun package installé.");
    else if (out.trim()) notifications.info(`MAJ disponible pour ${app.name}`, out.split("\n").slice(0, 3).join(" "));
    else notifications.info(`Vérification terminée pour ${app.name}`);
  } catch (e: unknown) {
    notifications.error(`Impossible de vérifier MAJ pour ${app.name}`, String(e));
  }
  checkingUpdateIds.value = new Set([...checkingUpdateIds.value].filter((id) => id !== app.id));
}

// ── Export script de déploiement ───────────────────────────────
async function exportDeployScript() {
  const selected = toInstall.value.filter((a) => a.winget_id);
  const skipped = toInstall.value.length - selected.length;
  if (!selected.length) { notifications.warning("Aucune app avec WinGet ID sélectionnée"); return; }
  exportingScript.value = true;
  const lines = [
    "@echo off",
    "chcp 65001 >nul",
    ":: Script de déploiement généré par NiTriTe",
    `:: ${new Date().toLocaleString("fr-FR")}`,
    "",
    ":: Vérification des droits administrateur",
    "NET SESSION >nul 2>&1",
    "IF %ERRORLEVEL% NEQ 0 (",
    "    echo ERREUR : Ce script doit etre execute en tant qu'administrateur.",
    "    echo Clic droit sur le fichier ^> Executer en tant qu'administrateur.",
    "    pause",
    "    exit /b 1",
    ")",
    "",
    "set FAILED=0",
    "echo === Installation des logiciels ===",
    "",
  ];
  for (const app of selected) {
    lines.push(`echo Installation de ${app.name.replace(/[&|<>^%]/g, "")}...`);
    lines.push(`winget install --id ${app.winget_id} --exact --silent --accept-package-agreements --accept-source-agreements --disable-interactivity || set /a FAILED+=1`);
    lines.push("");
  }
  lines.push("echo === Terminé : %FAILED% échec(s) ===", "pause");
  const content = lines.join("\r\n");
  try {
    await invoke("save_export_file", { filename: "deploy_nitrite.bat", content });
    notifications.success("Script exporté", skipped ? `deploy_nitrite.bat — ${skipped} app(s) sans WinGet ID ignorée(s)` : "deploy_nitrite.bat");
  } catch {
    try {
      await navigator.clipboard.writeText(content);
      notifications.info("Script copié dans le presse-papier");
    } catch { notifications.error("Export échoué"); }
  }
  exportingScript.value = false;
}

// ── Journal en direct (événements backend) ─────────────────────
let unlistenLog: (() => void) | null = null;

onMounted(async () => {
  try {
    const result = await cachedInvoke<CatalogApp[]>("get_apps");
    apps.value = result.map((a) => ({ ...a, checked: false, installed: false }));
  } catch {
    notifications.warning("Impossible de charger la base de données");
    return;
  }
  try {
    const { listen } = await import("@tauri-apps/api/event");
    unlistenLog = await listen<{ line?: string }>("install-log", (e) => {
      const line = e.payload?.line?.trim();
      if (line && busy.value) lastLogLine.value = line.slice(0, 160);
    });
  } catch { /* hors Tauri */ }
  void detectInstalled();
});

onUnmounted(() => { unlistenLog?.(); });
</script>

<template>
  <div class="master-install">
    <!-- Header -->
    <div class="page-header">
      <div>
        <h1>Master Install</h1>
        <p class="page-subtitle">
          <strong>{{ totalCount }}</strong> applications —
          <span v-if="detecting" class="detecting"><NSpinner :size="10" /> détection des apps installées…</span>
          <template v-else><strong>{{ installedCount }}</strong> déjà installée(s)</template>
          — installation et désinstallation en un clic (WinGet → Chocolatey → Scoop → téléchargement direct)
        </p>
      </div>
      <div class="header-actions">
        <NButton variant="ghost" size="sm" :disabled="busy" @click="selectAllVisible">
          <CheckSquare :size="14" />
          Tout sélectionner
        </NButton>
        <NButton variant="ghost" size="sm" :disabled="busy || selectedCount === 0" @click="deselectAll">
          <Square :size="14" />
          Désélectionner
        </NButton>
        <NButton variant="ghost" size="sm" :loading="detecting" :disabled="busy" title="Relancer la détection des apps installées" @click="detectInstalled">
          <RefreshCw :size="14" />
        </NButton>
        <NButton variant="ghost" size="sm" :disabled="toInstall.length === 0" @click="showDryRun = true">
          <Eye :size="14" />
          Prévisualiser
        </NButton>
        <NButton variant="ghost" size="sm" :loading="exportingScript" :disabled="toInstall.length === 0" @click="exportDeployScript">
          <FileCode :size="14" />
          Export .bat
        </NButton>
        <NButton v-if="toUninstall.length" variant="danger" size="sm" :disabled="busy" @click="uninstallSelection">
          <Trash2 :size="14" />
          Désinstaller ({{ toUninstall.length }})
        </NButton>
        <NButton variant="primary" size="sm" :loading="busyMode === 'install'" :disabled="busy || toInstall.length === 0" @click="installSelection">
          <Download :size="14" />
          Installer ({{ toInstall.length }})
        </NButton>
      </div>
    </div>

    <!-- Profils prédéfinis -->
    <NCard>
      <template #header>
        <div style="display:flex;align-items:center;gap:8px">
          <Layers :size="16" />
          <span>Profils Prédéfinis</span>
          <span style="font-size:12px;color:var(--text-muted);margin-left:4px">— Sélection rapide par usage</span>
        </div>
      </template>
      <div class="profiles-grid">
        <button
          v-for="profile in PROFILES"
          :key="profile.id"
          class="profile-card"
          :style="{ '--p-color': profile.color }"
          :disabled="busy"
          @click="applyProfile(profile)"
        >
          <component :is="PROFILE_ICONS[profile.id] ?? Star" :size="20" :style="{ color: profile.color }" />
          <span class="profile-label">{{ profile.label }}</span>
          <span class="profile-count">{{ profile.wingetIds.length }} apps</span>
        </button>
      </div>
    </NCard>

    <!-- Recherche + filtre de statut -->
    <div class="filter-row">
      <NSearchBar v-model="search" placeholder="Rechercher une application, une catégorie, un ID WinGet…" class="filter-search" />
      <div class="status-filter" role="group" aria-label="Filtrer par statut">
        <button :class="{ active: statusFilter === 'all' }" @click="statusFilter = 'all'">Toutes</button>
        <button :class="{ active: statusFilter === 'missing' }" @click="statusFilter = 'missing'">Non installées</button>
        <button :class="{ active: statusFilter === 'installed' }" @click="statusFilter = 'installed'">Installées ({{ installedCount }})</button>
      </div>
    </div>

    <!-- Progression globale -->
    <NCard v-if="busy" class="progress-card">
      <div class="install-progress-global">
        <div class="install-status-row">
          <NSpinner :size="14" />
          <span class="install-label">
            {{ busyMode === 'install' ? 'Installation' : 'Désinstallation' }} de <strong>{{ currentApp }}</strong>
          </span>
          <NBadge variant="info">{{ queueIndex }}/{{ queueTotal }}</NBadge>
          <NBadge v-if="etaLabel" variant="neutral">{{ etaLabel }}</NBadge>
          <NButton variant="ghost" size="sm" :disabled="cancelRequested" @click="cancelRequested = true">
            <XCircle :size="14" />
            {{ cancelRequested ? 'Arrêt après cette app…' : 'Annuler la suite' }}
          </NButton>
        </div>
        <NProgress :value="queueProgress" :max="100" size="lg" :show-label="true" :glow="true" />
        <div v-if="lastLogLine" class="live-log" :title="lastLogLine">{{ lastLogLine }}</div>
      </div>
    </NCard>

    <!-- Catégories -->
    <NTabs :tabs="categoryTabs" v-model="activeCategory" wrap>
      <template #default>
        <div v-if="Object.keys(groupedApps).length === 0" class="empty-state">
          <Package :size="40" class="empty-icon" />
          <p>Aucune application trouvée</p>
        </div>

        <div v-else class="categories-list">
          <template v-for="(catApps, catId) in groupedApps" :key="catId">
            <NCard class="category-card">
              <template #header>
                <div class="section-header" @click="toggleCategory(catId as string)">
                  <component :is="getCategoryInfo(catId as string).icon" :size="16" />
                  <span>{{ getCategoryInfo(catId as string).label }}</span>
                  <NBadge variant="neutral">{{ catApps.length }}</NBadge>
                  <span class="spacer" />
                  <button class="select-cat-btn" :disabled="busy" @click.stop="toggleSelectCategory(catId as string)">
                    {{ allCatChecked(catId as string) ? 'Tout désélectionner' : 'Tout sélectionner' }}
                  </button>
                  <component
                    :is="collapsedCategories.has(catId as string) ? ChevronRight : ChevronDown"
                    :size="14"
                    class="collapse-icon"
                  />
                </div>
              </template>

              <div v-if="!collapsedCategories.has(catId as string)" class="apps-grid">
                <div
                  v-for="app in catApps"
                  :key="app.id"
                  class="app-item"
                  :class="{ 'app-item--checked': app.checked, 'app-item--installed': app.installed, 'app-item--remove': app.checked && app.installed }"
                  @click="toggleApp(app)"
                >
                  <div class="app-checkbox">
                    <CheckSquare v-if="app.checked" :size="18" :class="app.installed ? 'check-remove' : 'check-on'" />
                    <Square v-else :size="18" class="check-off" />
                  </div>
                  <div class="app-info">
                    <span class="app-name">{{ app.name }}</span>
                    <span class="app-desc">{{ app.description }}</span>
                  </div>
                  <template v-if="app.installed">
                    <NBadge variant="success">Installé</NBadge>
                    <button
                      class="app-action-btn"
                      title="Vérifier les mises à jour"
                      :disabled="checkingUpdateIds.has(app.id) || !app.winget_id"
                      @click.stop="checkAppUpdate(app)"
                    >
                      <NSpinner v-if="checkingUpdateIds.has(app.id)" :size="12" />
                      <RotateCcw v-else :size="12" />
                    </button>
                    <button
                      class="app-action-btn app-action-btn--danger"
                      title="Désinstaller"
                      :disabled="busy"
                      @click.stop="uninstallApp(app)"
                    >
                      <Trash2 :size="12" />
                    </button>
                  </template>
                  <template v-else>
                    <NBadge :variant="primaryMethod(app) === 'winget' ? 'info' : primaryMethod(app) === 'auto' ? 'neutral' : 'warning'" class="winget-badge" :title="METHOD_LABEL[primaryMethod(app)]">
                      {{ primaryMethod(app) === 'winget' ? 'WinGet' : primaryMethod(app) === 'choco' ? 'Choco' : primaryMethod(app) === 'direct' ? 'URL' : 'Auto' }}
                    </NBadge>
                    <button
                      class="app-action-btn app-action-btn--install"
                      title="Installer maintenant"
                      :disabled="busy"
                      @click.stop="installApp(app)"
                    >
                      <Download :size="12" />
                    </button>
                  </template>
                </div>
              </div>
            </NCard>
          </template>
        </div>
      </template>
    </NTabs>
  </div>

  <!-- Modal Prévisualisation -->
  <NModal :open="showDryRun" @close="showDryRun = false" title="Prévisualisation — Apps à installer">
    <div v-if="toInstall.length === 0" style="text-align:center;padding:24px;color:var(--text-muted);font-size:13px">
      Aucune application sélectionnée.
    </div>
    <div v-else style="display:flex;flex-direction:column;gap:6px;max-height:420px;overflow-y:auto">
      <div v-for="app in toInstall" :key="app.id" class="dryrun-item">
        <div class="dryrun-name">{{ app.name }}</div>
        <code v-if="app.winget_id" class="dryrun-cmd">winget install --id {{ app.winget_id }} --exact --silent</code>
        <span v-else class="dryrun-nowinget">{{ METHOD_LABEL[primaryMethod(app)] }} (repli automatique si échec)</span>
      </div>
    </div>
    <template #footer>
      <NBadge variant="neutral" style="margin-right:auto">{{ toInstall.length }} app(s)</NBadge>
      <NButton variant="ghost" @click="showDryRun = false">Fermer</NButton>
      <NButton variant="primary" :disabled="busy || toInstall.length === 0" @click="showDryRun = false; installSelection()">
        <Download :size="14" /> Installer
      </NButton>
    </template>
  </NModal>

  <!-- Modal Résumé -->
  <NModal :open="showSummary" @close="showSummary = false" :title="resultsMode === 'uninstall' ? 'Résumé de la désinstallation' : 'Résumé de l\'installation'">
    <div style="display:flex;flex-direction:column;gap:6px;max-height:420px;overflow-y:auto">
      <div v-for="r in results" :key="r.id" class="summary-item" :class="r.success ? 'summary-ok' : 'summary-fail'">
        <span class="summary-status">{{ r.success ? '✓' : '✗' }}</span>
        <div class="summary-info">
          <span class="summary-name">{{ r.name }}</span>
          <span class="summary-msg" :class="{ 'summary-msg--ok': r.success }">{{ r.message }}</span>
        </div>
        <a v-if="r.manualUrl" :href="r.manualUrl" target="_blank" rel="noopener" class="summary-manual-link">Télécharger</a>
      </div>
    </div>
    <template #footer>
      <NBadge variant="success" style="margin-right:auto">
        {{ results.filter(r => r.success).length }} succès
      </NBadge>
      <NBadge v-if="results.some(r => !r.success)" variant="danger">
        {{ results.filter(r => !r.success).length }} échec(s)
      </NBadge>
      <NButton v-if="results.some(r => !r.success)" variant="secondary" @click="retryFailed" style="margin-left:8px">
        <RotateCcw :size="14" /> Réessayer les échecs
      </NButton>
      <NButton variant="primary" @click="showSummary = false" style="margin-left:8px">Fermer</NButton>
    </template>
  </NModal>
</template>

<style scoped>
.master-install {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.page-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  flex-wrap: wrap;
  gap: 12px;
}

.page-header h1 { font-size: 24px; font-weight: 700; }
.page-subtitle { color: var(--text-muted); font-size: 13px; margin-top: 2px; }
.header-actions { display: flex; gap: 8px; flex-wrap: wrap; align-items: center; }

.section-header {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  user-select: none;
}

.spacer { flex: 1; }

.select-cat-btn {
  font-size: 11px;
  color: var(--accent-primary);
  background: none;
  border: none;
  cursor: pointer;
  padding: 2px 8px;
  border-radius: var(--radius-sm);
  transition: background var(--transition-fast);
}
.select-cat-btn:hover { background: var(--accent-muted); }

.collapse-icon { color: var(--text-muted); }

.install-progress {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.install-status {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  color: var(--text-secondary);
}

.spin-icon { animation: spin 1s linear infinite; }

@keyframes spin { to { transform: rotate(360deg); } }

.categories-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.apps-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 4px;
}

.app-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.app-item:hover { background: var(--bg-tertiary); }
.app-item--checked { background: var(--accent-muted); }
.app-item--remove { background: var(--danger-muted); }
.app-item--installed .app-name, .app-item--installed .app-desc { opacity: 0.6; }

.app-checkbox { flex-shrink: 0; display: flex; }
.check-on { color: var(--accent-primary); }
.check-off { color: var(--text-muted); }
.check-remove { color: var(--danger); }

.app-info { display: flex; flex-direction: column; gap: 1px; flex: 1; min-width: 0; }

.app-name {
  font-size: 12px;
  font-weight: 500;
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.app-desc {
  font-size: 11px;
  color: var(--text-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.winget-badge { flex-shrink: 0; font-size: 10px; }

.app-action-btn {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
  background: var(--bg-secondary);
  color: var(--text-secondary);
  cursor: pointer;
  transition: all var(--transition-fast);
}
.app-action-btn:hover { background: var(--bg-tertiary); color: var(--text-primary); }
.app-action-btn:disabled { opacity: 0.5; cursor: not-allowed; }
.app-action-btn--danger:hover { color: var(--danger); border-color: var(--danger); }
.app-action-btn--install:hover { color: var(--accent-primary); border-color: var(--accent-primary); }

.empty-state {
  text-align: center;
  padding: 48px;
  color: var(--text-muted);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
}

.empty-icon { opacity: 0.3; }

/* ── Progression globale ──────────────────────────── */
.progress-card { border-color: var(--accent-muted); }
.install-progress-global { display: flex; flex-direction: column; gap: 10px; }
.install-status-row { display: flex; align-items: center; gap: 8px; font-size: 13px; }
.install-label { flex: 1; color: var(--text-secondary); }

/* ── Dry run ──────────────────────────────────────── */
.dryrun-item { padding: 8px 10px; border: 1px solid var(--border); border-radius: var(--radius-md); background: var(--bg-tertiary); }
.dryrun-name { font-size: 12px; font-weight: 600; color: var(--text-primary); margin-bottom: 4px; }
.dryrun-cmd { font-size: 11px; color: var(--accent-primary); font-family: "JetBrains Mono", monospace; display: block; white-space: pre-wrap; word-break: break-all; }
.dryrun-nowinget { font-size: 11px; color: var(--text-muted); font-style: italic; }

/* ── Résumé ───────────────────────────────────────── */
.summary-item { display: flex; align-items: flex-start; gap: 10px; padding: 8px 10px; border-radius: var(--radius-md); border: 1px solid var(--border); }
.summary-ok { background: color-mix(in srgb, var(--success) 8%, var(--bg-secondary)); border-color: color-mix(in srgb, var(--success) 30%, var(--border)); }
.summary-fail { background: color-mix(in srgb, var(--danger) 8%, var(--bg-secondary)); border-color: color-mix(in srgb, var(--danger) 30%, var(--border)); }
.summary-status { font-size: 14px; font-weight: 700; flex-shrink: 0; }
.summary-ok .summary-status { color: var(--success); }
.summary-fail .summary-status { color: var(--danger); }
.summary-info { display: flex; flex-direction: column; gap: 2px; }
.summary-name { font-size: 12px; font-weight: 500; color: var(--text-primary); }
.summary-msg { font-size: 11px; color: var(--danger); font-family: "JetBrains Mono", monospace; }
.summary-msg--ok { color: var(--text-muted); }
.summary-manual-link { margin-left: auto; align-self: center; font-size: 11px; color: var(--accent-primary); text-decoration: underline; white-space: nowrap; flex-shrink: 0; }

.profiles-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(130px, 1fr));
  gap: 10px;
}
.profile-card {
  display: flex; flex-direction: column; align-items: center; gap: 6px;
  padding: 14px 10px; border-radius: var(--radius-lg);
  border: 1px solid var(--border); background: var(--bg-tertiary);
  cursor: pointer; transition: all var(--transition-fast);
}
.profile-card:hover {
  border-color: var(--p-color, var(--accent-primary));
  background: var(--bg-secondary);
  transform: translateY(-1px);
  box-shadow: 0 4px 12px rgba(0,0,0,0.15);
}
.profile-label { font-size: 13px; font-weight: 600; color: var(--text-primary); }
.profile-count { font-size: 11px; color: var(--text-muted); }
.profile-card:disabled { opacity: 0.5; cursor: not-allowed; }

/* ── Filtres ──────────────────────────────────────── */
.detecting { display: inline-flex; align-items: center; gap: 4px; }
.filter-row { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; }
.filter-search { flex: 1; min-width: 240px; }
.status-filter { display: inline-flex; border: 1px solid var(--border); border-radius: var(--radius-md); overflow: hidden; }
.status-filter button {
  padding: 6px 12px; font-size: 12px; background: var(--bg-secondary); color: var(--text-secondary);
  border: none; cursor: pointer; font-family: inherit;
}
.status-filter button + button { border-left: 1px solid var(--border); }
.status-filter button.active { background: var(--accent-muted); color: var(--accent-primary); font-weight: 600; }

.live-log {
  font-size: 11px; color: var(--text-muted); font-family: "JetBrains Mono", monospace;
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
}

/* Les catégories hors écran ne sont ni mises en page ni peintes : 700+ apps
   restent fluides sur les petites configurations. */
.category-card { content-visibility: auto; contain-intrinsic-size: auto 320px; }
</style>
