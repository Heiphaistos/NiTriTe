<script setup lang="ts">
import { scaledInterval } from "@/utils/perfProfile";
import { ref, onMounted, onUnmounted, computed } from "vue";
import { invoke } from "@/utils/invoke";
import NSpinner from "@/components/ui/NSpinner.vue";
import { RefreshCw, Cpu, HardDrive, Wifi, Activity } from "lucide-vue-next";

interface DiskPerf { name: string; read_mb: number; write_mb: number; queue_length: number; }
interface NetPerf { name: string; recv_mb: number; sent_mb: number; }
interface PerfSnapshot {
  cpu_percent: number; ram_used_gb: number; ram_total_gb: number; ram_percent: number;
  page_file_used_gb: number; page_file_total_gb: number;
  disks: DiskPerf[]; network: NetPerf[];
  handle_count: number; thread_count: number; process_count: number; uptime_hours: number;
}

const MAX_HISTORY = 30;

const data        = ref<PerfSnapshot | null>(null);
const loading     = ref(true);
const error       = ref("");
const autoRefresh = ref(false);
let timer: ReturnType<typeof setInterval> | null = null;

// Historique sparkline
const cpuHistory = ref<number[]>([]);
const ramHistory = ref<number[]>([]);

async function load() {
  loading.value = true;
  try {
    data.value = await invoke<PerfSnapshot>("get_perf_snapshot");
    error.value = "";
    // Store history
    if (data.value) {
      cpuHistory.value = [...cpuHistory.value, data.value.cpu_percent].slice(-MAX_HISTORY);
      ramHistory.value = [...ramHistory.value, data.value.ram_percent].slice(-MAX_HISTORY);
    }
  } catch (e: unknown) { error.value = (e instanceof Error ? e.message : String(e)) || "Erreur"; }
  finally { loading.value = false; }
}

function toggleAuto() {
  autoRefresh.value = !autoRefresh.value;
  if (autoRefresh.value) { timer = setInterval(load, scaledInterval(3000)); }
  else { if (timer) { clearInterval(timer); timer = null; } }
}

function pctColor(p: number) {
  if (p >= 90) return "#ef4444";
  if (p >= 70) return "#f59e0b";
  return "#22c55e";
}
function uptimeStr(h: number) {
  if (h >= 24) return `${(h / 24).toFixed(1)} jours`;
  return `${h.toFixed(1)} h`;
}

// Sparkline SVG generator
function sparklinePath(values: number[], w = 160, h = 32): string {
  if (values.length < 2) return "";
  const max = Math.max(...values, 1);
  const pts = values.map((v, i) => {
    const x = (i / (values.length - 1)) * w;
    const y = h - (v / max) * h;
    return `${x.toFixed(1)},${y.toFixed(1)}`;
  });
  return `M ${pts.join(" L ")}`;
}

function sparklineArea(values: number[], w = 160, h = 32): string {
  if (values.length < 2) return "";
  const max = Math.max(...values, 1);
  const pts = values.map((v, i) => {
    const x = (i / (values.length - 1)) * w;
    const y = h - (v / max) * h;
    return `${x.toFixed(1)},${y.toFixed(1)}`;
  });
  const first = `${(0).toFixed(1)},${h}`;
  const last  = `${w.toFixed(1)},${h}`;
  return `M ${first} L ${pts.join(" L ")} L ${last} Z`;
}

const cpuAvg = computed(() =>
  cpuHistory.value.length ? Math.round(cpuHistory.value.reduce((a, b) => a + b, 0) / cpuHistory.value.length) : 0
);
const cpuPeak = computed(() =>
  cpuHistory.value.length ? Math.round(Math.max(...cpuHistory.value)) : 0
);
const ramAvg = computed(() =>
  ramHistory.value.length ? Math.round(ramHistory.value.reduce((a, b) => a + b, 0) / ramHistory.value.length) : 0
);
const ramPeak = computed(() =>
  ramHistory.value.length ? Math.round(Math.max(...ramHistory.value)) : 0
);

onMounted(load);
onUnmounted(() => { if (timer) clearInterval(timer); });
</script>

<template>
  <!-- Loading initial -->
  <div v-if="loading && !data" class="perf-loading">
    <NSpinner :size="16" /><span>Collecte des métriques système...</span>
  </div>
  <div v-else-if="error && !data" class="perf-error">⚠ {{ error }}</div>

  <div v-else-if="data" class="perf-root">

    <!-- Banner -->
    <div class="perf-banner">
      <div class="perf-banner-icon"><Activity :size="22" /></div>
      <div class="perf-banner-body">
        <div class="perf-banner-title">Performances en temps réel</div>
        <div class="perf-banner-desc">CPU · RAM · I/O Disques · Réseau · Historique {{ cpuHistory.length }}/{{ 30 }} pts</div>
      </div>
      <div class="perf-actions">
        <button class="diag-btn diag-btn-primary" @click="load" :disabled="loading">
          <RefreshCw :size="13" :class="loading ? 'spin' : ''" /> Actualiser
        </button>
        <button class="diag-btn" :class="autoRefresh ? 'btn-auto-on' : ''" @click="toggleAuto">
          {{ autoRefresh ? '⏹ Stop auto' : '▶ Auto 3s' }}
        </button>
        <span v-if="loading && data" class="perf-updating">Mise à jour...</span>
      </div>
    </div>

    <!-- CPU + RAM big cards with sparklines -->
    <div class="perf-duo">
      <!-- CPU -->
      <div class="perf-metric-card">
        <div class="pmc-header">
          <Cpu :size="15" class="pmc-icon" />
          <span class="pmc-title">Processeur</span>
          <div class="pmc-stats">
            <span class="pmc-stat">Moy <strong>{{ cpuAvg }}%</strong></span>
            <span class="pmc-stat">Pic <strong :style="{color:pctColor(cpuPeak)}">{{ cpuPeak }}%</strong></span>
          </div>
        </div>
        <div class="pmc-val" :style="{ color: pctColor(data.cpu_percent) }">
          {{ data.cpu_percent.toFixed(1) }}<span class="pmc-unit">%</span>
        </div>
        <div class="pmc-bar-track">
          <div class="pmc-bar-fill" :style="{ width: data.cpu_percent + '%', background: pctColor(data.cpu_percent) }" />
        </div>
        <!-- Sparkline -->
        <div v-if="cpuHistory.length >= 2" class="spark-wrap">
          <svg viewBox="0 0 160 32" width="100%" height="32" preserveAspectRatio="none">
            <defs>
              <linearGradient id="cpu-grad" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" :stop-color="pctColor(data.cpu_percent)" stop-opacity="0.3"/>
                <stop offset="100%" :stop-color="pctColor(data.cpu_percent)" stop-opacity="0.02"/>
              </linearGradient>
            </defs>
            <path :d="sparklineArea(cpuHistory)" fill="url(#cpu-grad)" />
            <path :d="sparklinePath(cpuHistory)" :stroke="pctColor(data.cpu_percent)" stroke-width="1.5" fill="none" />
          </svg>
        </div>
      </div>

      <!-- RAM -->
      <div class="perf-metric-card">
        <div class="pmc-header">
          <HardDrive :size="15" class="pmc-icon" />
          <span class="pmc-title">Mémoire RAM</span>
          <span class="pmc-sub">{{ data.ram_used_gb.toFixed(1) }} / {{ data.ram_total_gb.toFixed(1) }} GB</span>
          <div class="pmc-stats">
            <span class="pmc-stat">Moy <strong>{{ ramAvg }}%</strong></span>
            <span class="pmc-stat">Pic <strong :style="{color:pctColor(ramPeak)}">{{ ramPeak }}%</strong></span>
          </div>
        </div>
        <div class="pmc-val" :style="{ color: pctColor(data.ram_percent) }">
          {{ data.ram_percent.toFixed(1) }}<span class="pmc-unit">%</span>
        </div>
        <div class="pmc-bar-track">
          <div class="pmc-bar-fill" :style="{ width: data.ram_percent + '%', background: pctColor(data.ram_percent) }" />
        </div>
        <!-- Sparkline -->
        <div v-if="ramHistory.length >= 2" class="spark-wrap">
          <svg viewBox="0 0 160 32" width="100%" height="32" preserveAspectRatio="none">
            <defs>
              <linearGradient id="ram-grad" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" :stop-color="pctColor(data.ram_percent)" stop-opacity="0.3"/>
                <stop offset="100%" :stop-color="pctColor(data.ram_percent)" stop-opacity="0.02"/>
              </linearGradient>
            </defs>
            <path :d="sparklineArea(ramHistory)" fill="url(#ram-grad)" />
            <path :d="sparklinePath(ramHistory)" :stroke="pctColor(data.ram_percent)" stroke-width="1.5" fill="none" />
          </svg>
        </div>
      </div>
    </div>

    <!-- Sys stats -->
    <div class="card-block">
      <p class="diag-section-label" style="margin:0 0 12px 0">Informations système</p>
      <div class="info-grid">
        <div class="info-row"><span>Uptime</span><span>{{ uptimeStr(data.uptime_hours) }}</span></div>
        <div class="info-row"><span>Processus actifs</span><span>{{ data.process_count }}</span></div>
        <div class="info-row"><span>Threads</span><span>{{ data.thread_count.toLocaleString() }}</span></div>
        <div class="info-row"><span>Handles ouverts</span><span>{{ data.handle_count.toLocaleString() }}</span></div>
        <div class="info-row"><span>Fichier page (swap)</span>
          <span>{{ data.page_file_used_gb.toFixed(1) }} / {{ data.page_file_total_gb.toFixed(1) }} GB</span>
        </div>
      </div>
    </div>

    <!-- Disques I/O -->
    <div v-if="data.disks.length" class="card-block">
      <p class="diag-section-label" style="margin:0 0 12px 0"><HardDrive :size="13" /> I/O Disques</p>
      <div v-for="(d, i) in data.disks" :key="i" class="perf-io-row">
        <span class="perf-io-name">{{ d.name }}</span>
        <div class="perf-io-metrics">
          <span class="perf-io-chip io-read">↓ {{ d.read_mb.toFixed(2) }} MB/s</span>
          <span class="perf-io-chip io-write">↑ {{ d.write_mb.toFixed(2) }} MB/s</span>
          <span class="perf-io-chip io-queue" :class="{ 'io-queue-warn': d.queue_length > 5 }">Q: {{ d.queue_length.toFixed(1) }}</span>
        </div>
      </div>
    </div>

    <!-- Réseau I/O -->
    <div v-if="data.network.length" class="card-block">
      <p class="diag-section-label" style="margin:0 0 12px 0"><Wifi :size="13" /> I/O Réseau</p>
      <div v-for="(n, i) in data.network" :key="i" class="perf-io-row">
        <span class="perf-io-name perf-net-name">{{ n.name }}</span>
        <div class="perf-io-metrics">
          <span class="perf-io-chip io-read">↓ {{ (n.recv_mb * 1000).toFixed(1) }} KB/s</span>
          <span class="perf-io-chip io-write">↑ {{ (n.sent_mb * 1000).toFixed(1) }} KB/s</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.perf-root { display: flex; flex-direction: column; gap: 14px; }

.perf-loading { display: flex; align-items: center; gap: 10px; padding: 20px; font-size: 13px; color: var(--text-secondary); }
.perf-error { padding: 16px; color: var(--danger); background: var(--danger-muted); border-radius: 10px; border: 1px solid rgba(239,68,68,.3); }

/* Banner */
.perf-banner { display: flex; align-items: center; gap: 14px; padding: 16px 20px;
  background: linear-gradient(135deg, rgba(249,115,22,.12), rgba(234,88,12,.06));
  border: 1px solid rgba(249,115,22,.25); border-radius: 14px; }
.perf-banner-icon { width: 44px; height: 44px; border-radius: 11px; background: linear-gradient(135deg,var(--accent-primary),#ea580c);
  display: flex; align-items: center; justify-content: center; color: #fff; flex-shrink: 0; box-shadow: 0 4px 12px rgba(249,115,22,.35); }
.perf-banner-body { flex: 1; }
.perf-banner-title { font-size: 16px; font-weight: 700; color: var(--text-primary); margin-bottom: 2px; }
.perf-banner-desc { font-size: 12px; color: var(--text-secondary); }
.perf-actions { display: flex; align-items: center; gap: 8px; }
.perf-updating { font-size: 11px; color: var(--text-muted); }
.btn-auto-on { background: rgba(249,115,22,.2) !important; color: var(--accent-primary) !important; border-color: rgba(249,115,22,.4) !important; }

/* CPU / RAM big cards */
.perf-duo { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
.perf-metric-card { background: var(--bg-secondary); border: 1px solid var(--border-hover); border-radius: 14px; padding: 18px 20px; }
.pmc-header { display: flex; align-items: center; gap: 8px; margin-bottom: 10px; flex-wrap: wrap; }
.pmc-icon { color: var(--text-secondary); }
.pmc-title { font-size: 13px; font-weight: 600; color: var(--text-primary); }
.pmc-sub { font-size: 11px; color: var(--text-secondary); margin-left: auto; }
.pmc-stats { display: flex; gap: 8px; margin-left: auto; }
.pmc-stat { font-size: 10px; color: var(--text-muted); }
.pmc-stat strong { font-size: 11px; }
.pmc-val { font-size: 36px; font-weight: 800; line-height: 1; margin-bottom: 12px; }
.pmc-unit { font-size: 20px; font-weight: 600; opacity: .7; }
.pmc-bar-track { height: 8px; background: var(--bg-elevated); border-radius: 99px; overflow: hidden; border: 1px solid var(--border-hover); }
.pmc-bar-fill { height: 100%; border-radius: 99px; transition: width .5s ease; }
.spark-wrap { margin-top: 10px; border-radius: 4px; overflow: hidden; }

/* I/O rows */
.perf-io-row { display: flex; align-items: center; gap: 12px; padding: 8px 0; border-bottom: 1px solid var(--border); flex-wrap: wrap; }
.perf-io-row:last-child { border-bottom: none; }
.perf-io-name { font-size: 13px; font-weight: 500; color: var(--text-primary); flex-shrink: 0; min-width: 80px; }
.perf-net-name { font-size: 11px; color: var(--text-secondary); max-width: 220px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.perf-io-metrics { display: flex; gap: 8px; flex-wrap: wrap; margin-left: auto; }
.perf-io-chip { font-size: 11px; font-family: "JetBrains Mono", monospace; padding: 3px 9px; border-radius: 6px; font-weight: 500; }
.io-read  { background: rgba(34,197,94,.1);  color: #22c55e; border: 1px solid rgba(34,197,94,.2); }
.io-write { background: rgba(59,130,246,.1); color: #60a5fa; border: 1px solid rgba(59,130,246,.2); }
.io-queue { background: var(--bg-tertiary); color: var(--text-secondary); border: 1px solid var(--border-hover); }
.io-queue-warn { background: rgba(249,115,22,.1); color: var(--warning); border-color: rgba(249,115,22,.3); }

@keyframes spin { to { transform: rotate(360deg); } }
.spin { animation: spin 1s linear infinite; }
</style>
