<script setup lang="ts">
// Port de TokaISO 7.9 (MIT, Cédrick Roy) : modèles de clés Ventoy (usb_workshop.js, usb_templates.py).
import { computed, ref, watch } from "vue";
import { open, save } from "@tauri-apps/plugin-dialog";
import NButton from "@/components/ui/NButton.vue";
import { FileArchive, Upload, Wand2 } from "lucide-vue-next";
import { useUsbWorkshop, displayBytes, errorText } from "./useUsbWorkshop";

const ws = useUsbWorkshop();
const { device, drive, busy, paths } = ws;

interface TemplateFile { path: string; name: string; size: number; sha256: string; present: boolean; available: boolean }
interface Template { name: string; digest: string; files: TemplateFile[]; ready: boolean; path: string }

const name = ref("");
const template = ref<Template | null>(null);
const nameInput = ref<HTMLInputElement | null>(null);

const canApply = computed(() => !!drive.value?.ventoy && !!template.value?.ready);

async function act(operation: () => Promise<unknown>) {
  try { await operation(); } catch (error) { ws.notify(errorText(error)); }
}

async function exportTemplate() {
  if (!name.value.trim()) {
    ws.notify("Donne un nom au modèle USB.");
    nameInput.value?.focus();
    return;
  }
  const target = await save({ defaultPath: "modele-usb.zip", filters: [{ name: "Modèle USB", extensions: ["zip"] }] });
  if (!target) return;
  const result = await ws.run<{ count: number }>("usb_ws_template_export", { id: device.value, name: name.value, target }, 14400000);
  ws.notify(`Modèle exporté avec ${result.count} image(s) référencée(s).`);
}

async function previewTemplate(path: string) {
  const result = await ws.run<Omit<Template, "path">>("usb_ws_template_preview",
    { path, id: drive.value?.ventoy ? device.value : "", sources: paths.value }, 45000);
  template.value = { ...result, path };
}

async function importTemplate() {
  const chosen = await open({ multiple: false, filters: [{ name: "Modèle USB", extensions: ["zip"] }] });
  if (typeof chosen === "string") await previewTemplate(chosen);
}

async function applyTemplate() {
  if (!template.value?.ready || !device.value) return;
  const result = await ws.run<{ copied: number; same: number }>("usb_ws_template_apply",
    { id: device.value, path: template.value.path, digest: template.value.digest, sources: paths.value }, 14400000);
  ws.notify(`Modèle recréé · ${result.copied} image(s) copiée(s), ${result.same} déjà présente(s).`);
}

function fileState(row: TemplateFile) {
  return row.present ? "Sur la clé, à vérifier" : row.available ? "Copie locale disponible" : "Image à récupérer";
}

watch([device, paths], () => { if (template.value) act(() => previewTemplate(template.value!.path)); });
</script>

<template>
  <section>
    <div class="usbws-fields">
      <label>Nom du modèle
        <input ref="nameInput" v-model="name" type="text" maxlength="72" placeholder="Clé de secours" />
      </label>
      <NButton variant="secondary" size="sm" :disabled="busy || !drive?.ventoy" @click="act(exportTemplate)"><Upload :size="14" /> Exporter la clé comme modèle</NButton>
      <NButton variant="secondary" size="sm" :disabled="busy" @click="act(importTemplate)"><FileArchive :size="14" /> Ouvrir un modèle ZIP</NButton>
    </div>
    <p class="usbws-muted">Le modèle enregistre la liste des images (avec leur SHA-256), le thème et le menu de la clé, sans copier les images.
      Pour recréer la clé, NiTriTe cherche les images correspondantes parmi les images sélectionnées en haut de page et le dossier de téléchargements.</p>
    <template v-if="template">
      <h3>{{ template.name }}</h3>
      <div v-for="row in template.files" :key="row.path" class="usbws-row">
        <div class="usbws-row-main"><strong>{{ row.name }}</strong><span>{{ displayBytes(row.size) }} · {{ fileState(row) }}</span></div>
      </div>
      <NButton size="sm" :disabled="busy || !canApply" @click="act(applyTemplate)"><Wand2 :size="14" /> Recréer sur la clé sélectionnée</NButton>
    </template>
  </section>
</template>
