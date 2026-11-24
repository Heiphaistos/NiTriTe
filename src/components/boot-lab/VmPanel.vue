<script setup lang="ts">
// Port de TokaISO 7.9 (MIT, Cédrick Roy) : préparation de VM (extras.js #vm-dialog) et VM installable (vm75.js).
import { computed, onMounted, reactive, ref, watch } from "vue";
import NCard from "@/components/ui/NCard.vue";
import NButton from "@/components/ui/NButton.vue";
import NBadge from "@/components/ui/NBadge.vue";
import { Server, HardDrive, FolderOpen, RefreshCw, Disc, ExternalLink } from "lucide-vue-next";
import { useNotificationStore } from "@/stores/notifications";
import {
  call, fileName, installPrefill, launchMessage, openFolder, openLink, pickFiles, platformNames, useVmList, vmStates,
  type ControlResult, type Machine,
} from "./useBootLab";

const notify = useNotificationStore();
const { vmList, refresh, replace } = useVmList();
const loading = ref(false);
const busy = ref("");

const imp = reactive({ platform: "virtualbox", file: "", name: "NiTriTe VM", ram: 2048, cpu: 2, firmware: "auto", network: "nat", adapter: "" });
const inst = reactive({ platform: "virtualbox", path: "", name: "NiTriTe VM", ram: 4096, cpu: 2, disk: 64, firmware: "efi", network: "off", adapter: "" });

const hv = computed(() => vmList.value?.hypervisors);
const detected = computed(() => Object.entries(hv.value ?? {}).filter(([, found]) => found).map(([key]) => platformNames[key]));
const nothing = computed(() => hv.value && !hv.value.virtualbox && !hv.value.vmware);
const prepared = computed(() => (vmList.value?.machines ?? []).filter((m) => !m.test && !m.install_iso));
const installable = computed(() => (vmList.value?.machines ?? []).filter((m) => m.install_iso));
const interfaces = computed(() => vmList.value?.interfaces ?? []);
const importBridge = computed(() => imp.platform === "virtualbox" && imp.network === "bridged");
const installBridge = computed(() => inst.platform === "virtualbox" && inst.network === "bridged");
const canImport = computed(() => !!hv.value?.[imp.platform as "virtualbox"] && !!imp.file && (!importBridge.value || !!imp.adapter));
const canInstall = computed(() =>
  !!vmList.value?.installers[inst.platform as "virtualbox"] && !!inst.path && (!installBridge.value || !!inst.adapter));

async function load() {
  loading.value = true;
  try {
    const data = await refresh();
    const first = (["virtualbox", "vmware"] as const).find((k) => data.hypervisors[k]);
    if (first && !data.hypervisors[imp.platform as "virtualbox"]) imp.platform = first;
    const inst1 = (["virtualbox", "vmware"] as const).find((k) => data.installers[k]);
    if (inst1 && !data.installers[inst.platform as "virtualbox"]) inst.platform = inst1;
    if (!data.interfaces.includes(imp.adapter)) imp.adapter = data.interfaces[0] ?? "";
    if (!data.interfaces.includes(inst.adapter)) inst.adapter = data.interfaces[0] ?? "";
  } catch (error) {
    notify.error("Hyperviseurs", String(error));
  } finally {
    loading.value = false;
  }
}

async function pickImage() {
  const [path] = await pickFiles("Image de VM", ["ova", "ovf", "vdi", "vmdk", "zip", "7z", "gz", "xz"]);
  if (path) {
    imp.file = path;
    imp.name = fileName(path).replace(/\.[^.]+$/, "").slice(0, 56);
  }
}

async function pickIso() {
  const [path] = await pickFiles("Image ISO", ["iso"]);
  if (path) inst.path = path;
}

async function doImport() {
  busy.value = "import";
  const machine = await call<Machine>("boot_lab_vm_import", { request: { ...imp } }, 1800000);
  busy.value = "";
  if (!machine) return;
  replace(machine);
  notify.success("VM préparée dans le dossier Machines NiTriTe.");
}

async function doInstall() {
  busy.value = "install";
  const machine = await call<Machine>("boot_lab_vm_install_iso", { request: { ...inst } }, 1800000);
  busy.value = "";
  if (!machine) return;
  replace(machine);
  notify.success(machine.disk_exceeds_free
    ? "VM prête. Le disque pourra dépasser l’espace libre actuel pendant son utilisation."
    : "VM prête. Ouvre la console pour installer le système.");
}

async function control(action: string, machine: Machine) {
  busy.value = action + machine.id;
  const result = await call<ControlResult>("boot_lab_vm_action", { action, id: machine.id }, action === "vm-start" ? 90000 : 45000);
  busy.value = "";
  if (!result?.machine) return;
  replace(result.machine);
  if (action === "vm-eject-iso") notify.success("ISO éjectée. Le disque virtuel reste attaché.");
  else notify.info(launchMessage(result) || (action === "vm-console" && result.machine.install_iso
    ? "Continue l’installation dans la console de la VM." : vmStates[result.machine.state] || result.machine.state));
}

function detail(m: Machine): string {
  const parts = [platformNames[m.platform], m.firmware === "efi" ? "UEFI" : m.firmware === "bios" ? "BIOS" : "Démarrage automatique",
    `${m.ram} Mio · ${m.cpu} vCPU`];
  if (m.install_iso) parts.push(`Disque virtuel de ${m.disk} Gio`, m.iso_attached ? "ISO d’installation attachée" : "ISO éjectée");
  return parts.join(" · ");
}

watch(installPrefill, (value) => {
  if (!value) return;
  inst.path = value.iso ?? "";
  inst.platform = value.platform;
  inst.firmware = value.firmware === "bios" ? "bios" : "efi";
  installPrefill.value = null;
}, { immediate: true });
onMounted(load);
</script>

<template>
  <div class="bl-panel">
    <NCard>
      <div class="bl-panel">
        <div class="bl-title">
          <Server :size="16" /> Hyperviseurs
          <NButton size="sm" variant="ghost" :loading="loading" @click="load"><RefreshCw :size="14" /></NButton>
        </div>
        <p v-if="detected.length" class="bl-text">Détecté sur ce PC : {{ detected.join(" · ") }}</p>
        <template v-if="nothing">
          <p class="bl-text bl-ko">VirtualBox et VMware Workstation sont introuvables sur ce PC. Installe l’un d’eux depuis son site officiel :</p>
          <div class="bl-actions">
            <NButton size="sm" variant="secondary" @click="openLink(vmList!.links.virtualbox)"><ExternalLink :size="14" /> VirtualBox</NButton>
            <NButton size="sm" variant="secondary" @click="openLink(vmList!.links.vmware)"><ExternalLink :size="14" /> VMware Workstation</NButton>
          </div>
        </template>
        <p v-if="hv?.hyperv" class="bl-muted">
          Hyper-V est présent sur ce PC. Le labo pilote VirtualBox et VMware Workstation ; Hyper-V reste à gérer depuis son
          gestionnaire.
          <a href="#" @click.prevent="openLink(vmList!.links.hyperv)">Documentation Hyper-V</a>
        </p>
        <p v-if="vmList && !vmList.seven_zip" class="bl-muted">7-Zip est absent : les images .7z et .xz devront être décompressées à la main.</p>
      </div>
    </NCard>

    <NCard>
      <div class="bl-panel">
        <div class="bl-title"><HardDrive :size="16" /> Importer une image de VM</div>
        <p class="bl-text">OVA, OVF, VDI, VMDK ou archive (ZIP, 7z, tar.gz, tar.xz, .vdi.xz, .vmdk.xz). L’archive est contrôlée puis extraite dans un dossier privé.</p>
        <div class="bl-fields">
          <label>Hyperviseur
            <select v-model="imp.platform">
              <option value="virtualbox" :disabled="!hv?.virtualbox">VirtualBox</option>
              <option value="vmware" :disabled="!hv?.vmware">VMware Workstation</option>
            </select>
          </label>
          <NButton variant="secondary" @click="pickImage">Choisir un fichier local</NButton>
          <label>Nom <input v-model="imp.name" maxlength="56" /></label>
          <label>Mémoire (Mio) <input v-model.number="imp.ram" type="number" min="512" max="65536" /></label>
          <label>Processeurs <input v-model.number="imp.cpu" type="number" min="1" max="16" /></label>
          <label>Démarrage
            <select v-model="imp.firmware"><option value="auto">Automatique</option><option value="efi">UEFI</option><option value="bios">BIOS</option></select>
          </label>
          <label>Réseau
            <select v-model="imp.network"><option value="nat">NAT</option><option value="off">Déconnecté</option><option value="bridged">Accès au réseau local (pont)</option></select>
          </label>
          <label v-if="importBridge">Carte réseau
            <select v-model="imp.adapter"><option v-for="name in interfaces" :key="name" :value="name">{{ name }}</option></select>
          </label>
          <NButton :disabled="!canImport || vmList?.busy" :loading="busy === 'import'" @click="doImport">Importer ou créer la VM</NButton>
        </div>
        <p v-if="imp.file" class="bl-muted">{{ imp.file }}</p>
      </div>
    </NCard>

    <NCard>
      <div class="bl-panel">
        <div class="bl-title"><Disc :size="16" /> Créer une VM installable depuis l’ISO</div>
        <p class="bl-text">Choisis une ISO pour PC x86 ou x64. Le disque virtuel grandit pendant l’installation.</p>
        <div class="bl-fields">
          <NButton variant="secondary" @click="pickIso">Choisir une ISO</NButton>
          <label>Nom de la VM <input v-model="inst.name" maxlength="56" /></label>
          <label>Hyperviseur
            <select v-model="inst.platform">
              <option value="virtualbox" :disabled="!vmList?.installers.virtualbox">VirtualBox</option>
              <option value="vmware" :disabled="!vmList?.installers.vmware">VMware Workstation</option>
            </select>
          </label>
          <label>Démarrage <select v-model="inst.firmware"><option value="efi">UEFI</option><option value="bios">BIOS</option></select></label>
          <label>Mémoire vive (Mio) <input v-model.number="inst.ram" type="number" min="512" max="65536" step="4" /></label>
          <label>Processeurs <input v-model.number="inst.cpu" type="number" min="1" max="16" /></label>
          <label>Disque virtuel (Gio) <input v-model.number="inst.disk" type="number" min="4" max="2048" /></label>
          <label>Réseau
            <select v-model="inst.network"><option value="off">Déconnecté</option><option value="nat">NAT</option><option value="bridged">En pont</option></select>
          </label>
          <label v-if="installBridge">Carte réseau
            <select v-model="inst.adapter"><option v-for="name in interfaces" :key="name" :value="name">{{ name }}</option></select>
          </label>
          <NButton :disabled="!canInstall || vmList?.busy" :loading="busy === 'install'" @click="doInstall">Préparer la VM installable</NButton>
        </div>
        <p class="bl-muted">{{ inst.path ? fileName(inst.path) : "Choisis une ISO pour préparer ta VM." }}</p>
        <p v-if="vmList && !vmList.installers.virtualbox && !vmList.installers.vmware" class="bl-muted">
          Aucun hyperviseur compatible avec la création de disque n’a été détecté.
        </p>
      </div>
    </NCard>

    <NCard>
      <div class="bl-panel">
        <div class="bl-title">Mes machines préparées</div>
        <p v-if="!prepared.length && !installable.length" class="bl-muted">Aucune VM préparée pour l’instant.</p>
        <div class="bl-list">
          <div v-for="m in [...installable, ...prepared]" :key="m.id" class="bl-row">
            <div class="bl-row-main">
              <span class="bl-row-name">{{ m.name }}</span>
              <span class="bl-muted">{{ detail(m) }}</span>
              <span><NBadge :variant="m.state === 'imported' ? 'neutral' : m.state === 'unknown' ? 'warning' : 'success'">{{ vmStates[m.state] || m.state }}</NBadge></span>
            </div>
            <div class="bl-actions">
              <NButton size="sm" :loading="busy === 'vm-start' + m.id" @click="control('vm-start', m)">Démarrer</NButton>
              <NButton v-if="m.platform === 'vmware' || m.install_iso" size="sm" variant="secondary" :loading="busy === 'vm-console' + m.id" @click="control('vm-console', m)">
                Ouvrir la console
              </NButton>
              <NButton size="sm" variant="secondary" :loading="busy === 'vm-status' + m.id" @click="control('vm-status', m)">Vérifier l’état</NButton>
              <NButton v-if="m.install_iso && m.iso_attached" size="sm" variant="secondary" :loading="busy === 'vm-eject-iso' + m.id" @click="control('vm-eject-iso', m)">
                Éjecter l’ISO
              </NButton>
              <NButton size="sm" variant="ghost" @click="openFolder(m.folder)"><FolderOpen :size="14" /> Ouvrir le dossier</NButton>
            </div>
          </div>
        </div>
      </div>
    </NCard>
  </div>
</template>
