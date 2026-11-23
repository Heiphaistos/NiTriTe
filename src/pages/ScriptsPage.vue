<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from "vue";
import { invoke, isTauriContext } from "@/utils/invoke";
/** Le Rust coupe un script a 300 s (executor.rs) : les 30 s par defaut d'invoke
 *  declaraient en echec un script encore en cours. */
const SCRIPT_UI_TIMEOUT_MS = 310_000;
import NCard from "@/components/ui/NCard.vue";
import NButton from "@/components/ui/NButton.vue";
import NTabs from "@/components/ui/NTabs.vue";
import NBadge from "@/components/ui/NBadge.vue";
import NSpinner from "@/components/ui/NSpinner.vue";
import NDropdown from "@/components/ui/NDropdown.vue";
import NInput from "@/components/ui/NInput.vue";
import NModal from "@/components/ui/NModal.vue";
import { useNotificationStore } from "@/stores/notifications";
import SnippetsTab from "@/components/scripts/SnippetsTab.vue";
import {
  Terminal, Play, Shield, FileCode, Code, Trash2, ScrollText,
  FolderOpen, Save, RefreshCw, BookOpen, Clock, History, Layers,
} from "lucide-vue-next";

type MainTab = "scripts" | "snippets";
const mainTab = ref<MainTab>("scripts");

const notify = useNotificationStore();

// ─── Builtin scripts ───────────────────────────────────────────────────────
interface BuiltinScript {
  name: string; description: string; category: string;
  script_type: "cmd" | "powershell"; content: string; requires_admin: boolean;
}

const scripts = ref<BuiltinScript[]>([]);
const scriptsLoading = ref(true);
const activeCategory = ref("");

const categories = computed(() => {
  const cats = [...new Set(scripts.value.map(s => s.category))];
  return cats.map(c => ({ id: c, label: c }));
});

const filteredScripts = computed(() =>
  !activeCategory.value ? scripts.value : scripts.value.filter(s => s.category === activeCategory.value)
);

async function loadScripts() {
  scriptsLoading.value = true;
  try {
    scripts.value = await invoke<BuiltinScript[]>("get_builtin_scripts");
  } catch {
    scripts.value = [
      { name: "Flush DNS", description: "Vide le cache DNS pour résoudre les problèmes de connexion", category: "Réseau", script_type: "cmd", content: "ipconfig /flushdns", requires_admin: true },
      { name: "Reset Winsock", description: "Réinitialise le catalogue Winsock", category: "Réseau", script_type: "cmd", content: "netsh winsock reset", requires_admin: true },
      { name: "Renouveler IP", description: "Release et renouvellement de l'adresse IP", category: "Réseau", script_type: "cmd", content: "ipconfig /release && ipconfig /renew", requires_admin: true },
      { name: "SFC /scannow", description: "Vérifie et répare les fichiers système corrompus", category: "Réparation", script_type: "cmd", content: "sfc /scannow", requires_admin: true },
      { name: "DISM Health", description: "Répare l'image Windows avec DISM", category: "Réparation", script_type: "cmd", content: "DISM /Online /Cleanup-Image /RestoreHealth", requires_admin: true },
      { name: "Check Disk", description: "Vérifie l'intégrité du système de fichiers", category: "Réparation", script_type: "cmd", content: "chkdsk C: /f", requires_admin: true },
      { name: "Vider les logs", description: "Supprime tous les journaux d'événements Windows", category: "Nettoyage", script_type: "powershell", content: "Get-EventLog -LogName * | ForEach { Clear-EventLog $_.Log }", requires_admin: true },
      { name: "Vider cache icônes", description: "Réinitialise le cache des icônes Windows", category: "Nettoyage", script_type: "cmd", content: "ie4uinit.exe -show", requires_admin: false },
      { name: "Gros fichiers", description: "Liste les fichiers de plus de 500 MB sur le disque C:", category: "Analyse", script_type: "powershell", content: "Get-ChildItem C:\\ -Recurse -ErrorAction SilentlyContinue | Where-Object { $_.Length -gt 500MB } | Sort-Object Length -Descending | Select-Object FullName, @{N='SizeMB';E={[math]::Round($_.Length/1MB,1)}} | Format-Table -AutoSize", requires_admin: false },
      { name: "Rapport batterie", description: "Génère un rapport détaillé de la batterie", category: "Analyse", script_type: "cmd", content: "powercfg /batteryreport /output %USERPROFILE%\\battery-report.html && start %USERPROFILE%\\battery-report.html", requires_admin: true },
    ];
  } finally {
    scriptsLoading.value = false;
    if (categories.value.length && !activeCategory.value)
      activeCategory.value = categories.value[0].id;
  }
}

// ─── Script templates ──────────────────────────────────────────────────────
interface ScriptTemplate { id: string; name: string; type: "cmd"|"powershell"; content: string; desc: string }

const TEMPLATES: ScriptTemplate[] = [
  {
    id: "t1", name: "Info système rapide", type: "powershell", desc: "OS, CPU, RAM, uptime",
    content: `# Informations système rapides
$os = Get-WmiObject Win32_OperatingSystem
$cpu = Get-WmiObject Win32_Processor | Select -First 1
$ram = [math]::Round($os.TotalVisibleMemorySize / 1MB, 1)
Write-Host "OS      : $($os.Caption) Build $($os.BuildNumber)"
Write-Host "CPU     : $($cpu.Name)"
Write-Host "RAM     : $ram GB"
Write-Host "Uptime  : $((Get-Date) - $os.ConvertToDateTime($os.LastBootUpTime))"`,
  },
  {
    id: "t2", name: "Nettoyage fichiers temp", type: "powershell", desc: "Supprime %TEMP% et C:\\Windows\\Temp",
    content: `# Nettoyage fichiers temporaires
$paths = @($env:TEMP, "C:\\Windows\\Temp")
foreach ($p in $paths) {
  $items = Get-ChildItem $p -Recurse -ErrorAction SilentlyContinue
  $count = 0
  foreach ($i in $items) {
    try { Remove-Item $i.FullName -Force -Recurse -ErrorAction Stop; $count++ } catch {}
  }
  Write-Host "Supprime $count elements dans $p"
}`,
  },
  {
    id: "t3", name: "Ports ouverts (écoute)", type: "powershell", desc: "Liste les ports TCP en écoute avec processus",
    content: `# Ports TCP en écoute
Get-NetTCPConnection -State Listen |
  Select-Object LocalAddress, LocalPort,
    @{N='Processus';E={(Get-Process -Id $_.OwningProcess -ErrorAction SilentlyContinue).Name}} |
  Sort-Object LocalPort |
  Format-Table -AutoSize`,
  },
  {
    id: "t4", name: "Rapport Wi-Fi complet", type: "cmd", desc: "Exporte et ouvre le rapport Wi-Fi Windows",
    content: `netsh wlan show wlanreport
start %SystemRoot%\\system32\\WlanReport\\wlan-report-latest.html`,
  },
  {
    id: "t5", name: "Disques et partitions", type: "powershell", desc: "Taille, libre, partition pour chaque volume",
    content: `# État des disques
Get-PSDrive -PSProvider FileSystem | Select-Object Name,
  @{N='Total GB';E={[math]::Round($_.Used/1GB + $_.Free/1GB,1)}},
  @{N='Libre GB';E={[math]::Round($_.Free/1GB,1)}},
  @{N='Libre %';E={[math]::Round($_.Free/($_.Used+$_.Free)*100,1)}} |
  Format-Table -AutoSize`,
  },
  {
    id: "t6", name: "Processus CPU top 10", type: "powershell", desc: "Top 10 processus par consommation CPU",
    content: `# Top 10 processus par CPU
Get-Process | Sort-Object CPU -Descending | Select -First 10 |
  Select Name, Id,
    @{N='CPU (s)';E={[math]::Round($_.CPU,1)}},
    @{N='RAM (MB)';E={[math]::Round($_.WorkingSet/1MB,1)}} |
  Format-Table -AutoSize`,
  },
];

// ─── Exécution + console ───────────────────────────────────────────────────
const runningScriptId = ref<string | null>(null);
const outputLines = ref<string[]>([]);
const outputRef = ref<HTMLElement | null>(null);
let unlisten: (() => void) | null = null;
const elapsed = ref(0);
let elapsedTimer: ReturnType<typeof setInterval> | null = null;

function startElapsed() { elapsed.value = 0; elapsedTimer = setInterval(() => { elapsed.value++; }, 1000); }
function stopElapsed()  { if (elapsedTimer) { clearInterval(elapsedTimer); elapsedTimer = null; } }

async function setupListener() {
  try {
    const { listen } = await import("@tauri-apps/api/event");
    unlisten = await listen<string>("script-output", ev => {
      outputLines.value.push(ev.payload);
      scrollOutput();
    });
  } catch { /* mode dev */ }
}

function scrollOutput() {
  setTimeout(() => { if (outputRef.value) outputRef.value.scrollTop = outputRef.value.scrollHeight; }, 50);
}

function extractPaths(line: string): string[] {
  const matches = line.match(/([A-Z]:\\[^\s"<>|*?]+)/gi) ?? [];
  return matches.filter(p => p.length > 3);
}

const detectedPaths = ref<string[]>([]);

async function openPath(p: string) {
  try { await invoke("open_path", { path: p }); } catch { window.open(`file:///${p}`, "_blank"); }
}

// ─── Historique des exécutions ─────────────────────────────────────────────
interface ExecRecord {
  id: string; name: string; type: string; ran_at: string;
  elapsed_s: number; exit_code: number; success: boolean;
}
const execHistory = ref<ExecRecord[]>([]);
const showHistory = ref(false);

function loadHistory() {
  try { execHistory.value = JSON.parse(localStorage.getItem("nitrite_exec_history") ?? "[]"); }
  catch { execHistory.value = []; }
}

function pushHistory(rec: Omit<ExecRecord, "id">) {
  execHistory.value.unshift({ id: Date.now().toString(), ...rec });
  if (execHistory.value.length > 50) execHistory.value = execHistory.value.slice(0, 50);
  localStorage.setItem("nitrite_exec_history", JSON.stringify(execHistory.value));
}

function clearHistory() { execHistory.value = []; localStorage.removeItem("nitrite_exec_history"); }

async function exportScriptAsBat(script: BuiltinScript) {
  try {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const { writeTextFile } = await import("@tauri-apps/plugin-fs");
    const ext = script.script_type === "powershell" ? "ps1" : "bat";
    const safeName = script.name.replace(/[^a-zA-Z0-9_-]/g, "_");
    const path = await save({
      defaultPath: `${safeName}.${ext}`,
      filters: [{ name: script.script_type === "powershell" ? "PowerShell" : "Batch", extensions: [ext] }],
    });
    if (!path) return;
    let content: string;
    if (script.script_type === "powershell") {
      content = `# ${script.name}\n# ${script.description}\n# Genere par Nitrite v6.0\n\n${script.content}`;
    } else {
      content = `@echo off\r\n:: ${script.name}\r\n:: ${script.description}\r\n:: Genere par Nitrite v6.0\r\n\r\n${script.content}`;
    }
    await writeTextFile(path, content);
    notify.success("Script exporté", path);
  } catch (e: unknown) {
    notify.error("Erreur export", String(e));
  }
}

async function executeBuiltinScript(script: BuiltinScript) {
  runningScriptId.value = script.name;
  outputLines.value = [`> Exécution de "${script.name}"...`, ""];
  detectedPaths.value = [];
  startElapsed();
  try {
    const result = await invoke<{ success: boolean; output: string; exit_code: number }>("execute_builtin_script", {
      name: script.name,
    }, SCRIPT_UI_TIMEOUT_MS);
    outputLines.value.push(result.output);
    outputLines.value.push("", `--- Terminé en ${elapsed.value}s (code: ${result.exit_code}) ---`);
    detectedPaths.value = [...new Set(extractPaths(result.output))].slice(0, 8);
    pushHistory({ name: script.name, type: script.script_type, ran_at: new Date().toLocaleString("fr-FR"), elapsed_s: elapsed.value, exit_code: result.exit_code, success: result.success });
    if (result.success) notify.success("Script terminé", `${script.name} — ${elapsed.value}s`);
    else notify.warning("Script terminé avec erreurs", `${script.name} — code ${result.exit_code}`);
  } catch (e: unknown) {
    if (!isTauriContext()) {
      outputLines.value.push("Exécution simulée en mode dev...", "Opération terminée.", "", "--- Terminé (code: 0) ---");
      pushHistory({ name: script.name, type: script.script_type, ran_at: new Date().toLocaleString("fr-FR"), elapsed_s: elapsed.value, exit_code: 0, success: true });
      notify.info("Mode dev", `Simulation : ${script.name}`);
    } else {
      const msg = e instanceof Error ? e.message : String(e);
      outputLines.value.push(`Erreur : ${msg}`, "", "--- Échec ---");
      pushHistory({ name: script.name, type: script.script_type, ran_at: new Date().toLocaleString("fr-FR"), elapsed_s: elapsed.value, exit_code: 1, success: false });
      notify.error("Erreur script", msg.slice(0, 120));
    }
  } finally {
    stopElapsed();
    runningScriptId.value = null;
  }
}

// ─── Script personnalisé + autosave ───────────────────────────────────────
const customScript = ref("");
const customType   = ref("cmd");
const customRunning = ref(false);
const customName   = ref("");
const autosaveLabel = ref("");

// Autosave brouillon toutes les 2s de inactivité
let autosaveTimer: ReturnType<typeof setTimeout> | null = null;
watch(customScript, () => {
  if (autosaveTimer) clearTimeout(autosaveTimer);
  autosaveTimer = setTimeout(() => {
    if (customScript.value.trim()) {
      localStorage.setItem("nitrite_custom_draft", JSON.stringify({ content: customScript.value, type: customType.value, name: customName.value }));
      autosaveLabel.value = `Brouillon sauvegardé à ${new Date().toLocaleTimeString("fr-FR")}`;
    }
  }, 2000);
});

function loadDraft() {
  try {
    const d = JSON.parse(localStorage.getItem("nitrite_custom_draft") ?? "null");
    if (d) { customScript.value = d.content; customType.value = d.type; customName.value = d.name || ""; autosaveLabel.value = "Brouillon restauré"; }
  } catch {}
}

// Bibliothèque
interface SavedScript { id: string; name: string; type: string; content: string; created_at: string }
const savedScripts = ref<SavedScript[]>([]);
const showLibrary  = ref(false);

function loadLibrary() {
  try { savedScripts.value = JSON.parse(localStorage.getItem("nitrite_custom_scripts") ?? "[]"); }
  catch { savedScripts.value = []; }
}
function persistLibrary() { localStorage.setItem("nitrite_custom_scripts", JSON.stringify(savedScripts.value)); }

function saveToLibrary() {
  if (!customScript.value.trim()) { notify.warning("Script vide", "Entrez un script avant de sauvegarder."); return; }
  const name = customName.value.trim() || `Script ${savedScripts.value.length + 1}`;
  savedScripts.value.unshift({ id: Date.now().toString(), name, type: customType.value, content: customScript.value, created_at: new Date().toLocaleString("fr-FR") });
  persistLibrary();
  notify.success("Sauvegardé", `"${name}" ajouté à la bibliothèque`);
  customName.value = "";
}

function loadFromLibrary(s: SavedScript) {
  customScript.value = s.content; customType.value = s.type; customName.value = s.name;
  showLibrary.value = false;
  notify.info("Script chargé", s.name);
}
function deleteFromLibrary(id: string) { savedScripts.value = savedScripts.value.filter(s => s.id !== id); persistLibrary(); }

// Charger template
const showTemplates = ref(false);
function loadTemplate(t: ScriptTemplate) {
  customScript.value = t.content; customType.value = t.type; customName.value = t.name;
  showTemplates.value = false;
  autosaveLabel.value = `Template "${t.name}" chargé`;
  notify.info("Template chargé", t.name);
}

const typeOptions = [{ value: "cmd", label: "CMD" }, { value: "powershell", label: "PowerShell" }];

async function executeCustomScript() {
  if (!customScript.value.trim()) { notify.warning("Script vide", "Entrez un script à exécuter."); return; }
  customRunning.value = true;
  outputLines.value = [`> Script personnalisé (${customType.value})...`, ""];
  startElapsed();
  try {
    const result = await invoke<{ success: boolean; output: string; exit_code: number }>("execute_script", {
      content: customScript.value, scriptType: customType.value,
    }, SCRIPT_UI_TIMEOUT_MS);
    outputLines.value.push(result.output);
    outputLines.value.push("", `--- Terminé en ${elapsed.value}s (code: ${result.exit_code}) ---`);
    const name = customName.value.trim() || "Script personnalisé";
    pushHistory({ name, type: customType.value, ran_at: new Date().toLocaleString("fr-FR"), elapsed_s: elapsed.value, exit_code: result.exit_code, success: result.success });
    if (result.success) notify.success("Script terminé", `${name} — ${elapsed.value}s`);
    else notify.warning("Script terminé avec erreurs", `${name} — code ${result.exit_code}`);
  } catch (e: unknown) {
    if (!isTauriContext()) {
      outputLines.value.push("Exécution simulée en mode dev...", customScript.value, "", "--- Terminé (code: 0) ---");
      notify.info("Mode dev", "Simulation : script personnalisé");
    } else {
      const msg = e instanceof Error ? e.message : String(e);
      outputLines.value.push(`Erreur : ${msg}`, "", "--- Échec ---");
      notify.error("Erreur script", msg.slice(0, 120));
    }
  } finally {
    stopElapsed();
    customRunning.value = false;
  }
}

function clearOutput() { outputLines.value = []; detectedPaths.value = []; }

// ─── Script File Browser ───────────────────────────────────────────────────
interface ScriptFile { name: string; path: string; size_bytes: number; script_type: string; }
const scriptFiles = ref<ScriptFile[]>([]);
const scriptFilesLoading = ref(false);
const scriptBrowseDir = ref("C:\\Scripts");
const selectedFile = ref<ScriptFile | null>(null);
const fileContent = ref("");
const fileEdited = ref(false);

async function loadScriptFiles() {
  scriptFilesLoading.value = true;
  try {
    scriptFiles.value = await invoke<ScriptFile[]>("list_script_files", { dir: scriptBrowseDir.value });
  } catch (e: unknown) {
    if (!isTauriContext()) {
      scriptFiles.value = [
        { name: "cleanup.ps1", path: "C:\\Scripts\\cleanup.ps1", size_bytes: 2048, script_type: "powershell" },
        { name: "backup.bat",  path: "C:\\Scripts\\backup.bat",  size_bytes: 512,  script_type: "cmd" },
      ];
    } else {
      scriptFiles.value = [];
      notify.error("Erreur", (e instanceof Error ? e.message : String(e)).slice(0, 120));
    }
  } finally { scriptFilesLoading.value = false; }
}

async function openScriptFile(file: ScriptFile) {
  selectedFile.value = file; fileEdited.value = false;
  try { fileContent.value = await invoke<string>("read_script_file", { path: file.path }); }
  catch (e: unknown) {
    if (!isTauriContext()) { fileContent.value = `# Contenu simulé de ${file.name}\necho "Hello World"`; }
    else { fileContent.value = ""; notify.error("Erreur lecture", (e instanceof Error ? e.message : String(e)).slice(0, 120)); }
  }
}

async function saveScriptFile() {
  if (!selectedFile.value) return;
  try {
    await invoke("save_script_file", { path: selectedFile.value.path, content: fileContent.value });
    notify.success("Fichier sauvegardé"); fileEdited.value = false;
  } catch (e: unknown) {
    if (!isTauriContext()) { notify.info("Mode dev", "Sauvegarde simulée"); fileEdited.value = false; }
    else { notify.error("Erreur sauvegarde", (e instanceof Error ? e.message : String(e)).slice(0, 120)); }
  }
}

async function runScriptFile() {
  if (!selectedFile.value) return;
  const st = selectedFile.value.script_type === "powershell" ? "powershell" : "cmd";
  outputLines.value = [`> Exécution de "${selectedFile.value.name}"...`, ""];
  startElapsed();
  try {
    const result = await invoke<{ success: boolean; output: string; exit_code: number }>("execute_script", {
      content: fileContent.value, scriptType: st,
    }, SCRIPT_UI_TIMEOUT_MS);
    outputLines.value.push(result.output, "", `--- Terminé (code: ${result.exit_code}) ---`);
    pushHistory({ name: selectedFile.value.name, type: st, ran_at: new Date().toLocaleString("fr-FR"), elapsed_s: elapsed.value, exit_code: result.exit_code, success: result.success });
    if (result.success) notify.success("Script terminé");
    else notify.warning("Script terminé avec erreurs", `Code ${result.exit_code}`);
  } catch (e: unknown) {
    if (!isTauriContext()) {
      outputLines.value.push("Exécution simulée en mode dev...", "", "--- Terminé (code: 0) ---");
      notify.info("Mode dev", "Simulation");
    } else {
      const msg = e instanceof Error ? e.message : String(e);
      outputLines.value.push(`Erreur : ${msg}`, "", "--- Échec ---");
      notify.error("Erreur script", msg.slice(0, 120));
    }
  } finally { stopElapsed(); }
}

onMounted(() => { loadScripts(); setupListener(); loadLibrary(); loadHistory(); loadDraft(); });
onUnmounted(() => { if (unlisten) unlisten(); stopElapsed(); if (autosaveTimer) clearTimeout(autosaveTimer); });
</script>

<template>
  <div class="scripts">
    <div class="page-header">
      <div>
        <h1>Scripts & Snippets</h1>
        <p class="page-subtitle">Scripts de maintenance, diagnostic et bibliothèque de code</p>
      </div>
      <div style="display:flex;gap:8px;align-items:center">
        <div style="display:flex;gap:3px;background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius-md);padding:3px">
          <button style="display:flex;align-items:center;gap:5px;padding:5px 14px;border-radius:calc(var(--radius-md) - 2px);border:none;font-size:12px;font-family:inherit;cursor:pointer;transition:all .15s"
            :style="{ background: mainTab==='scripts' ? 'var(--accent-muted)' : 'transparent', color: mainTab==='scripts' ? 'var(--accent-primary)' : 'var(--text-secondary)', fontWeight: mainTab==='scripts' ? '600' : '400' }"
            @click="mainTab = 'scripts'">Scripts</button>
          <button style="display:flex;align-items:center;gap:5px;padding:5px 14px;border-radius:calc(var(--radius-md) - 2px);border:none;font-size:12px;font-family:inherit;cursor:pointer;transition:all .15s"
            :style="{ background: mainTab==='snippets' ? 'var(--accent-muted)' : 'transparent', color: mainTab==='snippets' ? 'var(--accent-primary)' : 'var(--text-secondary)', fontWeight: mainTab==='snippets' ? '600' : '400' }"
            @click="mainTab = 'snippets'">Snippets</button>
        </div>
        <NButton v-if="mainTab==='scripts'" variant="secondary" size="sm" @click="showHistory = true">
          <History :size="14" /> Historique ({{ execHistory.length }})
        </NButton>
      </div>
    </div>

    <!-- Onglet Snippets -->
    <SnippetsTab v-if="mainTab === 'snippets'" />

    <div v-if="mainTab === 'scripts'" class="scripts-layout">
      <!-- Gauche : scripts + éditeur -->
      <div class="scripts-panel">
        <!-- Scripts intégrés -->
        <NCard>
          <template #header>
            <div class="section-header">
              <FileCode :size="16" />
              <span>Scripts intégrés</span>
            </div>
          </template>
          <div v-if="scriptsLoading" class="loading-state"><NSpinner :size="24" /><p>Chargement...</p></div>
          <template v-else>
            <NTabs v-if="categories.length" :tabs="categories" v-model="activeCategory">
              <template #default>
                <div class="scripts-grid">
                  <div v-for="script in filteredScripts" :key="script.name" class="script-card">
                    <div class="script-header">
                      <span class="script-name">{{ script.name }}</span>
                      <div class="script-badges">
                        <NBadge :variant="script.script_type === 'powershell' ? 'accent' : 'info'">
                          {{ script.script_type === 'powershell' ? 'PowerShell' : 'CMD' }}
                        </NBadge>
                        <NBadge v-if="script.requires_admin" variant="warning">
                          <Shield :size="10" /> Admin
                        </NBadge>
                      </div>
                    </div>
                    <p class="script-desc">{{ script.description }}</p>
                    <div style="display:flex;gap:6px;flex-wrap:wrap">
                      <NButton variant="secondary" size="sm"
                        :loading="runningScriptId === script.name"
                        :disabled="runningScriptId !== null"
                        @click="executeBuiltinScript(script)">
                        <Play :size="14" /> Exécuter
                      </NButton>
                      <NButton variant="ghost" size="sm" @click="exportScriptAsBat(script)" title="Exporter en .bat/.ps1">
                        <Save :size="13" /> .bat
                      </NButton>
                    </div>
                  </div>
                </div>
              </template>
            </NTabs>
          </template>
        </NCard>

        <!-- Script personnalisé -->
        <NCard>
          <template #header>
            <div class="section-header">
              <Code :size="16" />
              <span>Script personnalisé</span>
              <div style="margin-left:auto;display:flex;gap:6px">
                <NButton variant="ghost" size="sm" @click="showTemplates = true">
                  <Layers :size="13" /> Templates ({{ TEMPLATES.length }})
                </NButton>
                <NButton variant="ghost" size="sm" @click="showLibrary = true">
                  <BookOpen :size="13" /> Bibliothèque ({{ savedScripts.length }})
                </NButton>
              </div>
            </div>
          </template>

          <div class="custom-script">
            <div class="custom-top">
              <NInput v-model="customName" placeholder="Nom du script (optionnel)..." style="flex:1" />
              <NDropdown :options="typeOptions" v-model="customType" placeholder="Type" />
              <NButton variant="secondary" size="sm" @click="saveToLibrary" title="Sauvegarder dans la bibliothèque">
                <Save :size="13" />
              </NButton>
              <NButton variant="primary" size="sm"
                :loading="customRunning"
                :disabled="customRunning || !customScript.trim()"
                @click="executeCustomScript">
                <Play :size="14" /> Exécuter
              </NButton>
            </div>
            <textarea v-model="customScript" class="custom-textarea" placeholder="Entrez votre script ici..." rows="8" spellcheck="false"></textarea>
            <div v-if="autosaveLabel" class="autosave-label">
              <Save :size="10" /> {{ autosaveLabel }}
            </div>
          </div>
        </NCard>

        <!-- Script File Browser -->
        <NCard>
          <template #header>
            <div class="section-header">
              <FolderOpen :size="16" />
              <span>Scripts locaux</span>
              <NButton variant="secondary" size="sm" :loading="scriptFilesLoading" @click="loadScriptFiles" style="margin-left:auto">
                <RefreshCw :size="14" />
              </NButton>
            </div>
          </template>
          <div class="file-browser">
            <div class="browse-bar">
              <input v-model="scriptBrowseDir" class="browse-input" placeholder="C:\Scripts" @keyup.enter="loadScriptFiles" />
              <NButton variant="secondary" size="sm" @click="loadScriptFiles">
                <FolderOpen :size="14" /> Parcourir
              </NButton>
            </div>
            <div v-if="scriptFiles.length" class="file-list">
              <button v-for="f in scriptFiles" :key="f.path" class="file-item" :class="{ active: selectedFile?.path === f.path }" @click="openScriptFile(f)">
                <FileCode :size="14" />
                <span class="file-name">{{ f.name }}</span>
                <NBadge variant="info">{{ f.script_type }}</NBadge>
              </button>
            </div>
            <p v-else class="file-empty">Aucun script trouvé. Cliquez "Parcourir".</p>
            <div v-if="selectedFile" class="file-editor">
              <div class="editor-header">
                <span class="editor-filename">{{ selectedFile.name }}</span>
                <div class="editor-actions">
                  <NButton variant="secondary" size="sm" @click="runScriptFile"><Play :size="14" /> Exécuter</NButton>
                  <NButton variant="primary" size="sm" :disabled="!fileEdited" @click="saveScriptFile"><Save :size="14" /> Sauvegarder</NButton>
                </div>
              </div>
              <textarea v-model="fileContent" class="custom-textarea" rows="10" spellcheck="false" @input="fileEdited = true"></textarea>
            </div>
          </div>
        </NCard>
      </div>

      <!-- Droite : console -->
      <NCard class="console-card">
        <template #header>
          <div class="section-header">
            <Terminal :size="16" />
            <span>Console de sortie</span>
            <span v-if="runningScriptId || customRunning" class="elapsed-badge">{{ elapsed }}s</span>
            <NButton variant="secondary" size="sm" @click="clearOutput" style="margin-left:auto">
              <Trash2 :size="14" /> Vider
            </NButton>
          </div>
        </template>
        <div ref="outputRef" class="console-output">
          <div v-if="outputLines.length === 0" class="console-empty">
            <ScrollText :size="24" style="opacity:.3" />
            <p>Exécutez un script pour voir la sortie ici</p>
          </div>
          <pre v-else class="console-text"><template v-for="(line, i) in outputLines" :key="i">{{ line }}
</template></pre>
        </div>
        <div v-if="detectedPaths.length" class="detected-paths">
          <p class="paths-title"><FolderOpen :size="13" /> Chemins détectés — cliquer pour ouvrir :</p>
          <button v-for="p in detectedPaths" :key="p" class="path-chip" @click="openPath(p)">{{ p }}</button>
        </div>
      </NCard>
    </div>

    <!-- Modal templates -->
    <NModal :open="showTemplates" @close="showTemplates = false" title="Templates de scripts">
      <div class="templates-grid">
        <div v-for="t in TEMPLATES" :key="t.id" class="template-card" @click="loadTemplate(t)">
          <div class="template-top">
            <span class="template-name">{{ t.name }}</span>
            <NBadge :variant="t.type === 'powershell' ? 'accent' : 'info'" size="sm">{{ t.type }}</NBadge>
          </div>
          <p class="template-desc">{{ t.desc }}</p>
          <pre class="template-preview">{{ t.content.split('\n').slice(0, 3).join('\n') }}{{ t.content.split('\n').length > 3 ? '\n...' : '' }}</pre>
        </div>
      </div>
      <template #footer><NButton variant="ghost" @click="showTemplates = false">Fermer</NButton></template>
    </NModal>

    <!-- Modal bibliothèque -->
    <NModal :open="showLibrary" @close="showLibrary = false" title="Bibliothèque de scripts">
      <div v-if="savedScripts.length === 0" style="text-align:center;padding:30px;color:var(--text-muted);font-size:13px">
        <BookOpen :size="32" style="opacity:.3;margin-bottom:10px" />
        <p>Aucun script sauvegardé.<br>Créez un script et cliquez <strong>Sauvegarder</strong>.</p>
      </div>
      <div v-else style="display:flex;flex-direction:column;gap:8px;max-height:400px;overflow-y:auto">
        <div v-for="s in savedScripts" :key="s.id" class="lib-item">
          <div class="lib-info">
            <span class="lib-name">{{ s.name }}</span>
            <div style="display:flex;align-items:center;gap:6px;margin-top:2px">
              <NBadge :variant="s.type === 'powershell' ? 'accent' : 'info'" style="font-size:10px">{{ s.type }}</NBadge>
              <span style="font-size:10px;color:var(--text-muted)"><Clock :size="10" style="display:inline" /> {{ s.created_at }}</span>
            </div>
            <pre class="lib-preview">{{ s.content.slice(0, 80) }}{{ s.content.length > 80 ? '...' : '' }}</pre>
          </div>
          <div style="display:flex;flex-direction:column;gap:4px">
            <NButton variant="primary" size="sm" @click="loadFromLibrary(s)"><Play :size="12" /> Charger</NButton>
            <NButton variant="danger" size="sm" @click="deleteFromLibrary(s.id)"><Trash2 :size="12" /></NButton>
          </div>
        </div>
      </div>
      <template #footer><NButton variant="ghost" @click="showLibrary = false">Fermer</NButton></template>
    </NModal>

    <!-- Modal historique -->
    <NModal :open="showHistory" @close="showHistory = false" title="Historique des exécutions">
      <div v-if="execHistory.length === 0" style="text-align:center;padding:30px;color:var(--text-muted);font-size:13px">
        <History :size="32" style="opacity:.3;margin-bottom:10px" />
        <p>Aucune exécution enregistrée.</p>
      </div>
      <div v-else style="display:flex;flex-direction:column;gap:6px;max-height:420px;overflow-y:auto">
        <div v-for="r in execHistory" :key="r.id" class="hist-item">
          <div class="hist-status" :class="r.success ? 'hist-ok' : 'hist-fail'"></div>
          <div class="hist-info">
            <span class="hist-name">{{ r.name }}</span>
            <div class="hist-meta">
              <NBadge :variant="r.type === 'powershell' ? 'accent' : 'info'" size="sm">{{ r.type }}</NBadge>
              <span class="hist-time"><Clock :size="10" style="display:inline;margin-right:3px" />{{ r.ran_at }}</span>
              <span class="hist-dur">{{ r.elapsed_s }}s</span>
              <span class="hist-code" :class="r.exit_code === 0 ? 'code-ok' : 'code-fail'">code {{ r.exit_code }}</span>
            </div>
          </div>
        </div>
      </div>
      <template #footer>
        <NButton variant="danger" size="sm" @click="clearHistory"><Trash2 :size="13" /> Effacer tout</NButton>
        <NButton variant="ghost" @click="showHistory = false">Fermer</NButton>
      </template>
    </NModal>
  </div>
</template>

<style scoped>
.scripts { display:flex; flex-direction:column; gap:16px; }
.page-header { display:flex; justify-content:space-between; align-items:flex-start; flex-wrap:wrap; gap:10px; }
.page-header h1 { font-size:24px; font-weight:700; }
.page-subtitle { color:var(--text-muted); font-size:13px; margin-top:2px; }

.scripts-layout { display:grid; grid-template-columns:1fr 1fr; gap:16px; align-items:start; }
@media(max-width:1100px){ .scripts-layout { grid-template-columns:1fr; } }

.scripts-panel { display:flex; flex-direction:column; gap:16px; }
.section-header { display:flex; align-items:center; gap:8px; width:100%; }
.loading-state { display:flex; flex-direction:column; align-items:center; gap:12px; padding:40px; color:var(--text-muted); }
.scripts-grid { display:flex; flex-direction:column; gap:8px; }

.script-card { padding:14px; border-radius:var(--radius-md); background:var(--bg-tertiary); display:flex; flex-direction:column; gap:8px; }
.script-header { display:flex; align-items:center; justify-content:space-between; gap:8px; }
.script-name { font-size:14px; font-weight:600; color:var(--text-primary); }
.script-badges { display:flex; gap:4px; flex-shrink:0; }
.script-desc { font-size:12px; color:var(--text-muted); line-height:1.4; }

.custom-script { display:flex; flex-direction:column; gap:10px; }
.custom-top { display:flex; align-items:center; gap:8px; }
.custom-textarea {
  width:100%; padding:12px; background:var(--bg-primary); border:1px solid var(--border);
  border-radius:var(--radius-md); color:var(--text-primary); font-family:"JetBrains Mono",monospace;
  font-size:12px; resize:vertical; outline:none; transition:border-color var(--transition-fast); min-height:120px;
}
.custom-textarea:focus { border-color:var(--accent-primary); }
.custom-textarea::placeholder { color:var(--text-muted); }

.autosave-label { font-size:10px; color:var(--text-muted); display:flex; align-items:center; gap:5px; }

.console-card { position:sticky; top:16px; }
.console-output {
  background:var(--bg-primary); border:1px solid var(--border); border-radius:var(--radius-md);
  min-height:400px; max-height:600px; overflow-y:auto; padding:12px;
}
.console-empty { display:flex; flex-direction:column; align-items:center; justify-content:center; gap:8px; height:300px; color:var(--text-muted); font-size:13px; }
.console-text { font-family:"JetBrains Mono",monospace; font-size:12px; line-height:1.6; color:var(--text-secondary); white-space:pre-wrap; word-break:break-all; margin:0; }
.detected-paths { margin-top:12px; padding:10px 12px; background:var(--bg-tertiary); border-radius:var(--radius-md); border:1px solid var(--border); }
.paths-title { font-size:12px; color:var(--text-muted); display:flex; align-items:center; gap:6px; margin-bottom:8px; }
.path-chip { display:inline-block; margin:3px 4px 3px 0; padding:3px 10px; background:var(--bg-secondary); border:1px solid var(--accent-primary); border-radius:20px; color:var(--accent-primary); font-family:"JetBrains Mono",monospace; font-size:11px; cursor:pointer; transition:background var(--transition-fast); }
.path-chip:hover { background:color-mix(in srgb, var(--accent-primary) 15%, transparent); }

.file-browser { display:flex; flex-direction:column; gap:12px; }
.browse-bar { display:flex; gap:8px; }
.browse-input { flex:1; padding:6px 10px; background:var(--bg-primary); border:1px solid var(--border); border-radius:var(--radius-md); color:var(--text-primary); font-family:"JetBrains Mono",monospace; font-size:12px; }
.browse-input:focus { outline:none; border-color:var(--accent-primary); }
.file-list { display:flex; flex-direction:column; gap:2px; max-height:200px; overflow-y:auto; }
.file-item { display:flex; align-items:center; gap:8px; padding:6px 10px; border:none; border-radius:var(--radius-md); background:transparent; cursor:pointer; font-family:inherit; text-align:left; width:100%; transition:background var(--transition-fast); }
.file-item:hover { background:var(--bg-tertiary); }
.file-item.active { background:var(--accent-muted); }
.file-name { flex:1; font-size:13px; color:var(--text-primary); }
.file-empty { font-size:12px; color:var(--text-muted); text-align:center; padding:16px; }
.file-editor { display:flex; flex-direction:column; gap:8px; }
.editor-header { display:flex; align-items:center; justify-content:space-between; }
.editor-filename { font-size:13px; font-weight:500; color:var(--text-primary); font-family:"JetBrains Mono",monospace; }
.editor-actions { display:flex; gap:6px; }

.elapsed-badge { font-family:"JetBrains Mono",monospace; font-size:11px; color:var(--accent-primary); background:var(--accent-muted); padding:2px 8px; border-radius:99px; border:1px solid color-mix(in srgb, var(--accent-primary) 30%, transparent); animation:heartbeat 1s ease-in-out infinite; }

/* Templates modal */
.templates-grid { display:grid; grid-template-columns:1fr 1fr; gap:10px; max-height:480px; overflow-y:auto; }
@media(max-width:600px){ .templates-grid { grid-template-columns:1fr; } }
.template-card { padding:12px; border:1px solid var(--border); border-radius:var(--radius-md); background:var(--bg-secondary); cursor:pointer; transition:border-color .15s, background .15s; }
.template-card:hover { border-color:var(--accent-primary); background:var(--bg-tertiary); }
.template-top { display:flex; align-items:center; justify-content:space-between; gap:8px; margin-bottom:4px; }
.template-name { font-size:13px; font-weight:600; color:var(--text-primary); }
.template-desc { font-size:11px; color:var(--text-muted); margin-bottom:8px; }
.template-preview { font-family:"JetBrains Mono",monospace; font-size:10px; color:var(--text-muted); background:var(--bg-tertiary); padding:6px 8px; border-radius:4px; white-space:pre-wrap; word-break:break-all; max-height:54px; overflow:hidden; margin:0; }

/* Bibliothèque */
.lib-item { display:flex; gap:12px; align-items:flex-start; padding:12px; border:1px solid var(--border); border-radius:8px; background:var(--bg-secondary); transition:border-color .15s; }
.lib-item:hover { border-color:var(--accent-primary); }
.lib-info { flex:1; min-width:0; }
.lib-name { font-size:13px; font-weight:600; color:var(--text-primary); }
.lib-preview { margin-top:6px; font-family:"JetBrains Mono",monospace; font-size:11px; color:var(--text-muted); background:var(--bg-tertiary); padding:6px 8px; border-radius:4px; white-space:pre-wrap; word-break:break-all; max-height:48px; overflow:hidden; }

/* Historique */
.hist-item { display:flex; align-items:center; gap:10px; padding:10px 12px; border:1px solid var(--border); border-radius:var(--radius-md); background:var(--bg-secondary); }
.hist-status { width:8px; height:8px; border-radius:50%; flex-shrink:0; }
.hist-ok   { background:var(--success); }
.hist-fail { background:var(--danger); }
.hist-info { flex:1; min-width:0; }
.hist-name { font-size:13px; font-weight:600; color:var(--text-primary); display:block; margin-bottom:4px; }
.hist-meta { display:flex; align-items:center; gap:8px; flex-wrap:wrap; }
.hist-time { font-size:10px; color:var(--text-muted); }
.hist-dur  { font-size:10px; color:var(--text-muted); font-family:monospace; }
.hist-code { font-size:10px; font-family:monospace; padding:1px 6px; border-radius:3px; }
.code-ok   { color:var(--success); background:rgba(74,222,128,.1); }
.code-fail { color:var(--danger);  background:rgba(239,68,68,.1);  }
</style>
