<script setup lang="ts">
// Port de TokaISO 7.9 (MIT, Cédrick Roy) : démarrage réseau PXE (deployment.js, studio.js, index.html).
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import NCard from "@/components/ui/NCard.vue";
import NButton from "@/components/ui/NButton.vue";
import NBadge from "@/components/ui/NBadge.vue";
import { Network, FolderOutput, Play, Square, Stethoscope, RefreshCw, Plus, Trash2 } from "lucide-vue-next";
import { useNotificationStore } from "@/stores/notifications";
import TestVmCard from "./TestVmCard.vue";
import { call, dateText, fileName, openFolder, pickFiles, pickFolder, type InstallSource, type Machine } from "./useBootLab";

interface Entry { index: number; label: string; file: string; sha256: string }
interface Project { id: string; folder: string; entries: Entry[]; wimboot: boolean }
interface LogRow { at: string; client: string; method: string; path: string; status: number; bytes: number }
interface Status { projects: Project[]; url: string; running: boolean; command: string; log: LogRow[] }
interface Iface { name: string; address: string }
interface Diagnose { running: boolean; http: boolean; external: boolean; client: { address: string; at: string } | null; command: string; port: number }

const emit = defineEmits<{ install: [source: InstallSource] }>();
const notify = useNotificationStore();
const files = ref<string[]>([]);
const san = ref(false);
const status = ref<Status>({ projects: [], url: "", running: false, command: "", log: [] });
const interfaces = ref<Iface[]>([]);
const project = ref("");
const bind = ref("127.0.0.1");
const diagnostic = ref<Diagnose | null>(null);
const busy = ref("");
const testPlatform = ref("virtualbox");
const testFirmware = ref("bios");
const testVm = ref<Machine | null>(null);
let timer: ReturnType<typeof setInterval> | undefined;

const selected = computed(() => status.value.projects.find((p) => p.id === project.value));

async function refreshStatus() {
  const data = await call<Status>("boot_lab_pxe_status");
  if (!data) return;
  status.value = data;
  if (!data.projects.some((p) => p.id === project.value)) project.value = data.projects[data.projects.length - 1]?.id ?? "";
}

async function loadInterfaces() {
  const rows = await call<Iface[]>("boot_lab_pxe_interfaces", undefined, 15000);
  if (rows) interfaces.value = rows;
}

function pickInterface(event: Event) {
  const value = (event.target as HTMLSelectElement).value;
  if (value) bind.value = value;
}

async function addFiles() {
  const picked = await pickFiles("Images ISO ou WIM", ["iso", "wim"], true);
  files.value = [...new Set([...files.value, ...picked])].slice(0, 24);
}

async function exportProject() {
  if (!files.value.length) return;
  const destination = await pickFolder();
  if (!destination) return;
  busy.value = "export";
  const result = await call<Project>("boot_lab_pxe_export", { request: { paths: files.value, destination, san: san.value } }, 1800000);
  busy.value = "";
  if (!result) return;
  notify.success("Dossier PXE exporté", result.folder);
  await refreshStatus();
  project.value = result.id;
}

async function start() {
  busy.value = "start";
  const data = await call<Status>("boot_lab_pxe_start", { id: project.value, bind: bind.value.trim() });
  busy.value = "";
  if (data) status.value = data;
}

async function stop() {
  busy.value = "stop";
  const data = await call<Status>("boot_lab_pxe_stop");
  busy.value = "";
  if (data) status.value = data;
  diagnostic.value = null;
}

async function diagnose() {
  busy.value = "diagnose";
  diagnostic.value = await call<Diagnose>("boot_lab_pxe_diagnose", undefined, 15000);
  busy.value = "";
}

async function testVmCreate() {
  busy.value = "test";
  testVm.value = await call<Machine>("boot_lab_pxe_test", { platform: testPlatform.value, firmware: testFirmware.value }, 180000);
  busy.value = "";
}

async function copyCommand() {
  try {
    await navigator.clipboard.writeText(status.value.command);
    notify.success("Commande copiée");
  } catch (error) {
    notify.warning("Copie impossible", String(error));
  }
}

// Journal des requêtes : actualisé tant que le serveur tourne et que l'onglet est ouvert.
watch(() => status.value.running, (running) => {
  clearInterval(timer);
  if (running) timer = setInterval(refreshStatus, 2000);
});
onMounted(() => { refreshStatus(); loadInterfaces(); });
onBeforeUnmount(() => clearInterval(timer));
</script>

<template>
  <div class="bl-panel">
    <NCard>
      <div class="bl-panel">
        <div class="bl-title"><Network :size="16" /> Exporter un dossier PXE</div>
        <p class="bl-text">
          Menus iPXE autonomes : chaque ISO est démarrée en SAN par HTTP, chaque WIM WinPE avec wimboot (téléchargé depuis
          le dépôt officiel et contrôlé par empreinte SHA-256).
        </p>
        <div class="bl-fields">
          <NButton variant="secondary" @click="addFiles"><Plus :size="14" /> Ajouter des images ISO ou WIM</NButton>
          <label class="bl-check"><input v-model="san" type="checkbox" /> Mes ISO acceptent le démarrage iPXE par HTTP</label>
          <NButton :disabled="!files.length" :loading="busy === 'export'" @click="exportProject">
            <FolderOutput :size="14" /> Exporter le dossier PXE
          </NButton>
        </div>
        <div v-if="files.length" class="bl-list">
          <div v-for="(path, i) in files" :key="path" class="bl-row">
            <div class="bl-row-main"><span class="bl-row-name">{{ String(i + 1).padStart(2, "0") }} · {{ fileName(path) }}</span><span class="bl-muted">{{ path }}</span></div>
            <NButton size="sm" variant="ghost" @click="files = files.filter((f) => f !== path)"><Trash2 :size="14" /></NButton>
          </div>
        </div>
      </div>
    </NCard>

    <NCard>
      <div class="bl-panel">
        <div class="bl-title">
          <Play :size="16" /> Servir le menu sur le réseau
          <NBadge :variant="status.running ? 'success' : 'neutral'" dot>{{ status.running ? "Serveur actif" : "Serveur arrêté" }}</NBadge>
        </div>
        <p class="bl-text">
          Le serveur écoute uniquement sur l’adresse IPv4 privée choisie, en lecture seule, et ne sert que les fichiers du projet.
          Il s’arrête avec le bouton ou à la fermeture de NiTriTe.
        </p>
        <div class="bl-fields">
          <label>Carte réseau
            <select :value="''" @change="pickInterface">
              <option value="">Choisir une interface</option>
              <option v-for="row in interfaces" :key="row.address + row.name" :value="row.address">{{ row.name }} · {{ row.address }}</option>
            </select>
          </label>
          <NButton variant="secondary" @click="loadInterfaces"><RefreshCw :size="14" /> Actualiser les interfaces</NButton>
        </div>
        <div class="bl-fields">
          <label>Projet exporté
            <select v-model="project">
              <option v-for="p in status.projects" :key="p.id" :value="p.id">{{ fileName(p.folder) }}</option>
            </select>
          </label>
          <label>Adresse IPv4 de ce PC <input v-model="bind" maxlength="15" /></label>
          <NButton :disabled="!project || status.running" :loading="busy === 'start'" @click="start"><Play :size="14" /> Démarrer le serveur</NButton>
          <NButton variant="danger" :disabled="!status.running" :loading="busy === 'stop'" @click="stop"><Square :size="14" /> Arrêter le serveur</NButton>
          <NButton variant="secondary" :loading="busy === 'diagnose'" @click="diagnose"><Stethoscope :size="14" /> Tester le menu PXE</NButton>
        </div>
        <div v-if="selected" class="bl-actions">
          <span class="bl-muted">{{ selected.entries.length }} image(s){{ selected.wimboot ? " · wimboot" : "" }}</span>
          <NButton size="sm" variant="ghost" @click="openFolder(selected.folder)">Ouvrir le dossier</NButton>
        </div>
        <template v-if="status.command">
          <pre class="bl-pre">{{ status.command }}</pre>
          <div class="bl-actions"><NButton size="sm" variant="secondary" @click="copyCommand">Copier la commande</NButton></div>
        </template>
        <div v-if="diagnostic" class="bl-text" role="status">
          <p v-if="!diagnostic.running">Démarre le serveur PXE pour tester le menu.</p>
          <template v-else>
            <p :class="diagnostic.http ? 'bl-ok' : 'bl-ko'">
              {{ diagnostic.http ? "Menu iPXE accessible depuis ce PC." : "Le menu iPXE ne répond pas sur l’adresse choisie." }}
            </p>
            <p>Port HTTP · {{ diagnostic.port }}</p>
            <p v-if="!diagnostic.external">Choisis une adresse du réseau local pour accéder au menu depuis un autre appareil.</p>
            <p v-else-if="diagnostic.client">Dernière lecture depuis un client · {{ diagnostic.client.address }} · {{ dateText(diagnostic.client.at) }}</p>
            <p v-else>Démarre iPXE sur un autre appareil et exécute la commande pour confirmer l’accès réseau.</p>
          </template>
        </div>
        <div v-if="status.log.length" class="bl-log">
          <table>
            <thead><tr><th>Heure</th><th>Client</th><th>Requête</th><th>Statut</th><th>Octets</th></tr></thead>
            <tbody>
              <tr v-for="(row, i) in status.log" :key="i">
                <td>{{ dateText(row.at) }}</td><td>{{ row.client }}</td><td>{{ row.method }} {{ row.path }}</td>
                <td :class="row.status < 400 ? 'bl-ok' : 'bl-ko'">{{ row.status }}</td><td>{{ row.bytes }}</td>
              </tr>
            </tbody>
          </table>
        </div>
        <p v-else-if="status.running" class="bl-muted">Aucune requête reçue pour l’instant.</p>
      </div>
    </NCard>

    <NCard>
      <div class="bl-panel">
        <div class="bl-title">Préparer une VM iPXE</div>
        <p class="bl-text">VM d’essai démarrant l’ISO iPXE officielle (empreinte SHA-256 vérifiée), réseau NAT, pour essayer le menu.</p>
        <div class="bl-fields">
          <label>Hyperviseur
            <select v-model="testPlatform"><option value="virtualbox">VirtualBox</option><option value="vmware">VMware Workstation</option></select>
          </label>
          <label>Démarrage <select v-model="testFirmware"><option value="bios">BIOS</option><option value="efi">UEFI</option></select></label>
          <NButton variant="secondary" :loading="busy === 'test'" @click="testVmCreate">Préparer une VM iPXE</NButton>
        </div>
        <TestVmCard v-if="testVm" :machine="testVm" @update="(m) => (testVm = m)" @removed="testVm = null" @install="(m) => emit('install', m)" />
      </div>
    </NCard>
  </div>
</template>
