<script setup lang="ts">
// Port de TokaISO 7.9 (MIT, Cédrick Roy) — file de téléchargement : pause, reprise,
// annulation, priorité, miroirs, débit, créneau horaire et dossier de destination.
import { computed, ref, watch } from "vue";
import NCard from "@/components/ui/NCard.vue";
import NButton from "@/components/ui/NButton.vue";
import NBadge from "@/components/ui/NBadge.vue";
import NModal from "@/components/ui/NModal.vue";
import { FolderOpen, Pause, Play, X, ArrowUp, Shuffle, Trash2, ShieldCheck, ShieldAlert, Clock } from "lucide-vue-next";
import { useNotificationStore } from "@/stores/notifications";
import { api, displayBytes, errorText, useIsoCatalog, STATES, type IsoJob, type Schedule } from "./useIsoCatalog";

const notifications = useNotificationStore();
const { snapshot, queueAction } = useIsoCatalog();

const jobs = computed(() => snapshot.value?.jobs ?? []);
const schedule = ref<Schedule>({ enabled: false, start: "22:00", end: "07:00", retry: false });
watch(() => snapshot.value?.schedule, s => { if (s) schedule.value = { ...s }; }, { immediate: true });

const LIMITS: [number, string][] = [[0, "Illimité"], [512, "512 Ko/s"], [1024, "1 Mo/s"], [2048, "2 Mo/s"], [5120, "5 Mo/s"], [10240, "10 Mo/s"]];

async function act(action: string, payload: Record<string, unknown> = {}, done?: string) {
  try {
    await queueAction(action, payload);
    if (done) notifications.success("Téléchargements", done);
  } catch (error) {
    notifications.error("Téléchargements", errorText(error));
  }
}

async function pickFolder() {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const dir = await open({ directory: true, title: "Dossier de téléchargement des ISO" });
  if (typeof dir === "string") await act("folder", { folder: dir }, "Dossier de téléchargement mis à jour.");
}

function eta(seconds: number | null | undefined): string {
  if (!seconds) return "";
  const h = Math.floor(seconds / 3600), m = Math.round((seconds % 3600) / 60);
  return h ? `${h} h ${m} min` : `${m} min`;
}

function percent(job: IsoJob): number {
  return job.total ? Math.min(100, (job.received / job.total) * 100) : 0;
}

function line(job: IsoJob): string {
  const retry = job.retry_at && job.state === "queued" ? `nouvel essai à ${new Date(job.retry_at * 1000).toLocaleTimeString("fr-FR")}` : "";
  return [STATES[job.state], job.received ? `${displayBytes(job.received)}${job.total ? ` / ${displayBytes(job.total)}` : ""}` : "",
    job.speed ? `${displayBytes(job.speed)}/s` : "", job.deferred && job.state === "queued" && !retry ? "hors créneau" : "", retry]
    .filter(Boolean).join(" · ");
}

// Comparaison des miroirs de même empreinte (« Tester les miroirs »).
interface MirrorRow { index: number; label: string; ok: boolean; status: string; message: string; latency_ms?: number; rate?: number }
const mirrorJob = ref<IsoJob | null>(null);
const mirrors = ref<MirrorRow[]>([]);
const mirrorBusy = ref(false);

async function compareMirrors(job: IsoJob) {
  mirrorJob.value = job;
  mirrors.value = [];
  mirrorBusy.value = true;
  try {
    mirrors.value = (await api<{ mirrors: MirrorRow[] }>("source-mirrors", { item: job.item, source: job.source_index }, 60_000)).mirrors;
  } catch (error) {
    notifications.error("Miroirs", errorText(error));
  } finally {
    mirrorBusy.value = false;
  }
}

async function useMirror(index: number) {
  if (!mirrorJob.value) return;
  await act("switch-mirror", { id: mirrorJob.value.id, source: index }, "Miroir sélectionné.");
  mirrorJob.value = null;
}
</script>

<template>
  <div class="iso-stack">
    <NCard>
      <div class="iso-stack">
        <div class="iso-row">
          <span class="iso-section-title"><FolderOpen :size="16" /> Dossier</span>
          <code class="iso-hash" style="flex: 1">{{ snapshot?.folder }}</code>
          <NButton size="sm" variant="secondary" @click="pickFolder">Changer</NButton>
        </div>
        <div class="iso-row">
          <label class="iso-label">Limite de débit
            <select class="iso-field" :value="snapshot?.limit_kib ?? 0" @change="act('bandwidth', { limit: Number(($event.target as HTMLSelectElement).value) })">
              <option v-for="[v, l] in LIMITS" :key="v" :value="v">{{ l }}</option>
            </select>
          </label>
          <label class="iso-row iso-small"><input v-model="schedule.enabled" type="checkbox" /> Télécharger seulement de</label>
          <input v-model="schedule.start" type="time" class="iso-field" />
          <span class="iso-small">à</span>
          <input v-model="schedule.end" type="time" class="iso-field" />
          <label class="iso-row iso-small"><input v-model="schedule.retry" type="checkbox" /> Réessayer automatiquement après une coupure</label>
          <NButton size="sm" variant="secondary" @click="act('schedule', { ...schedule }, 'Créneau enregistré.')"><Clock :size="14" /> Enregistrer</NButton>
        </div>
        <span v-if="snapshot?.schedule.enabled" class="iso-muted">
          {{ snapshot.schedule_open ? "Créneau ouvert : les téléchargements avancent." : "Hors créneau : les téléchargements attendent l’ouverture." }}
        </span>
      </div>
    </NCard>

    <div class="iso-row" style="justify-content: space-between">
      <span class="iso-muted">{{ jobs.length }} tâche(s){{ snapshot?.eta ? ` · fin estimée dans ${eta(snapshot.eta)}` : "" }}</span>
      <NButton size="sm" variant="ghost" @click="act('clear')"><Trash2 :size="14" /> Retirer les tâches terminées</NButton>
    </div>

    <div class="iso-list">
      <p v-if="!jobs.length" class="iso-small">La file est vide. Choisis un fichier dans le catalogue complet.</p>
      <div v-for="job in jobs" :key="job.id" class="iso-line">
        <div class="iso-line-main">
          <div class="iso-row">
            <strong>{{ job.name }}</strong>
            <NBadge v-if="job.kind === 'community'" variant="warning">Communauté</NBadge>
            <NBadge v-if="job.priority" variant="accent">Prioritaire</NBadge>
            <NBadge v-if="job.verification === 'verified'" variant="success"><ShieldCheck :size="11" /> SHA-256 vérifié</NBadge>
            <NBadge v-else-if="job.verification === 'mismatch'" variant="danger"><ShieldAlert :size="11" /> Empreinte différente</NBadge>
            <NBadge v-else-if="job.state === 'done'" variant="neutral">Empreinte non publiée</NBadge>
          </div>
          <span class="iso-muted">{{ line(job) }}</span>
          <span class="iso-muted">{{ job.mirror_used ? "Miroir de secours · " : "" }}{{ job.source }}{{ job.file ? ` · ${job.file}` : "" }}</span>
          <span v-if="job.group" class="iso-muted">Trousse · {{ job.folder.split(/[/\\]/).pop() }}</span>
          <span v-if="job.error" :class="job.verification === 'mismatch' ? 'iso-error' : 'iso-muted'">{{ job.error }}</span>
          <div v-if="job.total || job.state === 'running'" class="iso-progress" :class="{ 'iso-progress--mismatch': job.verification === 'mismatch' }">
            <span :style="{ width: `${percent(job)}%` }" />
          </div>
        </div>
        <div class="iso-row">
          <NButton v-if="['queued', 'running'].includes(job.state)" size="sm" variant="ghost" @click="act('pause', { id: job.id })"><Pause :size="14" /> Pause</NButton>
          <NButton v-if="job.state === 'queued'" size="sm" variant="ghost" @click="act('prioritize', { id: job.id })">
            <ArrowUp :size="14" /> {{ job.priority ? "Retirer la priorité" : "Prioriser" }}
          </NButton>
          <NButton v-if="['paused', 'failed'].includes(job.state) && !job.busy" size="sm" variant="secondary" @click="act('resume', { id: job.id })"><Play :size="14" /> Reprendre</NButton>
          <NButton v-if="job.mirror_count && ['queued', 'paused', 'failed'].includes(job.state) && !job.busy" size="sm" variant="ghost" @click="compareMirrors(job)">
            <Shuffle :size="14" /> Tester les miroirs
          </NButton>
          <NButton v-if="!['done', 'cancelled'].includes(job.state)" size="sm" variant="ghost" @click="act('cancel', { id: job.id })"><X :size="14" /> Annuler</NButton>
        </div>
      </div>
    </div>

    <NModal :open="!!mirrorJob" :title="`Miroirs · ${mirrorJob?.name ?? ''}`" width="640px" @close="mirrorJob = null">
      <p v-if="mirrorBusy" class="iso-small">Contrôle des miroirs de même empreinte…</p>
      <div class="iso-list">
        <div v-for="m in mirrors" :key="m.index" class="iso-line">
          <div class="iso-line-main">
            <strong>{{ m.label }}</strong>
            <span class="iso-muted" :class="m.ok ? 'iso-ok' : 'iso-warn'">
              {{ [m.message, m.latency_ms ? `${m.latency_ms} ms` : "", m.rate ? `${displayBytes(m.rate)}/s` : ""].filter(Boolean).join(" · ") }}
            </span>
          </div>
          <NButton size="sm" :disabled="!m.ok || m.index === mirrorJob?.source_index" @click="useMirror(m.index)">Utiliser</NButton>
        </div>
      </div>
    </NModal>
  </div>
</template>
