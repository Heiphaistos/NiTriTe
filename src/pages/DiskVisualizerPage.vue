<script setup lang="ts">
defineOptions({ name: "DiskVisualizerPage" });
import { ref, shallowRef, computed, onMounted } from "vue";
import { localDataDir } from "@tauri-apps/api/path";
import { invoke, invokeRaw } from "@/utils/invoke";
import NCard from "@/components/ui/NCard.vue";
import NButton from "@/components/ui/NButton.vue";
import NSpinner from "@/components/ui/NSpinner.vue";
import { useNotificationStore } from "@/stores/notifications";
import { HardDrive, FolderOpen, ChevronRight, Home, ArrowUpDown } from "lucide-vue-next";

const notify = useNotificationStore();

interface DiskNode { name: string; path: string; size_bytes: number; is_dir: boolean; children: DiskNode[]; }

const rootNode   = shallowRef<DiskNode | null>(null);
const currentNode = shallowRef<DiskNode | null>(null);
const breadcrumb  = ref<DiskNode[]>([]);
const loading     = ref(false);
const scanPath    = ref("C:\\Users");
const maxDepth    = ref(2);
const sortDesc    = ref(true);

const COLORS = ["#f97316","#8b5cf6","#3b82f6","#22c55e","#eab308","#ef4444","#06b6d4","#ec4899","#14b8a6","#a855f7","#f59e0b","#10b981"];

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 ** 3) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

function percent(node: DiskNode, parent: DiskNode): number {
  return parent.size_bytes > 0 ? Math.max(0.5, (node.size_bytes / parent.size_bytes) * 100) : 0;
}

const sortedChildren = computed(() => {
  if (!currentNode.value) return [];
  return [...currentNode.value.children].sort((a, b) =>
    sortDesc.value ? b.size_bytes - a.size_bytes : a.size_bytes - b.size_bytes
  );
});

// Assign stable color per child based on sorted index
function childColor(idx: number): string {
  return COLORS[idx % COLORS.length];
}

// Top 5 for legend
const top5 = computed(() =>
  sortedChildren.value.slice(0, 5).map((c, i) => ({ node: c, color: childColor(i) }))
);

async function scan() {
  if (!scanPath.value) return;
  loading.value = true;
  try {
    const node = await invokeRaw<DiskNode>("get_disk_tree", { path: scanPath.value, maxDepth: maxDepth.value });
    rootNode.value = node;
    currentNode.value = node;
    breadcrumb.value = [node];
  } catch (e: unknown) {
    if (String(e).includes("annulée")) notify.info("Analyse annulée", "Analyse interrompue.");
    else notify.error("Erreur scan", String(e));
  } finally {
    loading.value = false;
  }
}

function cancelScan() {
  invoke("cancel_disk_scan").catch((e: unknown) => notify.error("Annulation", String(e)));
}

function navigate(node: DiskNode) {
  if (!node.is_dir || !node.children?.length) return;
  currentNode.value = node;
  const idx = breadcrumb.value.findIndex(b => b.path === node.path);
  if (idx !== -1) breadcrumb.value = breadcrumb.value.slice(0, idx + 1);
  else breadcrumb.value.push(node);
}

function navigateBreadcrumb(node: DiskNode) {
  currentNode.value = node;
  const idx = breadcrumb.value.findIndex(b => b.path === node.path);
  if (idx !== -1) breadcrumb.value = breadcrumb.value.slice(0, idx + 1);
}

async function openInExplorer(path: string) {
  try { await invoke("open_path", { path }); }
  catch (e: unknown) { notify.error("Ouverture impossible", String(e)); }
}

const commonPaths = ref([
  { label: "C:\\", path: "C:\\" },
  { label: "Users", path: "C:\\Users" },
  { label: "Program Files", path: "C:\\Program Files" },
  { label: "Temp", path: "C:\\Windows\\Temp" },
]);
// L'ancien raccourci « C:\Users\Public\AppData\Local » n'existe pas : on
// résout le vrai %LOCALAPPDATA% de l'utilisateur.
onMounted(async () => {
  try { commonPaths.value.push({ label: "AppData Local", path: await localDataDir() }); }
  catch { /* hors Tauri (tests) : raccourci simplement absent */ }
});
</script>

<template>
  <div class="disk-page">
    <div class="page-header">
      <div>
        <h1>Visualiseur d'Espace Disque</h1>
        <p class="page-subtitle">Carte interactive de l'occupation disque par dossiers</p>
      </div>
    </div>

    <!-- Contrôles -->
    <NCard>
      <div class="controls-row">
        <div class="path-group">
          <input v-model="scanPath" class="path-input" placeholder="C:\Users" @keyup.enter="scan" />
          <select v-model="maxDepth" class="depth-select">
            <option :value="2">Profondeur 2 (rapide)</option>
            <option :value="3">Profondeur 3 (lent)</option>
            <option :value="4">Profondeur 4 (très lent)</option>
          </select>
          <NButton variant="primary" :loading="loading" @click="scan">
            <HardDrive :size="14" /> Scanner
          </NButton>
        </div>
        <div class="quick-paths">
          <button v-for="p in commonPaths" :key="p.path" class="quick-btn" @click="scanPath = p.path; scan()">{{ p.label }}</button>
        </div>
      </div>
    </NCard>

    <!-- Treemap -->
    <NCard v-if="currentNode">
      <template #header>
        <div style="display:flex;align-items:center;gap:6px;flex-wrap:wrap">
          <button class="bread-btn" @click="navigateBreadcrumb(breadcrumb[0])"><Home :size="13" /></button>
          <template v-for="(b, i) in breadcrumb" :key="b.path">
            <ChevronRight v-if="i > 0" :size="13" style="color:var(--text-muted)" />
            <button class="bread-btn" :class="{active: i === breadcrumb.length-1}" @click="navigateBreadcrumb(b)">{{ b.name }}</button>
          </template>
          <span style="margin-left:auto;font-size:12px;color:var(--text-muted)">Total : <strong style="color:var(--text-primary)">{{ formatSize(currentNode.size_bytes) }}</strong></span>
          <button class="sort-btn" @click="sortDesc = !sortDesc" :title="sortDesc ? 'Tri décroissant' : 'Tri croissant'">
            <ArrowUpDown :size="13" />
            {{ sortDesc ? 'Plus grand d\'abord' : 'Plus petit d\'abord' }}
          </button>
        </div>
      </template>

      <!-- Légende top 5 -->
      <div v-if="top5.length" class="legend">
        <div v-for="item in top5" :key="item.node.path" class="legend-item">
          <span class="legend-dot" :style="{ background: item.color }"></span>
          <span class="legend-name">{{ item.node.name }}</span>
          <span class="legend-size">{{ formatSize(item.node.size_bytes) }}</span>
          <span class="legend-pct">{{ currentNode ? percent(item.node, currentNode).toFixed(1) : '0.0' }}%</span>
        </div>
      </div>

      <div class="treemap">
        <div
          v-for="(child, idx) in sortedChildren"
          :key="child.path"
          class="treemap-cell"
          :class="{ folder: child.is_dir && child.children.length > 0 }"
          :style="{
            width: (currentNode ? percent(child, currentNode) : 0) + '%',
            background: child.is_dir ? childColor(idx) + '18' : 'var(--bg-tertiary)',
            borderColor: child.is_dir ? childColor(idx) : 'var(--border)'
          }"
          @click="navigate(child)"
          :title="child.name + ' — ' + formatSize(child.size_bytes)"
        >
          <div class="cell-bar" :style="{ background: child.is_dir ? childColor(idx) : '#6b7280' }"></div>
          <div class="cell-info">
            <span class="cell-name">{{ child.name }}</span>
            <span class="cell-size">{{ formatSize(child.size_bytes) }}</span>
            <span class="cell-pct">{{ percent(child, currentNode!).toFixed(1) }}%</span>
          </div>
          <button v-if="child.is_dir" class="open-btn" @click.stop="openInExplorer(child.path)" title="Ouvrir dans l'Explorateur">
            <FolderOpen :size="11" />
          </button>
        </div>
        <div v-if="sortedChildren.length === 0" class="no-children">
          Ce dossier n'a pas de sous-dossiers scannés (trop profond ou vide)
        </div>
      </div>
    </NCard>

    <div v-if="loading" style="display:flex;justify-content:center;padding:40px;align-items:center;gap:12px">
      <NSpinner :size="28" /><span style="color:var(--text-muted)">Analyse en cours...</span>
      <NButton variant="ghost" size="sm" @click="cancelScan">Annuler</NButton>
    </div>
  </div>
</template>

<style scoped>
.disk-page { display:flex; flex-direction:column; gap:16px; }
.page-header h1 { font-size:24px; font-weight:700; }
.page-subtitle { color:var(--text-muted); font-size:13px; margin-top:2px; }

.controls-row { display:flex; flex-direction:column; gap:10px; }
.path-group { display:flex; gap:8px; flex-wrap:wrap; }
.path-input {
  flex:1; min-width:200px; padding:8px 12px; border:1px solid var(--border); border-radius:var(--radius-md);
  background:var(--bg-tertiary); color:var(--text-primary); font-family:monospace; font-size:13px; outline:none;
}
.path-input:focus { border-color:var(--accent-primary); }
.depth-select {
  padding:8px 10px; border:1px solid var(--border); border-radius:var(--radius-md);
  background:var(--bg-tertiary); color:var(--text-secondary); font-size:12px; cursor:pointer;
}
.quick-paths { display:flex; gap:6px; flex-wrap:wrap; }
.quick-btn {
  padding:3px 10px; border:1px solid var(--border); border-radius:var(--radius-sm);
  background:var(--bg-secondary); color:var(--text-secondary); font-size:11px; font-family:monospace; cursor:pointer;
  transition:all var(--transition-fast);
}
.quick-btn:hover { border-color:var(--accent-primary); color:var(--accent-primary); }

.bread-btn { background:none; border:none; color:var(--text-secondary); cursor:pointer; font-size:12px; padding:2px 4px; border-radius:4px; }
.bread-btn:hover, .bread-btn.active { color:var(--accent-primary); }

.sort-btn {
  display:flex; align-items:center; gap:4px; background:none; border:1px solid var(--border);
  border-radius:var(--radius-sm); color:var(--text-muted); cursor:pointer; font-size:11px; padding:2px 8px;
  transition:all var(--transition-fast);
}
.sort-btn:hover { border-color:var(--accent-primary); color:var(--accent-primary); }

.legend {
  display:flex; flex-wrap:wrap; gap:8px; padding:8px 0 12px;
  border-bottom:1px solid var(--border); margin-bottom:8px;
}
.legend-item { display:flex; align-items:center; gap:5px; font-size:11px; }
.legend-dot { width:8px; height:8px; border-radius:50%; flex-shrink:0; }
.legend-name { color:var(--text-secondary); max-width:120px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.legend-size { font-family:monospace; color:var(--text-primary); font-weight:600; }
.legend-pct { color:var(--text-muted); }

.treemap { display:flex; flex-wrap:wrap; gap:6px; padding:8px 0; }
.treemap-cell {
  min-width:80px; min-height:60px; border:1px solid; border-radius:var(--radius-md);
  display:flex; align-items:stretch; overflow:hidden; position:relative;
  transition:all var(--transition-fast); cursor:default;
}
.treemap-cell.folder { cursor:pointer; }
.treemap-cell.folder:hover { transform:translateY(-2px); box-shadow:0 4px 12px rgba(0,0,0,.2); }
.cell-bar { width:4px; flex-shrink:0; }
.cell-info { flex:1; padding:8px; display:flex; flex-direction:column; gap:2px; min-width:0; }
.cell-name { font-size:12px; font-weight:600; color:var(--text-primary); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.cell-size { font-size:11px; color:var(--text-secondary); }
.cell-pct { font-size:10px; color:var(--text-muted); }
.open-btn { position:absolute; top:4px; right:4px; background:rgba(0,0,0,.3); border:none; color:white; cursor:pointer; border-radius:4px; padding:2px; opacity:0; transition:opacity var(--transition-fast); }
.treemap-cell:hover .open-btn { opacity:1; }
.no-children { color:var(--text-muted); font-size:13px; padding:24px; width:100%; text-align:center; }
</style>
