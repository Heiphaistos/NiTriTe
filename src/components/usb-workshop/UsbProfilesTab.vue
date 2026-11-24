<script setup lang="ts">
// Port de TokaISO 7.9 (MIT, Cédrick Roy) : profils Ventoy, comparaison et synchro (deployment.js, ventoy_profiles.py).
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import NButton from "@/components/ui/NButton.vue";
import { Save, Trash2, GitCompare, RefreshCcw } from "lucide-vue-next";
import { useUsbWorkshop, displayBytes, errorText } from "./useUsbWorkshop";

const ws = useUsbWorkshop();
const { device, drive, busy, paths } = ws;

interface Profile { id: string; name: string; paths: string[] }
interface Row { name: string; status: "add" | "same" | "replace"; source: { size: number } }
interface Comparison { rows: Row[]; free: number; needed: number; fat32: boolean; token: string; can_apply: boolean }

const statusLabels: Record<string, string> = { add: "À ajouter", same: "Déjà identique", replace: "À remplacer" };
const profiles = ref<Profile[]>([]);
const profile = ref("");
const name = ref("");
const comparison = ref<Comparison | null>(null);
const replace = ref(false);
const progress = ref("");
let unlisten: UnlistenFn | null = null;

const hasReplace = computed(() => !!comparison.value?.rows.some((row) => row.status === "replace"));
const canApply = computed(() => !!comparison.value?.can_apply && (!hasReplace.value || replace.value));
const selectedProfile = computed(() => profiles.value.find((row) => row.id === profile.value));

function reset() { comparison.value = null; replace.value = false; }

async function act(operation: () => Promise<unknown>) {
  try { await operation(); } catch (error) { ws.notify(errorText(error)); }
  finally { progress.value = ""; }
}

function show(list: Profile[]) {
  profiles.value = list;
  if (!list.some((row) => row.id === profile.value)) profile.value = list[0]?.id || "";
}

async function saveProfile() {
  const result = await ws.run<{ profiles: Profile[]; id: string }>("usb_ws_profile_save", { name: name.value, paths: paths.value });
  reset();
  show(result.profiles);
  profile.value = result.id;
  ws.notify("Profil Ventoy enregistré.");
}

async function removeProfile() {
  const result = await ws.run<{ profiles: Profile[] }>("usb_ws_profile_remove", { profile: profile.value });
  reset();
  show(result.profiles);
}

async function preview() {
  reset();
  comparison.value = await ws.run<Comparison>("usb_ws_profile_preview", { id: device.value, profile: profile.value }, 14400000);
}

async function apply() {
  const result = await ws.run<{ updated: number }>("usb_ws_profile_apply",
    { id: device.value, profile: profile.value, token: comparison.value?.token, replace: replace.value }, 14400000);
  reset();
  ws.notify(`${result.updated} fichier(s) mis à jour et vérifiés.`);
}

watch([device, profile], reset);

onMounted(async () => {
  unlisten = await listen<{ phase: string; done: number; total: number }>("usb-workshop-profile-progress", (event) => {
    const { phase, done, total } = event.payload;
    progress.value = `${phase === "copy" ? "Copie et vérification" : "Comparaison des fichiers"} · ${done}/${total}`;
  });
  act(async () => show((await ws.run<{ profiles: Profile[] }>("usb_ws_profile_list", null)).profiles));
});
onUnmounted(() => { unlisten?.(); });
</script>

<template>
  <section>
    <p class="usbws-muted">Un profil retient une liste d’images locales. La comparaison montre ce qui manque ou diffère dans le dossier TokaISO de la clé, puis seuls ces fichiers sont copiés et vérifiés.</p>
    <div class="usbws-fields">
      <label>Nom du profil
        <input v-model="name" type="text" maxlength="72" placeholder="Clé de dépannage" />
      </label>
      <NButton variant="secondary" size="sm" :disabled="busy || !paths.length || !name.trim()" @click="act(saveProfile)"><Save :size="14" /> Enregistrer les images sélectionnées</NButton>
    </div>
    <div class="usbws-fields">
      <label>Profil Ventoy
        <select v-model="profile" :disabled="busy">
          <option v-if="!profiles.length" value="">Aucun profil</option>
          <option v-for="row in profiles" :key="row.id" :value="row.id">{{ row.name }} · {{ row.paths.length }} image(s)</option>
        </select>
      </label>
      <NButton variant="ghost" size="sm" :disabled="busy || !profile" @click="act(removeProfile)"><Trash2 :size="14" /> Supprimer</NButton>
      <NButton variant="secondary" size="sm" :disabled="busy || !profile || !drive?.ventoy" @click="act(preview)"><GitCompare :size="14" /> Comparer avec la clé</NButton>
    </div>
    <p v-if="selectedProfile" class="usbws-muted">{{ selectedProfile.paths.join(" · ") }}</p>
    <p v-if="progress" class="usbws-muted" role="status">{{ progress }}</p>

    <template v-if="comparison">
      <div v-for="row in comparison.rows" :key="row.name" class="usbws-row">
        <div class="usbws-row-main"><strong>{{ row.name }}</strong><span>{{ statusLabels[row.status] }} · {{ displayBytes(row.source.size) }}</span></div>
      </div>
      <p class="usbws-muted">Espace de travail nécessaire · {{ displayBytes(comparison.needed) }} | Espace libre · {{ displayBytes(comparison.free) }}</p>
      <p v-if="comparison.fat32" class="usbws-warning">Une image dépasse la limite FAT32.</p>
      <div class="usbws-fields">
        <label v-if="hasReplace" class="usbws-check"><input v-model="replace" type="checkbox" /> Remplacer les fichiers différents sur la clé</label>
        <NButton size="sm" :disabled="busy || !canApply" @click="act(apply)"><RefreshCcw :size="14" /> Appliquer les changements</NButton>
      </div>
    </template>
  </section>
</template>
