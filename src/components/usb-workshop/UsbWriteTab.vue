<script setup lang="ts">
// Port de TokaISO 7.9 (MIT, Cédrick Roy) : outils d'écriture (usb_workshop.js) et opérations sur la clé (usb_media.js).
import { computed, onMounted, ref, watch } from "vue";
import { open, save } from "@tauri-apps/plugin-dialog";
import NButton from "@/components/ui/NButton.vue";
import { useNotificationStore } from "@/stores/notifications";
import { useClipboard } from "@/composables/useClipboard";
import { FolderOpen, ExternalLink, Copy, ShieldCheck, Download } from "lucide-vue-next";
import { useUsbWorkshop, displayBytes, errorText, type UsbVolume } from "./useUsbWorkshop";

const ws = useUsbWorkshop();
const { images, device, drive, method, busy, paths } = ws;
const notifications = useNotificationStore();
const { copy } = useClipboard();

// ── Outils officiels (Rufus, balenaEtcher) ──────────────────────────────────
const writers = ref<Record<string, string>>({});

const supported = computed(() => images.value.length === 1 && images.value[0].methods.includes(method.value));
const methodInfo = computed(() => {
  const explanation = method.value === "ventoy" ? "Installe ou mets à jour Ventoy, puis ajoute plusieurs images sur la même clé." :
    method.value === "rufus" ? "Rufus s’ouvre avec ton image sélectionnée. Confirme la clé et l’écriture dans Rufus." :
      "Dans balenaEtcher, choisis Flash from file et colle le chemin de ton image.";
  const info = [explanation];
  if (method.value !== "ventoy") {
    info.push(writers.value[method.value] ? "Outil détecté." :
      "Outil introuvable : installe Rufus ou balenaEtcher depuis leur site officiel, ou indique son exécutable.");
    if (images.value.length && !supported.value) info.push("Choisis une seule image prise en charge par cet outil.");
  }
  return info.join(" ");
});

async function act(operation: () => Promise<unknown>) {
  try { await operation(); } catch (error) { ws.notify(errorText(error)); }
}

async function refreshWriters() {
  writers.value = (await ws.run<{ writers: Record<string, string> }>("usb_ws_writers", null, 18000)).writers;
}

async function pickWriter() {
  const tool = method.value;
  const chosen = await open({ multiple: false, filters: [{ name: "Exécutable", extensions: ["exe"] }] });
  if (typeof chosen !== "string") return;
  await ws.run("usb_ws_set_writer", { tool, path: chosen });
  await refreshWriters();
}

async function launchWriter() {
  const data = await ws.run<{ preselected: boolean }>("usb_ws_open_writer",
    { tool: method.value, path: paths.value[0], id: device.value }, 45000);
  ws.notify(data.preselected ? "Rufus a reçu l’image. Vérifie le support dans sa fenêtre." :
    "balenaEtcher est ouvert. Utilise Flash from file pour sélectionner ton image.");
}

async function copyPath() {
  const image = paths.value[0];
  if (image && await copy(image)) ws.notify("Chemin copié.");
}

// ── Opérations sur la clé (UsbMedia) ────────────────────────────────────────
type Operation = "install" | "copy" | "update" | "verify-ventoy" | "verify-raw" | "storage";
const operations: Record<Operation, string> = {
  install: "Installer Ventoy",
  copy: "Copier les images sur Ventoy",
  update: "Mettre à jour Ventoy",
  "verify-ventoy": "Vérifier les images Ventoy",
  "verify-raw": "Relire une clé écrite en image brute",
  storage: "Remettre la clé en stockage",
};
const writeActions: Operation[] = ["install", "update", "storage"];
const statusLabels: Record<string, string> = { match: "Empreinte conforme", different: "Empreinte différente", missing: "Fichier manquant" };
const eraseText = "Cette opération efface les données de la clé sélectionnée.";
const summaries: Record<Operation, string> = {
  install: "L’installation efface les données de la clé. Les images sont facultatives.",
  copy: "Les images seront copiées dans le dossier TokaISO de la clé.",
  update: "Ventoy mettra à jour ses fichiers de démarrage. Les images restent sur la partition de données.",
  "verify-ventoy": "Compare les fichiers du dossier TokaISO aux images choisies ou aux rapports de copie de la clé.",
  "verify-raw": "Pour une image écrite avec balenaEtcher ou le mode DD de Rufus.",
  storage: eraseText,
};

interface PlanDevice { name: string; number: number; drive: string; size: number; free: number; format: string;
  partitionStyle: string; hardwareSerial: string; volumes: UsbVolume[] }
interface Plan { token: string; confirm: string; operation: Operation; erases: boolean; device: PlanDevice; paths: { name: string; size: number }[] }
interface Line { title: string; detail: string }
interface MediaResult {
  prepared?: boolean; updated?: boolean; restored?: boolean; prepared_id?: string; report?: string;
  files?: { name: string; sha256?: string; status?: string; expected?: string; actual?: string | null }[];
  match?: boolean; size?: number; expected?: string; actual?: string;
}

const operation = ref<Operation>("install");
const filesystem = ref("exFAT");
const rawMode = ref("");
const gpt = ref(false);
const typed = ref("");
const valid = ref<{ request: string; payload: Record<string, unknown>; plan: Plan } | null>(null);
const checking = ref(false);
const resultMessage = ref("");
const resultRows = ref<Line[]>([]);
const report = ref("");
let previousMethod = method.value;

const available = computed<Operation[]>(() =>
  [...(drive.value?.ventoy ? (["copy", "update", "verify-ventoy"] as Operation[]) : (["install"] as Operation[])), "verify-raw", "storage"]);

function settings() {
  return { id: device.value, operation: operation.value, paths: paths.value,
    gpt: gpt.value, filesystem: filesystem.value, mode: rawMode.value };
}

const needsConfirmation = computed(() => !!valid.value && writeActions.includes(valid.value.plan.operation));
const canApply = computed(() => !busy.value && !!valid.value &&
  (!needsConfirmation.value || typed.value.trim().toUpperCase() === valid.value.plan.confirm));

function settingsChanged() {
  valid.value = null;
  typed.value = "";
}

function selectionChanged() {
  if (busy.value) return;
  if (method.value !== previousMethod && (method.value === "etcher" || method.value === "rufus")) operation.value = "verify-raw";
  else if (!available.value.includes(operation.value)) operation.value = available.value[0];
  previousMethod = method.value;
  if (method.value === "etcher" && !rawMode.value) rawMode.value = "etcher";
  settingsChanged();
}

watch([device, method, paths, () => drive.value?.ventoy], selectionChanged);
watch([operation, filesystem, rawMode, gpt], settingsChanged);

async function check() {
  if (busy.value) return;
  settingsChanged();
  report.value = "";
  resultMessage.value = "";
  resultRows.value = [];
  checking.value = true;
  try {
    const payload = settings();
    const request = JSON.stringify(payload);
    const plan = await ws.run<Plan>("usb_ws_media_preview", payload, 30000);
    if (JSON.stringify(settings()) !== request) return;
    valid.value = { request, payload, plan };
  } catch (error) {
    ws.notify(errorText(error));
  } finally {
    checking.value = false;
  }
}

async function commit() {
  if (!valid.value || !canApply.value) return;
  const { request, payload, plan } = valid.value;
  if (JSON.stringify(settings()) !== request) { settingsChanged(); return; }
  const confirmation = typed.value.trim().toUpperCase();
  const timeout = ["update", "storage"].includes(plan.operation) ? 330000 : 14400000;
  try {
    const result = await ws.run<MediaResult>("usb_ws_media_run", { ...payload, token: plan.token, confirm: confirmation }, timeout);
    valid.value = null;
    if (result.prepared || result.updated || result.restored) await act(() => ws.refreshDevices(result.prepared_id));
    renderResult(result, plan);
  } catch (error) {
    ws.notify(errorText(error));
    notifications.error("Atelier USB", errorText(error));
    if (writeActions.includes(plan.operation)) {
      valid.value = null;
      await act(() => ws.refreshDevices());
    }
  } finally {
    typed.value = "";
  }
}

function renderResult(result: MediaResult, plan: Plan) {
  const lines = [operations[plan.operation], plan.device.name];
  const rows: Line[] = [];
  let message: string;
  report.value = "";
  if (plan.operation === "verify-raw") {
    message = result.match ? "La clé correspond à l’image brute." : "La clé diffère de l’image brute.";
    rows.push({ title: "Données relues", detail: displayBytes(result.size) }, { title: "SHA-256", detail: result.actual ?? "" });
    lines.push(message, "Empreinte attendue · " + result.expected, "Empreinte relue · " + result.actual);
    report.value = lines.join("\n");
  } else if (plan.operation === "verify-ventoy") {
    message = result.match ? "Les images Ventoy sont conformes." : "Des images Ventoy manquent ou diffèrent.";
    for (const row of result.files ?? []) {
      const label = statusLabels[row.status ?? ""] ?? "";
      rows.push({ title: row.name, detail: label });
      lines.push(row.name + " · " + label, "Empreinte attendue · " + (row.expected ?? ""));
      if (row.actual) lines.push("Empreinte relue · " + row.actual);
    }
    report.value = [message, ...lines].join("\n");
  } else if (result.restored) {
    message = "La clé est prête pour le stockage.";
  } else if (result.updated) {
    message = "Ventoy est à jour sur la clé.";
  } else if (result.prepared && !result.files?.length) {
    message = "Ventoy est installé. Tu peux ajouter des images ou choisir un thème.";
  } else {
    const files = result.files ?? [];
    message = `${files.length} image(s) copiée(s) et vérifiée(s).`;
    rows.push(...files.map((row) => ({ title: row.name, detail: "SHA-256 · " + row.sha256 })));
    if (result.report) rows.push({ title: "Rapport d’empreintes", detail: result.report });
  }
  resultMessage.value = message;
  resultRows.value = rows;
  ws.notify(message);
  if (plan.operation.startsWith("verify") && !result.match) notifications.warning("Atelier USB", message);
  else notifications.success("Atelier USB", message);
}

async function exportReport() {
  if (!report.value) return;
  const target = await save({ defaultPath: "verification-usb.txt", filters: [{ name: "Texte", extensions: ["txt"] }] });
  if (!target) return;
  await ws.run("usb_ws_save_text", { path: target, text: "NiTriTe · Atelier USB\n" + report.value });
  ws.notify("Résultat exporté.");
}

function volumes(list: UsbVolume[]) {
  return list.map((v) => [v.drive, v.label, v.format, displayBytes(v.size)].filter(Boolean).join(" ")).join(" · ");
}

onMounted(() => { act(refreshWriters); selectionChanged(); });
</script>

<template>
  <section>
    <div class="usbws-fields">
      <label>Méthode
        <select v-model="method" :disabled="busy">
          <option value="ventoy">Ventoy</option>
          <option value="rufus">Rufus</option>
          <option value="etcher">balenaEtcher</option>
        </select>
      </label>
      <template v-if="method !== 'ventoy'">
        <NButton variant="secondary" size="sm" :disabled="busy" @click="act(pickWriter)"><FolderOpen :size="14" /> Indiquer l’exécutable</NButton>
        <NButton size="sm" :disabled="busy || !supported || !drive || !writers[method]" @click="act(launchWriter)"><ExternalLink :size="14" /> Ouvrir l’outil</NButton>
        <NButton v-if="method === 'etcher'" variant="secondary" size="sm" :disabled="!supported" @click="act(copyPath)"><Copy :size="14" /> Copier le chemin de l’image</NButton>
      </template>
    </div>
    <p class="usbws-muted">{{ methodInfo }}</p>

    <details open>
      <summary>Préparer, vérifier ou formater la clé</summary>
      <template v-if="drive">
        <div class="usbws-row">
          <div class="usbws-row-main">
            <strong>{{ drive.name || "Clé USB" }}</strong>
            <span>{{ [drive.drive || `Disque ${drive.number}`, displayBytes(drive.size), drive.partitionStyle, drive.format].filter(Boolean).join(" · ") }}</span>
          </div>
        </div>
        <p v-if="drive.hardwareSerial" class="usbws-muted">Numéro de série · {{ drive.hardwareSerial }}</p>
        <p v-if="drive.volumes?.length" class="usbws-muted">Volumes · {{ volumes(drive.volumes) }}</p>
      </template>
      <p v-else class="usbws-muted">Choisis une clé USB détectée par Windows.</p>

      <div class="usbws-fields">
        <label>Opération USB
          <select v-model="operation" :disabled="busy">
            <option v-for="key in available" :key="key" :value="key">{{ operations[key] }}</option>
          </select>
        </label>
        <label v-if="operation === 'install'" class="usbws-check"><input v-model="gpt" type="checkbox" :disabled="busy" /> Utiliser GPT pour Ventoy</label>
        <label v-if="operation === 'storage'">Format de stockage
          <select v-model="filesystem" :disabled="busy"><option value="exFAT">exFAT</option><option value="NTFS">NTFS</option></select>
        </label>
        <label v-if="operation === 'verify-raw'">Écriture utilisée
          <select v-model="rawMode" :disabled="busy">
            <option value="">Mode à confirmer</option>
            <option value="etcher">balenaEtcher</option>
            <option value="rufus-dd">Mode DD de Rufus</option>
          </select>
        </label>
        <NButton variant="secondary" size="sm" :loading="checking" :disabled="busy || !device" @click="check"><ShieldCheck :size="14" /> Vérifier le plan USB</NButton>
      </div>
      <p class="usbws-muted">{{ summaries[operation] }}</p>

      <div v-if="valid" class="usbws-plan">
        <h3>Ce qui va se passer</h3>
        <p v-if="valid.plan.erases" class="usbws-warning">{{ eraseText }}</p>
        <p class="usbws-muted">
          {{ operations[valid.plan.operation] }} sur {{ valid.plan.device.name }} ·
          {{ valid.plan.device.drive || `Disque ${valid.plan.device.number}` }} · {{ displayBytes(valid.plan.device.size) }}
          <template v-if="valid.plan.device.hardwareSerial"> · série {{ valid.plan.device.hardwareSerial }}</template>
        </p>
        <div v-for="row in valid.plan.paths" :key="row.name" class="usbws-row">
          <div class="usbws-row-main"><strong>{{ row.name }}</strong><span>{{ displayBytes(row.size) }}</span></div>
        </div>
        <div class="usbws-fields">
          <label v-if="needsConfirmation">Tape la lettre ou le numéro de la clé ({{ valid.plan.confirm }})
            <input v-model="typed" type="text" autocomplete="off" :placeholder="valid.plan.confirm" :disabled="busy" />
          </label>
          <NButton :variant="valid.plan.erases ? 'danger' : 'primary'" size="sm" :disabled="!canApply" :loading="busy && !checking" @click="commit">
            Confirmer et continuer
          </NButton>
        </div>
      </div>

      <div v-if="resultMessage" class="usbws-result">
        <p><strong>{{ resultMessage }}</strong></p>
        <div v-for="(row, index) in resultRows" :key="index" class="usbws-row">
          <div class="usbws-row-main"><strong>{{ row.title }}</strong><code>{{ row.detail }}</code></div>
        </div>
        <NButton v-if="report" variant="secondary" size="sm" :disabled="busy" @click="act(exportReport)"><Download :size="14" /> Exporter le résultat</NButton>
      </div>
    </details>
  </section>
</template>

<style scoped>
.usbws-plan { border-top: 1px solid var(--border); margin-top: 8px; padding-top: 4px; }
.usbws-result { margin-top: 10px; }
</style>
