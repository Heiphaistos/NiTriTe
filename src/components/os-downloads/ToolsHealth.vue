<script setup lang="ts">
// Port de TokaISO 7.9 (MIT, Cédrick Roy) — santé des liens du catalogue (contrôle
// par échantillon de 64 Ko, 6 en parallèle) et tableau de bord par état.
import { onBeforeUnmount, onMounted, ref } from "vue";
import NCard from "@/components/ui/NCard.vue";
import NButton from "@/components/ui/NButton.vue";
import { Activity } from "lucide-vue-next";
import { api, errorText, familyLabel } from "./useIsoCatalog";

interface Progress { running: boolean; done: number; total: number; family: string; cancelled: boolean }
interface Row { item: string; name: string; source: number; label: string; filename: string; family: string; check: { status: string; message: string; checked?: string } | null }
interface Dashboard { progress: Progress; categories: string[]; page: number; pages: number; count: number; filtered: number; totals: Record<string, number>; rows: Row[] }

const STATUS: Record<string, string> = {
  unverified: "Non contrôlé", file: "Fichier accessible", portal: "Portail accessible", expired: "Lien expiré",
  blocked: "Accès bloqué", changed: "Taille modifiée", page: "Page au lieu du fichier", error: "Erreur",
};

const family = ref("");
const status = ref("");
const page = ref(0);
const data = ref<Dashboard | null>(null);
const error = ref("");
let timer: ReturnType<typeof setTimeout> | undefined;

async function load() {
  clearTimeout(timer);
  try {
    data.value = await api<Dashboard>("source-dashboard", { family: family.value, status: status.value, page: page.value });
    error.value = "";
    if (data.value.progress.running) timer = setTimeout(load, 1500);
  } catch (e) {
    error.value = errorText(e);
  }
}
onMounted(load);
onBeforeUnmount(() => clearTimeout(timer));

async function scan(start: boolean) {
  try {
    await api(start ? "source-scan-start" : "source-scan-stop", { family: family.value });
    await load();
  } catch (e) {
    error.value = errorText(e);
  }
}

function filter() {
  page.value = 0;
  load();
}
</script>

<template>
  <NCard>
    <div class="iso-stack">
      <span class="iso-section-title"><Activity :size="16" /> Santé des sources</span>
      <div class="iso-row">
        <select v-model="family" class="iso-field" @change="filter">
          <option value="">Tout le catalogue</option>
          <option v-for="c in data?.categories ?? []" :key="c" :value="c">{{ familyLabel(c) }}</option>
        </select>
        <select v-model="status" class="iso-field" @change="filter">
          <option value="">Tous les états</option>
          <option v-for="(label, key) in STATUS" :key="key" :value="key">{{ label }} ({{ data?.totals[key] ?? 0 }})</option>
        </select>
        <NButton v-if="!data?.progress.running" size="sm" @click="scan(true)">Contrôler les liens</NButton>
        <NButton v-else size="sm" variant="secondary" @click="scan(false)">Arrêter ({{ data.progress.done }} / {{ data.progress.total }})</NButton>
      </div>
      <p v-if="error" class="iso-error">{{ error }}</p>
      <p v-if="data?.progress.cancelled" class="iso-muted">Contrôle interrompu avant la fin.</p>
      <table v-if="data?.rows.length" class="iso-table">
        <thead><tr><th>Fiche</th><th>Source</th><th>État</th></tr></thead>
        <tbody>
          <tr v-for="row in data.rows" :key="`${row.item}-${row.source}`">
            <td>{{ row.name }}</td>
            <td>{{ row.label }}<br /><span class="iso-muted">{{ row.filename }}</span></td>
            <td>
              <span :class="row.check?.status === 'file' || row.check?.status === 'portal' ? 'iso-ok' : row.check ? 'iso-warn' : 'iso-muted'">
                {{ STATUS[row.check?.status ?? "unverified"] }}
              </span>
              <br /><span class="iso-muted">{{ row.check?.message }}{{ row.check?.checked ? ` · ${new Date(row.check.checked).toLocaleString("fr-FR")}` : "" }}</span>
            </td>
          </tr>
        </tbody>
      </table>
      <div v-if="data && data.pages > 1" class="iso-row">
        <NButton size="sm" variant="ghost" :disabled="page === 0" @click="page--; load()">Précédent</NButton>
        <span class="iso-muted">Page {{ data.page + 1 }} / {{ data.pages }} · {{ data.filtered }} source(s)</span>
        <NButton size="sm" variant="ghost" :disabled="page >= data.pages - 1" @click="page++; load()">Suivant</NButton>
      </div>
    </div>
  </NCard>
</template>
