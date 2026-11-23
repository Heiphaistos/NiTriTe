<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke, invokeRaw } from "@/utils/invoke";
import { save, confirm } from "@tauri-apps/plugin-dialog";
import NCard from "@/components/ui/NCard.vue";
import NButton from "@/components/ui/NButton.vue";
import NBadge from "@/components/ui/NBadge.vue";
import NSkeleton from "@/components/ui/NSkeleton.vue";
import { useNotificationStore } from "@/stores/notifications";
import { Terminal, RefreshCw, Play, Package, Settings2, StopCircle, Upload, ArrowUp, Trash2, ExternalLink } from "lucide-vue-next";

const notify = useNotificationStore();

interface WslDistro {
  name: string; state: string; version: number; is_default: boolean;
}
interface WslInfo {
  installed: boolean; default_version: number;
  distros: WslDistro[];
  kernel_version: string; wsl_version: string; error: string;
}

const info = ref<WslInfo | null>(null);
const loading = ref(true);
const cmdInput = ref("");
const cmdDistro = ref("");
const cmdOutput = ref("");
const runningCmd = ref(false);
const settingVersion = ref(false);

// Per-distro action states
const startingDistro = ref<string | null>(null);
const stoppingDistro = ref<string | null>(null);
const exportingDistro = ref<string | null>(null);
const convertingDistro = ref<string | null>(null);
const unregisteringDistro = ref<string | null>(null);

async function load() {
  loading.value = true;
  try {
    info.value = await invoke<WslInfo>("get_wsl_info");
    if (info.value?.distros.length) {
      const def = info.value.distros.find(d => d.is_default);
      cmdDistro.value = def?.name ?? info.value.distros[0].name;
    }
  } catch (e: unknown) {
    notify.error("Erreur WSL", String(e));
  }
  loading.value = false;
}

async function runCmd() {
  if (!cmdInput.value.trim()) return;
  runningCmd.value = true;
  cmdOutput.value = "";
  try {
    const result = await invoke<string>("wsl_run_command", {
      distro: cmdDistro.value,
      command: cmdInput.value,
    });
    cmdOutput.value = result;
  } catch (e: unknown) {
    cmdOutput.value = `Erreur : ${String(e)}`;
  }
  runningCmd.value = false;
}

async function setDefaultVersion(v: number) {
  settingVersion.value = true;
  try {
    await invoke("wsl_set_default_version", { version: v });
    notify.success("Version WSL modifiée", `WSL ${v} défini par défaut`);
    await load();
  } catch (e: unknown) {
    notify.error("Erreur", String(e));
  }
  settingVersion.value = false;
}

async function startDistro(d: WslDistro) {
  startingDistro.value = d.name;
  try {
    const r = await invoke<{ success: boolean; stderr: string }>("run_system_command", { cmd: "wsl", args: ["-d", d.name] });
    if (!r.success) throw new Error(r.stderr?.trim() || "La commande WSL a échoué");
    notify.success("Distro démarrée", d.name);
    await load();
  } catch (e: unknown) {
    notify.error("Erreur démarrage", String(e));
  }
  startingDistro.value = null;
}

async function stopDistro(d: WslDistro) {
  stoppingDistro.value = d.name;
  try {
    const r = await invoke<{ success: boolean; stderr: string }>("run_system_command", { cmd: "wsl", args: ["--terminate", d.name] });
    if (!r.success) throw new Error(r.stderr?.trim() || "La commande WSL a échoué");
    notify.success("Distro arrêtée", d.name);
    await load();
  } catch (e: unknown) {
    notify.error("Erreur arrêt", String(e));
  }
  stoppingDistro.value = null;
}

async function exportDistro(d: WslDistro) {
  // pick_save_path n'a jamais existé côté Rust (invoke() échouait toujours,
  // le bouton Exporter tombait systématiquement sur le message manuel) —
  // dialog natif Tauri comme partout ailleurs dans le code (ProfilesPage.vue).
  let exportPath: string | null = null;
  try {
    exportPath = await save({
      defaultPath: `${d.name}.tar`,
      filters: [{ name: "TAR Archive", extensions: ["tar"] }],
    });
  } catch (e: unknown) {
    notify.error("Sélection impossible", `Entrez le chemin dans le terminal : wsl --export ${d.name} C:\\backup\\${d.name}.tar (${String(e)})`);
    return;
  }
  if (!exportPath) return;

  exportingDistro.value = d.name;
  try {
    await invokeRaw<string>("wsl_long_action", { action: "export", distro: d.name, path: exportPath });
    notify.success("Export terminé", exportPath);
  } catch (e: unknown) {
    notify.error("Erreur export", String(e));
  }
  exportingDistro.value = null;
}

async function convertToWsl2(d: WslDistro) {
  if (!(await confirm(`Convertir "${d.name}" de WSL 1 vers WSL 2 ? Cette opération peut prendre plusieurs minutes.`, { title: "Nitrite", kind: "warning" }))) return;
  convertingDistro.value = d.name;
  try {
    await invokeRaw<string>("wsl_long_action", { action: "convert", distro: d.name });
    notify.success("Conversion terminée", `${d.name} → WSL 2`);
    await load();
  } catch (e: unknown) {
    notify.error("Erreur conversion", String(e));
  }
  convertingDistro.value = null;
}

async function unregisterDistro(d: WslDistro) {
  if (!(await confirm(`Supprimer la distribution "${d.name}" ? Toutes les données seront perdues définitivement.`, { title: "Nitrite", kind: "warning" }))) return;
  unregisteringDistro.value = d.name;
  try {
    const r = await invoke<{ success: boolean; stderr: string }>("run_system_command", { cmd: "wsl", args: ["--unregister", d.name] });
    if (!r.success) throw new Error(r.stderr?.trim() || "La suppression WSL a échoué");
    notify.success("Distribution supprimée", d.name);
    await load();
  } catch (e: unknown) {
    notify.error("Erreur suppression", String(e));
  }
  unregisteringDistro.value = null;
}

async function openInTerminal(d: WslDistro) {
  try {
    await invoke("run_system_command", { cmd: "wt", args: ["-p", d.name] });
  } catch {
    try {
      await invoke("run_system_command", { cmd: "wsl", args: ["-d", d.name] });
    } catch (e: unknown) {
      notify.error("Erreur terminal", String(e));
    }
  }
}

onMounted(load);
</script>

<template>
  <div class="wsl-page">
    <div class="page-header">
      <div class="header-icon"><Terminal :size="22" /></div>
      <div>
        <h1>WSL — Windows Subsystem for Linux</h1>
        <p class="subtitle">Distributions Linux installées et console intégrée</p>
      </div>
      <NButton variant="ghost" size="sm" :loading="loading" @click="load" style="margin-left:auto">
        <RefreshCw :size="13" /> Actualiser
      </NButton>
    </div>

    <div v-if="loading">
      <NSkeleton v-for="i in 3" :key="i" height="60px" style="margin-bottom:8px" />
    </div>

    <template v-else-if="info">
      <!-- Statut WSL -->
      <NCard>
        <template #header>
          <div class="section-header"><Settings2 :size="15" /><span>Configuration WSL</span></div>
        </template>
        <div class="wsl-status-grid">
          <div class="status-item">
            <span class="status-label">WSL disponible</span>
            <NBadge :variant="info.installed ? 'success' : 'danger'">
              {{ info.installed ? 'Oui' : 'Non installé' }}
            </NBadge>
          </div>
          <div class="status-item">
            <span class="status-label">Version installée</span>
            <span class="status-val">{{ info.wsl_version || '—' }}</span>
          </div>
          <div class="status-item">
            <span class="status-label">Version par défaut</span>
            <div style="display:flex;gap:6px">
              <NButton
                :variant="info.default_version === 1 ? 'primary' : 'secondary'"
                size="sm" :loading="settingVersion"
                @click="setDefaultVersion(1)"
              >WSL 1</NButton>
              <NButton
                :variant="info.default_version === 2 ? 'primary' : 'secondary'"
                size="sm" :loading="settingVersion"
                @click="setDefaultVersion(2)"
              >WSL 2</NButton>
            </div>
          </div>
          <div class="status-item">
            <span class="status-label">Distributions</span>
            <span class="status-val">{{ info.distros.length }}</span>
          </div>
        </div>
      </NCard>

      <!-- Distributions -->
      <NCard>
        <template #header>
          <div class="section-header"><Package :size="15" /><span>Distributions ({{ info.distros.length }})</span></div>
        </template>

        <div v-if="!info.distros.length" class="empty-state">
          <Terminal :size="28" style="opacity:.25" />
          <p>Aucune distribution Linux installée</p>
          <p class="empty-hint">Installez depuis le Microsoft Store : Ubuntu, Debian, Arch…</p>
        </div>

        <div v-else class="distros-list">
          <div v-for="d in info.distros" :key="d.name" class="distro-card" :class="{ 'is-default': d.is_default }">
            <div class="distro-icon">
              <Terminal :size="18" />
            </div>
            <div class="distro-info">
              <span class="distro-name">{{ d.name }}</span>
              <span class="distro-meta">WSL {{ d.version }} · {{ info.kernel_version || 'Kernel inconnu' }}</span>
            </div>
            <div class="distro-badges">
              <NBadge v-if="d.is_default" variant="success" dot>Défaut</NBadge>
              <NBadge :variant="d.state === 'Running' ? 'success' : 'neutral'">{{ d.state }}</NBadge>
              <NBadge variant="neutral">WSL {{ d.version }}</NBadge>
            </div>
            <!-- Actions distro -->
            <div class="distro-actions">
              <!-- Ouvrir dans Terminal Windows -->
              <NButton variant="ghost" size="sm" @click="openInTerminal(d)" title="Ouvrir dans Terminal Windows">
                <ExternalLink :size="12" />
              </NButton>
              <!-- Start / Stop -->
              <NButton
                v-if="d.state !== 'Running'"
                variant="primary"
                size="sm"
                :loading="startingDistro === d.name"
                @click="startDistro(d)"
              >
                <Play :size="12" /> Démarrer
              </NButton>
              <NButton
                v-else
                variant="secondary"
                size="sm"
                :loading="stoppingDistro === d.name"
                @click="stopDistro(d)"
              >
                <StopCircle :size="12" /> Arrêter
              </NButton>
              <!-- Export -->
              <NButton
                variant="ghost"
                size="sm"
                :loading="exportingDistro === d.name"
                @click="exportDistro(d)"
                title="Exporter la distro en .tar"
              >
                <Upload :size="12" /> Exporter
              </NButton>
              <!-- Conversion WSL1 → WSL2 -->
              <NButton
                v-if="d.version === 1"
                variant="secondary"
                size="sm"
                :loading="convertingDistro === d.name"
                @click="convertToWsl2(d)"
              >
                <ArrowUp :size="12" /> → WSL 2
              </NButton>
              <!-- Supprimer -->
              <NButton
                variant="danger"
                size="sm"
                :loading="unregisteringDistro === d.name"
                @click="unregisterDistro(d)"
                title="Supprimer définitivement la distro"
              >
                <Trash2 :size="12" />
              </NButton>
            </div>
          </div>
        </div>
      </NCard>

      <!-- Console intégrée -->
      <NCard v-if="info.distros.length">
        <template #header>
          <div class="section-header"><Terminal :size="15" /><span>Console WSL</span></div>
        </template>
        <div class="console-section">
          <div class="cmd-row">
            <select v-model="cmdDistro" class="distro-select">
              <option v-for="d in info.distros" :key="d.name" :value="d.name">{{ d.name }}</option>
            </select>
            <input
              v-model="cmdInput"
              class="cmd-input"
              placeholder="commande bash…"
              @keydown.enter="runCmd"
            />
            <NButton variant="primary" size="sm" :loading="runningCmd" @click="runCmd">
              <Play :size="13" /> Exécuter
            </NButton>
          </div>
          <div v-if="cmdOutput" class="cmd-output">
            <pre>{{ cmdOutput }}</pre>
          </div>
        </div>
      </NCard>
    </template>

    <div v-else class="empty-state">
      <Terminal :size="32" style="opacity:.2" />
      <p>Impossible de charger les informations WSL</p>
    </div>
  </div>
</template>

<style scoped>
.wsl-page { display: flex; flex-direction: column; gap: 14px; }
.page-header { display: flex; align-items: center; gap: 12px; }
.header-icon { width: 42px; height: 42px; border-radius: var(--radius-lg); background: var(--bg-elevated); display: flex; align-items: center; justify-content: center; color: var(--text-secondary); flex-shrink: 0; }
h1 { font-size: 22px; font-weight: 700; }
.subtitle { font-size: 12px; color: var(--text-muted); }
.section-header { display: flex; align-items: center; gap: 8px; }
.wsl-status-grid { display: grid; grid-template-columns: repeat(2, 1fr); gap: 12px; }
@media (max-width: 700px) { .wsl-status-grid { grid-template-columns: 1fr; } }
.status-item { display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 10px 14px; background: var(--bg-tertiary); border-radius: var(--radius-md); }
.status-label { font-size: 12px; color: var(--text-muted); }
.status-val { font-family: "JetBrains Mono", monospace; font-size: 13px; font-weight: 600; color: var(--text-primary); }
.distros-list { display: flex; flex-direction: column; gap: 8px; }
.distro-card { display: flex; align-items: center; gap: 12px; padding: 14px; background: var(--bg-tertiary); border-radius: var(--radius-lg); border: 1px solid var(--border); transition: border-color .15s; flex-wrap: wrap; }
.distro-card.is-default { border-color: var(--success); background: var(--success-muted); }
.distro-icon { width: 38px; height: 38px; border-radius: var(--radius-md); background: var(--bg-elevated); display: flex; align-items: center; justify-content: center; color: var(--text-muted); flex-shrink: 0; }
.distro-info { flex: 1; display: flex; flex-direction: column; gap: 2px; min-width: 120px; }
.distro-name { font-size: 13px; font-weight: 600; color: var(--text-primary); }
.distro-meta { font-size: 11px; color: var(--text-muted); font-family: monospace; }
.distro-badges { display: flex; gap: 6px; flex-shrink: 0; flex-wrap: wrap; }
.distro-actions { display: flex; gap: 4px; flex-wrap: wrap; align-items: center; flex-shrink: 0; }
.console-section { display: flex; flex-direction: column; gap: 10px; }
.cmd-row { display: flex; gap: 8px; align-items: center; }
.distro-select { padding: 6px 10px; background: var(--bg-tertiary); border: 1px solid var(--border); border-radius: var(--radius-sm); color: var(--text-primary); font-size: 12px; cursor: pointer; flex-shrink: 0; }
.cmd-input { flex: 1; padding: 7px 12px; background: var(--bg-tertiary); border: 1px solid var(--border); border-radius: var(--radius-md); color: var(--text-primary); font-size: 13px; font-family: "JetBrains Mono", monospace; outline: none; transition: border-color .15s; }
.cmd-input:focus { border-color: var(--accent-primary); }
.cmd-output { background: #0c0c10; border: 1px solid var(--border); border-radius: var(--radius-md); padding: 12px 16px; max-height: 280px; overflow-y: auto; }
.cmd-output pre { color: #a8ff78; font-family: "JetBrains Mono", monospace; font-size: 12px; white-space: pre-wrap; word-break: break-word; margin: 0; line-height: 1.6; }
.empty-state { display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 40px; color: var(--text-muted); font-size: 13px; text-align: center; }
.empty-hint { font-size: 11px; color: var(--text-muted); opacity: .7; }
</style>
