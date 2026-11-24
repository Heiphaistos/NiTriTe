<script setup lang="ts">
// Port de TokaISO 7.9 (MIT, Cédrick Roy) : thèmes et menu Ventoy (usb79_themes.js, usb79_themes.css).
import { computed, onMounted, reactive, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import NButton from "@/components/ui/NButton.vue";
import { Palette, FileArchive, FolderOpen, Download, RefreshCw, Eye, Check, History } from "lucide-vue-next";
import { invoke } from "@/utils/invoke";
import { useUsbWorkshop, errorText } from "./useUsbWorkshop";

const ws = useUsbWorkshop();
const { drive, device: selectedDevice, paths } = ws;

interface Theme { id: string; name: string; author?: string; origin?: string; source?: string; available: boolean; builtin?: boolean }
interface Entry { path: string; name: string; alias: string }
interface Backup { id: string; date: string; title: string }
interface Change { key: string; before: string; after: string }
interface Edit { path: string; alias: string; folder: string }

const groups = ["Windows", "Linux", "Secours", "Serveurs", "Autres"];
const themes = ref<Theme[]>([]);
const entries = ref<Entry[]>([]);
const history = ref<Backup[]>([]);
const edits = reactive(new Map<string, Edit>());
const selected = ref("citrus");
const device = ref("");
const ratio = ref("16:9");
const layout = ref("icons");
const mode = ref("");
const useTheme = ref(true);
const backup = ref("");
const previewData = ref<{ colors: Record<string, string>; image: string; icons: Record<string, string> } | null>(null);
const diff = ref<Change[]>([]);
const pending = ref<Record<string, unknown> | null>(null);
const themeStatus = ref("");
const status = ref("");
const savingTheme = ref(false);
let request = 0;
let previewRequest = 0;

const theme = computed(() => themes.value.find((row) => row.id === selected.value));
const quickDisabled = computed(() => !device.value || !theme.value?.available || savingTheme.value);

function tell(value: string) { status.value = value; }
function clearPlan() { pending.value = null; diff.value = []; }

function themeSelection() {
  return { device: device.value, theme: selected.value, ratio: ratio.value, layout: layout.value, mode: mode.value };
}

// Aperçu du menu : noms d'affichage, dossiers et icônes (renderEntries).
const previewRows = computed(() => {
  const fallback = paths.value.map((local) => { const name = local.split(/[/\\]/).pop() || local; return { name, path: "/" + name }; });
  let rows = entries.value.length ? entries.value.map((row) => {
    const edit = edits.get(row.path);
    return { name: (edit ? edit.alias : row.alias) || row.name, path: edit?.folder ? `/TokaISO/${edit.folder}/${row.name}` : row.path };
  }) : fallback;
  if (!rows.length) rows = [{ name: "Ubuntu.iso", path: "/Linux/Ubuntu.iso" },
    { name: "Windows 11.iso", path: "/Windows/Windows 11.iso" }, { name: "WinPE.wim", path: "/WinPE.wim" }];
  const visible = new Map<string, { name: string; icon: string }>();
  for (const row of rows) {
    const folder = mode.value === "tree" ? row.path.split("/").slice(1, -1)[0] : "";
    const ext = (row.path.split(".").pop() || "").toLowerCase();
    const imageType = ({ vhdx: "vhd", img: "img", wim: "wim", efi: "efi", vhd: "vhd", vtoy: "vtoy" } as Record<string, string>)[ext] || "iso";
    visible.set(folder ? "/" + folder : row.path, { name: folder ? folder + "/" : row.name, icon: "vtoy" + (folder ? "dir" : imageType) });
  }
  return [...visible.values()].slice(0, 7).map((row) => ({ ...row, src: /^data:image\/png;base64,/.test(previewData.value?.icons[row.icon] || "") ? previewData.value?.icons[row.icon] : "" }));
});
const previewNote = computed(() => entries.value.length || paths.value.length ? "Aperçu du menu avec les noms de tes images." :
  "Exemple de menu. Tu peux enregistrer le thème sur une clé Ventoy vide.");
const previewStyle = computed(() => {
  const palette: Record<string, string> = { "desktop-color": "--design-bg", item_color: "--design-fg", selected_item_color: "--design-accent" };
  const style: Record<string, string> = {};
  for (const [key, value] of Object.entries(previewData.value?.colors || {})) {
    if (palette[key] && /^#[0-9a-f]{6}$/i.test(value)) style[palette[key]] = value;
  }
  return style;
});
const previewImage = computed(() => /^data:image\/(?:png|jpeg);base64,/.test(previewData.value?.image || "") ? previewData.value?.image : "");

async function updatePreview() {
  const revision = ++previewRequest;
  previewData.value = null;
  if (!theme.value?.available) return;
  try {
    const data = await invoke<{ colors: Record<string, string>; image: string; icons: Record<string, string> }>(
      "usb_ws_design_preview", { payload: { theme: selected.value, ratio: ratio.value, layout: layout.value } });
    if (revision === previewRequest) previewData.value = { ...data, icons: data.icons || {} };
  } catch (error) { if (revision === previewRequest) tell(errorText(error)); }
}

function choose(id: string) { selected.value = id; clearPlan(); updatePreview(); }

function adopt(result: { themes: Theme[]; selected: string }) {
  themes.value = result.themes;
  selected.value = result.selected;
  clearPlan();
  updatePreview();
  tell("Thème Ventoy importé.");
}

async function loadGallery() {
  try {
    themes.value = (await invoke<{ themes: Theme[] }>("usb_ws_design_gallery")).themes;
    await updatePreview();
  } catch (error) { tell(errorText(error)); }
}

async function download() {
  try { adopt(await ws.run("usb_ws_design_download", null, 90000)); } catch (error) { tell(errorText(error)); }
}

async function importTheme(directory: boolean) {
  const chosen = await open(directory ? { directory: true } : { multiple: false, filters: [{ name: "Thème ZIP", extensions: ["zip"] }] });
  if (typeof chosen !== "string") return;
  try { adopt(await ws.run("usb_ws_design_import", { path: chosen }, 90000)); } catch (error) { tell(errorText(error)); }
}

async function loadMenu(force = false, preserveEdits = false) {
  const actual = drive.value?.ventoy ? selectedDevice.value : "";
  if (!force && actual === device.value) return;
  if (actual !== device.value) themeStatus.value = "";
  if (actual !== device.value || !preserveEdits) edits.clear();
  device.value = actual;
  const current = ++request;
  entries.value = [];
  history.value = [];
  clearPlan();
  if (!actual) return;
  try {
    const result = await invoke<{ images: Entry[]; backups: Backup[] }>("usb_ws_design_menu", { payload: { device: actual } });
    if (current !== request) return;
    entries.value = result.images;
    history.value = result.backups;
    backup.value = history.value[0]?.id || "";
  } catch (error) { if (current === request) tell(errorText(error)); }
}

function currentEdit(row: Entry): Edit {
  return edits.get(row.path) || { path: row.path, alias: row.alias, folder: "" };
}

function edited(row: Entry, alias: string, folder: string) {
  clearPlan();
  if (alias !== row.alias || folder) edits.set(row.path, { path: row.path, alias, folder });
  else edits.delete(row.path);
}

async function quickApply() {
  if (quickDisabled.value) return;
  const draft = themeSelection();
  clearPlan();
  savingTheme.value = true;
  themeStatus.value = "Enregistrement du thème Ventoy…";
  try {
    const result = await ws.run<{ token: string }>("usb_ws_design_plan", { ...draft, edits: [], restore: false });
    if (draft.device !== device.value) return;
    await ws.run("usb_ws_design_apply", { ...draft, edits: [], restore: false, token: result.token });
    await loadMenu(true, true);
    if (draft.device === device.value) themeStatus.value = "Thème Ventoy enregistré. Redémarre sur la clé pour le voir.";
  } catch (error) { themeStatus.value = errorText(error); }
  finally { savingTheme.value = false; }
}

async function plan(restore: boolean) {
  clearPlan();
  const draft: Record<string, unknown> = restore ? { device: device.value, backup: backup.value, restore: true } :
    { ...themeSelection(), theme: useTheme.value && theme.value?.available ? selected.value : "", edits: [...edits.values()], restore: false };
  try {
    const result = await ws.run<{ changes: Change[]; token: string }>("usb_ws_design_plan", draft);
    if (draft.device !== device.value) return;
    diff.value = result.changes;
    if (!result.changes.length) { tell("Aucune modification à appliquer."); return; }
    pending.value = { ...draft, token: result.token };
    tell("Relis les modifications avant de les appliquer.");
  } catch (error) { tell(errorText(error)); }
}

async function apply() {
  if (!pending.value) return;
  try {
    const result = await ws.run<{ backup: string }>("usb_ws_design_apply", pending.value);
    tell(result.backup ? "Configuration Ventoy enregistrée. La version précédente est sauvegardée." : "Aucune modification à appliquer.");
    await loadMenu(true);
  } catch (error) { tell(errorText(error)); }
}

watch([selectedDevice, () => drive.value?.ventoy], () => loadMenu());
watch([ratio, layout], () => { clearPlan(); updatePreview(); });
watch([mode, useTheme, backup], clearPlan);

onMounted(() => { loadGallery(); loadMenu(true); });
</script>

<template>
  <section class="design">
    <div class="usbws-actions">
      <NButton size="sm" :disabled="quickDisabled" :loading="savingTheme" @click="quickApply"><Palette :size="14" /> Appliquer le thème à la clé</NButton>
      <NButton variant="secondary" size="sm" @click="importTheme(false)"><FileArchive :size="14" /> Importer un ZIP de thème</NButton>
      <NButton variant="secondary" size="sm" @click="importTheme(true)"><FolderOpen :size="14" /> Importer un dossier de thème</NButton>
    </div>
    <p class="usbws-muted" role="status">{{ themeStatus }}</p>

    <div class="showcase">
      <div>
        <div class="gallery">
          <div v-for="row in themes" :key="row.id" class="card" :class="{ wide: row.id === 'popos' }">
            <button class="theme-btn" :aria-pressed="row.id === selected" @click="choose(row.id)">{{ row.name }}</button>
            <small v-if="!row.builtin">{{ row.author }} · {{ row.origin }}</small>
            <NButton v-if="row.id === 'popos' && !row.available" variant="secondary" size="sm" @click="download"><Download :size="14" /> Télécharger ce thème</NButton>
          </div>
        </div>
        <p v-if="theme?.source" class="source"><span>Source publiée</span><code>{{ theme.source }}</code></p>
      </div>
      <div>
        <div class="usbws-fields">
          <label>Format de l’aperçu
            <select v-model="ratio"><option value="16:9">16:9</option><option value="4:3">4:3</option></select>
          </label>
          <label v-if="theme?.builtin">Présentation du menu
            <select v-model="layout">
              <option value="icons">Liste avec icônes</option>
              <option value="compact">Liste compacte avec mini-icônes</option>
              <option value="text">Liste sans icônes</option>
            </select>
          </label>
          <label>Affichage au démarrage
            <select v-model="mode"><option value="">Conserver le mode</option><option value="list">Liste</option><option value="tree">Arborescence</option></select>
          </label>
        </div>
        <div class="preview" :class="{ legacy: ratio === '4:3', compact: theme?.builtin && layout === 'compact' }" :style="previewStyle">
          <img v-if="previewImage" :src="previewImage" alt="" />
          <div class="sample">
            <strong>Menu Ventoy</strong>
            <ul>
              <li v-for="(row, index) in previewRows" :key="index">
                <img v-if="row.src" :src="row.src" alt="" />
                <span :title="row.name">{{ row.name }}</span>
              </li>
            </ul>
          </div>
        </div>
        <p class="usbws-muted">{{ previewNote }}</p>
      </div>
    </div>

    <h3>Organisation du menu</h3>
    <NButton variant="secondary" size="sm" :disabled="!device" @click="loadMenu(true)"><RefreshCw :size="14" /> Actualiser le menu</NButton>
    <p v-if="!device" class="usbws-muted">Choisis une clé Ventoy pour modifier le menu.</p>
    <p v-else-if="!entries.length" class="usbws-muted">Aucune image trouvée sur la clé.</p>
    <div v-for="row in entries" :key="row.path" class="menu-row">
      <strong>{{ row.path }}</strong>
      <label class="usbws-field">Nom d’affichage
        <input type="text" maxlength="100" :placeholder="row.name" :value="currentEdit(row).alias"
          @input="edited(row, ($event.target as HTMLInputElement).value, currentEdit(row).folder)" />
      </label>
      <label class="usbws-field">Dossier
        <select :value="currentEdit(row).folder" @change="edited(row, currentEdit(row).alias, ($event.target as HTMLSelectElement).value)">
          <option value="">Conserver le dossier</option>
          <option v-for="group in groups" :key="group" :value="group">{{ group }}</option>
        </select>
      </label>
    </div>

    <h3>Sauvegardes et restauration</h3>
    <div class="usbws-fields">
      <label>Sauvegardes de configuration
        <select v-model="backup">
          <option v-if="!history.length" value="">Aucune sauvegarde</option>
          <option v-for="record in history" :key="record.id" :value="record.id">{{ record.title }} · {{ new Date(record.date).toLocaleString("fr-FR") }}</option>
        </select>
      </label>
      <NButton variant="secondary" size="sm" :disabled="!device || !history.length" @click="plan(true)"><History :size="14" /> Prévisualiser la restauration</NButton>
    </div>

    <div class="usbws-fields">
      <label class="usbws-check"><input v-model="useTheme" type="checkbox" :disabled="!theme?.available" /> Appliquer le thème sélectionné</label>
      <NButton variant="secondary" size="sm" :disabled="!device" @click="plan(false)"><Eye :size="14" /> Afficher les modifications</NButton>
      <NButton size="sm" :disabled="!pending" @click="apply"><Check :size="14" /> Appliquer les modifications</NButton>
    </div>
    <p class="usbws-muted" role="status">{{ status }}</p>
    <div class="diff">
      <div v-for="change in diff" :key="change.key + change.before" class="menu-row">
        <strong>{{ change.key }}</strong>
        <label class="usbws-field">Avant<code>{{ change.before }}</code></label>
        <label class="usbws-field">Après<code>{{ change.after }}</code></label>
      </div>
    </div>
  </section>
</template>

<style scoped>
.showcase { display: grid; grid-template-columns: minmax(0, 2fr) minmax(0, 3fr); gap: 18px; margin: 10px 0 20px; }
.gallery { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 7px; }
.card { display: grid; gap: 4px; min-width: 0; align-content: start; }
.card.wide { grid-column: 1 / -1; display: flex; align-items: center; gap: 6px; flex-wrap: wrap; padding: 7px; border: 1px solid var(--border); border-radius: 8px; }
.card small, .source { font-size: 12px; color: var(--text-muted); }
.source { display: flex; flex-wrap: wrap; gap: 0.5rem; overflow-wrap: anywhere; }
.theme-btn {
  text-align: left; white-space: normal; overflow-wrap: anywhere; background: var(--bg-secondary); color: var(--text-primary);
  border: 1px solid var(--border); border-radius: var(--radius-md); padding: 7px 10px; font-family: inherit; font-size: 13px; cursor: pointer;
}
.card.wide .theme-btn { width: auto; }
.theme-btn[aria-pressed="true"] { box-shadow: inset 0 0 0 2px var(--accent-primary); }
.usbws-field { display: flex; flex-direction: column; gap: 3px; font-size: 12px; min-width: 0; }
.usbws-field input, .usbws-field select { width: 100%; }
.preview {
  position: relative; isolation: isolate; contain: paint; width: min(100%, 520px); aspect-ratio: 16 / 9;
  background: var(--design-bg, #182f32); color: var(--design-fg, #fff8dc); border: 1px solid var(--border); border-radius: 8px; overflow: hidden; margin: 8px auto;
}
.preview.legacy { width: min(100%, 390px); aspect-ratio: 4 / 3; }
.preview > img { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; z-index: -1; }
.sample { padding: 7% 9%; min-width: 0; }
.sample > strong, .sample li:first-child { color: var(--design-accent, #e5ef73); }
.preview ul { padding: 0; list-style: none; margin: 7px 0 0; }
.preview li { display: flex; align-items: center; gap: 8px; padding: 3px 0; font-size: clamp(11px, 1.5vw, 14px); }
.preview li span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.preview li img { width: 17px; height: 17px; flex: none; image-rendering: pixelated; }
.preview.compact li { gap: 5px; padding: 1px 0; }
.preview.compact li img { width: 11px; height: 11px; }
.menu-row { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 5px 12px; padding: 8px 0; border-bottom: 1px solid var(--border); }
.menu-row > strong { grid-column: 1 / -1; overflow-wrap: anywhere; font-size: 13px; }
.diff { max-height: 280px; overflow: auto; }
@media (max-width: 720px) { .showcase, .menu-row { grid-template-columns: 1fr; } }
</style>
