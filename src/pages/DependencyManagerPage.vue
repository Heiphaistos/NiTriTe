<script setup lang="ts">
defineOptions({ name: "DependencyManagerPage" });
import { ref, computed, onMounted, onUnmounted } from "vue";
import { invoke, invokeRaw } from "@/utils/invoke";
import { listen } from "@tauri-apps/api/event";
import { useNotificationStore } from "@/stores/notifications";
import NButton from "@/components/ui/NButton.vue";
import NSpinner from "@/components/ui/NSpinner.vue";
import NProgress from "@/components/ui/NProgress.vue";
import NBadge from "@/components/ui/NBadge.vue";
import NInput from "@/components/ui/NInput.vue";
import {
  Package, RefreshCw, Download, CheckCircle, XCircle,
  Search, AlertTriangle, ExternalLink, Layers, FlaskConical,
} from "lucide-vue-next";

const notify = useNotificationStore();

interface Dependency {
  id:                  string;
  name:                string;
  category:            string;
  description:         string;
  installed:           boolean;
  version:             string;
  recommended_version?: string;
  winget_id:           string;
  install_url:         string;
  download_url?:       string;
  required?:           boolean;
  test_command?:       string;
  cmd?:                string;
  test_args?:          string[];
}

interface DepsProgress { checked: number; total: number; item: Dependency; done: boolean; }

const deps          = ref<Dependency[]>([]);
const loading       = ref(false);
const scanChecked   = ref(0);
const scanTotal     = ref(17);
const installing    = ref<string | null>(null);
const filterText    = ref("");
const selectedCat   = ref("Tous");
const lastChecked   = ref<string | null>(null);

// Filtre Requises/Optionnelles/Toutes
type RequiredFilter = "Toutes" | "Requises" | "Optionnelles";
const requiredFilter = ref<RequiredFilter>("Toutes");

// Test post-installation
const testingDep    = ref<string | null>(null);
const testResults   = ref<Record<string, "ok" | "fail" | "pending">>({});

let unlistenDeps: (() => void) | null = null;

const categories = computed(() => {
  const cats = new Set(deps.value.map(d => d.category));
  return ["Tous", ...Array.from(cats).sort()];
});

const filtered = computed(() => {
  let list = deps.value;

  // Filtre requis/optionnel
  if (requiredFilter.value === "Requises") {
    list = list.filter(d => d.required !== false);
  } else if (requiredFilter.value === "Optionnelles") {
    list = list.filter(d => d.required === false);
  }

  if (selectedCat.value !== "Tous") list = list.filter(d => d.category === selectedCat.value);
  if (filterText.value.trim()) {
    const q = filterText.value.toLowerCase();
    list = list.filter(d => d.name.toLowerCase().includes(q) || d.description.toLowerCase().includes(q));
  }
  return list;
});

const stats = computed(() => ({
  total:     deps.value.length,
  installed: deps.value.filter(d => d.installed).length,
  missing:   deps.value.filter(d => !d.installed).length,
}));

// Vérifie si la version correspond à la version recommandée
function versionMatch(dep: Dependency): "ok" | "mismatch" | "unknown" {
  if (!dep.installed || !dep.version) return "unknown";
  if (!dep.recommended_version) return "unknown";
  return dep.version === dep.recommended_version ? "ok" : "mismatch";
}

async function checkAll() {
  deps.value = [];
  scanChecked.value = 0;
  loading.value = true;
  testResults.value = {};

  if (unlistenDeps) { unlistenDeps(); unlistenDeps = null; }
  unlistenDeps = await listen<DepsProgress>("deps:progress", (ev) => {
    const p = ev.payload;
    scanChecked.value = p.checked;
    scanTotal.value   = p.total;
    const idx = deps.value.findIndex(d => d.id === p.item.id);
    if (idx >= 0) deps.value[idx] = p.item;
    else deps.value.push(p.item);
    if (p.done) {
      loading.value = false;
      lastChecked.value = new Date().toLocaleTimeString("fr-FR");
    }
  });

  // 60 s : chaque vérification est bornée à 20 s côté Rust (en parallèle).
  invoke("scan_dependencies_stream", undefined, 60_000)
    .catch((e: unknown) => notify.error("Analyse des dépendances", String(e).slice(0, 160)))
    .finally(() => { loading.value = false; });
}

async function installDep(dep: Dependency) {
  if (!dep.winget_id && !dep.install_url && !dep.download_url) {
    // Fallback : recherche Google
    const googleUrl = `https://www.google.com/search?q=download+${encodeURIComponent(dep.name)}`;
    await invoke("open_url", { url: googleUrl }).catch(() => window.open(googleUrl, "_blank"));
    notify.info(dep.name, "Aucune source d'installation — recherche Google ouverte.");
    return;
  }
  if (!dep.winget_id) {
    // Fallback vers download_url ou install_url
    const url = dep.download_url ?? dep.install_url;
    await invoke("open_url", { url }).catch(() => window.open(url, "_blank"));
    return;
  }
  installing.value = dep.id;
  notify.info(dep.name, "Installation via WinGet en cours...");
  try {
    const msg = await invokeRaw<string>("install_dependency", { wingetId: dep.winget_id });
    notify.success(dep.name, msg);
    await checkAll();
  } catch (e: unknown) {
    notify.error(dep.name, String(e));
    // Fallback manuel si WinGet échoue
    if (dep.download_url) {
      notify.info(dep.name, "Échec WinGet — ouverture du lien de téléchargement manuel.");
      await invoke("open_url", { url: dep.download_url }).catch(() => window.open(dep.download_url, "_blank"));
    } else {
      const googleUrl = `https://www.google.com/search?q=download+${encodeURIComponent(dep.name)}`;
      notify.info(dep.name, "Recherche manuelle ouverte.");
      await invoke("open_url", { url: googleUrl }).catch(() => window.open(googleUrl, "_blank"));
    }
  }
  installing.value = null;
}

async function testDep(dep: Dependency) {
  if (!dep.test_command && !dep.cmd) return;
  testingDep.value = dep.id;
  testResults.value[dep.id] = "pending";
  try {
    // Via `cmd /c <test>`, cmd.exe se lance mais l'échec de la commande interne
    // est masqué : execute_system_command résout quand même. Vérifier success,
    // sinon une dépendance absente serait rapportée « ok ».
    const r = await invoke<{ success: boolean }>("run_system_command", {
      cmd: dep.cmd ?? "cmd",
      args: dep.test_args ?? ["/c", dep.test_command ?? "echo ok"],
    });
    if (!r.success) throw new Error("Test échoué");
    testResults.value[dep.id] = "ok";
    notify.success(dep.name, "Test réussi");
  } catch {
    testResults.value[dep.id] = "fail";
    notify.error(dep.name, "Test échoué");
  }
  testingDep.value = null;
}

async function openUrl(url: string) {
  if (!url) return;
  await invoke("open_url", { url }).catch(() => {});
}

function openManualDownload(dep: Dependency) {
  const url = dep.download_url
    ? dep.download_url
    : `https://www.google.com/search?q=download+${encodeURIComponent(dep.name)}`;
  invoke("open_url", { url }).catch(() => window.open(url, "_blank"));
}

onMounted(checkAll);
onUnmounted(() => { if (unlistenDeps) unlistenDeps(); });
</script>

<template>
  <div class="dep-page">
    <!-- Header -->
    <div class="page-header">
      <div>
        <h1><Package :size="22" style="margin-right:10px;vertical-align:middle;color:var(--accent-primary)" />Gestionnaire de Dépendances</h1>
        <p class="page-subtitle">Vérifiez et installez les outils essentiels pour votre environnement</p>
      </div>
      <div style="display:flex;gap:8px;align-items:center">
        <!-- Re-scanner -->
        <NButton variant="secondary" :disabled="loading" @click="checkAll">
          <NSpinner v-if="loading" :size="14" />
          <RefreshCw v-else :size="14" />
          Re-scanner
        </NButton>
        <NButton variant="primary" :disabled="loading" @click="checkAll">
          <NSpinner v-if="loading" :size="14" />
          <RefreshCw v-else :size="14" />
          {{ loading ? "Vérification..." : "Tout vérifier" }}
        </NButton>
      </div>
    </div>

    <!-- Progression streaming -->
    <div v-if="loading" class="scan-progress-bar">
      <NSpinner :size="13" />
      <span class="scan-label">Vérification {{ scanChecked }}/{{ scanTotal }}…</span>
      <NProgress :value="scanChecked" :max="scanTotal" size="sm" style="flex:1" />
      <span class="scan-hint">Vous pouvez naviguer librement</span>
    </div>

    <!-- Stats globales -->
    <div v-if="deps.length" class="stats-bar">
      <div class="stat-item ok">
        <CheckCircle :size="16" />
        <span>{{ stats.installed }} installé{{ stats.installed > 1 ? 's' : '' }}</span>
      </div>
      <div class="stat-item warn">
        <XCircle :size="16" />
        <span>{{ stats.missing }} manquant{{ stats.missing > 1 ? 's' : '' }}</span>
      </div>
      <div class="stat-item muted">
        <Layers :size="16" />
        <span>{{ stats.total }} au total</span>
      </div>
      <div class="progress-bar-wrap">
        <div class="progress-bar-fill" :style="{ width: (stats.installed / stats.total * 100) + '%' }" />
      </div>
      <span v-if="lastChecked" class="muted" style="font-size:11px;margin-left:auto">Vérifié à {{ lastChecked }}</span>
    </div>

    <!-- Loading -->
    <div v-if="loading && !deps.length" class="loading-state">
      <NSpinner :size="28" />
      <p>Vérification de l'environnement...</p>
    </div>

    <template v-else-if="deps.length">
      <!-- Filtres -->
      <div class="filters">
        <NInput v-model="filterText" placeholder="Rechercher..." style="max-width:240px">
          <template #prefix><Search :size="14" /></template>
        </NInput>

        <!-- Filtre Requises/Optionnelles/Toutes -->
        <div class="cat-pills">
          <button v-for="opt in (['Toutes', 'Requises', 'Optionnelles'] as const)" :key="opt"
            :class="['cat-pill', { active: requiredFilter === opt }]"
            @click="requiredFilter = opt">
            {{ opt }}
          </button>
        </div>

        <div class="cat-pills">
          <button v-for="cat in categories" :key="cat"
            :class="['cat-pill', { active: selectedCat === cat }]"
            @click="selectedCat = cat">
            {{ cat }}
            <span v-if="cat !== 'Tous'" class="cat-count">
              {{ deps.filter(d => d.category === cat && !d.installed).length > 0
                ? deps.filter(d => d.category === cat && !d.installed).length + ' ✗'
                : '✓' }}
            </span>
          </button>
        </div>
      </div>

      <!-- Tableau dépendances -->
      <div class="dep-table-wrap">
        <table class="dep-table">
          <thead>
            <tr>
              <th>État</th>
              <th>Outil</th>
              <th>Catégorie</th>
              <th>Version</th>
              <th>Action</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="dep in filtered" :key="dep.id" :class="{ 'row-missing': !dep.installed }">
              <td class="cell-status">
                <CheckCircle v-if="dep.installed" :size="16" class="ic-ok" />
                <XCircle v-else :size="16" class="ic-err" />
              </td>
              <td>
                <div style="display:flex;align-items:center;gap:6px">
                  <div class="dep-name">{{ dep.name }}</div>
                  <NBadge v-if="dep.required === false" variant="neutral" style="font-size:9px">Optionnel</NBadge>
                  <NBadge v-else-if="dep.required === true" variant="warning" style="font-size:9px">Requis</NBadge>
                </div>
                <div class="dep-desc">{{ dep.description }}</div>
              </td>
              <td>
                <NBadge variant="neutral" style="font-size:10px">{{ dep.category }}</NBadge>
              </td>
              <td class="cell-version">
                <template v-if="dep.installed && dep.version">
                  <span class="version-text">{{ dep.version }}</span>
                  <!-- Badge version mismatch -->
                  <template v-if="versionMatch(dep) === 'ok'">
                    <NBadge variant="success" style="font-size:9px;margin-left:4px">Recommandée</NBadge>
                  </template>
                  <template v-else-if="versionMatch(dep) === 'mismatch'">
                    <NBadge variant="warning" style="font-size:9px;margin-left:4px" :title="`Recommandée : ${dep.recommended_version}`">
                      v{{ dep.recommended_version }} dispo
                    </NBadge>
                  </template>
                </template>
                <span v-else-if="dep.installed" class="muted">—</span>
                <NBadge v-else variant="danger" style="font-size:10px">Non installé</NBadge>
              </td>
              <td class="cell-action">
                <div v-if="!dep.installed" style="display:flex;gap:6px;align-items:center;flex-wrap:wrap">
                  <NButton v-if="dep.winget_id" variant="primary" size="sm"
                    :disabled="installing === dep.id"
                    @click="installDep(dep)">
                    <NSpinner v-if="installing === dep.id" :size="12" />
                    <Download v-else :size="12" />
                    {{ installing === dep.id ? 'Installation...' : 'Installer' }}
                  </NButton>
                  <!-- Fallback manuel -->
                  <NButton variant="ghost" size="sm" @click="openManualDownload(dep)">
                    <ExternalLink :size="12" />
                    {{ dep.download_url ? 'Télécharger' : 'Rechercher' }}
                  </NButton>
                </div>
                <div v-else style="display:flex;gap:6px;align-items:center;flex-wrap:wrap">
                  <span class="ok-text">Installé</span>
                  <!-- Bouton Tester -->
                  <NButton v-if="dep.test_command || dep.cmd" variant="secondary" size="sm"
                    :disabled="testingDep === dep.id"
                    @click="testDep(dep)">
                    <NSpinner v-if="testingDep === dep.id" :size="12" />
                    <FlaskConical v-else :size="12" />
                    Tester
                  </NButton>
                  <!-- Résultat du test -->
                  <NBadge v-if="testResults[dep.id] === 'ok'" variant="success" style="font-size:10px">OK</NBadge>
                  <NBadge v-else-if="testResults[dep.id] === 'fail'" variant="danger" style="font-size:10px">FAIL</NBadge>
                  <NButton v-if="dep.install_url" variant="ghost" size="sm"
                    @click="openUrl(dep.install_url)">
                    <ExternalLink :size="12" />
                  </NButton>
                </div>
              </td>
            </tr>
            <tr v-if="!filtered.length">
              <td colspan="5" style="text-align:center;color:var(--text-muted);padding:24px;font-size:13px">
                Aucun résultat pour "{{ filterText }}"
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <!-- Note WinGet -->
      <div class="note-box">
        <AlertTriangle :size="14" style="color:var(--warning);flex-shrink:0;margin-top:1px" />
        <span>Les installations via <strong>WinGet</strong> s'exécutent silencieusement en arrière-plan.
          Relancez une vérification après quelques instants pour confirmer.
          Pour les outils sans ID WinGet (Scoop, Chocolatey), cliquez sur <ExternalLink :size="11" style="vertical-align:middle" /> pour accéder à la page officielle.
          En cas d'échec WinGet, un lien de téléchargement manuel ou une recherche Google sera ouvert automatiquement.</span>
      </div>
    </template>
  </div>
</template>

<style scoped>
.dep-page { display:flex;flex-direction:column;gap:18px; }
.page-header { display:flex;justify-content:space-between;align-items:flex-start;flex-wrap:wrap;gap:12px; }
.page-header h1 { font-size:22px;font-weight:700; }
.page-subtitle { color:var(--text-muted);font-size:13px;margin-top:3px; }
.scan-progress-bar { display:flex;align-items:center;gap:10px;padding:9px 14px;background:color-mix(in srgb,var(--accent-primary) 8%,var(--bg-secondary));border:1px solid color-mix(in srgb,var(--accent-primary) 25%,var(--border));border-radius:var(--radius-md); }
.scan-label { font-size:12px;color:var(--accent-primary);font-weight:600;white-space:nowrap; }
.scan-hint { font-size:11px;color:var(--text-muted);white-space:nowrap; }

.stats-bar { display:flex;align-items:center;gap:16px;padding:12px 16px;
  background:var(--bg-secondary);border:1px solid var(--border);border-radius:8px;flex-wrap:wrap; }
.stat-item { display:flex;align-items:center;gap:6px;font-size:13px;font-weight:500; }
.stat-item.ok { color:var(--success); }
.stat-item.warn { color:var(--danger); }
.stat-item.muted { color:var(--text-muted); }
.progress-bar-wrap { flex:1;min-width:120px;height:6px;background:var(--bg-tertiary);border-radius:3px;overflow:hidden; }
.progress-bar-fill { height:100%;background:var(--success);border-radius:3px;transition:width .4s ease; }

.loading-state { display:flex;flex-direction:column;align-items:center;gap:12px;padding:50px;color:var(--text-secondary);font-size:13px; }

.filters { display:flex;align-items:center;gap:12px;flex-wrap:wrap; }
.cat-pills { display:flex;gap:6px;flex-wrap:wrap; }
.cat-pill { padding:4px 12px;border-radius:16px;border:1px solid var(--border);background:transparent;
  font-size:12px;color:var(--text-secondary);cursor:pointer;transition:all .15s;display:flex;align-items:center;gap:5px; }
.cat-pill:hover { border-color:var(--accent-primary);color:var(--accent-primary); }
.cat-pill.active { background:var(--accent-primary);border-color:var(--accent-primary);color:#fff; }
.cat-count { font-size:10px;opacity:.75; }

.dep-table-wrap { overflow-x:auto; }
.dep-table { width:100%;border-collapse:collapse;font-size:13px; }
.dep-table th { text-align:left;padding:8px 12px;color:var(--text-muted);font-weight:500;font-size:11px;
  text-transform:uppercase;letter-spacing:.5px;border-bottom:1px solid var(--border);background:var(--bg-secondary); }
.dep-table td { padding:10px 12px;border-bottom:1px solid var(--border);vertical-align:middle; }
.dep-table tbody tr:hover { background:var(--bg-secondary); }
.dep-table .row-missing { background:color-mix(in srgb,var(--danger) 4%,transparent); }
.cell-status { width:40px;text-align:center; }
.dep-name { font-weight:500;color:var(--text-primary); }
.dep-desc { font-size:11px;color:var(--text-muted);margin-top:2px; }
.cell-version { font-family:"JetBrains Mono",monospace;font-size:11px;color:var(--text-muted); }
.version-text { color:var(--success);font-weight:500; }
.cell-action { width:200px; }
.ok-text { font-size:12px;color:var(--success);font-weight:500; }
.ic-ok { color:var(--success); }
.ic-err { color:var(--danger); }

.note-box { display:flex;gap:10px;align-items:flex-start;font-size:12px;color:var(--text-secondary);
  padding:10px 14px;background:color-mix(in srgb,var(--warning) 8%,transparent);
  border:1px solid color-mix(in srgb,var(--warning) 30%,transparent);border-radius:6px;line-height:1.6; }
</style>
