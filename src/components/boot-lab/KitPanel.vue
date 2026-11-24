<script setup lang="ts">
// Port de TokaISO 7.9 (MIT, Cédrick Roy) : trousse hors ligne (workshop.py export, options de labo).
import { onMounted, reactive, ref } from "vue";
import NCard from "@/components/ui/NCard.vue";
import NButton from "@/components/ui/NButton.vue";
import { Package, Plus, Trash2, FolderOpen } from "lucide-vue-next";
import { useNotificationStore } from "@/stores/notifications";
import { call, fileName, openFolder, pickFiles, pickFolder } from "./useBootLab";

interface Profile { arch: string; firmware: string; winget: boolean }
interface KitResult { folder: string; count: number; notes: string[] }
interface Labs { vm: Record<string, { name: string }>; platforms: { key: string; name: string }[] }

const notify = useNotificationStore();
const files = ref<string[]>([]);
const profile = ref<Profile | null>(null);
const labs = ref<Labs | null>(null);
const busy = ref(false);
const result = ref<KitResult | null>(null);
const opts = reactive({ name: "Trousse", ventoy: false, drivers: false, apps: false, arch: "x64", firmware: "UEFI", lab: "", platform: "virtualbox" });

async function add() {
  const picked = await pickFiles("Fichiers", [], true);
  files.value = [...new Set([...files.value, ...picked])].slice(0, 24);
}

async function exportKit() {
  const destination = await pickFolder();
  if (!destination) return;
  busy.value = true;
  const lab = opts.lab ? { id: opts.lab, platform: opts.platform } : null;
  result.value = await call<KitResult>("boot_lab_kit_export", {
    request: { paths: files.value, destination, name: opts.name, ventoy: opts.ventoy, drivers: opts.drivers, apps: opts.apps,
      arch: opts.arch, firmware: opts.firmware, lab },
  }, 3600000);
  busy.value = false;
  if (result.value) notify.success("Trousse exportée", result.value.folder);
}

onMounted(async () => {
  profile.value = await call<Profile>("boot_lab_profile");
  labs.value = await call<Labs>("boot_lab_labs");
  if (profile.value && profile.value.arch !== "Inconnu") opts.arch = profile.value.arch;
  if (profile.value && profile.value.firmware !== "Inconnu") opts.firmware = profile.value.firmware;
});
</script>

<template>
  <div class="bl-panel">
    <NCard>
      <div class="bl-panel">
        <div class="bl-title"><Package :size="16" /> Trousse hors ligne</div>
        <p class="bl-text">
          Copie vérifiée (SHA-256) des fichiers choisis vers une clé ou un disque, avec SHA256.txt, une fiche JSON et une page
          « Commencer-ici.html » à cocher. Les images ISO/IMG peuvent aller dans un dossier Ventoy.
        </p>
        <p v-if="profile" class="bl-muted">Ce PC : {{ profile.arch }} · {{ profile.firmware }} · WinGet {{ profile.winget ? "présent" : "absent" }}</p>
        <div class="bl-fields">
          <NButton variant="secondary" @click="add"><Plus :size="14" /> Ajouter des fichiers</NButton>
          <label>Nom de la trousse <input v-model="opts.name" maxlength="48" /></label>
          <label>Architecture cible
            <select v-model="opts.arch"><option>x64</option><option>ARM64</option><option>x86</option></select>
          </label>
          <label>Démarrage cible <select v-model="opts.firmware"><option>UEFI</option><option>BIOS</option></select></label>
          <label>Labo VM
            <select v-model="opts.lab">
              <option value="">Aucun</option>
              <option v-for="(lab, key) in labs?.vm ?? {}" :key="key" :value="key">{{ lab.name }}</option>
            </select>
          </label>
          <label v-if="opts.lab">Hyperviseur du labo
            <select v-model="opts.platform"><option v-for="p in labs?.platforms ?? []" :key="p.key" :value="p.key">{{ p.name }}</option></select>
          </label>
        </div>
        <div class="bl-fields">
          <label class="bl-check"><input v-model="opts.ventoy" type="checkbox" /> Placer les ISO/IMG dans un dossier Ventoy</label>
          <label class="bl-check"><input v-model="opts.drivers" type="checkbox" /> Exporter les pilotes tiers de ce PC</label>
          <label class="bl-check"><input v-model="opts.apps" type="checkbox" :disabled="profile?.winget === false" /> Exporter la liste des applications WinGet</label>
        </div>
        <div v-if="files.length" class="bl-list">
          <div v-for="path in files" :key="path" class="bl-row">
            <div class="bl-row-main"><span class="bl-row-name">{{ fileName(path) }}</span><span class="bl-muted">{{ path }}</span></div>
            <NButton size="sm" variant="ghost" @click="files = files.filter((f) => f !== path)"><Trash2 :size="14" /></NButton>
          </div>
        </div>
        <div class="bl-actions">
          <NButton :disabled="!files.length" :loading="busy" @click="exportKit">Exporter la trousse</NButton>
        </div>
        <div v-if="result" class="bl-row">
          <div class="bl-row-main">
            <span class="bl-row-name">{{ result.count }} fichier(s) copiés</span>
            <span class="bl-muted">{{ result.folder }}</span>
            <span v-for="note in result.notes" :key="note" class="bl-muted">{{ note }}</span>
          </div>
          <NButton size="sm" variant="secondary" @click="openFolder(result.folder)"><FolderOpen :size="14" /> Ouvrir le dossier</NButton>
        </div>
      </div>
    </NCard>
  </div>
</template>
