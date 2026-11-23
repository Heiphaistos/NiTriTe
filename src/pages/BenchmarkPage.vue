<script setup lang="ts">
defineOptions({ name: "BenchmarkPage" });
import { ref, computed, onMounted, onUnmounted } from "vue";
import { invokeRaw as invoke } from "@/utils/invoke";
import NCard from "@/components/ui/NCard.vue";
import NButton from "@/components/ui/NButton.vue";
import NBadge from "@/components/ui/NBadge.vue";
import NProgress from "@/components/ui/NProgress.vue";
import { useNotificationStore } from "@/stores/notifications";
import { Cpu, MemoryStick, HardDrive, Play, Zap, Trophy, RefreshCw, History, Trash2, Lock, Archive, GitFork } from "lucide-vue-next";

const notify = useNotificationStore();

const HISTORY_KEY = "nitrite_bench_history";
const MAX_HISTORY = 10;

interface BenchResult { score: number; details: string; duration_ms: number; name: string; unit: string; }
interface HistoryEntry {
  date: string;
  cpu: number | null;
  cpuMt: number | null;
  ram: number | null;
  disk: number | null;
  crypto: number | null;
  compression: number | null;
  overall: number;
}

// Résultats
const cpuResult         = ref<BenchResult | null>(null);
const cpuMtResult       = ref<BenchResult | null>(null);
const ramResult         = ref<BenchResult | null>(null);
const diskResult        = ref<BenchResult | null>(null);
const cryptoResult      = ref<BenchResult | null>(null);
const compressionResult = ref<BenchResult | null>(null);

// États running
const runningCpu         = ref(false);
const runningCpuMt       = ref(false);
const runningRam         = ref(false);
const runningDisk        = ref(false);
const runningCrypto      = ref(false);
const runningCompression = ref(false);

// Timers élaps par bench
const elapsedCpu         = ref(0);
const elapsedCpuMt       = ref(0);
const elapsedRam         = ref(0);
const elapsedDisk        = ref(0);
const elapsedCrypto      = ref(0);
const elapsedCompression = ref(0);
const timerHandles: Record<string, ReturnType<typeof setInterval>> = {};

function startTimer(key: string, elapsed: { value: number }) {
  elapsed.value = 0;
  clearInterval(timerHandles[key]);
  timerHandles[key] = setInterval(() => { elapsed.value++; }, 1000);
}
function stopTimer(key: string) {
  clearInterval(timerHandles[key]);
}

const showHistory = ref(false);
const history     = ref<HistoryEntry[]>([]);

const anyRunning = computed(() =>
  runningCpu.value || runningCpuMt.value || runningRam.value ||
  runningDisk.value || runningCrypto.value || runningCompression.value
);

// Score global pondéré : CPU mono 20% / CPU mt 20% / RAM 20% / Disk 20% / Crypto 10% / Compression 10%
const overallScore = computed<number | null>(() => {
  const pairs: [number | null, number][] = [
    [cpuResult.value?.score ?? null,         0.20],
    [cpuMtResult.value?.score ?? null,       0.20],
    [ramResult.value?.score ?? null,         0.20],
    [diskResult.value?.score ?? null,        0.20],
    [cryptoResult.value?.score ?? null,      0.10],
    [compressionResult.value?.score ?? null, 0.10],
  ];
  const available = pairs.filter(([s]) => s !== null && s! > 0) as [number, number][];
  if (!available.length) return null;
  const totalWeight = available.reduce((a, [, w]) => a + w, 0);
  const weighted = available.reduce((a, [s, w]) => a + s * w, 0);
  return Math.round(weighted / totalWeight);
});

function scoreColor(score: number) {
  if (score > 750) return "var(--success)";
  if (score > 400) return "var(--warning)";
  return "var(--danger)";
}
function scoreLabel(score: number) {
  if (score > 750) return "Excellent";
  if (score > 500) return "Bon";
  if (score > 300) return "Moyen";
  return "Faible";
}
function scoreVariant(score: number): "success" | "warning" | "danger" {
  if (score > 750) return "success";
  if (score > 400) return "warning";
  return "danger";
}

// ── Référentiels matériels ──────────────────────────────────────────
interface RefTier {
  label: string;
  examples: string;
  cpu1t: number; cpuMt: number; ram: number; disk: number; overall: number;
  color: string;
}
const REF_TIERS: RefTier[] = [
  { label: "Ultra Low-end",  examples: "Celeron N / Atom / i3 vieux",        cpu1t: 40,  cpuMt: 70,  ram: 150, disk: 80,  overall: 90,  color: "#888" },
  { label: "Entry",          examples: "i3-10 / Ryzen 3 / Core 2 Duo",       cpu1t: 90,  cpuMt: 200, ram: 350, disk: 200, overall: 210, color: "#64b5f6" },
  { label: "Mid-range",      examples: "i5-10/11 / Ryzen 5 5600",             cpu1t: 200, cpuMt: 500, ram: 600, disk: 500, overall: 450, color: "#4dd0e1" },
  { label: "High-end",       examples: "i7-12 / Ryzen 7 5800X / i5-13",      cpu1t: 350, cpuMt: 750, ram: 800, disk: 750, overall: 700, color: "#81c784" },
  { label: "Enthousiastes",  examples: "i9-13 / Ryzen 9 7950X / Xeon",       cpu1t: 500, cpuMt: 950, ram: 900, disk: 900, overall: 850, color: "#ffb74d" },
];

function closestTier(score: number | null): RefTier | null {
  if (score === null) return null;
  return [...REF_TIERS].sort((a, b) =>
    Math.abs(a.overall - score) - Math.abs(b.overall - score)
  )[0];
}
const userTier = computed(() => closestTier(overallScore.value));

function loadHistory() {
  try {
    const raw = localStorage.getItem(HISTORY_KEY);
    history.value = raw ? JSON.parse(raw) : [];
  } catch { history.value = []; }
}

function saveToHistory() {
  if (overallScore.value === null) return;
  const entry: HistoryEntry = {
    date: new Date().toLocaleString("fr-FR"),
    cpu: cpuResult.value?.score ?? null,
    cpuMt: cpuMtResult.value?.score ?? null,
    ram: ramResult.value?.score ?? null,
    disk: diskResult.value?.score ?? null,
    crypto: cryptoResult.value?.score ?? null,
    compression: compressionResult.value?.score ?? null,
    overall: overallScore.value,
  };
  history.value = [entry, ...history.value].slice(0, MAX_HISTORY);
  try { localStorage.setItem(HISTORY_KEY, JSON.stringify(history.value)); } catch {}
}

function clearHistory() {
  history.value = [];
  try { localStorage.removeItem(HISTORY_KEY); } catch {}
}

onMounted(loadHistory);
onUnmounted(() => {
  runningCpu.value = false; runningCpuMt.value = false;
  runningRam.value = false; runningDisk.value = false;
  runningCrypto.value = false; runningCompression.value = false;
  Object.values(timerHandles).forEach(clearInterval);
});

async function runCpu() {
  runningCpu.value = true;
  startTimer("cpu", elapsedCpu);
  notify.info("Benchmark CPU", "Calcul float natif mono-thread (8s)…");
  try {
    cpuResult.value = await invoke<BenchResult>("run_cpu_bench");
    notify.success("CPU terminé", `Score : ${cpuResult.value.score} pts`);
  } catch (e: unknown) { notify.error("Erreur CPU", String(e)); }
  stopTimer("cpu");
  runningCpu.value = false;
}

async function runCpuMt() {
  runningCpuMt.value = true;
  startTimer("cpuMt", elapsedCpuMt);
  notify.info("Benchmark CPU Multi-Thread", "Tous les cœurs pendant 4s…");
  try {
    cpuMtResult.value = await invoke<BenchResult>("run_cpu_mt_bench");
    notify.success("CPU MT terminé", `Score : ${cpuMtResult.value.score} pts`);
  } catch (e: unknown) { notify.error("Erreur CPU MT", String(e)); }
  stopTimer("cpuMt");
  runningCpuMt.value = false;
}

async function runRam() {
  runningRam.value = true;
  startTimer("ram", elapsedRam);
  notify.info("Benchmark RAM", "Test bande passante 256 MB…");
  try {
    ramResult.value = await invoke<BenchResult>("run_ram_bench");
    notify.success("RAM terminé", `Score : ${ramResult.value.score} pts`);
  } catch (e: unknown) { notify.error("Erreur RAM", String(e)); }
  stopTimer("ram");
  runningRam.value = false;
}

async function runDisk() {
  runningDisk.value = true;
  startTimer("disk", elapsedDisk);
  notify.info("Benchmark Disque", "Test lecture/écriture 256 MB + IOPS 4K (≈15s)…");
  try {
    diskResult.value = await invoke<BenchResult>("run_disk_bench", { drive: null });
    // Score 0 = test en échec (le backend met la cause dans details).
    if (diskResult.value.score > 0) notify.success("Disque terminé", `Score : ${diskResult.value.score} pts`);
    else notify.error("Erreur Disque", diskResult.value.details || "Test disque en échec");
  } catch (e: unknown) { notify.error("Erreur Disque", String(e)); }
  stopTimer("disk");
  runningDisk.value = false;
}

async function runCrypto() {
  runningCrypto.value = true;
  startTimer("crypto", elapsedCrypto);
  notify.info("Benchmark Crypto", "ChaCha20-like XOR/ADD/ROT (4s)…");
  try {
    cryptoResult.value = await invoke<BenchResult>("run_crypto_bench");
    notify.success("Crypto terminé", `Score : ${cryptoResult.value.score} pts`);
  } catch (e: unknown) { notify.error("Erreur Crypto", String(e)); }
  stopTimer("crypto");
  runningCrypto.value = false;
}

async function runCompression() {
  runningCompression.value = true;
  startTimer("compression", elapsedCompression);
  notify.info("Benchmark Compression", "DEFLATE 8 MB…");
  try {
    compressionResult.value = await invoke<BenchResult>("run_compression_bench");
    notify.success("Compression terminée", `Score : ${compressionResult.value.score} pts`);
  } catch (e: unknown) { notify.error("Erreur Compression", String(e)); }
  stopTimer("compression");
  runningCompression.value = false;
}

async function runAll() {
  // SÉQUENTIEL, jamais Promise.all : lancer les benchs en parallèle les ferait
  // se disputer CPU/RAM/disque (ex. le bench mono-thread mesuré pendant que le
  // multi-thread sature tous les cœurs) → tous les scores faussés. Chaque
  // composant doit être mesuré isolément.
  await runCpu();
  await runCpuMt();
  await runRam();
  await runDisk();
  await runCrypto();
  await runCompression();
  saveToHistory();
}

function reset() {
  cpuResult.value = null; cpuMtResult.value = null;
  ramResult.value = null; diskResult.value = null;
  cryptoResult.value = null; compressionResult.value = null;
}
</script>

<template>
  <div class="bench-page">
    <div class="page-header">
      <div class="header-icon"><Zap :size="22" /></div>
      <div>
        <h1>Benchmark Système</h1>
        <p class="subtitle">CPU mono/multi-thread · RAM · Disque · Crypto · Compression — en simultané</p>
      </div>
      <div style="display:flex;gap:8px;margin-left:auto">
        <NButton variant="ghost" size="sm" @click="showHistory = !showHistory">
          <History :size="13" /> Historique ({{ history.length }})
        </NButton>
        <NButton variant="primary" size="sm" :loading="anyRunning" :disabled="anyRunning" @click="runAll">
          <Play :size="13" /> {{ anyRunning ? 'Tests en cours…' : 'Tout lancer' }}
        </NButton>
      </div>
    </div>

    <!-- Score global -->
    <div v-if="overallScore !== null" class="global-score">
      <Trophy :size="22" style="color:var(--warning)" />
      <span class="score-val">{{ overallScore }}</span>
      <span class="score-label">Score global / 1000</span>
      <NBadge :variant="scoreVariant(overallScore)">{{ scoreLabel(overallScore) }}</NBadge>
      <NButton variant="secondary" size="sm" @click="saveToHistory" style="margin-left:auto">
        <History :size="13" /> Sauvegarder
      </NButton>
      <NButton variant="ghost" size="sm" @click="reset">
        <RefreshCw :size="13" /> Réinitialiser
      </NButton>
    </div>

    <!-- Positionnement par rapport aux références -->
    <NCard>
      <template #header><div style="display:flex;align-items:center;gap:8px"><Trophy :size="15" /><span>Positionnement</span><span v-if="userTier" class="tier-your-label" :style="{ color: userTier.color }">→ Votre PC correspond à : <strong>{{ userTier.label }}</strong></span></div></template>
      <div class="ref-note">⚠ Valeurs de référence en mode <strong>release</strong>. En mode dev (<code>npm run tauri dev</code>) les scores sont 5-10× plus faibles — construisez en release pour des mesures précises.</div>
      <table class="ref-table">
        <thead><tr><th>Profil</th><th>Exemples</th><th>CPU 1T</th><th>CPU MT</th><th>RAM</th><th>Disque</th><th>Score global</th></tr></thead>
        <tbody>
          <tr v-for="tier in REF_TIERS" :key="tier.label"
              :class="{ 'ref-row-active': userTier?.label === tier.label }">
            <td><span class="tier-dot" :style="{ background: tier.color }"></span>{{ tier.label }}</td>
            <td class="muted small">{{ tier.examples }}</td>
            <td class="mono">~{{ tier.cpu1t }}</td>
            <td class="mono">~{{ tier.cpuMt }}</td>
            <td class="mono">~{{ tier.ram }}</td>
            <td class="mono">~{{ tier.disk }}</td>
            <td>
              <div style="display:flex;align-items:center;gap:8px">
                <div class="ref-bar"><div class="ref-bar-fill" :style="{ width: tier.overall/10+'%', background: tier.color }"></div></div>
                <span class="mono" :style="{ color: tier.color }">~{{ tier.overall }}</span>
              </div>
            </td>
          </tr>
          <tr v-if="overallScore !== null" class="ref-row-user">
            <td colspan="2"><strong>Votre PC</strong></td>
            <td class="mono">{{ cpuResult?.score ?? '—' }}</td>
            <td class="mono">{{ cpuMtResult?.score ?? '—' }}</td>
            <td class="mono">{{ ramResult?.score ?? '—' }}</td>
            <td class="mono">{{ diskResult?.score ?? '—' }}</td>
            <td><span class="mono score-highlight">{{ overallScore }}</span></td>
          </tr>
        </tbody>
      </table>
    </NCard>

    <!-- Historique -->
    <NCard v-if="showHistory">
      <template #header>
        <div style="display:flex;align-items:center;gap:8px;width:100%">
          <History :size="15" /><span>Historique</span>
          <NButton variant="ghost" size="sm" @click="clearHistory" style="margin-left:auto">
            <Trash2 :size="13" /> Effacer
          </NButton>
        </div>
      </template>
      <div v-if="history.length === 0" style="text-align:center;padding:24px;color:var(--text-muted);font-size:13px">Aucun historique</div>
      <table v-else class="history-table">
        <thead>
          <tr><th>Date</th><th>CPU 1T</th><th>CPU MT</th><th>RAM</th><th>Disque</th><th>Crypto</th><th>Compr.</th><th>Score</th></tr>
        </thead>
        <tbody>
          <tr v-for="(h, i) in history" :key="i">
            <td class="muted small">{{ h.date }}</td>
            <td><span class="score-cell" :style="{ color: h.cpu ? scoreColor(h.cpu) : 'var(--text-muted)' }">{{ h.cpu ?? '—' }}</span></td>
            <td><span class="score-cell" :style="{ color: h.cpuMt ? scoreColor(h.cpuMt) : 'var(--text-muted)' }">{{ h.cpuMt ?? '—' }}</span></td>
            <td><span class="score-cell" :style="{ color: h.ram ? scoreColor(h.ram) : 'var(--text-muted)' }">{{ h.ram ?? '—' }}</span></td>
            <td><span class="score-cell" :style="{ color: h.disk ? scoreColor(h.disk) : 'var(--text-muted)' }">{{ h.disk ?? '—' }}</span></td>
            <td><span class="score-cell" :style="{ color: h.crypto ? scoreColor(h.crypto) : 'var(--text-muted)' }">{{ h.crypto ?? '—' }}</span></td>
            <td><span class="score-cell" :style="{ color: h.compression ? scoreColor(h.compression) : 'var(--text-muted)' }">{{ h.compression ?? '—' }}</span></td>
            <td>
              <div style="display:flex;align-items:center;gap:6px">
                <span class="score-cell big" :style="{ color: scoreColor(h.overall) }">{{ h.overall }}</span>
                <NBadge :variant="scoreVariant(h.overall)" size="sm">{{ scoreLabel(h.overall) }}</NBadge>
              </div>
            </td>
          </tr>
        </tbody>
      </table>
    </NCard>

    <div class="bench-grid">
      <!-- CPU Mono-thread -->
      <NCard>
        <template #header><div class="section-header"><Cpu :size="15" /><span>CPU Mono-thread</span></div></template>
        <div class="bench-card-body">
          <p class="bench-desc">Calcul float natif 8s — sqrt + sin + ln. Mesure la puissance mono-cœur.</p>
          <div v-if="cpuResult" class="result-block">
            <div class="score-big" :style="{ color: scoreColor(cpuResult.score) }">{{ cpuResult.score }}</div>
            <div class="score-sub">/ 1000 pts</div>
            <NBadge :variant="scoreVariant(cpuResult.score)">{{ scoreLabel(cpuResult.score) }}</NBadge>
            <NProgress :value="Math.min(cpuResult.score / 10, 100)" :glow="true" size="sm" style="margin-top:8px" />
            <p class="result-detail">{{ cpuResult.details }}</p>
            <p class="result-time">{{ (cpuResult.duration_ms / 1000).toFixed(1) }}s</p>
          </div>
          <div v-else-if="runningCpu" class="running-state">
            <div class="running-spinner"></div>
            <span>Calcul (8s) — {{ elapsedCpu }}s écoulées</span>
          </div>
          <NButton variant="secondary" size="sm" :loading="runningCpu" :disabled="runningCpu" @click="runCpu">
            <Play :size="13" /> {{ runningCpu ? 'En cours…' : 'Lancer' }}
          </NButton>
        </div>
      </NCard>

      <!-- CPU Multi-thread -->
      <NCard>
        <template #header><div class="section-header"><GitFork :size="15" /><span>CPU Multi-Thread</span></div></template>
        <div class="bench-card-body">
          <p class="bench-desc">Tous les cœurs physiques pendant 4s — charge maximale parallèle.</p>
          <div v-if="cpuMtResult" class="result-block">
            <div class="score-big" :style="{ color: scoreColor(cpuMtResult.score) }">{{ cpuMtResult.score }}</div>
            <div class="score-sub">/ 1000 pts</div>
            <NBadge :variant="scoreVariant(cpuMtResult.score)">{{ scoreLabel(cpuMtResult.score) }}</NBadge>
            <NProgress :value="Math.min(cpuMtResult.score / 10, 100)" :glow="true" size="sm" style="margin-top:8px" />
            <p class="result-detail">{{ cpuMtResult.details }}</p>
            <p class="result-time">{{ (cpuMtResult.duration_ms / 1000).toFixed(1) }}s</p>
          </div>
          <div v-else-if="runningCpuMt" class="running-state">
            <div class="running-spinner"></div>
            <span>Multi-thread (4s) — {{ elapsedCpuMt }}s écoulées</span>
          </div>
          <NButton variant="secondary" size="sm" :loading="runningCpuMt" :disabled="runningCpuMt" @click="runCpuMt">
            <Play :size="13" /> {{ runningCpuMt ? 'En cours…' : 'Lancer' }}
          </NButton>
        </div>
      </NCard>

      <!-- RAM -->
      <NCard>
        <template #header><div class="section-header"><MemoryStick :size="15" /><span>Mémoire (RAM)</span></div></template>
        <div class="bench-card-body">
          <p class="bench-desc">Bande passante séquentielle 256 MB + latence accès aléatoire. DDR4-3200 ≈ 25 GB/s.</p>
          <div v-if="ramResult" class="result-block">
            <div class="score-big" :style="{ color: scoreColor(ramResult.score) }">{{ ramResult.score }}</div>
            <div class="score-sub">/ 1000 pts</div>
            <NBadge :variant="scoreVariant(ramResult.score)">{{ scoreLabel(ramResult.score) }}</NBadge>
            <NProgress :value="Math.min(ramResult.score / 10, 100)" :glow="true" size="sm" style="margin-top:8px" />
            <p class="result-detail">{{ ramResult.details }}</p>
            <p class="result-time">{{ (ramResult.duration_ms / 1000).toFixed(1) }}s</p>
          </div>
          <div v-else-if="runningRam" class="running-state">
            <div class="running-spinner"></div>
            <span>Bande passante 256 MB — {{ elapsedRam }}s</span>
          </div>
          <NButton variant="secondary" size="sm" :loading="runningRam" :disabled="runningRam" @click="runRam">
            <Play :size="13" /> {{ runningRam ? 'En cours…' : 'Lancer' }}
          </NButton>
        </div>
      </NCard>

      <!-- Disque -->
      <NCard>
        <template #header><div class="section-header"><HardDrive :size="15" /><span>Stockage (Disque)</span></div></template>
        <div class="bench-card-body">
          <p class="bench-desc">Lecture/écriture séquentielle 256 MB + IOPS 4K aléatoire 3s. Score composite 60%+40%.</p>
          <div v-if="diskResult" class="result-block">
            <div class="score-big" :style="{ color: scoreColor(diskResult.score) }">{{ diskResult.score }}</div>
            <div class="score-sub">/ 1000 pts</div>
            <NBadge :variant="scoreVariant(diskResult.score)">{{ scoreLabel(diskResult.score) }}</NBadge>
            <NProgress :value="Math.min(diskResult.score / 10, 100)" :glow="true" size="sm" style="margin-top:8px" />
            <p class="result-detail">{{ diskResult.details }}</p>
            <p class="result-time">{{ (diskResult.duration_ms / 1000).toFixed(1) }}s</p>
          </div>
          <div v-else-if="runningDisk" class="running-state">
            <div class="running-spinner"></div>
            <span>Test disque (≈15s) — {{ elapsedDisk }}s</span>
          </div>
          <NButton variant="secondary" size="sm" :loading="runningDisk" :disabled="runningDisk" @click="runDisk">
            <Play :size="13" /> {{ runningDisk ? 'En cours…' : 'Lancer' }}
          </NButton>
        </div>
      </NCard>

      <!-- Crypto -->
      <NCard>
        <template #header><div class="section-header"><Lock :size="15" /><span>Crypto (ChaCha20-like)</span></div></template>
        <div class="bench-card-body">
          <p class="bench-desc">Opérations XOR/ADD/ROT 64 rounds pendant 4s — simule un cipher symétrique.</p>
          <div v-if="cryptoResult" class="result-block">
            <div class="score-big" :style="{ color: scoreColor(cryptoResult.score) }">{{ cryptoResult.score }}</div>
            <div class="score-sub">/ 1000 pts</div>
            <NBadge :variant="scoreVariant(cryptoResult.score)">{{ scoreLabel(cryptoResult.score) }}</NBadge>
            <NProgress :value="Math.min(cryptoResult.score / 10, 100)" :glow="true" size="sm" style="margin-top:8px" />
            <p class="result-detail">{{ cryptoResult.details }}</p>
            <p class="result-time">{{ (cryptoResult.duration_ms / 1000).toFixed(1) }}s</p>
          </div>
          <div v-else-if="runningCrypto" class="running-state">
            <div class="running-spinner"></div>
            <span>XOR/ROT (4s) — {{ elapsedCrypto }}s</span>
          </div>
          <NButton variant="secondary" size="sm" :loading="runningCrypto" :disabled="runningCrypto" @click="runCrypto">
            <Play :size="13" /> {{ runningCrypto ? 'En cours…' : 'Lancer' }}
          </NButton>
        </div>
      </NCard>

      <!-- Compression -->
      <NCard>
        <template #header><div class="section-header"><Archive :size="15" /><span>Compression DEFLATE</span></div></template>
        <div class="bench-card-body">
          <p class="bench-desc">Compression DEFLATE de 8 MB via zip Rust natif. 500 MB/s = excellent.</p>
          <div v-if="compressionResult" class="result-block">
            <div class="score-big" :style="{ color: scoreColor(compressionResult.score) }">{{ compressionResult.score }}</div>
            <div class="score-sub">/ 1000 pts</div>
            <NBadge :variant="scoreVariant(compressionResult.score)">{{ scoreLabel(compressionResult.score) }}</NBadge>
            <NProgress :value="Math.min(compressionResult.score / 10, 100)" :glow="true" size="sm" style="margin-top:8px" />
            <p class="result-detail">{{ compressionResult.details }}</p>
            <p class="result-time">{{ (compressionResult.duration_ms / 1000).toFixed(1) }}s</p>
          </div>
          <div v-else-if="runningCompression" class="running-state">
            <div class="running-spinner"></div>
            <span>DEFLATE 8 MB — {{ elapsedCompression }}s</span>
          </div>
          <NButton variant="secondary" size="sm" :loading="runningCompression" :disabled="runningCompression" @click="runCompression">
            <Play :size="13" /> {{ runningCompression ? 'En cours…' : 'Lancer' }}
          </NButton>
        </div>
      </NCard>
    </div>
  </div>
</template>

<style scoped>
.bench-page { display: flex; flex-direction: column; gap: 16px; }
.page-header { display: flex; align-items: center; gap: 12px; }
.header-icon { width: 42px; height: 42px; border-radius: var(--radius-lg); background: var(--warning-muted); display: flex; align-items: center; justify-content: center; color: var(--warning); flex-shrink: 0; }
h1 { font-size: 22px; font-weight: 700; }
.subtitle { font-size: 12px; color: var(--text-muted); }
.global-score { display: flex; align-items: center; gap: 12px; padding: 14px 20px; background: var(--bg-secondary); border: 1px solid var(--border); border-radius: var(--radius-xl); }
.score-val { font-size: 28px; font-weight: 900; color: var(--text-primary); font-family: "JetBrains Mono", monospace; }
.score-label { font-size: 13px; color: var(--text-muted); }
.bench-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 14px; }
@media (max-width: 1000px) { .bench-grid { grid-template-columns: repeat(2, 1fr); } }
@media (max-width: 640px)  { .bench-grid { grid-template-columns: 1fr; } }
.section-header { display: flex; align-items: center; gap: 8px; }
.bench-card-body { display: flex; flex-direction: column; gap: 12px; }
.bench-desc { font-size: 12px; color: var(--text-muted); line-height: 1.5; }
.result-block { display: flex; flex-direction: column; gap: 4px; padding: 12px; background: var(--bg-tertiary); border-radius: var(--radius-md); }
.score-big { font-size: 36px; font-weight: 900; line-height: 1; font-family: "JetBrains Mono", monospace; }
.score-sub { font-size: 12px; color: var(--text-muted); }
.result-detail { font-size: 11px; color: var(--text-secondary); margin-top: 4px; font-family: "JetBrains Mono", monospace; line-height: 1.5; }
.result-time { font-size: 11px; color: var(--text-muted); }
.running-state { display: flex; align-items: center; gap: 10px; padding: 14px; background: var(--bg-tertiary); border-radius: var(--radius-md); font-size: 12px; color: var(--text-muted); }
.running-spinner { width: 16px; height: 16px; border: 2px solid var(--border); border-top-color: var(--accent-primary); border-radius: 50%; animation: spin 0.8s linear infinite; flex-shrink: 0; }
@keyframes spin { to { transform: rotate(360deg); } }

.ref-note { font-size: 11px; color: var(--text-muted); background: var(--bg-tertiary); border-radius: var(--radius-md); padding: 8px 12px; margin-bottom: 12px; line-height: 1.6; }
.ref-note code { background: var(--bg-secondary); padding: 1px 4px; border-radius: 3px; font-family: "JetBrains Mono", monospace; }
.ref-table { width: 100%; border-collapse: collapse; font-size: 12px; }
.ref-table th { padding: 6px 10px; text-align: left; color: var(--text-muted); font-size: 10px; font-weight: 700; text-transform: uppercase; letter-spacing: .06em; border-bottom: 1px solid var(--border); }
.ref-table td { padding: 6px 10px; border-bottom: 1px solid var(--border); }
.ref-row-active td { background: rgba(255,152,0,.08) !important; font-weight: 700; }
.ref-row-user td { background: rgba(var(--accent-rgb,255,152,0),.06); border-top: 2px solid var(--accent-primary); }
.tier-dot { display: inline-block; width: 8px; height: 8px; border-radius: 50%; margin-right: 6px; }
.tier-your-label { font-size: 12px; font-weight: 500; margin-left: auto; }
.ref-bar { width: 80px; height: 6px; background: var(--bg-tertiary); border-radius: 3px; overflow: hidden; }
.ref-bar-fill { height: 100%; border-radius: 3px; transition: width .3s; }
.score-highlight { font-size: 15px; font-weight: 900; color: var(--accent-primary); }

.history-table { width: 100%; border-collapse: collapse; font-size: 12px; }
.history-table th { padding: 6px 10px; text-align: left; color: var(--text-muted); font-size: 10px; font-weight: 700; text-transform: uppercase; letter-spacing: .06em; border-bottom: 1px solid var(--border); }
.history-table td { padding: 6px 10px; border-bottom: 1px solid var(--border); }
.history-table tr:last-child td { border-bottom: none; }
.history-table tr:hover td { background: var(--bg-tertiary); }
.score-cell { font-weight: 700; font-family: "JetBrains Mono", monospace; }
.score-cell.big { font-size: 15px; }
.muted { color: var(--text-muted); }
.small { font-size: 11px; }
</style>
