<script setup lang="ts">
import { ref, computed, onUnmounted } from "vue";
import { invoke } from "@/utils/invoke";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import NButton from "@/components/ui/NButton.vue";
import NProgress from "@/components/ui/NProgress.vue";
import NSpinner from "@/components/ui/NSpinner.vue";
import { useNotificationStore } from "@/stores/notifications";
import {
  HardDrive, FolderOpen, Play, CheckCircle, AlertCircle,
  Activity, Timer, Zap, Shield,
} from "lucide-vue-next";

interface DiskSmartInfo {
  disk_index: number; label: string; health: string;
  size_gb: number; media_type: string; serial: string;
  temperature?: number; power_on_hours?: number;
}
interface DiskImageResult {
  success: boolean; message: string; output_path: string;
  bad_sectors: number; bytes_copied: number; speed_mbs: number;
}
interface ImageProgress {
  bytes_done: number; total_bytes: number; percent: number;
  speed_mbs: number; bad_sectors: number; eta_secs: number;
}

const notify = useNotificationStore();

const disks = ref<DiskSmartInfo[]>([]);
const loadingDisks = ref(false);
const selectedDisk = ref<number | null>(null);
const outputPath = ref("");
const running = ref(false);
const result = ref<DiskImageResult | null>(null);

const progress = ref<ImageProgress | null>(null);
let unlistenProgress: UnlistenFn | null = null;

async function loadDisks() {
  loadingDisks.value = true;
  try {
    disks.value = await invoke<DiskSmartInfo[]>("get_disks_smart");
    if (disks.value.length > 0 && selectedDisk.value === null) {
      selectedDisk.value = disks.value[0].disk_index;
    }
  } catch (e: unknown) { notify.error("Chargement disques", String(e)); }
  loadingDisks.value = false;
}

function suggestPath() {
  if (selectedDisk.value === null) return;
  const d = disks.value.find(x => x.disk_index === selectedDisk.value);
  const label = d?.label.replace(/\s+/g, "_").replace(/[^a-zA-Z0-9_\-]/g, "") || `disk${selectedDisk.value}`;
  const date = new Date().toISOString().slice(0, 10).replace(/-/g, "");
  outputPath.value = `C:\\NiTriTe\\Images\\${label}_${date}.img`;
}

async function startImaging() {
  if (selectedDisk.value === null) { notify.warning("Disque requis", "Sélectionnez un disque source."); return; }
  if (!outputPath.value.trim()) { notify.warning("Chemin requis", "Définissez le fichier de destination."); return; }

  running.value = true;
  result.value = null;
  progress.value = null;

  unlistenProgress = await listen<ImageProgress>("disk-image-progress", e => {
    progress.value = e.payload;
  });

  try {
    const res = await invoke<DiskImageResult>("create_disk_image_cmd", {
      diskIndex: selectedDisk.value,
      outputPath: outputPath.value.trim(),
    });
    result.value = res;
    if (res.success) {
      notify.success("Image créée", res.message);
    } else {
      notify.error("Échec", res.message);
    }
  } catch (e: unknown) {
    notify.error("Erreur", String(e));
  } finally {
    running.value = false;
    if (unlistenProgress) { unlistenProgress(); unlistenProgress = null; }
  }
}

async function openOutputDir() {
  const dir = outputPath.value.split("\\").slice(0, -1).join("\\") || "C:\\";
  try { await invoke("open_in_explorer", { path: dir }); }
  catch {}
}

const selectedDiskInfo = computed(() =>
  disks.value.find(d => d.disk_index === selectedDisk.value) ?? null
);

const fmtBytes = (b: number) => {
  if (b >= 1_073_741_824) return `${(b / 1_073_741_824).toFixed(2)} GB`;
  if (b >= 1_048_576) return `${(b / 1_048_576).toFixed(1)} MB`;
  return `${(b / 1024).toFixed(1)} KB`;
};
const fmtEta = (s: number) => {
  if (s <= 0) return "--";
  if (s < 60) return `${s}s`;
  if (s < 3600) return `${Math.floor(s / 60)}m ${s % 60}s`;
  return `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m`;
};
const healthColor = (h: string) => {
  if (h === "Healthy") return "var(--success)";
  if (h === "Warning") return "var(--warning)";
  if (h === "Unhealthy") return "var(--error)";
  return "var(--text-muted)";
};

onUnmounted(() => {
  if (unlistenProgress) unlistenProgress();
});

loadDisks();
</script>

<template>
  <div class="disk-imager">
    <!-- En-tête -->
    <div class="imager-header">
      <div class="header-icon"><HardDrive :size="22" /></div>
      <div>
        <h2 class="header-title">Création d'image disque</h2>
        <p class="header-desc">Copie secteur par secteur d'un disque physique vers un fichier .img — récupération complète même sur disques endommagés</p>
      </div>
    </div>

    <!-- Sélection disque -->
    <div class="section-card">
      <div class="section-title"><HardDrive :size="14" /> Disque source</div>
      <div v-if="loadingDisks" class="loading-row"><NSpinner :size="16" /> Chargement des disques...</div>
      <div v-else-if="disks.length === 0" class="empty-row">
        <AlertCircle :size="16" />
        <span>Aucun disque physique détecté. Get-PhysicalDisk nécessite souvent les droits Administrateur —
          relancez Nitrite en tant qu'administrateur puis cliquez sur « Rafraîchir ».</span>
      </div>
      <div v-else class="disks-grid">
        <div
          v-for="disk in disks"
          :key="disk.disk_index"
          class="disk-card"
          :class="{ selected: selectedDisk === disk.disk_index }"
          @click="selectedDisk = disk.disk_index; suggestPath()"
        >
          <div class="disk-card-top">
            <HardDrive :size="20" :style="{ color: selectedDisk === disk.disk_index ? 'var(--accent-primary)' : 'var(--text-muted)' }" />
            <span class="disk-badge" :style="{ color: healthColor(disk.health) }">{{ disk.health }}</span>
          </div>
          <div class="disk-name">{{ disk.label || `Disque ${disk.disk_index}` }}</div>
          <div class="disk-meta">
            <span>{{ disk.size_gb.toFixed(1) }} GB</span>
            <span>{{ disk.media_type }}</span>
            <span v-if="disk.temperature">{{ disk.temperature }}°C</span>
          </div>
          <div class="disk-serial" v-if="disk.serial">S/N: {{ disk.serial }}</div>
        </div>
      </div>
      <div class="refresh-row">
        <NButton variant="ghost" size="sm" @click="loadDisks" :loading="loadingDisks">
          Rafraîchir
        </NButton>
      </div>
    </div>

    <!-- Destination -->
    <div class="section-card">
      <div class="section-title"><FolderOpen :size="14" /> Fichier de destination (.img)</div>
      <div class="path-row">
        <input
          v-model="outputPath"
          class="path-input"
          placeholder="Ex: D:\Images\disk0_backup.img"
          :disabled="running"
        />
        <NButton variant="secondary" size="sm" @click="suggestPath" :disabled="running">
          Suggérer
        </NButton>
      </div>
      <p class="path-hint">
        Assurez-vous que la destination a suffisamment d'espace libre (≥ taille du disque source).
        L'opération nécessite les droits Administrateur.
      </p>
    </div>

    <!-- Progression -->
    <div v-if="running || progress" class="section-card progress-card">
      <div class="section-title"><Activity :size="14" /> Progression</div>
      <NProgress
        :value="progress?.percent ?? 0"
        :max="100"
        :glow="true"
        color="accent"
        class="img-progress"
      />
      <div class="progress-stats" v-if="progress">
        <div class="pstat">
          <span class="pstat-label">Copié</span>
          <span class="pstat-val">{{ fmtBytes(progress.bytes_done) }} / {{ fmtBytes(progress.total_bytes) }}</span>
        </div>
        <div class="pstat">
          <span class="pstat-label">Vitesse</span>
          <span class="pstat-val accent">{{ progress.speed_mbs.toFixed(1) }} MB/s</span>
        </div>
        <div class="pstat">
          <span class="pstat-label">Secteurs défectueux</span>
          <span class="pstat-val" :style="{ color: progress.bad_sectors > 0 ? 'var(--error)' : 'var(--success)' }">
            {{ progress.bad_sectors }}
          </span>
        </div>
        <div class="pstat">
          <span class="pstat-label">ETA</span>
          <span class="pstat-val">{{ fmtEta(progress.eta_secs) }}</span>
        </div>
      </div>
      <div v-if="running" class="running-notice">
        <NSpinner :size="14" /> L'image est en cours de création — ne pas fermer l'application.
      </div>
    </div>

    <!-- Résultat -->
    <div v-if="result" class="section-card result-card" :class="result.success ? 'result-ok' : 'result-fail'">
      <div class="result-header">
        <component :is="result.success ? CheckCircle : AlertCircle" :size="20" />
        <span>{{ result.success ? 'Image créée avec succès' : 'Échec de la création' }}</span>
      </div>
      <p class="result-msg">{{ result.message }}</p>
      <div v-if="result.success" class="result-stats">
        <div class="rstat"><Zap :size="12" /> {{ result.speed_mbs.toFixed(1) }} MB/s vitesse moyenne</div>
        <div class="rstat"><Shield :size="12" /> {{ result.bad_sectors }} secteur(s) défectueux</div>
        <div class="rstat"><Activity :size="12" /> {{ fmtBytes(result.bytes_copied) }} copiés</div>
      </div>
      <div class="result-path" v-if="result.success">{{ result.output_path }}</div>
      <NButton v-if="result.success" variant="secondary" size="sm" @click="openOutputDir">
        <FolderOpen :size="13" /> Ouvrir le dossier
      </NButton>
    </div>

    <!-- Infos / Avertissements -->
    <div class="info-box">
      <div class="info-row"><Timer :size="13" /> <strong>Durée estimée :</strong> comptez ~5 min/100 GB pour un SSD, ~15 min/100 GB pour un HDD.</div>
      <div class="info-row"><Shield :size="13" /> <strong>Secteurs défectueux :</strong> les secteurs illisibles sont remplacés par des zéros dans l'image (comportement standard GetDataBack/DDRescue).</div>
      <div class="info-row"><AlertCircle :size="13" /> <strong>Admin requis :</strong> l'accès au disque physique brut nécessite des droits Administrateur Windows.</div>
    </div>

    <!-- Bouton principal -->
    <div class="action-row">
      <NButton
        variant="primary"
        size="lg"
        :disabled="selectedDisk === null || !outputPath.trim() || running"
        :loading="running"
        @click="startImaging"
      >
        <Play :size="15" v-if="!running" />
        {{ running ? 'Création en cours...' : 'Créer l\'image disque' }}
      </NButton>
    </div>
  </div>
</template>

<style scoped>
.disk-imager { display: flex; flex-direction: column; gap: 16px; padding: 4px 0; }

.imager-header {
  display: flex; align-items: flex-start; gap: 14px;
  background: linear-gradient(135deg, rgba(249,115,22,0.08), rgba(249,115,22,0.02));
  border: 1px solid rgba(249,115,22,0.2); border-radius: 10px; padding: 16px 18px;
}
.header-icon {
  width: 42px; height: 42px; background: rgba(249,115,22,0.15); border-radius: 10px;
  display: flex; align-items: center; justify-content: center; color: var(--accent-primary);
  flex-shrink: 0;
}
.header-title { font-size: 15px; font-weight: 700; color: var(--text-primary); margin-bottom: 4px; }
.header-desc { font-size: 12px; color: var(--text-muted); line-height: 1.4; }

.section-card {
  background: var(--surface-secondary); border: 1px solid var(--border);
  border-radius: 10px; padding: 16px;
}
.section-title {
  display: flex; align-items: center; gap: 7px; font-size: 11px; font-weight: 600;
  text-transform: uppercase; letter-spacing: 0.8px; color: var(--text-muted);
  margin-bottom: 12px;
}
.loading-row { display: flex; align-items: center; gap: 8px; color: var(--text-muted); font-size: 13px; }
.empty-row { display: flex; align-items: flex-start; gap: 8px; color: var(--text-muted); font-size: 12px; line-height: 1.5; padding: 10px 12px; background: rgba(234,179,8,0.06); border: 1px solid rgba(234,179,8,0.2); border-radius: 8px; }
.empty-row svg { flex-shrink: 0; margin-top: 1px; color: var(--warning); }

.disks-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 10px; }
.disk-card {
  background: var(--surface-primary); border: 1px solid var(--border);
  border-radius: 8px; padding: 12px; cursor: pointer;
  transition: all 0.2s; user-select: none;
}
.disk-card:hover { border-color: var(--border-hover); background: var(--surface-glass-hover); }
.disk-card.selected { border-color: var(--accent-primary); background: rgba(249,115,22,0.06); }
.disk-card-top { display: flex; align-items: center; justify-content: space-between; margin-bottom: 8px; }
.disk-badge { font-size: 10px; font-weight: 600; }
.disk-name { font-size: 13px; font-weight: 600; color: var(--text-primary); margin-bottom: 6px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.disk-meta { display: flex; gap: 8px; flex-wrap: wrap; font-size: 11px; color: var(--text-muted); margin-bottom: 4px; }
.disk-serial { font-size: 10px; color: var(--text-muted); opacity: 0.7; }

.refresh-row { margin-top: 10px; display: flex; justify-content: flex-end; }

.path-row { display: flex; gap: 8px; align-items: center; margin-bottom: 8px; }
.path-input {
  flex: 1; background: var(--surface-primary); border: 1px solid var(--border);
  border-radius: 6px; padding: 7px 12px; color: var(--text-primary);
  font-size: 13px; font-family: monospace;
  transition: border-color 0.2s;
}
.path-input:focus { outline: none; border-color: var(--accent-primary); }
.path-hint { font-size: 11px; color: var(--text-muted); line-height: 1.4; }

.progress-card { background: rgba(249,115,22,0.04); border-color: rgba(249,115,22,0.2); }
.img-progress { margin-bottom: 12px; }
.progress-stats { display: grid; grid-template-columns: repeat(4, 1fr); gap: 10px; margin-bottom: 10px; }
.pstat { display: flex; flex-direction: column; gap: 3px; }
.pstat-label { font-size: 10px; color: var(--text-muted); text-transform: uppercase; letter-spacing: 0.5px; }
.pstat-val { font-size: 15px; font-weight: 700; color: var(--text-primary); }
.pstat-val.accent { color: var(--accent-primary); }
.running-notice { display: flex; align-items: center; gap: 8px; font-size: 12px; color: var(--text-muted); padding: 8px; background: rgba(249,115,22,0.08); border-radius: 6px; }

.result-card { border-radius: 10px; }
.result-ok { border-color: rgba(34,197,94,0.3); background: rgba(34,197,94,0.05); }
.result-fail { border-color: rgba(239,68,68,0.3); background: rgba(239,68,68,0.05); }
.result-header { display: flex; align-items: center; gap: 10px; font-size: 14px; font-weight: 700; margin-bottom: 8px; }
.result-ok .result-header { color: var(--success); }
.result-fail .result-header { color: var(--error); }
.result-msg { font-size: 12px; color: var(--text-muted); margin-bottom: 12px; }
.result-stats { display: flex; gap: 16px; flex-wrap: wrap; margin-bottom: 10px; }
.rstat { display: flex; align-items: center; gap: 6px; font-size: 12px; color: var(--text-muted); }
.result-path { font-family: monospace; font-size: 11px; color: var(--accent-primary); background: var(--surface-primary); padding: 6px 10px; border-radius: 5px; margin-bottom: 10px; word-break: break-all; }

.info-box { background: rgba(99,102,241,0.05); border: 1px solid rgba(99,102,241,0.15); border-radius: 8px; padding: 12px 14px; }
.info-row { display: flex; align-items: flex-start; gap: 8px; font-size: 12px; color: var(--text-muted); margin-bottom: 6px; }
.info-row:last-child { margin-bottom: 0; }
.info-row strong { color: var(--text-secondary); }

.action-row { display: flex; justify-content: center; padding: 8px 0; }
</style>
