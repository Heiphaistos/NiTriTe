<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { invoke } from "@/utils/invoke";
import NCard from "@/components/ui/NCard.vue";
import NButton from "@/components/ui/NButton.vue";
import NBadge from "@/components/ui/NBadge.vue";
import NSpinner from "@/components/ui/NSpinner.vue";
import NSkeleton from "@/components/ui/NSkeleton.vue";
import { useNotificationStore } from "@/stores/notifications";
import {
  Globe, Plus, Trash2, Eye, EyeOff, Save,
  RefreshCw, AlertTriangle,
} from "lucide-vue-next";

const notify = useNotificationStore();

interface HostsEntry {
  ip: string; hostname: string; comment: string; active: boolean; line_number: number;
}

const entries = ref<HostsEntry[]>([]);
const loading = ref(true);
const search = ref("");
const newIp = ref("");
const newHostname = ref("");
const newComment = ref("");
const adding = ref(false);

const filtered = computed(() => {
  const q = search.value.toLowerCase();
  if (!q) return entries.value;
  return entries.value.filter(e =>
    e.ip.includes(q) || e.hostname.toLowerCase().includes(q) || e.comment.toLowerCase().includes(q)
  );
});

async function loadEntries() {
  loading.value = true;
  try {
    entries.value = await invoke<HostsEntry[]>("get_hosts_entries");
  } catch (e: unknown) {
    notify.error("Lecture hosts échouée", String(e));
  }
  loading.value = false;
}

function isValidIp(ip: string): boolean {
  const v4 = /^(\d{1,3}\.){3}\d{1,3}$/;
  const v6 = /^([\da-fA-F]{0,4}:){2,7}[\da-fA-F]{0,4}$|^::1?$|^::$/;
  if (v4.test(ip)) return ip.split(".").every(n => parseInt(n) <= 255);
  return v6.test(ip);
}

async function addEntry() {
  if (!newIp.value.trim() || !newHostname.value.trim()) {
    notify.warning("Champs requis", "IP et hostname sont obligatoires.");
    return;
  }
  if (!isValidIp(newIp.value.trim())) {
    notify.warning("IP invalide", "Entrez une adresse IPv4 (ex: 127.0.0.1) ou IPv6 valide.");
    return;
  }
  // Sauvegarde hosts.bak faite côté Rust avant chaque écriture (refus si impossible)
  adding.value = true;
  try {
    const msg = await invoke<string>("add_hosts_entry", {
      ip: newIp.value.trim(),
      hostname: newHostname.value.trim(),
      comment: newComment.value.trim(),
    });
    notify.success("Entrée ajoutée", msg);
    newIp.value = ""; newHostname.value = ""; newComment.value = "";
    await loadEntries();
  } catch (e: unknown) {
    notify.error("Erreur", String(e));
  }
  adding.value = false;
}

async function deleteEntry(line: number, hostname: string) {
  try {
    await invoke<string>("delete_hosts_entry", { lineNumber: line, expectedHostname: hostname });
    notify.success("Supprimé", hostname);
    await loadEntries();
  } catch (e: unknown) {
    notify.error("Erreur suppression", String(e));
  }
}

async function toggleEntry(line: number, enable: boolean, hostname: string) {
  try {
    await invoke<string>("toggle_hosts_entry", { lineNumber: line, enable, expectedHostname: hostname });
    notify.success(enable ? "Activé" : "Désactivé");
    await loadEntries();
  } catch (e: unknown) {
    notify.error("Erreur", String(e));
  }
}

async function backupHosts() {
  try {
    const msg = await invoke<string>("backup_hosts");
    notify.success("Sauvegarde créée", msg);
  } catch (e: unknown) {
    notify.error("Erreur sauvegarde", String(e));
  }
}

onMounted(loadEntries);
</script>

<template>
  <div class="hosts-page">
    <div class="page-header">
      <div>
        <h1>Éditeur Hosts</h1>
        <p class="subtitle">Gérez le fichier <code>C:\Windows\System32\drivers\etc\hosts</code></p>
      </div>
      <div class="header-actions">
        <NButton variant="ghost" size="sm" @click="backupHosts"><Save :size="13" /> Sauvegarder</NButton>
        <NButton variant="ghost" size="sm" :loading="loading" @click="loadEntries"><RefreshCw :size="13" /> Actualiser</NButton>
      </div>
    </div>

    <div class="info-banner">
      <AlertTriangle :size="14" />
      <span>Modifications nécessitent les droits administrateur. Faites une sauvegarde avant toute modification.</span>
    </div>

    <!-- Ajouter une entrée -->
    <NCard>
      <template #header>
        <div class="section-header"><Plus :size="15" /><span>Ajouter une entrée</span></div>
      </template>
      <div class="add-row">
        <input v-model="newIp" class="host-input" placeholder="IP (ex: 127.0.0.1)" />
        <input v-model="newHostname" class="host-input flex-1" placeholder="Hostname (ex: monsite.local)" />
        <input v-model="newComment" class="host-input flex-1" placeholder="Commentaire (optionnel)" />
        <NButton variant="primary" size="sm" :loading="adding" @click="addEntry">
          <Plus :size="13" /> Ajouter
        </NButton>
      </div>
    </NCard>

    <!-- Liste -->
    <NCard>
      <template #header>
        <div class="section-header">
          <Globe :size="15" />
          <span>Entrées</span>
          <NBadge variant="neutral" style="margin-left:6px">{{ entries.length }}</NBadge>
          <input v-model="search" class="host-search" placeholder="Filtrer..." style="margin-left:auto" />
        </div>
      </template>

      <div v-if="loading" class="loading-state">
        <NSkeleton v-for="i in 5" :key="i" height="40px" style="margin-bottom:4px" />
      </div>

      <div v-else-if="filtered.length === 0" class="empty-state">
        <Globe :size="28" style="opacity:.3" />
        <p>Aucune entrée trouvée</p>
      </div>

      <div v-else class="entries-table">
        <div class="table-head">
          <span>IP</span><span>Hostname</span><span>Commentaire</span><span>État</span><span></span>
        </div>
        <div
          v-for="e in filtered" :key="e.line_number"
          class="entry-row"
          :class="{ disabled: !e.active }"
        >
          <code class="cell-ip">{{ e.ip }}</code>
          <span class="cell-hostname">{{ e.hostname }}</span>
          <span class="cell-comment">{{ e.comment || '—' }}</span>
          <NBadge :variant="e.active ? 'success' : 'neutral'" size="sm">
            {{ e.active ? 'Actif' : 'Inactif' }}
          </NBadge>
          <div class="entry-actions">
            <button class="icon-btn" @click="toggleEntry(e.line_number, !e.active, e.hostname)" :title="e.active ? 'Désactiver' : 'Activer'">
              <EyeOff v-if="e.active" :size="13" />
              <Eye v-else :size="13" />
            </button>
            <button class="icon-btn danger" @click="deleteEntry(e.line_number, e.hostname)" title="Supprimer">
              <Trash2 :size="13" />
            </button>
          </div>
        </div>
      </div>
    </NCard>
  </div>
</template>

<style scoped>
.hosts-page { display: flex; flex-direction: column; gap: 14px; }
.page-header { display: flex; align-items: flex-start; justify-content: space-between; }
.page-header h1 { font-size: 22px; font-weight: 700; }
.subtitle { font-size: 12px; color: var(--text-muted); margin-top: 3px; }
.header-actions { display: flex; gap: 8px; }
.info-banner {
  display: flex; gap: 10px; align-items: flex-start; padding: 10px 14px;
  background: var(--warning-muted); border: 1px solid color-mix(in srgb, var(--warning) 30%, transparent);
  border-radius: var(--radius-md); font-size: 12px; color: var(--warning);
}
.section-header { display: flex; align-items: center; gap: 8px; width: 100%; }
.add-row { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
.host-input {
  padding: 7px 10px; border: 1px solid var(--border); border-radius: var(--radius-md);
  background: var(--bg-tertiary); color: var(--text-primary); font-family: "JetBrains Mono", monospace;
  font-size: 12px; outline: none; min-width: 130px;
}
.host-input:focus { border-color: var(--accent-primary); }
.host-input.flex-1 { flex: 1; }
.host-search {
  padding: 5px 10px; border: 1px solid var(--border); border-radius: var(--radius-md);
  background: var(--bg-secondary); color: var(--text-primary); font-size: 12px;
  outline: none; width: 180px;
}
.host-search:focus { border-color: var(--accent-primary); }
.table-head {
  display: grid; grid-template-columns: 130px 1fr 1fr 80px 70px;
  padding: 6px 10px; font-size: 10px; font-weight: 700; text-transform: uppercase;
  letter-spacing: .06em; color: var(--text-muted); border-bottom: 1px solid var(--border);
}
.entry-row {
  display: grid; grid-template-columns: 130px 1fr 1fr 80px 70px;
  align-items: center; padding: 8px 10px; border-bottom: 1px solid var(--border);
  transition: background var(--transition-fast); font-size: 12px;
}
.entry-row:hover { background: var(--bg-tertiary); }
.entry-row.disabled { opacity: .5; }
.entry-row:last-child { border-bottom: none; }
.cell-ip { font-family: "JetBrains Mono", monospace; color: var(--accent-primary); font-size: 12px; }
.cell-hostname { color: var(--text-primary); }
.cell-comment { color: var(--text-muted); font-size: 11px; }
.entry-actions { display: flex; gap: 4px; }
.icon-btn {
  display: flex; align-items: center; justify-content: center;
  width: 26px; height: 26px; border: none; border-radius: var(--radius-sm);
  background: var(--bg-tertiary); color: var(--text-secondary); cursor: pointer;
  transition: all var(--transition-fast);
}
.icon-btn:hover { background: var(--bg-elevated); color: var(--text-primary); }
.icon-btn.danger:hover { background: var(--danger-muted); color: var(--danger); }
.loading-state { display: flex; flex-direction: column; gap: 4px; }
.empty-state { display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 40px; color: var(--text-muted); }
code { font-family: "JetBrains Mono", monospace; }
</style>
