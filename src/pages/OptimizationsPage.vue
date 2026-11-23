<script setup lang="ts">
defineOptions({ name: "OptimizationsPage" });
import { ref, computed, onMounted, type Component } from "vue";
import { invoke, isTauriContext } from "@/utils/invoke";
import NCard from "@/components/ui/NCard.vue";
import NButton from "@/components/ui/NButton.vue";
import NSpinner from "@/components/ui/NSpinner.vue";
import DiagBanner from "@/components/ui/DiagBanner.vue";
import { useNotificationStore } from "@/stores/notifications";
import BrowserCleanupCard from "@/components/optimizations/BrowserCleanupCard.vue";
import StartupProgramsCard from "@/components/optimizations/StartupProgramsCard.vue";
import TurboModePage from "@/pages/TurboModePage.vue";
import {
  Trash2, FileX, HardDrive, RefreshCw,
  Rocket, CheckCircle, Play, AlertCircle,
  Shield, Wifi, Zap,
} from "lucide-vue-next";

type MainTab = "optimizations" | "turbo";
const mainTab = ref<MainTab>("optimizations");

const notify = useNotificationStore();

// --- Quick actions ---
interface ActionResult {
  loading: boolean;
  done: boolean;
  message: string;
}

const recycleBin = ref<ActionResult>({ loading: false, done: false, message: "" });
const tempFiles = ref<ActionResult>({ loading: false, done: false, message: "" });
const diskCleanup = ref<ActionResult>({ loading: false, done: false, message: "" });

async function emptyRecycleBin() {
  recycleBin.value = { loading: true, done: false, message: "" };
  try {
    const result = await invoke<{ message: string }>("empty_recycle_bin");
    recycleBin.value = { loading: false, done: true, message: result.message };
    notify.success("Corbeille videe", result.message);
    logAction("Vider la corbeille", true);
  } catch (e: unknown) {
    if (!isTauriContext()) {
      recycleBin.value = { loading: false, done: true, message: "Corbeille videe (demo)" };
      notify.info("Mode dev", "Simulation : corbeille videe");
    } else {
      const msg = e instanceof Error ? e.message : String(e);
      recycleBin.value = { loading: false, done: false, message: msg };
      notify.error("Erreur corbeille", msg.slice(0, 120));
    }
    logAction("Vider la corbeille", false);
  }
}

async function cleanTempFiles() {
  tempFiles.value = { loading: true, done: false, message: "" };
  try {
    const result = await invoke<{ files_deleted: number; space_freed: string; message: string }>("clean_temp_files");
    tempFiles.value = { loading: false, done: true, message: result.message };
    notify.success("Nettoyage termine", result.message);
    logAction("Nettoyer fichiers temp", true);
  } catch (e: unknown) {
    if (!isTauriContext()) {
      tempFiles.value = { loading: false, done: true, message: "247 fichiers supprimes, 1.2 GB liberes (demo)" };
      notify.info("Mode dev", "Simulation : fichiers temp nettoyes");
    } else {
      const msg = e instanceof Error ? e.message : String(e);
      tempFiles.value = { loading: false, done: false, message: msg };
      notify.error("Nettoyage temp", msg.slice(0, 120));
    }
    logAction("Nettoyer fichiers temp", false);
  }
}

async function runDiskCleanup() {
  diskCleanup.value = { loading: true, done: false, message: "" };
  try {
    const result = await invoke<{ message: string }>("run_disk_cleanup");
    diskCleanup.value = { loading: false, done: true, message: result.message };
    notify.success("Nettoyage disque", result.message);
    logAction("Nettoyage de disque", true);
  } catch (e: unknown) {
    if (!isTauriContext()) {
      diskCleanup.value = { loading: false, done: true, message: "Nettoyage de disque termine (demo)" };
      notify.info("Mode dev", "Simulation : nettoyage disque effectue");
    } else {
      const msg = e instanceof Error ? e.message : String(e);
      diskCleanup.value = { loading: false, done: false, message: msg };
      notify.error("Nettoyage disque", msg.slice(0, 120));
    }
    logAction("Nettoyage de disque", false);
  }
}

// === Historique des actions ===
const actionHistory = ref<Array<{ action: string; timestamp: string; success: boolean }>>([]);
const showHistory = ref(false);

function logAction(actionLabel: string, success: boolean) {
  actionHistory.value.unshift({
    action: actionLabel,
    timestamp: new Date().toLocaleString('fr-FR'),
    success,
  });
  if (actionHistory.value.length > 50) actionHistory.value.pop();
  localStorage.setItem('nitrite-optim-history', JSON.stringify(actionHistory.value.slice(0, 50)));
}

function loadActionHistory() {
  try {
    const raw = localStorage.getItem('nitrite-optim-history');
    actionHistory.value = raw ? JSON.parse(raw) : [];
  } catch { actionHistory.value = []; }
}

function clearActionHistory() {
  actionHistory.value = [];
  localStorage.removeItem('nitrite-optim-history');
}

// === Debloat ===
interface DebloatResult { action: string; success: boolean; message: string; }
interface DebloatBtn {
  id: string; label: string; icon: Component;
  cmd: string; param?: string;
  loading: boolean; result: DebloatResult | null;
}

function db(id: string, label: string, icon: Component, cmd = "debloat_run_extra"): DebloatBtn {
  return { id, label, icon, cmd, param: cmd === "debloat_run_extra" ? id : undefined, loading: false, result: null };
}

const debloatBtns = ref<DebloatBtn[]>([
  db("telemetry", "Désactiver Télémétrie", Shield, "debloat_disable_telemetry"),
  db("cortana", "Désactiver Cortana", Shield, "debloat_disable_cortana"),
  db("xbox", "Services Xbox", Shield, "debloat_disable_xbox"),
  db("superfetch", "Désactiver SysMain", Zap, "debloat_disable_superfetch"),
  db("tips", "Conseils Windows", Shield, "debloat_disable_tips"),
  db("bloatware", "Supprimer Bloatware UWP", Trash2, "debloat_remove_bloatware"),
  db("disable_gamebar", "Game Bar & DVR", Shield),
  db("disable_bing_search", "Désactiver Bing Start", Shield),
  db("disable_widgets", "Désactiver Widgets", Shield),
  db("disable_ads", "Publicités Windows", Shield),
  db("disable_activity_history", "Historique Activité", Shield),
  db("disable_remote_assistance", "Assistance à Distance", Shield),
  db("disable_startup_sound", "Son Démarrage", Shield),
  db("clear_prefetch", "Vider Prefetch", Trash2),
  db("remove_teams", "Supprimer Teams", Trash2),
  db("disable_recall", "Désactiver Recall/IA", Shield),
  db("disable_ink_workspace", "Windows Ink Workspace", Shield),
  db("disable_location", "Service Localisation", Shield),
  db("disable_feedback", "Retours Expérience", Shield),
  db("disable_consumer_features", "Promos Windows", Shield),
]);

const networkBtns = ref<DebloatBtn[]>([
  db("dns", "Flush DNS", Wifi, "debloat_flush_dns"),
  db("winsock", "Reset Winsock + TCP/IP", Wifi, "debloat_reset_network"),
  db("renew_ip", "Renouveler IP (release/renew)", Wifi),
  db("register_dns", "Réenregistrer DNS", Wifi),
  db("disable_ipv6", "Désactiver IPv6", Wifi),
  db("disable_teredo", "Désactiver Teredo", Wifi),
  db("disable_llmnr", "Désactiver LLMNR", Shield),
  db("disable_netbios", "Désactiver NetBIOS TCP", Shield),
  db("reset_firewall", "Reset Pare-feu", Shield),
  db("disable_wifi_sense", "Désactiver Wi-Fi Sense", Wifi),
  db("disable_nagle", "Désactiver Nagle (gaming)", Zap),
  db("purge_arp", "Purger Cache ARP", Wifi),
  db("set_dns_google", "DNS Google (8.8.8.8)", Wifi),
  db("set_dns_cloudflare", "DNS Cloudflare (1.1.1.1)", Wifi),
  db("optimize_mtu", "Optimiser MTU (1500)", Wifi),
  db("show_net_stats", "Statistiques Réseau", Wifi),
  db("reset_winsock_only", "Reset Winsock seul", Wifi),
  db("reset_tcpip_only", "Reset TCP/IP seul", Wifi),
  db("disable_rdp", "Désactiver Bureau à Distance", Shield),
  db("disable_mdns", "Désactiver mDNS local", Shield),
]);

const perfBtns = ref<DebloatBtn[]>([
  db("power", "Plan Haute Performance", Zap, "debloat_optimize_power"),
  db("visual", "Réduire Effets Visuels", Zap, "debloat_disable_visual_effects"),
  db("trim", "Activer TRIM SSD", HardDrive, "debloat_enable_trim"),
  db("wucache", "Vider Cache Windows Update", Trash2, "debloat_clear_wu_cache"),
  db("evtlogs", "Vider Journaux Événements", FileX, "debloat_clear_event_logs"),
  db("empty_standby", "Compacter RAM des processus", Zap),
  db("disable_search_index", "Désactiver Indexation", HardDrive),
  db("disable_error_reporting", "Désactiver Rapport Erreurs", Shield),
  db("boost_foreground", "Priorité Apps Foreground", Zap),
  db("disable_auto_maintenance", "Désactiver Maintenance Auto", RefreshCw),
  db("disable_bg_apps", "Apps Arrière-Plan OFF", Shield),
  db("disable_start_animations", "Animations Menu OFF", Zap),
  db("optimize_pagefile", "Fichier Échange Auto", HardDrive),
  db("disable_hpet", "Optimiser Timer (Gaming)", Zap),
  db("clean_old_shadows", "Supprimer Vieux Shadows", Trash2),
  db("set_power_min_processor", "CPU Min 5% (anti-throttle)", Zap),
  db("clear_prefetch", "Vider Prefetch", Trash2),
  db("disable_feedback", "Arrêter Télémétrie CEIP", Shield),
  db("disable_activity_history", "Désactiver Timeline", Shield),
  db("disable_gamebar", "Overlay Gaming OFF", Shield),
]);

async function runDebloat(btn: DebloatBtn) {
  btn.loading = true; btn.result = null;
  try {
    let res: DebloatResult | DebloatResult[];
    if (btn.id === "bloatware") {
      res = await invoke<DebloatResult[]>("debloat_remove_bloatware", { apps: [] });
    } else if (btn.param) {
      res = await invoke<DebloatResult>("debloat_run_extra", { action: btn.param });
    } else {
      res = await invoke<DebloatResult>(btn.cmd);
    }
    if (Array.isArray(res)) {
      const ok = res.filter(r => r.success).length;
      btn.result = { action: btn.label, success: ok > 0, message: `${ok}/${res.length} supprimés` };
    } else {
      btn.result = res as DebloatResult;
    }
    if (btn.result.success) { notify.success(btn.label, btn.result.message); logAction(btn.label, true); }
    else { notify.warning(btn.label, btn.result.message); logAction(btn.label, false); }
  } catch (e: unknown) {
    btn.result = { action: btn.label, success: false, message: (e instanceof Error ? e.message : String(e)) || "Erreur" };
    notify.error(btn.label);
    logAction(btn.label, false);
  } finally { btn.loading = false; }
}

onMounted(loadActionHistory);
</script>

<template>
  <div class="optimizations">
    <!-- Banner -->
    <DiagBanner
      :icon="Rocket"
      title="Optimisations Système"
      desc="Nettoyage, débloatware, réseau, démarrage et mode turbo"
      color="orange"
    />

    <!-- Onglets principaux -->
    <div class="main-tabs">
      <button class="main-tab" :class="{ active: mainTab === 'optimizations' }" @click="mainTab = 'optimizations'">
        <Zap :size="14" /> Optimisations
      </button>
      <button class="main-tab" :class="{ active: mainTab === 'turbo' }" @click="mainTab = 'turbo'">
        <Rocket :size="14" /> Mode Turbo
      </button>
    </div>

    <!-- Contenu Mode Turbo -->
    <TurboModePage v-if="mainTab === 'turbo'" style="padding:0" />

    <!-- Contenu Optimisations -->
    <template v-if="mainTab === 'optimizations'">

    <!-- Historique des optimisations -->
    <NCard v-if="actionHistory.length > 0">
      <template #header>
        <div style="display:flex;align-items:center;gap:8px;cursor:pointer;width:100%" @click="showHistory = !showHistory">
          <span style="font-size:13px;font-weight:600">Historique des actions ({{ actionHistory.length }})</span>
          <span style="margin-left:auto;color:var(--text-muted)">{{ showHistory ? '▲' : '▼' }}</span>
          <button @click.stop="clearActionHistory()"
            style="font-size:11px;padding:2px 8px;border:1px solid var(--border);border-radius:4px;background:none;color:var(--text-muted);cursor:pointer;margin-left:8px">
            Effacer
          </button>
        </div>
      </template>
      <div v-if="showHistory" style="display:flex;flex-direction:column;gap:4px;max-height:200px;overflow-y:auto">
        <div v-for="(h, i) in actionHistory" :key="i"
          style="display:flex;align-items:center;gap:8px;padding:5px 8px;border-radius:5px;font-size:12px"
          :style="{ background: h.success ? 'rgba(34,197,94,.05)' : 'rgba(239,68,68,.05)' }">
          <span :style="{ color: h.success ? 'var(--success)' : 'var(--danger)' }">{{ h.success ? '✓' : '✕' }}</span>
          <span style="flex:1">{{ h.action }}</span>
          <span style="color:var(--text-muted);font-size:11px">{{ h.timestamp }}</span>
        </div>
      </div>
    </NCard>

    <!-- Section: Actions rapides -->
    <div class="diag-section-label">
      <Trash2 :size="13" />
      Actions Rapides
    </div>

    <!-- Quick Actions -->
    <div class="actions-grid">
      <!-- Corbeille -->
      <NCard class="action-ncard">
        <div class="action-card">
          <div class="action-icon-wrap" style="background: linear-gradient(135deg, rgba(239,68,68,.18), rgba(185,28,28,.08)); border: 1px solid rgba(239,68,68,.25);">
            <Trash2 :size="24" style="color: var(--danger)" />
          </div>
          <h3>Vider la corbeille</h3>
          <p class="action-desc">Supprime definitivement les fichiers de la corbeille pour liberer de l'espace disque.</p>
          <div v-if="recycleBin.done" class="action-result">
            <CheckCircle :size="14" style="color: var(--success)" />
            <span>{{ recycleBin.message }}</span>
          </div>
          <NButton
            variant="primary"
            size="sm"
            :loading="recycleBin.loading"
            :disabled="recycleBin.loading"
            @click="emptyRecycleBin"
          >
            <Play :size="14" />
            Executer
          </NButton>
        </div>
      </NCard>

      <!-- Temp files -->
      <NCard class="action-ncard">
        <div class="action-card">
          <div class="action-icon-wrap" style="background: linear-gradient(135deg, rgba(245,158,11,.18), rgba(180,83,9,.08)); border: 1px solid rgba(245,158,11,.25);">
            <FileX :size="24" style="color: var(--warning)" />
          </div>
          <h3>Nettoyer fichiers temp</h3>
          <p class="action-desc">Supprime les fichiers temporaires du systeme et des applications (TEMP, cache, prefetch).</p>
          <div v-if="tempFiles.done" class="action-result">
            <CheckCircle :size="14" style="color: var(--success)" />
            <span>{{ tempFiles.message }}</span>
          </div>
          <NButton
            variant="primary"
            size="sm"
            :loading="tempFiles.loading"
            :disabled="tempFiles.loading"
            @click="cleanTempFiles"
          >
            <Play :size="14" />
            Executer
          </NButton>
        </div>
      </NCard>

      <!-- Disk cleanup -->
      <NCard class="action-ncard">
        <div class="action-card">
          <div class="action-icon-wrap" style="background: linear-gradient(135deg, rgba(249,115,22,.18), rgba(194,65,12,.08)); border: 1px solid rgba(249,115,22,.25);">
            <HardDrive :size="24" style="color: var(--accent-primary)" />
          </div>
          <h3>Nettoyage de disque</h3>
          <p class="action-desc">Lance l'utilitaire Windows de nettoyage de disque pour un nettoyage approfondi.</p>
          <div v-if="diskCleanup.done" class="action-result">
            <CheckCircle :size="14" style="color: var(--success)" />
            <span>{{ diskCleanup.message }}</span>
          </div>
          <NButton
            variant="primary"
            size="sm"
            :loading="diskCleanup.loading"
            :disabled="diskCleanup.loading"
            @click="runDiskCleanup"
          >
            <Play :size="14" />
            Executer
          </NButton>
        </div>
      </NCard>
    </div>

    <!-- Section: Navigateurs -->
    <div class="diag-section-label">
      <Zap :size="13" /> Caches Navigateurs
    </div>
    <BrowserCleanupCard />

    <!-- Section: Debloat -->
    <div class="diag-section-label">
      <Shield :size="13" />
      Debloat Windows
    </div>

    <!-- Debloat Windows -->
    <NCard>
      <template #header>
        <div class="section-header">
          <div class="section-icon-badge" style="background: linear-gradient(135deg, #8b5cf6, #6d28d9);">
            <Shield :size="14" style="color:#fff" />
          </div>
          <span>Debloat Windows</span>
        </div>
      </template>
      <div class="debloat-grid">
        <div v-for="btn in debloatBtns" :key="btn.id" class="debloat-item">
          <NButton variant="secondary" size="sm" :loading="btn.loading" @click="runDebloat(btn)">
            <component :is="btn.icon" :size="13" /> {{ btn.label }}
          </NButton>
          <div v-if="btn.result" class="debloat-result" :class="btn.result.success ? 'res-ok' : 'res-err'">
            <component :is="btn.result.success ? CheckCircle : AlertCircle" :size="12" />
            <span>{{ btn.result.message }}</span>
          </div>
        </div>
      </div>
    </NCard>

    <!-- Section: Réseau -->
    <div class="diag-section-label">
      <Wifi :size="13" />
      Optimisations Réseau
    </div>

    <!-- Réseau -->
    <NCard>
      <template #header>
        <div class="section-header">
          <div class="section-icon-badge" style="background: linear-gradient(135deg, #06b6d4, #0e7490);">
            <Wifi :size="14" style="color:#fff" />
          </div>
          <span>Réseau</span>
        </div>
      </template>
      <div class="debloat-grid">
        <div v-for="btn in networkBtns" :key="btn.id" class="debloat-item">
          <NButton variant="secondary" size="sm" :loading="btn.loading" @click="runDebloat(btn)">
            <component :is="btn.icon" :size="13" /> {{ btn.label }}
          </NButton>
          <div v-if="btn.result" class="debloat-result" :class="btn.result.success ? 'res-ok' : 'res-err'">
            <component :is="btn.result.success ? CheckCircle : AlertCircle" :size="12" />
            <span>{{ btn.result.message }}</span>
          </div>
        </div>
      </div>
    </NCard>

    <!-- Section: Performances -->
    <div class="diag-section-label">
      <Zap :size="13" />
      Performances
    </div>

    <!-- Performances -->
    <NCard>
      <template #header>
        <div class="section-header">
          <div class="section-icon-badge" style="background: linear-gradient(135deg, #f59e0b, #b45309);">
            <Zap :size="14" style="color:#fff" />
          </div>
          <span>Performances</span>
        </div>
      </template>
      <div class="debloat-grid">
        <div v-for="btn in perfBtns" :key="btn.id" class="debloat-item">
          <NButton variant="secondary" size="sm" :loading="btn.loading" @click="runDebloat(btn)">
            <component :is="btn.icon" :size="13" /> {{ btn.label }}
          </NButton>
          <div v-if="btn.result" class="debloat-result" :class="btn.result.success ? 'res-ok' : 'res-err'">
            <component :is="btn.result.success ? CheckCircle : AlertCircle" :size="12" />
            <span>{{ btn.result.message }}</span>
          </div>
        </div>
      </div>
    </NCard>

    <!-- Section: Démarrage -->
    <div class="diag-section-label">
      <Rocket :size="13" /> Programmes au Démarrage
    </div>
    <StartupProgramsCard />

    </template><!-- end optimizations tab -->
  </div>
</template>

<style scoped>
.optimizations {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.main-tabs {
  display: flex;
  gap: 4px;
  background: var(--bg-secondary);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  padding: 4px;
  width: fit-content;
}
.main-tab {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 7px 18px;
  border-radius: var(--radius-md);
  border: none;
  background: transparent;
  color: var(--text-secondary);
  font-size: 13px;
  font-family: inherit;
  cursor: pointer;
  transition: all .15s;
}
.main-tab:hover { color: var(--text-primary); background: var(--bg-tertiary); }
.main-tab.active { background: var(--accent-muted); color: var(--accent-primary); font-weight: 600; }

/* Section labels */
.diag-section-label {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--text-secondary);
  border-left: 3px solid var(--accent-primary);
  padding-left: 10px;
  margin-top: 4px;
}

/* Quick Actions */
.actions-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 16px;
}

@media (max-width: 1000px) {
  .actions-grid { grid-template-columns: 1fr; }
}

.action-ncard {
  border-radius: 12px !important;
  border: 1px solid var(--border-hover) !important;
}

.action-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  text-align: center;
  padding: 8px 0;
}

.action-icon-wrap {
  width: 60px;
  height: 60px;
  border-radius: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.action-card h3 {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
}

.action-desc {
  font-size: 12px;
  color: var(--text-secondary);
  line-height: 1.5;
  max-width: 240px;
}

.action-result {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--success);
  background: var(--success-muted);
  padding: 6px 12px;
  border-radius: var(--radius-md);
}

/* Section header */
.section-header {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
}

.section-icon-badge {
  width: 26px;
  height: 26px;
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

/* Loading / empty */
.loading-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 40px;
  color: var(--text-secondary);
}

.empty-state {
  color: var(--text-secondary);
  font-size: 13px;
  text-align: center;
  padding: 24px;
}

/* Startup table */
.startup-table-wrap {
  overflow-x: auto;
}

.startup-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}

.startup-table th {
  text-align: left;
  padding: 8px 12px;
  color: var(--text-secondary);
  font-weight: 600;
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.07em;
  border-bottom: 1px solid var(--border);
}

.startup-table td {
  padding: 10px 12px;
  border-bottom: 1px solid var(--border);
  color: var(--text-secondary);
}

.startup-table tr:last-child td {
  border-bottom: none;
}

.startup-table tr:hover td {
  background: var(--bg-tertiary);
}

.prog-name {
  font-weight: 600;
  color: var(--text-primary) !important;
  white-space: nowrap;
}

.prog-cmd {
  font-size: 11px;
  max-width: 300px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-secondary);
}

.prog-loc {
  font-size: 11px;
  max-width: 280px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-secondary);
}

.font-mono {
  font-family: "JetBrains Mono", monospace;
}

/* Browser Cleanup */
.cache-total {
  font-size: 12px;
  color: var(--warning);
  font-weight: 700;
  background: var(--warning-muted);
  padding: 2px 8px;
  border-radius: var(--radius-sm);
}

.browser-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.browser-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 14px;
  border: 1px solid transparent;
  border-radius: 10px;
  background: transparent;
  cursor: pointer;
  font-family: inherit;
  text-align: left;
  width: 100%;
  transition: all var(--transition-fast);
  color: var(--text-secondary);
}

.browser-item:hover { background: var(--bg-tertiary); border-color: var(--border); }
.browser-item.selected { background: color-mix(in srgb, var(--accent-primary) 6%, transparent); border-color: color-mix(in srgb, var(--accent-primary) 30%, transparent); }
.browser-item.selected .browser-check { color: var(--accent-primary); }
.browser-item:not(.selected) .browser-check { color: var(--text-secondary); }

.browser-info {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex: 1;
}

.browser-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--text-primary);
}

.browser-size {
  font-size: 12px;
  color: var(--warning);
  font-weight: 600;
  font-family: "JetBrains Mono", monospace;
}

.browser-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 12px;
  padding-top: 12px;
  margin-top: 8px;
  border-top: 1px solid var(--border);
}

.clean-result {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--success);
}

/* Debloat */
.debloat-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 8px; }
.debloat-item { display: flex; flex-direction: column; gap: 4px; }
.debloat-result { display: flex; align-items: center; gap: 5px; font-size: 11px; padding: 3px 8px; border-radius: var(--radius-sm); }
.res-ok { color: var(--success); background: var(--success-muted); }
.res-err { color: var(--danger); background: var(--danger-muted); }
</style>
