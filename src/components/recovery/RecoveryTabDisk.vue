<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invokeRaw as invoke } from "@/utils/invoke";
import NButton from "@/components/ui/NButton.vue";
import NSpinner from "@/components/ui/NSpinner.vue";
import NProgress from "@/components/ui/NProgress.vue";
import { useNotificationStore } from "@/stores/notifications";
import {
  HardDrive, Folder, FileText, RefreshCw, RotateCcw,
  FolderOpen, CheckSquare, Square, AlertTriangle, ChevronRight,
} from "lucide-vue-next";

const notify = useNotificationStore();

interface DiskInfo { letter: string; label: string; total_gb: number; free_gb: number; disk_type: string; file_system: string; }
interface DiskEntry { name: string; path: string; size_bytes: number; modified: string; is_dir: boolean; }
interface RecoverResult { success: boolean; restored_count: number; failed_count: number; message: string; }

// ── Sélection disque ──────────────────────────────────────────
const disks = ref<DiskInfo[]>([]);
const loadingDisks = ref(false);
const selectedDisk = ref<DiskInfo | null>(null);

async function loadDisks() {
  loadingDisks.value = true; disks.value = [];
  try {
    disks.value = await invoke<DiskInfo[]>("list_connected_disks");
  } catch (e: unknown) { notify.error("Erreur disques", String(e)); }
  loadingDisks.value = false;
}

function selectDisk(disk: DiskInfo) {
  selectedDisk.value = disk;
  browsePath(disk.letter + "\\");
}

// ── Navigation arborescence ───────────────────────────────────
const entries = ref<DiskEntry[]>([]);
const currentPath = ref("");
const pathStack = ref<{ label: string; path: string }[]>([]);
const loadingPath = ref(false);
const selectedItems = ref<Set<string>>(new Set());

async function browsePath(path: string) {
  currentPath.value = path;
  // Reconstruire le breadcrumb
  const disk = selectedDisk.value?.letter ?? "C:";
  const rel = path.slice(disk.length).replace(/^\\+/, "");
  const parts = rel ? rel.split("\\") : [];
  pathStack.value = [{ label: disk, path: disk + "\\" }];
  let acc = disk + "\\";
  for (const p of parts) {
    acc += p + "\\";
    pathStack.value.push({ label: p, path: acc });
  }
  selectedItems.value = new Set();
  loadingPath.value = true;
  entries.value = [];
  try {
    entries.value = await invoke<DiskEntry[]>("browse_disk_path", { path });
    // Dossiers en premier
    entries.value.sort((a, b) => (b.is_dir ? 1 : 0) - (a.is_dir ? 1 : 0) || a.name.localeCompare(b.name));
  } catch (e: unknown) { notify.error("Erreur lecture", String(e)); }
  loadingPath.value = false;
}

function navigateTo(path: string) { browsePath(path); }
function navigateInto(entry: DiskEntry) {
  if (entry.is_dir) browsePath(entry.path);
}

// ── Sélection multi ───────────────────────────────────────────
function toggleItem(path: string) {
  const s = new Set(selectedItems.value);
  if (s.has(path)) s.delete(path); else s.add(path);
  selectedItems.value = s;
}
function toggleAll() {
  if (selectedItems.value.size === entries.value.length) selectedItems.value = new Set();
  else selectedItems.value = new Set(entries.value.map(e => e.path));
}

// ── Récupération ──────────────────────────────────────────────
const restoreTarget = ref("D:\\Récupération");
const safeMode = ref(true);
const recovering = ref(false);
const recoverProgress = ref(0);
const recoverMsg = ref("");
const recoverResult = ref<RecoverResult | null>(null);

async function pickTarget() {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const dir = await open({ directory: true, multiple: false, title: "Dossier de destination" });
  if (dir) restoreTarget.value = dir as string;
}

async function startRecover() {
  if (selectedItems.value.size === 0 || !restoreTarget.value) return;
  recovering.value = true;
  recoverResult.value = null;
  recoverProgress.value = 5;
  recoverMsg.value = "Démarrage...";

  const { listen } = await import("@tauri-apps/api/event");
  const unlisten = await listen<{ percent: number; message: string }>("recover-disk-progress", ev => {
    recoverProgress.value = ev.payload.percent;
    recoverMsg.value = ev.payload.message;
  });
  try {
    recoverResult.value = await invoke<RecoverResult>("recover_files_safe", {
      files: Array.from(selectedItems.value),
      targetFolder: restoreTarget.value,
      safeMode: safeMode.value,
    });
    if (recoverResult.value.success)
      notify.success(`${recoverResult.value.restored_count} élément(s) récupéré(s)`, recoverResult.value.message);
    else
      notify.error("Récupération partielle", recoverResult.value.message);
  } catch (e: unknown) { notify.error("Erreur", String(e)); }
  unlisten();
  recovering.value = false;
}

function formatSize(bytes: number) {
  if (!bytes) return "—";
  if (bytes < 1024) return `${bytes} o`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} Ko`;
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} Mo`;
  return `${(bytes / 1024 ** 3).toFixed(2)} Go`;
}
function formatDate(s: string) { try { return new Date(s).toLocaleString("fr-FR"); } catch { return s; } }

onMounted(loadDisks);
</script>

<template>
  <div class="disk-recovery">
    <!-- Bannière avertissement disque défaillant -->
    <div class="warn-banner">
      <AlertTriangle :size="14" />
      <span>
        <strong>Disque défaillant ?</strong> Activez le <em>Mode Sécurisé</em> — 1 seule tentative, sans attente,
        copie directe via .NET FileStream. Pour les pannes matérielles graves (cliquetis, moteur), arrêtez immédiatement
        et utilisez <strong>ddrescue</strong> ou <strong>TestDisk</strong>.
      </span>
    </div>

    <!-- Sélection disque -->
    <div class="section-label">Lecteurs disponibles</div>
    <div class="toolbar">
      <NButton variant="ghost" size="sm" :loading="loadingDisks" @click="loadDisks">
        <RefreshCw :size="13" /> Actualiser
      </NButton>
    </div>
    <div v-if="loadingDisks" class="loading-state"><NSpinner :size="18" /><span>Détection des lecteurs...</span></div>
    <div v-else-if="disks.length === 0" class="empty"><HardDrive :size="28" /><p>Aucun lecteur détecté</p></div>
    <div v-else class="disks-grid">
      <button
        v-for="d in disks" :key="d.letter"
        class="disk-card"
        :class="{ selected: selectedDisk?.letter === d.letter }"
        @click="selectDisk(d)"
      >
        <HardDrive :size="22" class="disk-icon" />
        <div class="disk-info">
          <span class="disk-letter">{{ d.letter }}</span>
          <span class="disk-label">{{ d.label || d.disk_type }}</span>
          <span class="disk-size">{{ d.total_gb }} Go · {{ d.free_gb }} Go libres</span>
          <span class="disk-fs">{{ d.file_system }}</span>
        </div>
        <span class="disk-type-badge" :class="d.disk_type === 'Amovible' ? 'removable' : ''">{{ d.disk_type }}</span>
      </button>
    </div>

    <!-- Arborescence -->
    <template v-if="selectedDisk">
      <div class="section-label" style="margin-top:8px">Arborescence — {{ selectedDisk.letter }}</div>

      <!-- Breadcrumb -->
      <div class="breadcrumb">
        <template v-for="(seg, i) in pathStack" :key="seg.path">
          <ChevronRight v-if="i > 0" :size="12" class="crumb-sep" />
          <button
            class="crumb"
            :class="{ current: i === pathStack.length - 1 }"
            @click="i < pathStack.length - 1 && navigateTo(seg.path)"
          >{{ seg.label }}</button>
        </template>
      </div>

      <!-- Options récupération -->
      <div class="recover-options">
        <label class="safe-toggle" :class="{ active: safeMode }" @click="safeMode = !safeMode">
          <span class="safe-dot"></span>
          Mode Sécurisé (disque défaillant)
        </label>
        <div class="target-row">
          <input v-model="restoreTarget" class="target-input" placeholder="Destination..." />
          <NButton variant="ghost" size="sm" @click="pickTarget"><FolderOpen :size="13" /> Parcourir</NButton>
        </div>
      </div>

      <!-- Sélection & action -->
      <div class="batch-bar">
        <label class="check-all" @click="toggleAll">
          <CheckSquare v-if="selectedItems.size === entries.length && entries.length > 0" :size="14" />
          <Square v-else :size="14" />
          <span>{{ selectedItems.size > 0 ? `${selectedItems.size} sélectionné(s)` : 'Tout sélectionner' }}</span>
        </label>
        <NButton
          v-if="selectedItems.size > 0"
          variant="primary" size="sm"
          :loading="recovering"
          :disabled="!restoreTarget"
          @click="startRecover"
        >
          <RotateCcw :size="12" /> Récupérer ({{ selectedItems.size }})
        </NButton>
      </div>

      <!-- Contenu dossier -->
      <div v-if="loadingPath" class="loading-state"><NSpinner :size="16" /><span>Lecture...</span></div>
      <div v-else-if="entries.length === 0" class="empty-small">Dossier vide ou inaccessible</div>
      <div v-else class="entries-table">
        <div class="entry-row header-row">
          <span></span><span>Nom</span><span>Taille</span><span>Modifié</span>
        </div>
        <div
          v-for="e in entries" :key="e.path"
          class="entry-row"
          style="cursor:pointer"
          :class="{ 'is-dir': e.is_dir, 'is-selected': selectedItems.has(e.path) }"
          @click="e.is_dir ? navigateInto(e) : toggleItem(e.path)"
        >
          <span class="entry-check">
            <CheckSquare v-if="selectedItems.has(e.path)" :size="13" class="chk on" @click.stop="toggleItem(e.path)" />
            <Square v-else :size="13" class="chk" @click.stop="toggleItem(e.path)" />
          </span>
          <span class="entry-name">
            <Folder v-if="e.is_dir" :size="13" class="ic-dir" />
            <FileText v-else :size="13" class="ic-file" />
            {{ e.name }}
          </span>
          <span class="entry-size">{{ e.is_dir ? '—' : formatSize(e.size_bytes) }}</span>
          <span class="entry-date">{{ formatDate(e.modified) }}</span>
        </div>
      </div>

      <!-- Progression -->
      <div v-if="recovering || recoverProgress > 0" class="progress-section">
        <div class="progress-header"><NSpinner v-if="recovering" :size="14" /><span>{{ recoverMsg }}</span></div>
        <NProgress :value="recoverProgress" showLabel size="lg" color="accent" />
      </div>

      <!-- Résultat -->
      <div v-if="recoverResult" class="result-card" :class="recoverResult.success ? 'success' : 'error'">
        <div>
          <p class="result-title">{{ recoverResult.success ? `${recoverResult.restored_count} élément(s) récupéré(s)` : 'Récupération échouée' }}</p>
          <p class="result-msg">{{ recoverResult.message }}</p>
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.disk-recovery { display: flex; flex-direction: column; gap: 10px; }

.warn-banner {
  display: flex; gap: 8px; align-items: flex-start; padding: 10px 14px;
  border-radius: var(--radius-md); font-size: 12px; line-height: 1.5;
  background: color-mix(in srgb, #f59e0b 12%, transparent);
  color: #d97706; border: 1px solid #d97706;
}

.section-label { font-size: 11px; font-weight: 700; text-transform: uppercase; letter-spacing: .06em; color: var(--text-muted); }
.toolbar { display: flex; align-items: center; gap: 8px; }
.loading-state { display: flex; align-items: center; gap: 10px; padding: 20px; font-size: 13px; color: var(--text-muted); }
.empty { display: flex; flex-direction: column; align-items: center; gap: 10px; padding: 40px; color: var(--text-muted); }
.empty-small { padding: 16px; text-align: center; font-size: 12px; color: var(--text-muted); }

/* Disques */
.disks-grid { display: flex; flex-wrap: wrap; gap: 10px; }
.disk-card {
  display: flex; align-items: center; gap: 12px; padding: 12px 16px;
  border: 1.5px solid var(--border); border-radius: var(--radius-lg);
  background: var(--bg-secondary); cursor: pointer; text-align: left;
  transition: all 0.15s; font-family: inherit; min-width: 220px;
}
.disk-card:hover { border-color: var(--text-muted); }
.disk-card.selected { border-color: var(--accent-primary); background: color-mix(in srgb, var(--accent-primary) 8%, transparent); }
.disk-icon { color: var(--accent-primary); flex-shrink: 0; }
.disk-info { display: flex; flex-direction: column; gap: 2px; flex: 1; }
.disk-letter { font-size: 15px; font-weight: 700; color: var(--text-primary); }
.disk-label { font-size: 12px; color: var(--text-secondary); }
.disk-size { font-size: 11px; color: var(--text-muted); font-family: monospace; }
.disk-fs { font-size: 10px; color: var(--text-muted); }
.disk-type-badge { font-size: 10px; padding: 2px 7px; border-radius: 10px; background: var(--bg-tertiary); color: var(--text-muted); }
.disk-type-badge.removable { background: color-mix(in srgb, var(--accent-primary) 15%, transparent); color: var(--accent-primary); }

/* Breadcrumb */
.breadcrumb { display: flex; align-items: center; flex-wrap: wrap; gap: 2px; font-size: 12px; }
.crumb { background: none; border: none; padding: 2px 5px; cursor: pointer; color: var(--accent-primary); font-family: inherit; font-size: 12px; border-radius: var(--radius-sm); transition: background 0.12s; }
.crumb:hover { background: var(--bg-tertiary); }
.crumb.current { color: var(--text-primary); font-weight: 600; cursor: default; }
.crumb-sep { color: var(--text-muted); }

/* Options */
.recover-options { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
.safe-toggle {
  display: flex; align-items: center; gap: 6px; cursor: pointer; user-select: none;
  font-size: 12px; padding: 5px 10px; border-radius: var(--radius-md);
  border: 1px solid var(--border); background: var(--bg-secondary); color: var(--text-secondary);
  transition: all 0.15s;
}
.safe-toggle.active { border-color: #d97706; color: #d97706; background: color-mix(in srgb, #d97706 10%, transparent); }
.safe-dot {
  width: 8px; height: 8px; border-radius: 50%;
  background: var(--border); transition: background 0.15s;
}
.safe-toggle.active .safe-dot { background: #d97706; }
.target-row { display: flex; gap: 6px; align-items: center; flex: 1; min-width: 280px; }
.target-input {
  flex: 1; padding: 6px 10px; font-size: 12px; background: var(--bg-tertiary);
  border: 1px solid var(--border); border-radius: var(--radius-sm);
  color: var(--text-primary); font-family: monospace; outline: none;
}
.target-input:focus { border-color: var(--accent-primary); }

/* Batch bar */
.batch-bar { display: flex; align-items: center; gap: 12px; padding: 6px 4px; border-bottom: 1px solid var(--border); }
.check-all { display: flex; align-items: center; gap: 6px; font-size: 12px; cursor: pointer; color: var(--text-secondary); user-select: none; }

/* Entries table */
.entries-table { display: flex; flex-direction: column; border: 1px solid var(--border); border-radius: var(--radius-md); overflow: hidden; max-height: 420px; overflow-y: auto; }
.entry-row { display: grid; grid-template-columns: 24px 1fr 80px 140px; align-items: center; gap: 8px; padding: 5px 10px; border-bottom: 1px solid var(--border); font-size: 12px; }
.entry-row:last-child { border-bottom: none; }
.header-row { background: var(--bg-tertiary); font-size: 11px; font-weight: 700; color: var(--text-muted); text-transform: uppercase; letter-spacing: .05em; position: sticky; top: 0; }
.entry-row.is-dir { background: color-mix(in srgb, var(--accent-primary) 4%, transparent); }
.entry-row.is-selected { background: color-mix(in srgb, var(--accent-primary) 10%, transparent); }
.entry-check { display: flex; align-items: center; justify-content: center; }
.chk { cursor: pointer; color: var(--text-muted); transition: color 0.1s; }
.chk:hover, .chk.on { color: var(--accent-primary); }
.entry-name { display: flex; align-items: center; gap: 6px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-primary); }
.ic-dir { color: var(--accent-primary); flex-shrink: 0; }
.ic-file { color: var(--text-muted); flex-shrink: 0; }
.entry-size, .entry-date { color: var(--text-muted); font-family: monospace; }

/* Progress / result */
.progress-section { display: flex; flex-direction: column; gap: 8px; padding: 12px 14px; background: var(--bg-secondary); border-radius: var(--radius-md); border: 1px solid var(--border); }
.progress-header { display: flex; align-items: center; gap: 8px; font-size: 12px; color: var(--text-secondary); }
.result-card { display: flex; align-items: flex-start; gap: 12px; padding: 12px 14px; border-radius: var(--radius-lg); border: 1px solid; }
.result-card.success { background: var(--success-muted); border-color: var(--success); color: var(--success); }
.result-card.error { background: var(--danger-muted); border-color: var(--danger); color: var(--danger); }
.result-title { font-weight: 700; font-size: 13px; }
.result-msg { font-size: 12px; color: var(--text-secondary); margin-top: 3px; }
</style>
