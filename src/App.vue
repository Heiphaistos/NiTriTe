<script setup lang="ts">
import { ref, computed, provide, onMounted, onUnmounted, onErrorCaptured, nextTick } from "vue";
import { useRoute, useRouter } from "vue-router";
import NToast from "@/components/ui/NToast.vue";
import MissionBar from "@/components/mission/MissionBar.vue";
import NAlertBanner from "@/components/ui/NAlertBanner.vue";
import SearchModal from "@/components/shared/SearchModal.vue";
import KeyboardShortcutsModal from "@/components/ui/KeyboardShortcutsModal.vue";
import { useAppStore } from "@/stores/app";
import { useLayoutStore } from "@/stores/layoutStore";
import { useUiModelStore, UI_MODELS } from "@/stores/uiModel";
import { navigationSections } from "@/data/navigation";
import { shellFor } from "@/components/shells";
import { useDataCache } from "@/stores/dataCache";
import { useProactiveAlerts } from "@/composables/useProactiveAlerts";
import { logger } from "@/utils/logger";
import { sdiRelease } from "@/utils/sdiGuard";
import { checkForUpdate } from "@/composables/useAutoUpdate";
import { runLimited, startupConcurrency, pollMultiplier } from "@/utils/perfProfile";

const { start: startAlerts, stop: stopAlerts } = useProactiveAlerts();
const appVersion = __APP_VERSION__;
function handleKeyDown(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key === "k") { e.preventDefault(); searchOpen.value = !searchOpen.value; }
  // Ctrl+1 … Ctrl+9, Ctrl+0 : premier outil de la section correspondante (10 sections).
  if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey && /^[0-9]$/.test(e.key)) {
    const section = navigationSections[e.key === "0" ? 9 : Number(e.key) - 1];
    if (section) { e.preventDefault(); router.push(section.items[0].route); }
    return;
  }
  // Ctrl+Maj+M : modèle d'interface suivant.
  if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === "m") {
    e.preventDefault();
    const i = UI_MODELS.findIndex(m => m.id === uiModelStore.model);
    uiModelStore.setModel(UI_MODELS[(i + 1) % UI_MODELS.length].id);
    return;
  }
  // Alt+← / Alt+→ : page précédente / suivante.
  if (e.altKey && !e.ctrlKey && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
    e.preventDefault();
    if (e.key === "ArrowLeft") router.back(); else router.forward();
    return;
  }
  if ((e.ctrlKey || e.metaKey) && e.key === "b") { e.preventDefault(); toggleSidebar(); localStorage.setItem("nitrite-sidebar", String(sidebarCollapsed.value)); }
  if (e.key === "?" && !e.ctrlKey && !e.metaKey && !e.altKey) {
    const tag = (e.target as HTMLElement)?.tagName;
    if (tag !== "INPUT" && tag !== "TEXTAREA" && tag !== "SELECT") { e.preventDefault(); shortcutsOpen.value = !shortcutsOpen.value; }
  }
}

onUnmounted(() => { stopAlerts(); window.removeEventListener("keydown", handleKeyDown); });

const route       = useRoute();
const router      = useRouter();

// Pages exécutant des opérations longues (scan, mises à jour, install, nettoyage…) :
// mises en cache par <keep-alive> pour que l'opération et son état survivent à la
// navigation (sinon le composant est détruit et l'UI se réinitialise). Les pages
// purement « live » (Monitoring, Températures, Dashboard…) ne sont PAS listées et
// se rafraîchissent normalement à chaque entrée.
const persistentPages = [
  "DiagnosticPage", "OptimizationsPage", "MasterInstallPage", "UpdatesPage",
  "DriversPage", "BackupPage", "UninstallerPage", "ClonePage", "DataRecoveryPage",
  "ScanVirusPage", "BenchmarkPage", "CleanerPage", "TurboModePage",
  "DependencyManagerPage", "DuplicateFinderPage", "BigFilesFinderPage",
  "DiskVisualizerPage", "WinPEModePage", "RestorePointsPage", "PortScannerPage",
];
const appContent  = ref<HTMLElement | null>(null);
const appStore    = useAppStore();
const layoutStore = useLayoutStore();
const dataCache   = useDataCache();
const uiModelStore = useUiModelStore();
const shellComponent = computed(() => shellFor(uiModelStore.model));

const sidebarCollapsed = ref(false);
const searchOpen       = ref(false);
const shortcutsOpen    = ref(false);
const appReady         = ref(false);

// Vidéo de fond du splash : une fois terminée (elle ne boucle pas), on la
// démonte et on affiche sa dernière frame en image statique — évite de
// garder un <video> arrêté en mémoire si le chargement traîne encore.
// Profil léger : pas de décodage vidéo pendant le démarrage, image fixe directe.
const splashVideoEnded = ref(appStore.perfTier === "light");

// ── Preloader ──────────────────────────────────────────────────────────────────
interface LoadTask { label: string; status: "pending" | "running" | "done" | "error" }

const loadTasks = ref<LoadTask[]>([
  { label: "Interface & thème",              status: "pending" },
  { label: "Monitoring système",             status: "pending" },
  { label: "Informations CPU",               status: "pending" },
  { label: "Informations mémoire RAM",       status: "pending" },
  { label: "Disques & volumes",              status: "pending" },
  { label: "Réseau & connexions",            status: "pending" },
  { label: "GPU & affichage",                status: "pending" },
  { label: "Comptes utilisateurs",           status: "pending" },
  { label: "Logiciels installés",            status: "pending" },
  { label: "Pilotes système",                status: "pending" },
  { label: "Processus actifs",               status: "pending" },
  { label: "Services Windows",               status: "pending" },
  { label: "Journaux d'événements",          status: "pending" },
  { label: "Pare-feu & règles réseau",       status: "pending" },
  { label: "Licence & activation",           status: "pending" },
  { label: "Historique BSOD",                status: "pending" },
  { label: "Points de restauration",         status: "pending" },
  { label: "Bluetooth & périphériques",      status: "pending" },
  { label: "Partages réseau",                status: "pending" },
  { label: "Certificats système",            status: "pending" },
  { label: "Tâches planifiées",              status: "pending" },
  { label: "Variables d'environnement",      status: "pending" },
  { label: "Informations BIOS & carte mère", status: "pending" },
  { label: "Assistant IA",                   status: "pending" },
]);

/** Attente maximale d'une sonde au demarrage avant de passer a la suivante. */
const STARTUP_PROBE_WAIT_MS = 8000;

const doneCount    = computed(() => loadTasks.value.filter(t => t.status === "done" || t.status === "error").length);
const loadProgress = computed(() => Math.round((doneCount.value / loadTasks.value.length) * 100));
const currentLabel = computed(() => {
  const running = loadTasks.value.find(t => t.status === "running");
  return running?.label ?? (loadProgress.value === 100 ? "Prêt !" : "Chargement…");
});

// ── Layout ────────────────────────────────────────────────────────────────────
function toggleSidebar() { sidebarCollapsed.value = !sidebarCollapsed.value; }
function openSearch()    { searchOpen.value = true; }

provide("sidebarCollapsed", sidebarCollapsed);
provide("toggleSidebar",    toggleSidebar);
provide("openSearch",       openSearch);


// ── Scroll reset automatique à chaque navigation ─────────────────────────────
router.afterEach(() => {
  pageError.value = null;
  if (appContent.value) appContent.value.scrollTop = 0;
});

// ── Capture d'erreurs pages (diagnostic écrans noirs) ────────────────────────
const pageError = ref<{ message: string; stack?: string } | null>(null);

onErrorCaptured((err: unknown, _instance, info) => {
  const msg = err instanceof Error ? err.message : String(err);
  const stack = err instanceof Error ? err.stack : undefined;
  console.error("[Nitrite] Erreur Vue capturée :", info, err);
  logger.vue(info, err);
  pageError.value = { message: `[${info}] ${msg}`, stack };
  return false;
});

// Capture les erreurs de chargement de route (import() raté)
router.onError((err) => {
  const msg = err instanceof Error ? err.message : String(err);
  console.error("[Nitrite] Erreur Router :", err);
  logger.router(err);
  pageError.value = { message: `[router] ${msg}`, stack: err instanceof Error ? err.stack : undefined };
});

// ── Démarrage ─────────────────────────────────────────────────────────────────
onMounted(async () => {
  startAlerts(60000);
  // Masquer le préloader HTML natif dès que Vue est prêt
  window.__hideNativeBoot?.();

  await nextTick();

  // ── Mise a jour, AVANT les lots de chargement ────────────────────────────
  // Elle etait appelee apres les 23 sondes systeme du demarrage, donc elle en
  // dependait : mesure sur deux lancements de la meme machine, 68 s puis
  // 3 min 45. Sur un poste ou une de ces sondes traine -- un depot WMI abime,
  // un service Windows Update fige -- la mise a jour n'etait jamais proposee de
  // la session. Elle n'a aucune raison d'attendre un inventaire de pilotes :
  // l'appel ne bloque pas (`void`), il vit sa vie pendant le chargement.
  void checkForUpdate();

  // ── Tâche 0 : Interface (synchrone) ──
  loadTasks.value[0].status = "running";
  appStore.loadSavedTheme();
  appStore.loadSidebarState();
  sidebarCollapsed.value = appStore.sidebarCollapsed;
  layoutStore.applyToDocument();
  uiModelStore.load();
  window.addEventListener("keydown", handleKeyDown);
  loadTasks.value[0].status = "done";

  // ── Import Tauri ──
  let inv: ((cmd: string, args?: Record<string, unknown>) => Promise<unknown>) | null = null;
  try { const api = await import("@tauri-apps/api/core"); inv = api.invoke; } catch { /* dev */ }

  // ── Wrapper : marque la tâche, invoque, met en cache, marque done ──
  const load = async (idx: number, cmd: string, args?: Record<string, unknown>) => {
    loadTasks.value[idx].status = "running";
    if (inv) {
      const key = args ? `${cmd}::${JSON.stringify(args)}` : cmd;
      // Une sonde lente (WMI fige, VM, vieux PC) ne bloque plus l'ecran de
      // chargement : passe le delai, on continue et le resultat arrive en
      // cache plus tard (sinon la page le redemandera).
      const call = inv(cmd, args).then(r => { dataCache.set(key, r); }, () => { /* non critique */ });
      await Promise.race([call, new Promise(r => setTimeout(r, STARTUP_PROBE_WAIT_MS))]);
      loadTasks.value[idx].status = "done";
    } else {
      await new Promise(r => setTimeout(r, 300));
      loadTasks.value[idx].status = "done";
    }
  };

  // ── Tâche 1 : Monitoring (doit démarrer en premier, fournit les events) ──
  await load(1, "start_monitoring", { intervalScale: pollMultiplier(appStore.perfTier) });

  // ── Tâches 2-23 : sondes système, avec un nombre limité en vol ──────────
  // Toutes en parallèle, elles lançaient ~10 PowerShell/WMI à la fois : un
  // double cœur saturait et l'interface figeait. Le plafond suit le profil de
  // performance (2 en léger, 3 en équilibré, 8 en complet). Ordre : les plus
  // rapides et les plus utiles d'abord.
  const probes: [number, string, Record<string, unknown>?][] = [
    [2, "get_system_info"], [3, "get_ram_detailed"], [4, "get_storage_physical_info"],
    [5, "get_network_overview"], [6, "get_gpu_detailed"], [7, "get_user_accounts"],
    [8, "get_apps"], [9, "get_sys_drivers_list"], [10, "get_running_processes"],
    [11, "get_windows_services"], [12, "get_event_logs", { logName: "System", count: 50 }],
    [13, "get_firewall_rules"], [14, "get_windows_license"], [15, "get_bsod_history"],
    [16, "list_restore_points_cmd"], [17, "get_bluetooth_info"], [18, "get_network_shares"],
    [19, "get_certificates"], [20, "get_scheduled_tasks"], [21, "get_environment_variables"],
    [22, "get_bios_info"], [23, "ai_find_llamacpp_server"],
  ];
  await runLimited(probes.map(([idx, cmd, args]) => () => load(idx, cmd, args)), startupConcurrency(appStore.perfTier));

  // ── Handler fermeture ──
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const win = getCurrentWindow();
    await win.listen("tauri://close-requested", async () => {
      if (window.__nitrite_sdi_active) { sdiRelease(); return; }
      try { if (inv) await inv("cleanup_on_exit"); } catch { await win.destroy(); }
    });
  } catch { /* dev */ }

  // Transition vers l'app
  await new Promise(r => setTimeout(r, 400));
  appReady.value = true;
});
</script>

<template>
  <!-- ── Preloader ── -->
  <Transition name="splash">
    <div v-if="!appReady" class="splash-screen" :class="`splash--${uiModelStore.model}`">

      <!-- ── Fond vidéo (sans son) — se fige sur la dernière image si le
           chargement dure plus longtemps que la vidéo ── -->
      <video
        v-if="!splashVideoEnded"
        class="splash-bg-media"
        src="/splash.mp4"
        autoplay
        muted
        playsinline
        disablepictureinpicture
        preload="auto"
        @ended="splashVideoEnded = true"
      />
      <img
        v-else
        src="/splash-last-frame.jpg"
        class="splash-bg-media"
        alt=""
      />

      <!-- ── Fond statique ── -->
      <div class="splash-overlay" />

      <!-- ── Panel de chargement (bas de l'écran) ── -->
      <div class="splash-content">

        <!-- Logo + titre -->
        <div class="splash-brand">
          <div class="splash-brand-text">
            <div class="splash-title">NiTriTe</div>
            <div class="splash-version">v{{ appVersion }}</div>
          </div>
        </div>

        <!-- Modèle Orbital : progression en anneau -->
        <svg v-if="uiModelStore.model === 'orbital'" class="splash-ring" viewBox="0 0 120 120" aria-hidden="true">
          <circle cx="60" cy="60" r="52" class="splash-ring__track" />
          <circle cx="60" cy="60" r="52" class="splash-ring__fill" :stroke-dasharray="`${loadProgress * 3.267} 327`" transform="rotate(-90 60 60)" />
          <text x="60" y="66" text-anchor="middle" class="splash-ring__pct">{{ loadProgress }}%</text>
        </svg>

        <!-- Barre de progression -->
        <div v-else class="splash-progress-wrap">
          <div class="splash-progress-bar">
            <div class="splash-progress-fill" :style="{ width: `${loadProgress}%` }" />
          </div>
          <div class="splash-pct">{{ loadProgress }}%</div>
        </div>

        <!-- Label tâche active -->
        <div class="splash-label">{{ currentLabel }}</div>

        <!-- Grille de tâches (compact) -->
        <div class="splash-grid">
          <div
            v-for="(task, i) in loadTasks"
            :key="i"
            class="splash-task"
            :class="task.status"
          >
            <span class="task-icon">
              <svg v-if="task.status === 'done'" width="10" height="10" viewBox="0 0 10 10">
                <circle cx="5" cy="5" r="5" fill="#22c55e" fill-opacity="0.2"/>
                <path d="M2.5 5l1.7 1.7L7.5 3.3" stroke="#22c55e" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" fill="none"/>
              </svg>
              <svg v-else-if="task.status === 'running'" class="spin" width="10" height="10" viewBox="0 0 10 10">
                <circle cx="5" cy="5" r="4" stroke="var(--accent-primary, #f97316)" stroke-width="1.5" stroke-dasharray="16" stroke-dashoffset="8" stroke-linecap="round" fill="none"/>
              </svg>
              <span v-else class="task-dot" />
            </span>
            <span class="task-label">{{ task.label }}</span>
          </div>
        </div>

        <!-- Compteur -->
        <div class="splash-counter">{{ doneCount }} / {{ loadTasks.length }} modules chargés</div>
      </div>

    </div>
  </Transition>

  <NAlertBanner />

  <!-- ── Application ── -->
  <!-- La coque (modèle d'interface) n'entoure que la navigation : la zone de
       page ci-dessous (router-view, keep-alive, overlay d'erreur) est la même
       pour tous les modèles, donc chaque page garde ses fonctions et boutons.
       Elle est téléportée dans l'emplacement <PageSlot> de la coque : changer
       de modèle la déplace sans la recréer (opérations en cours préservées). -->
  <div
    v-if="appReady"
    class="app-root"
    :class="[`density-${layoutStore.state.density}`, `ui-model-${uiModelStore.model}`]"
    :data-density="layoutStore.state.density"
  >
    <component :is="shellComponent" />
    <Teleport defer :to="`[data-page-slot='${uiModelStore.model}']`">
      <main ref="appContent" class="app-content" :style="{ padding: `${layoutStore.state.contentPadding}px` }">
        <div
          class="app-content-inner"
          :style="{ maxWidth: layoutStore.state.contentMaxWidth === 'full' ? '100%' : layoutStore.state.contentMaxWidth, margin: '0 auto' }"
        >
          <MissionBar />
          <router-view v-slot="{ Component }">
            <transition name="page">
              <keep-alive :include="persistentPages">
                <component :is="Component" :key="route.path" />
              </keep-alive>
            </transition>
          </router-view>

          <!-- ── Overlay diagnostic erreur page ── -->
          <div v-if="pageError" class="page-error-overlay">
            <div class="page-error-box">
              <div class="page-error-title">⚠ Erreur de rendu détectée</div>
              <div class="page-error-msg">{{ pageError.message }}</div>
              <pre v-if="pageError.stack" class="page-error-stack">{{ pageError.stack }}</pre>
              <button class="page-error-dismiss" @click="pageError = null">Fermer</button>
            </div>
          </div>
        </div>
      </main>
    </Teleport>
  </div>

  <NToast />
  <SearchModal v-model="searchOpen" />
  <KeyboardShortcutsModal v-model="shortcutsOpen" />
</template>

<style scoped>
/* ── Splash ───────────────────────────────────────────────────────────────── */
.splash-screen {
  position: fixed; inset: 0; z-index: 99999;
  background: var(--bg-primary, #09090b); overflow: hidden;
}

/* Vidéo/image de fond — plein écran, sous l'overlay et le contenu */
.splash-bg-media {
  position: absolute; inset: 0; z-index: 0;
  width: 100%; height: 100%;
  object-fit: cover; object-position: center;
  pointer-events: none;
}

/* Overlay — assombrit la vidéo pour garder le texte lisible, glow orange au centre */
.splash-overlay {
  position: absolute; inset: 0; z-index: 1;
  background:
    radial-gradient(ellipse at 50% 30%, var(--accent-muted, rgba(249,115,22,0.10)) 0%, transparent 60%),
    linear-gradient(to bottom,
      color-mix(in srgb, var(--bg-primary, #09090b) 45%, transparent) 0%,
      color-mix(in srgb, var(--bg-primary, #09090b) 55%, transparent) 45%,
      color-mix(in srgb, var(--bg-primary, #09090b) 92%, transparent) 100%);
  pointer-events: none;
}


/* ── Panel chargement (bas de l'écran) ── */
.splash-content {
  position: absolute; bottom: 0; left: 0; right: 0;
  z-index: 2;
  display: flex; flex-direction: column; align-items: center; gap: 8px;
  padding: 24px 32px 32px;
  animation: splash-in 400ms ease forwards;
}
@keyframes splash-in { from { opacity: 0; transform: translateY(12px); } to { opacity: 1; transform: translateY(0); } }

/* Logo + titre alignés en ligne */
.splash-brand {
  display: flex; align-items: center; gap: 14px; margin-bottom: 4px;
}

.splash-brand-text { display: flex; flex-direction: column; gap: 2px; }
.splash-title {
  font-size: 22px; font-weight: 800; letter-spacing: -0.5px;
  background: linear-gradient(135deg, var(--text-primary, #fafafa) 40%, var(--accent-primary, #f97316));
  -webkit-background-clip: text; -webkit-text-fill-color: transparent; background-clip: text;
}
.splash-version {
  font-size: 10px; color: var(--text-muted, #52525b);
  font-family: "JetBrains Mono", monospace; letter-spacing: 0.05em;
}

/* Progress */
.splash-progress-wrap { display:flex; align-items:center; gap:8px; width:100%; max-width:520px; }
.splash-progress-bar  { flex:1; height:3px; background:var(--border, rgba(255,255,255,0.1)); border-radius:99px; overflow:hidden; }
.splash-progress-fill {
  height:100%; border-radius:99px;
  background: linear-gradient(90deg, var(--accent-primary, #ea580c), var(--accent-hover, #fb923c));
  box-shadow: var(--accent-glow-sm, 0 0 10px rgba(249,115,22,0.6));
  transition: width 300ms cubic-bezier(0.4,0,0.2,1);
}
.splash-pct { font-size:11px; font-weight:700; color:var(--accent-primary, #f97316); font-family:"JetBrains Mono",monospace; min-width:30px; text-align:right; }
.splash-label { font-size:11px; color:var(--text-muted, #71717a); min-height:15px; }

/* Grille de tâches */
.splash-grid {
  display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 2px 12px;
  width: 100%; max-width: 620px; margin-top: 4px;
  background: color-mix(in srgb, var(--bg-secondary, #111113) 75%, transparent);
  backdrop-filter: blur(16px);
  border: 1px solid var(--border, rgba(255,255,255,0.07));
  border-radius: 10px;
  padding: 10px 14px;
}
.splash-task {
  display: flex; align-items: center; gap: 5px;
  opacity: 0.25; transition: opacity 200ms ease;
}
.splash-task.running { opacity: 1; }
.splash-task.done    { opacity: 0.55; }

.task-icon { width:12px; height:12px; display:flex; align-items:center; justify-content:center; flex-shrink:0; }
.task-dot  { width:4px; height:4px; border-radius:50%; background:var(--border-strong, #3f3f46); display:block; }
.task-label { font-size:10px; color:var(--text-secondary, #a1a1aa); line-height:1.3; }
.splash-task.running .task-label { color:var(--accent-primary, #f97316); font-weight:600; }
.splash-task.done    .task-label { color:var(--success, #4ade80); }

.spin { animation: spin 0.9s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }

.splash-counter { font-size:10px; color:var(--text-muted, #52525b); font-family:"JetBrains Mono",monospace; }

/* ── Variantes par modèle d'interface ── */
.splash--console .splash-content, .splash--console .splash-title { font-family: "JetBrains Mono", Consolas, monospace; }
.splash--console .splash-title { -webkit-text-fill-color: var(--accent-primary, #f97316); }
.splash--console .splash-grid { border-radius: 2px; }
.splash--console .splash-progress-bar, .splash--console .splash-progress-fill { border-radius: 0; height: 6px; }
.splash--bento .splash-grid, .splash--glass-dock .splash-grid { border-radius: 20px; padding: 14px 18px; }
.splash--glass-dock .splash-grid { backdrop-filter: blur(24px) saturate(1.4); }
.splash--command-deck .splash-grid, .splash--columns .splash-grid, .splash--mission .splash-grid { border-radius: 6px; }
.splash--mission .splash-grid { border-top: 2px solid var(--accent-primary, #f97316); }
.splash-ring { width: 120px; height: 120px; }
.splash-ring__track { fill: none; stroke: var(--border, #26262c); stroke-width: 8; }
.splash-ring__fill { fill: none; stroke: var(--accent-primary, #f97316); stroke-width: 8; stroke-linecap: round; transition: stroke-dasharray 300ms ease; }
.splash-ring__pct { fill: var(--text-primary, #fafafa); font-size: 20px; font-weight: 800; font-family: "JetBrains Mono", monospace; }
html[data-perf="light"] .splash-grid { backdrop-filter: none; }

.splash-leave-active { transition: opacity 500ms ease; }
.splash-leave-to { opacity: 0; }

/* ── App Layout ─────────────────────────────────────────────────────────── */
.app-root {
  height:100vh; overflow:hidden;
  background:var(--bg-primary); color:var(--text-primary);
  animation: app-in 350ms ease forwards;
  font-size: var(--layout-font-size, 13px);
}
@keyframes app-in { from { opacity:0 } to { opacity:1 } }
.app-content { flex:1; overflow-y:auto; overflow-x:hidden; min-height:0; }
.app-content-inner { width:100%; position: relative; }

/* transition gérée globalement — voir <style> ci-dessous */

.density-compact  :deep(.ncard) { padding: calc(var(--layout-density-pad-md, 8px) * 0.72); }
.density-spacious :deep(button.nav-item) { padding: 10px 12px; }
.density-compact  :deep(button.nav-item) { padding: 5px 8px; font-size: 11px; }

/* ── Diagnostic d'erreur page ── */
.page-error-overlay {
  position: absolute; inset: 0; z-index: 9999;
  display: flex; align-items: center; justify-content: center;
  background: rgba(0, 0, 0, 0.75); backdrop-filter: blur(4px);
  pointer-events: all;
}
.page-error-box {
  max-width: 640px; width: 90%; padding: 24px;
  background: #1c0a0a; border: 1px solid #ef4444;
  border-radius: 12px; box-shadow: 0 0 32px rgba(239,68,68,0.4);
  display: flex; flex-direction: column; gap: 12px;
}
.page-error-title { font-size: 15px; font-weight: 700; color: #ef4444; }
.page-error-msg { font-size: 13px; color: #fca5a5; word-break: break-all; }
.page-error-stack {
  font-size: 10px; color: #71717a; font-family: "JetBrains Mono", monospace;
  white-space: pre-wrap; word-break: break-all; max-height: 200px;
  overflow-y: auto; background: #0c0c0e; border-radius: 6px; padding: 8px;
}
.page-error-dismiss {
  align-self: flex-end; padding: 6px 16px; font-size: 12px;
  background: #ef4444; color: #fff; border: none; border-radius: 6px;
  cursor: pointer; font-family: inherit;
}
.page-error-dismiss:hover { background: #dc2626; }
</style>

<!-- Transition de page : fade simultané (pas de mode out-in = pas de blocage) -->
<style>
.page-enter-active { transition: opacity 120ms ease; }
.page-leave-active { transition: opacity 120ms ease; position: absolute; width: 100%; top: 0; left: 0; pointer-events: none; }
.page-enter-from   { opacity: 0; }
.page-leave-to     { opacity: 0; }
</style>
