<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke } from "@/utils/invoke";
import { RefreshCw, CheckSquare, Square, Folder, FolderOpen, Save, CheckCircle, XCircle, Plus } from "lucide-vue-next";
import NButton from "@/components/ui/NButton.vue";
import NProgress from "@/components/ui/NProgress.vue";
import NSpinner from "@/components/ui/NSpinner.vue";
import { useNotificationStore } from "@/stores/notifications";

const notify = useNotificationStore();

interface UserFolder { name: string; path: string; size_mb: number; shadow_relative: string; }
interface BackupResult { success: boolean; message: string; duration_secs: number; folders_count: number; }

const userFolders = ref<UserFolder[]>([]);
const loadingFolders = ref(false);
const selectedFolders = ref<Set<string>>(new Set());
const backupTarget = ref("");
const backingUp = ref(false);
const backupProgress = ref(0);
const backupProgressMsg = ref("");
const backupResult = ref<BackupResult | null>(null);
const customFolder = ref("");

async function loadUserFolders(force = false) {
  if (!force && userFolders.value.length > 0) return;
  loadingFolders.value = true;
  try {
    userFolders.value = await invoke<UserFolder[]>("get_user_profile_folders");
    if (force || selectedFolders.value.size === 0) {
      selectedFolders.value = new Set(
        userFolders.value.filter(f => ["Documents", "Bureau"].includes(f.name)).map(f => f.path)
      );
    }
  } catch (e: unknown) { notify.error("Erreur profil", String(e)); }
  loadingFolders.value = false;
}

function toggleFolder(path: string) {
  if (selectedFolders.value.has(path)) selectedFolders.value.delete(path);
  else selectedFolders.value.add(path);
  selectedFolders.value = new Set(selectedFolders.value);
}

async function pickBackupTarget() {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const dir = await open({ directory: true, multiple: false, title: "Choisir le dossier de destination" });
  if (dir) backupTarget.value = dir as string;
}

async function pickCustomFolder() {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const dir = await open({ directory: true, multiple: false, title: "Ajouter un dossier personnalisé" });
  if (dir) customFolder.value = dir as string;
}

function addCustomFolder() {
  const f = customFolder.value.trim();
  if (!f) return;
  if (!userFolders.value.some(x => x.path === f)) {
    userFolders.value = [...userFolders.value, { name: f.split(/[\\/]/).pop() || f, path: f, size_mb: 0, shadow_relative: "" }];
  }
  selectedFolders.value.add(f);
  selectedFolders.value = new Set(selectedFolders.value);
  customFolder.value = "";
}

async function startProfileBackup() {
  if (!backupTarget.value || selectedFolders.value.size === 0) return;
  backingUp.value = true;
  backupResult.value = null;
  backupProgress.value = 5;
  backupProgressMsg.value = "Démarrage de la sauvegarde...";

  const { listen } = await import("@tauri-apps/api/event");
  const unlisten = await listen<{ folder: string; percent: number; message: string }>(
    "backup-profile-progress", ev => {
      backupProgress.value = ev.payload.percent;
      backupProgressMsg.value = ev.payload.message;
    }
  );
  try {
    backupResult.value = await invoke<BackupResult>("backup_user_folders", {
      folders: Array.from(selectedFolders.value),
      target: backupTarget.value,
    });
    if (backupResult.value.success) notify.success("Sauvegarde terminée", backupResult.value.message);
    else notify.error("Échec", backupResult.value.message);
  } catch (e: unknown) { notify.error("Erreur sauvegarde", String(e)); }
  unlisten();
  backingUp.value = false;
}

async function openFolder(path: string) {
  try { await invoke("open_in_explorer", { path }); } catch { /* ignore */ }
}

function formatSize_mb(mb: number) {
  if (mb < 1024) return `${mb} Mo`;
  return `${(mb / 1024).toFixed(1)} Go`;
}

onMounted(() => loadUserFolders());
</script>

<template>
  <div>
    <div class="info-banner">
      <Save :size="14" />
      <span>Sauvegardes vos dossiers utilisateur (Documents, Bureau, Images…) vers un disque externe ou un autre emplacement. Utilise Robocopy en mode backup pour contourner les restrictions d'accès.</span>
    </div>

    <div class="toolbar">
      <NButton variant="ghost" size="sm" :loading="loadingFolders" @click="loadUserFolders(true)">
        <RefreshCw :size="13" /> Actualiser les dossiers
      </NButton>
      <span class="count">{{ userFolders.length }} dossier(s) détecté(s)</span>
    </div>

    <div v-if="loadingFolders" class="loading-state"><NSpinner :size="20" /><span>Lecture du profil...</span></div>
    <div v-else class="folders-section">
      <p class="section-label">Dossiers à sauvegarder</p>
      <div class="folders-grid">
        <div
          v-for="f in userFolders" :key="f.path"
          class="folder-card"
          :class="{ selected: selectedFolders.has(f.path) }"
          @click="toggleFolder(f.path)"
        >
          <CheckSquare v-if="selectedFolders.has(f.path)" :size="15" class="ic-check" />
          <Square v-else :size="15" class="ic-uncheck" />
          <Folder :size="20" class="folder-icon" />
          <div class="folder-info">
            <span class="folder-name">{{ f.name }}</span>
            <span class="folder-size">{{ formatSize_mb(f.size_mb) }}</span>
          </div>
        </div>
      </div>

      <div class="custom-folder-row">
        <input v-model="customFolder" class="target-input-lg" placeholder="Ajouter un dossier personnalisé..." @keydown.enter="addCustomFolder" />
        <NButton variant="ghost" size="sm" @click="pickCustomFolder"><FolderOpen :size="13" /></NButton>
        <NButton variant="ghost" size="sm" :disabled="!customFolder.trim()" @click="addCustomFolder"><Plus :size="13" /> Ajouter</NButton>
      </div>

      <p class="section-label">Destination</p>
      <div class="target-row">
        <input v-model="backupTarget" class="target-input-lg" placeholder="Chemin de destination (ex: D:\Sauvegardes\Profil)" />
        <NButton variant="ghost" size="sm" @click="pickBackupTarget">
          <FolderOpen :size="13" /> Parcourir
        </NButton>
      </div>

      <div v-if="backingUp || backupProgress > 0" class="progress-section">
        <div class="progress-header">
          <NSpinner v-if="backingUp" :size="14" />
          <span>{{ backupProgressMsg }}</span>
        </div>
        <NProgress :value="backupProgress" showLabel size="lg" color="accent" />
      </div>

      <div v-if="backupResult" class="result-card" :class="backupResult.success ? 'success' : 'error'">
        <CheckCircle v-if="backupResult.success" :size="18" />
        <XCircle v-else :size="18" />
        <div>
          <p class="result-title">{{ backupResult.success ? 'Sauvegarde terminée' : 'Échec de la sauvegarde' }}</p>
          <p class="result-msg">{{ backupResult.message }}</p>
          <p class="result-meta">{{ backupResult.folders_count }} dossier(s) · {{ Math.floor(backupResult.duration_secs / 60) }}min {{ backupResult.duration_secs % 60 }}s</p>
        </div>
      </div>

      <div style="display:flex;gap:8px;align-items:center;flex-wrap:wrap">
        <NButton
          variant="primary"
          :disabled="!backupTarget || selectedFolders.size === 0 || backingUp"
          :loading="backingUp"
          @click="startProfileBackup"
        >
          <Save :size="14" /> Lancer la sauvegarde ({{ selectedFolders.size }} dossier(s))
        </NButton>
        <NButton v-if="backupResult?.success && backupTarget" variant="ghost" size="sm" @click="openFolder(backupTarget)">
          <FolderOpen :size="13" /> Ouvrir destination
        </NButton>
      </div>
    </div>
  </div>
</template>
