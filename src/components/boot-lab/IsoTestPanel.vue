<script setup lang="ts">
// Port de TokaISO 7.9 (MIT, Cédrick Roy) : essai isolé d'une ISO en VM (studio.js iso, vm75.js historique).
import { computed, onMounted, ref } from "vue";
import NCard from "@/components/ui/NCard.vue";
import NButton from "@/components/ui/NButton.vue";
import { FlaskConical, History, RefreshCw } from "lucide-vue-next";
import TestVmCard from "./TestVmCard.vue";
import { call, dateText, fileName, pickFiles, platformNames, useVmList, type InstallSource, type Machine } from "./useBootLab";

interface HistoryRow {
  id: string; name: string; platform: string; file: string; iso?: string; date?: string; boot: string[];
  firmware?: string; state?: string; powered_at?: string; menu_at?: string; removed_at?: string;
}

const emit = defineEmits<{ install: [source: InstallSource] }>();
const { vmList, refresh, replace, remove } = useVmList();
const iso = ref("");
const platform = ref("virtualbox");
const firmware = ref("efi");
const network = ref(false);
const busy = ref(false);
const current = ref<Machine | null>(null);
const history = ref<HistoryRow[]>([]);
const bootFilter = ref("all");
const platformFilter = ref("all");

const rows = computed(() => history.value.filter((row) =>
  (bootFilter.value === "all" || bootFilter.value === (row.firmware || "unknown"))
  && (platformFilter.value === "all" || platformFilter.value === row.platform)));

async function pickIso() {
  const [path] = await pickFiles("Image ISO", ["iso"]);
  if (path) iso.value = path;
}

async function loadHistory() {
  history.value = (await call<HistoryRow[]>("boot_lab_vm_test_history")) ?? [];
}

async function create() {
  busy.value = true;
  const machine = await call<Machine>("boot_lab_iso_test",
    { path: iso.value, platform: platform.value, firmware: firmware.value, network: network.value }, 120000);
  busy.value = false;
  if (!machine) return;
  current.value = machine;
  replace(machine);
  loadHistory();
}

function updated(machine: Machine) {
  current.value = machine;
  replace(machine);
  loadHistory();
}

function removed(id: string) {
  current.value = null;
  remove(id);
  loadHistory();
}

async function reopen(row: HistoryRow) {
  if (!vmList.value) await refresh();
  current.value = vmList.value?.machines.find((m) => m.id === row.id) ?? null;
  if (row.iso) iso.value = row.iso;
}

function state(row: HistoryRow): string {
  if (row.menu_at) return "Menu confirmé dans la console";
  return row.powered_at || ["running", "guest"].includes(row.state ?? "") ? "Mise sous tension observée" : "Démarrage à confirmer";
}

function detail(row: HistoryRow): string {
  const boot = row.firmware ? (row.firmware === "efi" ? "UEFI" : "BIOS") : "Mode non enregistré";
  return [platformNames[row.platform], boot, state(row), dateText(row.menu_at || row.date || row.removed_at), row.removed_at && "Test retiré"]
    .filter(Boolean).join(" · ");
}

onMounted(loadHistory);
</script>

<template>
  <div class="bl-panel">
    <NCard>
      <div class="bl-panel">
        <div class="bl-title"><FlaskConical :size="16" /> Créer la VM d’essai</div>
        <p class="bl-text">
          La VM d’essai (préfixe NiTriTe-Test-) démarre uniquement l’ISO, sans disque, avec 2 Gio de mémoire et 2 processeurs.
          Sans réseau par défaut ; coche la case pour un réseau NAT.
        </p>
        <div class="bl-fields">
          <NButton variant="secondary" @click="pickIso">Choisir une ISO</NButton>
          <label>Hyperviseur
            <select v-model="platform"><option value="virtualbox">VirtualBox</option><option value="vmware">VMware Workstation</option></select>
          </label>
          <label>Démarrage <select v-model="firmware"><option value="efi">UEFI</option><option value="bios">BIOS</option></select></label>
          <label class="bl-check"><input v-model="network" type="checkbox" /> Réseau NAT</label>
          <NButton :disabled="!iso" :loading="busy" @click="create">Créer la VM d’essai</NButton>
        </div>
        <p class="bl-muted">{{ iso ? fileName(iso) : "Aucune ISO choisie." }}</p>
        <TestVmCard v-if="current" :machine="current" @update="updated" @removed="removed" @install="(m) => emit('install', m)" />
      </div>
    </NCard>

    <NCard>
      <div class="bl-panel">
        <div class="bl-title"><History :size="16" /> Historique des essais ISO</div>
        <div class="bl-fields">
          <label>Démarrage
            <select v-model="bootFilter">
              <option value="all">Tous les modes</option><option value="efi">UEFI</option><option value="bios">BIOS</option>
              <option value="unknown">Mode non enregistré</option>
            </select>
          </label>
          <label>Hyperviseur
            <select v-model="platformFilter">
              <option value="all">Tous les hyperviseurs</option><option value="virtualbox">VirtualBox</option><option value="vmware">VMware Workstation</option>
            </select>
          </label>
          <NButton variant="secondary" @click="loadHistory"><RefreshCw :size="14" /> Actualiser</NButton>
        </div>
        <p v-if="!rows.length" class="bl-muted">Aucun essai ISO dans cette sélection.</p>
        <div class="bl-list">
          <div v-for="row in rows" :key="row.id" class="bl-row">
            <div class="bl-row-main"><span class="bl-row-name">{{ row.file }}</span><span class="bl-muted">{{ detail(row) }}</span></div>
            <div class="bl-actions">
              <NButton v-if="!row.removed_at" size="sm" variant="secondary" @click="reopen(row)">Rouvrir l’essai</NButton>
              <NButton v-if="row.iso" size="sm" variant="secondary"
                @click="emit('install', { iso: row.iso, platform: row.platform, firmware: row.firmware ?? '' })">
                Créer une VM avec cette ISO
              </NButton>
            </div>
          </div>
        </div>
      </div>
    </NCard>
  </div>
</template>
