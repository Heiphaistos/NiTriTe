<script setup lang="ts">
defineOptions({ name: "TurboModePage" });
import { ref, onMounted, computed, type Component } from "vue";
import { invoke } from "@/utils/invoke";
import NCard from "@/components/ui/NCard.vue";
import NButton from "@/components/ui/NButton.vue";
import NBadge from "@/components/ui/NBadge.vue";
import { useNotificationStore } from "@/stores/notifications";
import { Zap, Gamepad2, Briefcase, Leaf, CheckCircle, AlertTriangle, TrendingUp, Trash2, HardDrive, Wifi, Shield, RefreshCw, Monitor, Star, History } from "lucide-vue-next";
import { useDataCache } from "@/stores/dataCache";
import type { SysInfo } from "@/types/diagnostic";

const notify    = useNotificationStore();
const dataCache = useDataCache();

const modes = [
  { id: "turbo",  label: "Mode Turbo", icon: Zap,       color: "#f97316", grad: "linear-gradient(135deg,#f97316,#ea580c)", description: "Libère la RAM, vide le DNS, maximise les performances CPU/GPU. Usage général.", actions: ["Plan haute performance","Cache DNS vidé","RAM libérée","GPU Scheduling"] },
  { id: "gaming", label: "Gaming",     icon: Gamepad2,  color: "#8b5cf6", grad: "linear-gradient(135deg,#8b5cf6,#7c3aed)", description: "Optimisé FPS : haute performance, GPU Hardware Scheduling, Game Mode Windows activé.", actions: ["Plan haute performance","Xbox Game Bar OFF","GPU HW Scheduling","Game Mode ON"] },
  { id: "work",   label: "Travail",    icon: Briefcase, color: "#3b82f6", grad: "linear-gradient(135deg,#3b82f6,#2563eb)", description: "Équilibre perf/batterie. Effets visuels réduits. Presse-papiers nettoyé.", actions: ["Plan équilibré","Effets visuels optimisés","Clipboard vidé"] },
  { id: "eco",    label: "Économie",   icon: Leaf,      color: "#22c55e", grad: "linear-gradient(135deg,#22c55e,#16a34a)", description: "Économise la batterie au maximum. Plan économie d'énergie. Luminosité réduite.", actions: ["Plan économie d'énergie","Luminosité 50%"] },
];

const applying  = ref(false);
const activeMode = ref<string | null>(null);
const result    = ref<{ actions_done: string[]; errors: string[] } | null>(null);

// Stats avant/après
interface SysStats { ram_used_mb: number; ram_total_mb: number; ram_free_mb: number; }
const statsBefore = ref<SysStats | null>(null);
const statsAfter  = ref<SysStats | null>(null);

// ─── Historique gains ─────────────────────────────────────────────────────────
interface GainEntry {
  ts: string;             // ISO date
  mode: string;           // id du mode
  modeLabel: string;
  ramFreedMb: number;     // MB libérés (peut être 0 si pas de gain mesurable)
  actionsDone: number;
  errors: number;
}
const GAINS_KEY = "nitrite_turbo_gains";
const MAX_GAINS = 30;
const showHistory = ref(false);

function loadGains(): GainEntry[] {
  try {
    const raw = localStorage.getItem(GAINS_KEY);
    return raw ? (JSON.parse(raw) as GainEntry[]) : [];
  } catch { return []; }
}

function saveGain(entry: GainEntry) {
  try {
    const all = loadGains();
    all.push(entry);
    localStorage.setItem(GAINS_KEY, JSON.stringify(all.slice(-MAX_GAINS)));
  } catch { /* ignore */ }
}

function clearGains() {
  localStorage.removeItem(GAINS_KEY);
  gainsHistory.value = [];
}

const gainsHistory = ref<GainEntry[]>([]);

// Gain total cumulé
const totalRamFreed = computed(() =>
  gainsHistory.value.reduce((acc, g) => acc + g.ramFreedMb, 0)
);
const totalOptimizations = computed(() =>
  gainsHistory.value.reduce((acc, g) => acc + g.actionsDone, 0)
);

// fresh=true : bypasser le cache. Indispensable pour la mesure avant/après d'un
// mode Turbo : sinon statsBefore et statsAfter renvoient le MÊME snapshot caché
// → ramDiff toujours 0, aucun gain affiché même quand la RAM a été libérée.
async function fetchStats(fresh = false): Promise<SysStats | null> {
  try {
    const cached = fresh ? undefined : (dataCache.get("get_system_info") as SysInfo | undefined);
    const info = cached ?? await invoke<SysInfo>("get_system_info");
    if (!info) return null;
    const total_mb = Math.round((info.ram?.total_gb ?? 0) * 1024);
    const used_mb  = Math.round((info.ram?.used_gb ?? 0) * 1024);
    return { ram_total_mb: total_mb, ram_used_mb: used_mb, ram_free_mb: total_mb - used_mb };
  } catch { return null; }
}

onMounted(async () => {
  statsBefore.value = await fetchStats();
  gainsHistory.value = loadGains();
});

async function applyMode(modeId: string) {
  applying.value = true;
  activeMode.value = modeId;
  result.value = null;
  statsAfter.value = null;
  statsBefore.value = await fetchStats(true);
  try {
    const r = await invoke<{ actions_done: string[]; errors: string[] }>("apply_turbo_mode", { mode: modeId });
    result.value = r;
    // Attendre 1s que Windows applique, puis mesurer
    await new Promise(res => setTimeout(res, 1200));
    statsAfter.value = await fetchStats(true);
    // Persister le gain dans localStorage
    const freed = ramDiff();
    const modeLabel = modes.find(m => m.id === modeId)?.label ?? modeId;
    const entry: GainEntry = {
      ts: new Date().toISOString(),
      mode: modeId,
      modeLabel,
      ramFreedMb: freed > 0 ? freed : 0,
      actionsDone: r.actions_done.length,
      errors: r.errors.length,
    };
    saveGain(entry);
    gainsHistory.value = loadGains();

    if (r.errors.length === 0) notify.success("Mode appliqué", `${r.actions_done.length} optimisations effectuées`);
    else notify.warning("Mode appliqué (partiel)", `${r.errors.length} action(s) échouée(s)`);
  } catch (e: unknown) {
    notify.error("Erreur", String(e));
  } finally {
    applying.value = false;
  }
}

function ramDiff(): number {
  if (!statsBefore.value || !statsAfter.value) return 0;
  return statsBefore.value.ram_used_mb - statsAfter.value.ram_used_mb;
}
function ramDiffPct(): number {
  if (!statsBefore.value || statsBefore.value.ram_total_mb === 0) return 0;
  return Math.round((ramDiff() / statsBefore.value.ram_total_mb) * 100);
}
function formatMb(mb: number): string {
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
  return `${Math.round(mb)} MB`;
}

// ─── Optimisations individuelles ─────────────────────────────────────────────
interface QuickOpt { id: string; label: string; desc: string; icon: Component; color: string; }

const quickOpts: QuickOpt[] = [
  { id: "clean_temp",       label: "Nettoyer fichiers temporaires",     icon: Trash2,     color: "#ef4444", desc: "Supprime %TEMP% et C:\\Windows\\Temp" },
  { id: "flush_dns",        label: "Vider le cache DNS",                icon: Wifi,       color: "#3b82f6", desc: "Réinitialise le cache DNS Windows" },
  { id: "clean_eventlog",   label: "Vider les journaux d'événements",   icon: RefreshCw,  color: "#8b5cf6", desc: "Efface les journaux Application/System/Security" },
  { id: "disable_prefetch", label: "Optimiser le démarrage SSD",        icon: HardDrive,  color: "#f59e0b", desc: "Désactive Superfetch/SysMain (recommandé SSD)" },
  { id: "disable_telemetry",label: "Limiter la télémétrie Windows",     icon: Shield,     color: "#22c55e", desc: "Réduit les données envoyées à Microsoft au minimum" },
  { id: "visual_perf",      label: "Effets visuels : Performance",      icon: Monitor,    color: "#6b7280", desc: "Désactive animations et effets pour + de réactivité" },
  { id: "optimize_drives",  label: "Optimiser les disques (TRIM)",      icon: HardDrive,  color: "#14b8a6", desc: "Lance l'optimisation TRIM sur les volumes SSD" },
  { id: "clear_clipboard",  label: "Vider le presse-papiers",           icon: Trash2,     color: "#ec4899", desc: "Efface le contenu du presse-papiers Windows" },
];

const quickRunning = ref<string | null>(null);
const quickResults = ref<Record<string, { ok: boolean; msg: string }>>({});

async function runQuickOpt(opt: QuickOpt) {
  quickRunning.value = opt.id;
  try {
    const out = await invoke<string>("run_quick_optimization", { optId: opt.id });
    quickResults.value[opt.id] = { ok: true, msg: String(out).trim() || 'Effectué' };
    notify.success(opt.label, String(out).trim() || 'Optimisation effectuée');
  } catch (e: unknown) {
    quickResults.value[opt.id] = { ok: false, msg: String(e) };
    notify.error(opt.label, String(e));
  } finally {
    quickRunning.value = null;
  }
}
</script>

<template>
  <div class="turbo-page">
    <div class="page-header">
      <div>
        <h1>Mode Turbo & Profils</h1>
        <p class="page-subtitle">Optimisations système en 1 clic selon votre usage</p>
      </div>
      <!-- Stats RAM en temps réel -->
      <div v-if="statsBefore" class="ram-pill">
        <span class="ram-label">RAM libre</span>
        <span class="ram-val">{{ formatMb(statsBefore.ram_free_mb) }}</span>
        <span class="ram-total">/ {{ formatMb(statsBefore.ram_total_mb) }}</span>
      </div>
    </div>

    <div class="modes-grid">
      <NCard v-for="mode in modes" :key="mode.id" class="mode-card" :class="{ active: activeMode === mode.id && result }">
        <div class="mode-icon" :style="{ background: mode.grad }">
          <component :is="mode.icon" :size="28" color="white" />
        </div>
        <h3 class="mode-label">{{ mode.label }}</h3>
        <p class="mode-desc">{{ mode.description }}</p>
        <div class="mode-actions-preview">
          <div v-for="a in mode.actions" :key="a" class="action-chip">{{ a }}</div>
        </div>
        <NButton variant="primary" :loading="applying && activeMode === mode.id" :disabled="applying" @click="applyMode(mode.id)" style="margin-top:16px;width:100%">
          <Zap :size="14" /> Activer
        </NButton>
      </NCard>
    </div>

    <!-- Résultat + avant/après -->
    <NCard v-if="result">
      <template #header>
        <div style="display:flex;align-items:center;gap:8px">
          <CheckCircle :size="16" style="color:var(--success)" />
          <span>Résultat — {{ modes.find(m=>m.id===activeMode)?.label }}</span>
        </div>
      </template>

      <!-- Avant / Après RAM -->
      <div v-if="statsBefore && statsAfter" class="before-after">
        <div class="ba-block">
          <div class="ba-label">Avant</div>
          <div class="ba-bar-wrap">
            <div class="ba-bar" :style="{ width: `${(statsBefore.ram_used_mb / statsBefore.ram_total_mb) * 100}%`, background: 'var(--warning)' }"></div>
          </div>
          <div class="ba-stat">RAM utilisée : <strong>{{ formatMb(statsBefore.ram_used_mb) }}</strong> ({{ Math.round((statsBefore.ram_used_mb / statsBefore.ram_total_mb) * 100) }}%)</div>
        </div>
        <div class="ba-arrow"><TrendingUp :size="20" style="color:var(--success)" /></div>
        <div class="ba-block">
          <div class="ba-label after">Après</div>
          <div class="ba-bar-wrap">
            <div class="ba-bar" :style="{ width: `${(statsAfter.ram_used_mb / statsAfter.ram_total_mb) * 100}%`, background: 'var(--success)' }"></div>
          </div>
          <div class="ba-stat">RAM utilisée : <strong>{{ formatMb(statsAfter.ram_used_mb) }}</strong> ({{ Math.round((statsAfter.ram_used_mb / statsAfter.ram_total_mb) * 100) }}%)</div>
        </div>
        <div v-if="ramDiff() > 0" class="ba-gain">
          <NBadge variant="success">+{{ formatMb(ramDiff()) }} libéré ({{ ramDiffPct() }}%)</NBadge>
        </div>
      </div>

      <div class="result-grid">
        <div class="result-section">
          <div class="result-title ok">✓ Actions effectuées ({{ result.actions_done.length }})</div>
          <div v-for="a in result.actions_done" :key="a" class="result-item ok"><CheckCircle :size="13" /> {{ a }}</div>
          <div v-if="result.actions_done.length === 0" class="result-item muted">Aucune action</div>
        </div>
        <div v-if="result.errors.length" class="result-section">
          <div class="result-title warn">⚠ Erreurs ({{ result.errors.length }})</div>
          <div v-for="e in result.errors" :key="e" class="result-item warn"><AlertTriangle :size="13" /> {{ e }}</div>
        </div>
      </div>
    </NCard>

    <!-- Optimisations individuelles -->
    <NCard>
      <template #header>
        <div style="display:flex;align-items:center;gap:8px">
          <Star :size="16" style="color:var(--accent-primary)" />
          <span>Optimisations individuelles</span>
          <NBadge variant="info" style="font-size:10px">{{ quickOpts.length }} actions</NBadge>
        </div>
      </template>
      <div class="quick-opts-grid">
        <div v-for="opt in quickOpts" :key="opt.id" class="quick-opt-card" :class="{ 'opt-done': quickResults[opt.id]?.ok, 'opt-err': quickResults[opt.id] && !quickResults[opt.id].ok }">
          <div class="opt-icon" :style="{ background: opt.color + '22', color: opt.color }">
            <component :is="opt.icon" :size="18" />
          </div>
          <div class="opt-body">
            <div class="opt-label">{{ opt.label }}</div>
            <div class="opt-desc">{{ opt.desc }}</div>
            <div v-if="quickResults[opt.id]" class="opt-result" :class="quickResults[opt.id].ok ? 'res-ok' : 'res-err'">
              <component :is="quickResults[opt.id].ok ? CheckCircle : AlertTriangle" :size="11" />
              {{ quickResults[opt.id].msg }}
            </div>
          </div>
          <NButton variant="secondary" size="sm" :disabled="quickRunning !== null" :loading="quickRunning === opt.id" @click="runQuickOpt(opt)">
            <Zap :size="12" /> Appliquer
          </NButton>
        </div>
      </div>
    </NCard>

    <!-- Historique des gains ─────────────────────────────────────────────── -->
    <NCard v-if="gainsHistory.length > 0">
      <template #header>
        <div style="display:flex;align-items:center;gap:8px;width:100%">
          <History :size="16" style="color:var(--accent-primary)" />
          <span>Historique des gains</span>
          <NBadge variant="info" style="font-size:10px">{{ gainsHistory.length }} session(s)</NBadge>
          <div style="margin-left:auto;display:flex;align-items:center;gap:8px">
            <span style="font-size:11px;color:var(--text-muted)">
              Total : <strong style="color:var(--success)">{{ formatMb(totalRamFreed) }} libérés</strong>
              — <strong style="color:var(--accent-primary)">{{ totalOptimizations }}</strong> actions
            </span>
            <NButton variant="secondary" size="sm" @click="showHistory = !showHistory">
              {{ showHistory ? 'Masquer' : 'Voir tout' }}
            </NButton>
            <NButton variant="secondary" size="sm" @click="clearGains" title="Effacer l'historique">
              <Trash2 :size="12" />
            </NButton>
          </div>
        </div>
      </template>

      <!-- Résumé rapide (3 derniers) -->
      <div v-if="!showHistory" class="gains-summary">
        <div
          v-for="g in gainsHistory.slice(-3).reverse()"
          :key="g.ts"
          class="gain-row"
        >
          <span class="gain-mode" :class="`gain-mode--${g.mode}`">{{ g.modeLabel }}</span>
          <span class="gain-date">{{ new Date(g.ts).toLocaleDateString('fr-FR', { day:'2-digit', month:'short', hour:'2-digit', minute:'2-digit' }) }}</span>
          <span v-if="g.ramFreedMb > 0" class="gain-ram">+{{ formatMb(g.ramFreedMb) }}</span>
          <span v-else class="gain-ram muted">—</span>
          <span class="gain-acts">{{ g.actionsDone }} act.</span>
          <NBadge v-if="g.errors > 0" variant="warning" style="font-size:10px">{{ g.errors }} err</NBadge>
          <NBadge v-else variant="success" style="font-size:10px">OK</NBadge>
        </div>
      </div>

      <!-- Table complète -->
      <div v-else class="gains-table-wrap">
        <table class="gains-table">
          <thead>
            <tr>
              <th>Date</th>
              <th>Mode</th>
              <th>RAM libérée</th>
              <th>Actions</th>
              <th>Erreurs</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="g in gainsHistory.slice().reverse()" :key="g.ts">
              <td class="g-date">{{ new Date(g.ts).toLocaleDateString('fr-FR', { day:'2-digit', month:'short', year:'2-digit', hour:'2-digit', minute:'2-digit' }) }}</td>
              <td><span class="gain-mode" :class="`gain-mode--${g.mode}`">{{ g.modeLabel }}</span></td>
              <td class="g-ram">{{ g.ramFreedMb > 0 ? '+' + formatMb(g.ramFreedMb) : '—' }}</td>
              <td class="g-acts">{{ g.actionsDone }}</td>
              <td>
                <NBadge :variant="g.errors > 0 ? 'warning' : 'success'" style="font-size:10px">{{ g.errors > 0 ? g.errors + ' err' : 'OK' }}</NBadge>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </NCard>

    <NCard>
      <template #header><span>Informations</span></template>
      <p style="font-size:13px;color:var(--text-secondary);line-height:1.6">
        Les modes modifient le plan d'alimentation Windows, les paramètres GPU et quelques clés de registre.
        Ces changements sont <strong>réversibles</strong> — vous pouvez appliquer un autre mode à tout moment.
        Certaines optimisations (GPU Scheduling) nécessitent un redémarrage pour être pleinement actives.
      </p>
    </NCard>
  </div>
</template>

<style scoped>
.turbo-page { display:flex; flex-direction:column; gap:16px; }
.page-header { display:flex; justify-content:space-between; align-items:flex-start; flex-wrap:wrap; gap:12px; }
.page-header h1 { font-size:24px; font-weight:700; }
.page-subtitle { color:var(--text-muted); font-size:13px; margin-top:2px; }

.ram-pill { display:flex; align-items:center; gap:6px; padding:8px 16px; background:var(--bg-secondary); border:1px solid var(--border); border-radius:99px; }
.ram-label { font-size:11px; color:var(--text-muted); text-transform:uppercase; letter-spacing:.06em; }
.ram-val   { font-size:16px; font-weight:700; color:var(--success); }
.ram-total { font-size:12px; color:var(--text-muted); }

.modes-grid { display:grid; grid-template-columns:repeat(auto-fill,minmax(260px,1fr)); gap:16px; }
.mode-card { transition:all var(--transition-fast); }
.mode-card.active { border-color:var(--accent-primary); box-shadow:0 0 0 1px var(--accent-primary); }
.mode-icon { width:56px; height:56px; border-radius:16px; display:flex; align-items:center; justify-content:center; margin-bottom:12px; box-shadow:0 4px 12px rgba(0,0,0,.3); }
.mode-label { font-size:18px; font-weight:700; margin-bottom:8px; }
.mode-desc  { font-size:13px; color:var(--text-muted); margin-bottom:12px; line-height:1.5; }
.mode-actions-preview { display:flex; flex-wrap:wrap; gap:6px; }
.action-chip { font-size:11px; padding:2px 8px; background:var(--bg-tertiary); border:1px solid var(--border); border-radius:var(--radius-sm); color:var(--text-secondary); }

.before-after { display:flex; align-items:center; gap:12px; flex-wrap:wrap; padding:16px; background:var(--bg-tertiary); border-radius:var(--radius-md); margin-bottom:16px; }
.ba-block { flex:1; min-width:200px; display:flex; flex-direction:column; gap:6px; }
.ba-label { font-size:11px; font-weight:700; text-transform:uppercase; letter-spacing:.06em; color:var(--text-muted); }
.ba-label.after { color:var(--success); }
.ba-bar-wrap { height:6px; background:var(--bg-secondary); border-radius:99px; overflow:hidden; }
.ba-bar { height:100%; border-radius:99px; transition:width .5s ease; }
.ba-stat { font-size:12px; color:var(--text-secondary); }
.ba-arrow { display:flex; align-items:center; padding:0 4px; }
.ba-gain { width:100%; display:flex; justify-content:center; margin-top:4px; }

.result-grid { display:grid; grid-template-columns:1fr 1fr; gap:16px; }
.result-section { display:flex; flex-direction:column; gap:6px; }
.result-title { font-size:12px; font-weight:700; text-transform:uppercase; letter-spacing:.06em; margin-bottom:4px; }
.result-title.ok   { color:var(--success); }
.result-title.warn { color:var(--warning); }
.result-item { display:flex; align-items:center; gap:6px; font-size:13px; padding:4px 0; }
.result-item.ok   { color:var(--text-secondary); }
.result-item.warn { color:var(--warning); }
.result-item.muted{ color:var(--text-muted); }

.quick-opts-grid { display:flex; flex-direction:column; gap:8px; }
.quick-opt-card { display:flex; align-items:center; gap:12px; padding:12px 14px; background:var(--bg-tertiary); border:1px solid var(--border); border-radius:var(--radius-md); transition:all var(--transition-fast); }
.quick-opt-card:hover { border-color:var(--border-hover); }
.quick-opt-card.opt-done { border-color:rgba(34,197,94,.3); background:rgba(34,197,94,.04); }
.quick-opt-card.opt-err  { border-color:rgba(239,68,68,.3);  background:rgba(239,68,68,.04); }
.opt-icon { width:36px; height:36px; border-radius:9px; display:flex; align-items:center; justify-content:center; flex-shrink:0; }
.opt-body { flex:1; min-width:0; }
.opt-label { font-size:13px; font-weight:600; margin-bottom:2px; }
.opt-desc  { font-size:11px; color:var(--text-muted); line-height:1.4; }
.opt-result { display:flex; align-items:center; gap:4px; font-size:11px; margin-top:4px; }
.res-ok  { color:var(--success); }
.res-err { color:var(--danger); }

/* ─── Historique gains ──────────────────────────────────────────────────── */
.gains-summary { display:flex; flex-direction:column; gap:6px; }
.gain-row {
  display:flex; align-items:center; gap:10px; padding:8px 12px;
  background:var(--bg-tertiary); border:1px solid var(--border);
  border-radius:var(--radius-sm); font-size:12px;
}
.gain-mode {
  padding:2px 8px; border-radius:var(--radius-sm);
  font-size:11px; font-weight:700; text-transform:uppercase; letter-spacing:.04em;
}
.gain-mode--turbo  { background:rgba(249,115,22,.15); color:#f97316; }
.gain-mode--gaming { background:rgba(139,92,246,.15);  color:#8b5cf6; }
.gain-mode--work   { background:rgba(59,130,246,.15);  color:#3b82f6; }
.gain-mode--eco    { background:rgba(34,197,94,.15);   color:#22c55e; }
.gain-date { color:var(--text-muted); font-size:11px; flex:1; }
.gain-ram  { color:var(--success); font-weight:700; min-width:60px; text-align:right; }
.gain-ram.muted { color:var(--text-muted); }
.gain-acts { color:var(--text-secondary); min-width:50px; text-align:right; }

.gains-table-wrap { overflow-x:auto; }
.gains-table {
  width:100%; border-collapse:collapse; font-size:12px;
}
.gains-table th {
  padding:7px 12px; text-align:left; font-size:10px; font-weight:700;
  text-transform:uppercase; letter-spacing:.06em; color:var(--text-muted);
  background:var(--bg-tertiary); border-bottom:1px solid var(--border);
}
.gains-table td {
  padding:7px 12px; border-bottom:1px solid var(--border);
  color:var(--text-secondary);
}
.gains-table tr:last-child td { border-bottom:none; }
.gains-table tr:hover td { background:var(--surface-glass); }
.g-date { color:var(--text-muted); font-size:11px; white-space:nowrap; }
.g-ram  { color:var(--success); font-weight:700; }
.g-acts { color:var(--accent-primary); font-weight:600; }
</style>
