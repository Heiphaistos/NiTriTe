<script setup lang="ts">
import { computed, ref } from "vue";
import { invoke } from "@/utils/invoke";
import NProgress from "@/components/ui/NProgress.vue";
import NBadge from "@/components/ui/NBadge.vue";
import NButton from "@/components/ui/NButton.vue";
import NSpinner from "@/components/ui/NSpinner.vue";
import DiagBanner from "@/components/ui/DiagBanner.vue";
import { FolderOpen, Trash2, CheckCircle } from "lucide-vue-next";

import type { FolderEntry } from "@/types/diagnostic";

interface RepairResult { command: string; success: boolean; output: string; duration_secs: number; }

async function openFolder(path: string) {
  await invoke("open_path", { path }).catch(() => {});
}

const props = defineProps<{
  folders: FolderEntry[];
}>();

// Mapping labels → clés de réparation (run_repair_command)
const CLEAN_MAP: Record<string, string> = {
  "temp":         "temp_cleanup",
  "tmp":          "temp_cleanup",
  "préchargement":"clear_prefetch",
  "prefetch":     "clear_prefetch",
  "miniatures":   "thumbnail_cache",
  "thumbnails":   "thumbnail_cache",
  "thumbcache":   "thumbnail_cache",
  "dump":         "memory_dumps",
  "vidage":       "memory_dumps",
  "icône":        "icon_cache",
  "icon cache":   "icon_cache",
  "logs":         "clear_event_logs",
  "journaux":     "clear_event_logs",
  "delivery":     "delivery_opt",
  "download":     "delivery_opt",
};

function repairKeyFor(folder: FolderEntry): string | null {
  const lbl = (folder.label || "").toLowerCase();
  const path = (folder.path || "").toLowerCase();
  for (const [kw, key] of Object.entries(CLEAN_MAP)) {
    if (lbl.includes(kw) || path.includes(kw)) return key;
  }
  return null;
}

const cleaning = ref<string | null>(null);
const cleaned = ref<Set<string>>(new Set());
const cleanResults = ref<Record<string, string>>({});

async function cleanFolder(folder: FolderEntry) {
  const key = repairKeyFor(folder);
  if (!key) return;
  cleaning.value = folder.label;
  try {
    const r = await invoke<RepairResult>("run_repair_command", { repairType: key });
    cleaned.value = new Set([...cleaned.value, folder.label]);
    cleanResults.value[folder.label] = r?.success ? "OK" : (r?.output || "Terminé");
  } catch (e: unknown) {
    cleanResults.value[folder.label] = "Erreur: " + String(e);
  } finally {
    cleaning.value = null;
  }
}

async function cleanAllTemp() {
  const toClean = sorted.value.filter(f => repairKeyFor(f));
  for (const f of toClean) {
    if (!cleaned.value.has(f.label)) await cleanFolder(f);
  }
}

const maxSize = computed(() => Math.max(...props.folders.map(f => f.size_mb), 1));

function sizeStr(mb: number): string {
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
  return `${mb.toFixed(0)} MB`;
}

const sorted = computed(() => [...props.folders].sort((a, b) => b.size_mb - a.size_mb));
const totalMb = computed(() => props.folders.reduce((a, f) => a + f.size_mb, 0));
const totalFiles = computed(() => props.folders.reduce((a, f) => a + f.file_count, 0));
const cleanableMb = computed(() => sorted.value.filter(f => repairKeyFor(f)).reduce((a, f) => a + f.size_mb, 0));
const cleanableCount = computed(() => sorted.value.filter(f => repairKeyFor(f)).length);
const allCleaned = ref(false);
const bulkCleaning = ref(false);

async function cleanAll() {
  bulkCleaning.value = true;
  await cleanAllTemp();
  bulkCleaning.value = false;
  allCleaned.value = true;
}
</script>

<template>
  <div class="diag-tab-content">
    <DiagBanner :icon="FolderOpen" title="Dossiers Système" desc="Taille et occupation des dossiers Windows principaux" color="amber" />

    <div v-if="!folders.length" class="diag-empty">Calcul des tailles en cours...</div>
    <template v-else>
      <!-- Résumé stats -->
      <div style="display:flex;gap:12px;flex-wrap:wrap;margin-bottom:16px">
        <div class="card-block" style="flex:1;min-width:160px;margin-bottom:0">
          <p class="diag-section-label" style="margin:0 0 4px 0">Taille totale analysée</p>
          <span style="font-size:24px;font-weight:700;color:var(--accent)">{{ sizeStr(totalMb) }}</span>
        </div>
        <div class="card-block" style="flex:1;min-width:160px;margin-bottom:0">
          <p class="diag-section-label" style="margin:0 0 4px 0">Fichiers analysés</p>
          <span style="font-size:24px;font-weight:700">{{ totalFiles.toLocaleString() }}</span>
        </div>
        <div v-if="cleanableCount > 0" class="card-block" style="flex:1;min-width:160px;margin-bottom:0;border-color:var(--warning)">
          <p class="diag-section-label" style="margin:0 0 4px 0">Nettoyables ({{ cleanableCount }} dossiers)</p>
          <div style="display:flex;align-items:center;gap:10px">
            <span style="font-size:24px;font-weight:700;color:var(--warning)">{{ sizeStr(cleanableMb) }}</span>
            <NButton v-if="!allCleaned" variant="warning" size="sm" :disabled="bulkCleaning || cleaning !== null" @click="cleanAll">
              <NSpinner v-if="bulkCleaning" :size="12" />
              <Trash2 v-else :size="13" />
              {{ bulkCleaning ? 'Nettoyage...' : 'Tout nettoyer' }}
            </NButton>
            <span v-else style="color:var(--success);font-size:12px;font-weight:600"><CheckCircle :size="13" style="display:inline;margin-right:4px" />Nettoyé</span>
          </div>
        </div>
      </div>

      <p class="diag-section-label">Tailles par dossier</p>
      <div v-for="(f, i) in sorted" :key="i" class="card-block">
        <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:6px">
          <div>
            <span style="font-weight:600">{{ f.label }}</span>
            <span class="muted" style="font-size:11px;margin-left:8px">{{ f.path }}</span>
          </div>
          <div style="display:flex;gap:8px;align-items:center">
            <NBadge :variant="f.size_mb > 10240 ? 'danger' : f.size_mb > 1024 ? 'warning' : 'default'"
              :title="f.partial ? 'Mesure partielle (dossier trop gros ou sous-dossiers protégés) : taille minimale' : undefined">
              {{ f.partial ? '≥ ' : '' }}{{ sizeStr(f.size_mb) }}
            </NBadge>
            <span class="muted" style="font-size:11px">{{ f.file_count.toLocaleString() }} fichiers</span>

            <!-- Résultat de nettoyage -->
            <span v-if="cleaned.has(f.label)" style="color:var(--success);font-size:11px;font-weight:600">
              <CheckCircle :size="12" style="display:inline;margin-right:3px" />Nettoyé
            </span>

            <!-- Bouton nettoyage (si applicable) -->
            <NButton v-else-if="repairKeyFor(f)" variant="ghost" size="sm"
              :disabled="cleaning !== null"
              @click="cleanFolder(f)"
              style="padding:2px 8px;height:22px;font-size:11px;border-color:var(--warning);color:var(--warning)">
              <NSpinner v-if="cleaning === f.label" :size="10" />
              <Trash2 v-else :size="11" />
              {{ cleaning === f.label ? '...' : 'Vider' }}
            </NButton>

            <NButton variant="ghost" size="sm" @click="openFolder(f.path)" style="padding:2px 6px;height:22px;font-size:11px">
              <FolderOpen :size="11" /> Ouvrir
            </NButton>
          </div>
        </div>
        <NProgress
          :value="(f.size_mb / maxSize) * 100"
          :variant="f.size_mb > 10240 ? 'danger' : f.size_mb > 1024 ? 'warning' : 'default'"
          size="sm"
        />
        <!-- Résultat détaillé si erreur -->
        <div v-if="cleanResults[f.label] && cleanResults[f.label].startsWith('Erreur')"
          style="font-size:10px;color:var(--error);margin-top:4px">{{ cleanResults[f.label] }}</div>
      </div>
    </template>
  </div>
</template>
