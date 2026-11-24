<script setup lang="ts">
// Port de TokaISO 7.9 (MIT, Cédrick Roy) : diagnostic réseau et disques (extras.js #network-dialog, #disk-dialog).
import { computed, onMounted, reactive, ref } from "vue";
import NCard from "@/components/ui/NCard.vue";
import NButton from "@/components/ui/NButton.vue";
import NBadge from "@/components/ui/NBadge.vue";
import { Activity, HardDrive, Copy } from "lucide-vue-next";
import { useNotificationStore } from "@/stores/notifications";
import { call, displayBytes, pickFiles } from "./useBootLab";

interface Check {
  kind: string; host: string; port: number | null; elapsed: number; ok: boolean; label?: string;
  status?: string; output?: string; average?: number | null; sent?: number; received?: number; addresses?: string[]; address?: string;
}
interface Run { date: Date; checks: Check[] }
interface Drive { DeviceID: string; VolumeName: string; FileSystem: string; Size: number; FreeSpace: number; DriveType: number }
interface Report { drives: Drive[]; disks: Record<string, unknown>[]; partitions: Record<string, unknown>[]; health: Record<string, unknown>[] }
interface SpaceCheck { bytes: number; fits: boolean; fat32: string[]; drive: Drive }

const props = defineProps<{ preset?: { host: string; port: number } | null }>();
const notify = useNotificationStore();
const names: Record<string, string> = { overview: "Vue d’ensemble", ping: "Ping", traceroute: "Traceroute", dns: "DNS", tcp: "Port TCP" };
const roles: Record<string, string> = { gateway: "Passerelle", internet: "Internet", server: "Serveur" };
const form = reactive({ test: "overview", host: props.preset?.host ?? "", port: props.preset?.port ?? 443, internet: "1.1.1.1" });
const runs = ref<Run[]>([]);
const running = ref(false);
const report = ref<Report | null>(null);
const target = ref("");
const sizeGib = ref<number | null>(null);
const diskFile = ref("");
const space = ref<SpaceCheck | null>(null);

const rows = computed(() => runs.value.flatMap((run) => run.checks.map((check) => ({ run, check }))));

function title(c: Check): string {
  return [roles[c.label ?? ""], names[c.kind], c.host, c.port].filter(Boolean).join(" · ");
}

function summary(c: Check): string {
  if (c.kind === "ping" && c.received !== undefined) {
    return `${c.received} / ${c.sent} réponses` + (c.average != null ? ` · ${c.average} ms` : "") + (c.output ? ` · ${c.output}` : "");
  }
  if (c.kind === "dns") return c.ok ? (c.addresses ?? []).join(", ") : "Nom non résolu";
  if (c.kind === "tcp") return c.ok ? `Port ouvert${c.address ? " · " + c.address : ""}` : `Port fermé ou filtré · ${c.status ?? ""}`;
  return c.output ?? c.status ?? "";
}

async function runTest() {
  running.value = true;
  const result = await call<{ checks: Check[] }>("boot_lab_network_test", { request: { ...form } }, 42000);
  running.value = false;
  if (!result) return;
  runs.value = [{ date: new Date(), checks: result.checks }, ...runs.value].slice(0, 10);
  notify.success("Test terminé.");
}

async function copyReport() {
  const text = ["Diagnostic réseau · NiTriTe", ...runs.value.flatMap((run) => [run.date.toISOString(),
    ...run.checks.map((c) => `${title(c)} · ${c.ok ? "Réussi" : "Échec"} · ${c.elapsed} s · ${summary(c)}`)])].join("\n");
  try {
    await navigator.clipboard.writeText(text);
    notify.success("Rapport copié");
  } catch (error) {
    notify.warning("Copie impossible", String(error));
  }
}

async function loadDisks() {
  report.value = await call<Report>("boot_lab_drive_report", undefined, 25000);
  if (report.value && !report.value.drives.some((d) => d.DeviceID === target.value)) target.value = report.value.drives[0]?.DeviceID ?? "";
}

async function pickDiskFile() {
  const [path] = await pickFiles("Fichier", []);
  if (path) diskFile.value = path;
}

async function checkSpace() {
  const sizes = sizeGib.value ? [{ name: "Estimation", size: Math.round(sizeGib.value * 1024 ** 3) }] : [];
  space.value = await call<SpaceCheck>("boot_lab_disk_check",
    { drive: target.value, paths: diskFile.value ? [diskFile.value] : [], sizes }, 25000);
}

onMounted(loadDisks);
</script>

<template>
  <div class="bl-panel">
    <NCard>
      <div class="bl-panel">
        <div class="bl-title"><Activity :size="16" /> Diagnostic réseau</div>
        <div class="bl-fields">
          <label>Test
            <select v-model="form.test">
              <option v-for="(label, key) in names" :key="key" :value="key">{{ label }}</option>
            </select>
          </label>
          <label>IP ou nom du serveur <input v-model="form.host" maxlength="253" placeholder="127.0.0.1" /></label>
          <label v-if="form.test === 'tcp' || form.test === 'overview'">Port TCP <input v-model.number="form.port" type="number" min="1" max="65535" /></label>
          <label v-if="form.test === 'overview'">Cible Internet (TCP 443) <input v-model="form.internet" maxlength="253" /></label>
          <NButton :disabled="!form.host.trim()" :loading="running" @click="runTest">Lancer le test</NButton>
          <NButton variant="secondary" :disabled="!runs.length" @click="copyReport"><Copy :size="14" /> Copier le rapport</NButton>
        </div>
        <div class="bl-list">
          <div v-for="(row, i) in rows" :key="i" class="bl-row">
            <div class="bl-row-main">
              <span class="bl-row-name">{{ title(row.check) }}</span>
              <span class="bl-muted" style="white-space: pre-wrap">{{ summary(row.check) }}</span>
            </div>
            <NBadge :variant="row.check.ok ? 'success' : 'danger'">{{ row.check.ok ? "Réussi" : "Échec" }} · {{ row.check.elapsed }} s</NBadge>
          </div>
        </div>
      </div>
    </NCard>

    <NCard>
      <div class="bl-panel">
        <div class="bl-title"><HardDrive :size="16" /> Disques et clés USB <span class="bl-muted">(lecture seule)</span></div>
        <div class="bl-list">
          <div v-for="d in report?.drives ?? []" :key="d.DeviceID" class="bl-row">
            <div class="bl-row-main">
              <span class="bl-row-name">{{ d.DeviceID }} {{ d.VolumeName }}</span>
              <span class="bl-muted">{{ d.FileSystem || "?" }} · {{ displayBytes(d.FreeSpace) }} libres sur {{ displayBytes(d.Size) }}</span>
            </div>
            <NBadge :variant="d.DriveType === 2 ? 'info' : 'neutral'">{{ d.DriveType === 2 ? "Amovible" : d.DriveType === 4 ? "Réseau" : "Local" }}</NBadge>
          </div>
        </div>
        <div class="bl-fields">
          <label>Volume de destination
            <select v-model="target"><option v-for="d in report?.drives ?? []" :key="d.DeviceID" :value="d.DeviceID">{{ d.DeviceID }}</option></select>
          </label>
          <label>Taille estimée (Gio) <input v-model.number="sizeGib" type="number" step="0.01" min="0" max="100000" placeholder="4,5" /></label>
          <NButton variant="secondary" @click="pickDiskFile">Choisir un fichier</NButton>
          <NButton :disabled="!target" @click="checkSpace">Vérifier l’espace</NButton>
        </div>
        <p v-if="diskFile" class="bl-muted">{{ diskFile }}</p>
        <p v-if="space" class="bl-text" role="status" :class="space.fits && !space.fat32.length ? 'bl-ok' : 'bl-ko'">
          {{ displayBytes(space.bytes) }} à copier ·
          {{ space.fits ? "L’espace libre suffit." : "Espace libre insuffisant." }}
          <template v-if="space.fat32.length"> · Dépasse la limite FAT32 de 4 Gio : {{ space.fat32.join(", ") }}</template>
        </p>
        <details v-if="report">
          <summary class="bl-text">Partitions et état SMART</summary>
          <div class="bl-list" style="margin-top: 8px">
            <div v-for="(h, i) in report.health" :key="'h' + i" class="bl-row">
              <div class="bl-row-main">
                <span class="bl-row-name">{{ h.name }} · {{ h.bus }}</span>
                <span class="bl-muted">
                  État {{ h.health || "?" }}{{ h.temperature != null ? ` · ${h.temperature} °C` : "" }}{{ h.wear != null ? ` · usure ${h.wear} %` : "" }}{{ h.hours != null ? ` · ${h.hours} h` : "" }}{{ h.readErrors != null ? ` · erreurs lecture ${h.readErrors}` : "" }}{{ h.writeErrors != null ? ` · erreurs écriture ${h.writeErrors}` : "" }}
                </span>
              </div>
            </div>
            <div v-for="(p, i) in report.partitions" :key="'p' + i" class="bl-row">
              <span class="bl-muted">Disque {{ p.DiskNumber }} · partition {{ p.PartitionNumber }} {{ p.DriveLetter ? "(" + p.DriveLetter + ":)" : "" }} · {{ p.Type }} · {{ displayBytes(Number(p.Size)) }}</span>
            </div>
          </div>
        </details>
      </div>
    </NCard>
  </div>
</template>
