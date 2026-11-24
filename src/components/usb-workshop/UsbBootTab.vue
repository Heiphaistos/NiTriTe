<script setup lang="ts">
// Port de TokaISO 7.9 (MIT, Cédrick Roy) : persistance Linux et test de la clé en VM (usb_boot.js).
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { confirm } from "@tauri-apps/plugin-dialog";
import NButton from "@/components/ui/NButton.vue";
import { ListTree, Plus, MonitorPlay, Power, Trash2, Play } from "lucide-vue-next";
import { useUsbWorkshop, displayBytes, errorText } from "./useUsbWorkshop";

const ws = useUsbWorkshop();
const { device, drive, busy } = ws;

interface Machine { id: string; name: string; platform: string; file: string; size: number; firmware: string }
interface Progress { phase: "read" | "convert"; written: number; size: number }

const images = ref<{ image: string; label: string }[]>([]);
const presets = ref<Record<string, string>>({});
const sizes = ref<number[]>([]);
const image = ref("");
const profile = ref("");
const size = ref<number | "">("");
const machines = ref<Machine[]>([]);
const hypervisors = ref<{ vmware?: boolean; virtualbox?: boolean }>({});
const hypervisor = ref("");
const firmware = ref("efi");
const creating = ref(false);
const progress = ref<Progress | null>(null);
let unlisten: UnlistenFn | null = null;

const hypervisorChoices = computed(() =>
  ([["vmware", "VMware Workstation"], ["virtualbox", "VirtualBox"]] as const).filter(([key]) => hypervisors.value[key]));
const canPersist = computed(() => !!image.value && !!profile.value && !!size.value);
const progressText = computed(() => {
  const state = progress.value;
  if (!state) return creating.value ? "Préparation de la lecture de la clé USB…" : "";
  return state.phase === "read" ? `Lecture de la clé USB · ${displayBytes(state.written)} / ${displayBytes(state.size)}` :
    "Conversion du disque virtuel…";
});

function sizeLabel(amount: number) {
  return amount >= 1024 ? `${amount / 1024} Gio` : `${amount} Mio`;
}

async function act(operation: () => Promise<unknown>) {
  try { await operation(); } catch (error) { ws.notify(errorText(error)); }
}

watch(device, () => { images.value = []; presets.value = {}; sizes.value = []; image.value = ""; });
watch(image, () => {
  const match = images.value.find((row) => row.image === image.value);
  if (match?.label) profile.value = match.label;
});

async function scan() {
  const target = device.value;
  const result = await ws.run<{ images: { image: string; label: string }[]; presets: Record<string, string>; sizes: number[] }>(
    "usb_ws_boot_images", { id: target }, 45000);
  if (device.value !== target) return;
  images.value = result.images;
  presets.value = result.presets;
  sizes.value = result.sizes;
  if (!(profile.value in presets.value)) profile.value = Object.keys(presets.value)[0] || "";
  if (!sizes.value.includes(Number(size.value))) size.value = sizes.value[0] ?? "";
  image.value = images.value[0]?.image || "";
  if (!images.value.length) ws.notify("Aucune ISO trouvée sur la clé Ventoy.");
}

async function persist() {
  const data = await ws.run<{ backend: string }>("usb_ws_boot_persistence",
    { id: device.value, image: image.value, label: profile.value, size: Number(size.value) }, 1800000);
  ws.notify(`Persistance ajoutée · ${data.backend}`);
}

async function updateTests() {
  const response = await ws.run<{ machines: Machine[]; hypervisors: { vmware: boolean; virtualbox: boolean } }>("usb_ws_boot_list", null, 30000);
  hypervisors.value = response.hypervisors;
  machines.value = response.machines;
  if (!hypervisorChoices.value.some(([key]) => key === hypervisor.value)) hypervisor.value = hypervisorChoices.value[0]?.[0] || "";
}

async function create() {
  creating.value = true;
  progress.value = null;
  try {
    await ws.run("usb_ws_boot_create", { id: device.value, platform: hypervisor.value, firmware: firmware.value }, 14400000);
    ws.notify("Copie de la clé prête à démarrer.");
    await updateTests();
  } finally {
    creating.value = false;
    progress.value = null;
  }
}

async function control(action: "start" | "stop" | "discard", machine: Machine) {
  if (action === "discard" && !(await confirm("Retirer la VM et sa copie locale de la clé USB ?", { title: "Atelier USB", kind: "warning" }))) return;
  await ws.run("usb_ws_boot_control", { action, machine: machine.id }, 90000);
  if (action === "discard") ws.notify("Copie de test retirée.");
  await updateTests();
}

onMounted(async () => {
  unlisten = await listen<Progress>("usb-workshop-boot-progress", (event) => { if (creating.value) progress.value = event.payload; });
});
onUnmounted(() => { unlisten?.(); });
</script>

<template>
  <section>
    <details open>
      <summary>Persistance Linux</summary>
      <p class="usbws-muted">Choisis une ISO Linux Live compatible. Le modèle de persistance provient du projet Ventoy.</p>
      <div class="usbws-fields">
        <NButton variant="secondary" size="sm" :disabled="busy || !drive?.ventoy" @click="act(scan)"><ListTree :size="14" /> Lister les ISO Ventoy</NButton>
        <label>ISO sur la clé
          <select v-model="image" :disabled="busy">
            <option v-if="!images.length" value="">Sélectionne une ISO de la clé Ventoy</option>
            <option v-for="row in images" :key="row.image" :value="row.image">{{ row.image }}</option>
          </select>
        </label>
        <label>Distribution Linux
          <select v-model="profile" :disabled="busy">
            <option v-for="(title, key) in presets" :key="key" :value="key">{{ title }}</option>
          </select>
        </label>
        <label>Espace de persistance
          <select v-model="size" :disabled="busy">
            <option v-for="amount in sizes" :key="amount" :value="amount">{{ sizeLabel(amount) }}</option>
          </select>
        </label>
        <NButton size="sm" :disabled="busy || !canPersist" @click="act(persist)"><Plus :size="14" /> Ajouter la persistance</NButton>
      </div>
    </details>

    <details @toggle="(e) => (e.target as HTMLDetailsElement).open && act(updateTests)">
      <summary>Tester la clé en machine virtuelle</summary>
      <p class="usbws-muted">NiTriTe copie la clé en lecture seule. La VM démarre sur le disque virtuel ainsi créé. La copie utilise autant d’espace que la capacité de la clé.</p>
      <div class="usbws-fields">
        <label>Hyperviseur
          <select v-model="hypervisor" :disabled="busy">
            <option v-if="!hypervisorChoices.length" value="">Aucun hyperviseur détecté</option>
            <option v-for="[key, name] in hypervisorChoices" :key="key" :value="key">{{ name }}</option>
          </select>
        </label>
        <label>Démarrage
          <select v-model="firmware" :disabled="busy"><option value="efi">UEFI</option><option value="bios">BIOS</option></select>
        </label>
        <NButton size="sm" :disabled="busy || !device || !hypervisor" :loading="creating" @click="act(create)"><MonitorPlay :size="14" /> Créer une copie et préparer la VM</NButton>
      </div>
      <template v-if="creating">
        <progress :max="progress?.phase === 'read' ? progress.size : undefined" :value="progress?.phase === 'read' ? progress.written : undefined" />
        <p class="usbws-muted" role="status">{{ progressText }}</p>
      </template>
      <div v-for="machine in machines" :key="machine.id" class="usbws-row">
        <div class="usbws-row-main">
          <strong>{{ machine.name }}</strong>
          <span>{{ [machine.file, displayBytes(machine.size), machine.platform === "vmware" ? "VMware" : "VirtualBox", machine.firmware === "efi" ? "UEFI" : "BIOS"].join(" · ") }}</span>
        </div>
        <div class="usbws-actions">
          <NButton variant="secondary" size="sm" :disabled="busy" @click="act(() => control('start', machine))"><Play :size="14" /> Démarrer la VM</NButton>
          <NButton variant="secondary" size="sm" :disabled="busy" @click="act(() => control('stop', machine))"><Power :size="14" /> Éteindre la VM</NButton>
          <NButton variant="danger" size="sm" :disabled="busy" @click="act(() => control('discard', machine))"><Trash2 :size="14" /> Retirer la copie</NButton>
        </div>
      </div>
    </details>
  </section>
</template>
