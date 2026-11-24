<script setup lang="ts">
// Port de TokaISO 7.9 (MIT, Cédrick Roy) — conseiller d'image (objectif, architecture,
// langue, démarrage) et suivis de réinstallation (checklists post-installation).
import { onMounted, ref } from "vue";
import NCard from "@/components/ui/NCard.vue";
import NButton from "@/components/ui/NButton.vue";
import NBadge from "@/components/ui/NBadge.vue";
import { Compass, ListChecks } from "lucide-vue-next";
import { useNotificationStore } from "@/stores/notifications";
import { api, errorText, openPortal, useIsoCatalog, KIND_LABELS, KIND_VARIANTS, type SourceKind } from "./useIsoCatalog";

const notifications = useNotificationStore();
const { items, queueDownload } = useIsoCatalog();

interface Advice { id: string; name: string; source: number; label: string; boot: string; arch: string; format: string; lang: string; portal: boolean; kind: SourceKind }
const form = ref({ goal: "windows", arch: "all", language: "all", firmware: "all" });
const advice = ref<Advice[]>([]);
const adviceCount = ref<number | null>(null);

async function ask() {
  try {
    const r = await api<{ results: Advice[]; count: number }>("advisor", { ...form.value });
    advice.value = r.results;
    adviceCount.value = r.count;
  } catch (e) {
    notifications.error("Conseiller", errorText(e));
  }
}

async function take(row: Advice) {
  const item = items.value.find(i => i.id === row.id);
  if (!item) return;
  try {
    if (row.portal) await openPortal(item.sources[row.source].url);
    else if (await queueDownload(item, row.source)) notifications.success("Ajouté à la file", item.name);
  } catch (e) {
    notifications.error("Conseiller", errorText(e));
  }
}

interface Checklist { id: string; name: string; steps: { label: string; done: boolean }[] }
const lists = ref<Checklist[]>([]);
const newName = ref("");

async function checklist(action: string, payload: Record<string, unknown> = {}) {
  try { lists.value = (await api<{ lists: Checklist[] }>(`checklist-${action}`, payload)).lists; }
  catch (e) { notifications.error("Suivi", errorText(e)); }
}
onMounted(() => checklist("list"));

async function importList() {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const path = await open({ title: "Importer un suivi", filters: [{ name: "JSON", extensions: ["json"] }] });
  if (typeof path === "string") await checklist("import", { path });
}

async function exportList(list: Checklist) {
  const { save } = await import("@tauri-apps/plugin-dialog");
  const path = await save({ title: "Exporter le suivi", defaultPath: `${list.name}.json`, filters: [{ name: "JSON", extensions: ["json"] }] });
  if (!path) return;
  try { await api("checklist-export", { id: list.id, path }); notifications.success("Suivi exporté", path); }
  catch (e) { notifications.error("Suivi", errorText(e)); }
}
</script>

<template>
  <div class="iso-stack">
    <NCard>
      <div class="iso-stack">
        <span class="iso-section-title"><Compass :size="16" /> Conseiller d’image</span>
        <div class="iso-row">
          <select v-model="form.goal" class="iso-field">
            <option value="windows">Installer Windows</option><option value="linux">Installer Linux</option>
            <option value="rescue">Dépanner un PC</option><option value="server">Monter un serveur</option>
          </select>
          <select v-model="form.arch" class="iso-field">
            <option value="all">Toutes architectures</option><option value="x64">x64</option><option value="x86">x86</option><option value="ARM64">ARM64</option>
          </select>
          <select v-model="form.language" class="iso-field">
            <option value="all">Toutes langues</option><option value="fr">Français</option><option value="en">Anglais</option>
          </select>
          <select v-model="form.firmware" class="iso-field">
            <option value="all">BIOS ou UEFI</option><option value="UEFI">UEFI</option><option value="BIOS">BIOS</option>
          </select>
          <NButton size="sm" @click="ask">Proposer</NButton>
        </div>
        <p v-if="adviceCount !== null" class="iso-muted">{{ adviceCount }} image(s) compatible(s){{ adviceCount > 100 ? " · 100 premières affichées" : "" }}</p>
        <div class="iso-list">
          <div v-for="row in advice" :key="`${row.id}-${row.source}`" class="iso-line">
            <div class="iso-line-main">
              <strong>{{ row.name }}</strong>
              <span class="iso-muted">{{ [row.label, row.format, row.arch, row.boot, row.lang === "fr" ? "Français" : row.lang === "en" ? "Anglais" : ""].filter(Boolean).join(" · ") }}</span>
            </div>
            <NBadge :variant="KIND_VARIANTS[row.kind]">{{ KIND_LABELS[row.kind] }}</NBadge>
            <NButton size="sm" variant="secondary" @click="take(row)">{{ row.portal ? "Ouvrir le portail" : "Télécharger" }}</NButton>
          </div>
        </div>
      </div>
    </NCard>

    <NCard>
      <div class="iso-stack">
        <span class="iso-section-title"><ListChecks :size="16" /> Suivis de réinstallation</span>
        <div class="iso-row">
          <input v-model="newName" class="iso-field iso-field--grow" maxlength="90" placeholder="Nom du PC ou de l’intervention" />
          <NButton size="sm" :disabled="!newName.trim()" @click="checklist('create', { name: newName }).then(() => (newName = ''))">Créer</NButton>
          <NButton size="sm" variant="secondary" @click="importList">Importer</NButton>
        </div>
        <div v-for="list in lists" :key="list.id" class="iso-source">
          <div class="iso-row">
            <strong>{{ list.name }}</strong>
            <span class="iso-muted">{{ list.steps.filter(s => s.done).length }} / {{ list.steps.length }}</span>
            <NButton size="sm" variant="ghost" @click="exportList(list)">Exporter</NButton>
            <NButton size="sm" variant="ghost" @click="checklist('delete', { id: list.id })">Supprimer</NButton>
          </div>
          <label v-for="(step, index) in list.steps" :key="index" class="iso-row iso-small">
            <input type="checkbox" :checked="step.done" @change="checklist('toggle', { id: list.id, index, done: ($event.target as HTMLInputElement).checked })" />
            {{ step.label }}
          </label>
        </div>
      </div>
    </NCard>
  </div>
</template>
