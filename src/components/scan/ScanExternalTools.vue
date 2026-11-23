<script setup lang="ts">
import { invoke } from "@/utils/invoke";
import { ExternalLink } from "lucide-vue-next";

interface AntivirusTool {
  name: string; url: string; desc: string;
  category: "scan-local" | "scan-online" | "removal";
}

const tools: AntivirusTool[] = [
  // Scan local
  { name: "Malwarebytes Free", url: "https://www.malwarebytes.com/mwb-download", desc: "Anti-malware référence — détection avancée PUP/rootkit", category: "scan-local" },
  { name: "AdwCleaner", url: "https://toolslib.net/downloads/viewdownload/1-adwcleaner/", desc: "Nettoyeur adware, PUP, barre d'outils — par Malwarebytes", category: "scan-local" },
  { name: "RogueKiller", url: "https://www.adlice.com/roguekiller/", desc: "Anti-rootkit et anti-rogue avancé", category: "scan-local" },
  { name: "HitmanPro (Sophos)", url: "https://www.sophos.com/en-us/free-tools/hitmanpro", desc: "Second avis anti-malware — cloud-based", category: "scan-local" },
  { name: "Emsisoft Emergency Kit", url: "https://www.emsisoft.com/en/emergency-kit/", desc: "Kit portable — aucune installation requise", category: "scan-local" },
  { name: "Kaspersky Removal Tool", url: "https://support.kaspersky.com/kvrt2020", desc: "Outil de désinfection Kaspersky — sans installation", category: "scan-local" },
  { name: "Microsoft Safety Scanner", url: "https://learn.microsoft.com/fr-fr/defender-endpoint/safety-scanner-download", desc: "Outil officiel Microsoft pour scan ponctuel", category: "scan-local" },
  { name: "Dr.Web CureIt!", url: "https://free.drweb.com/cureit/", desc: "Scanner portable Dr.Web — très efficace sur ransomwares", category: "scan-local" },
  // Scan en ligne
  { name: "VirusTotal", url: "https://www.virustotal.com", desc: "Analyser un fichier ou URL avec 70+ moteurs AV", category: "scan-online" },
  { name: "Hybrid Analysis", url: "https://www.hybrid-analysis.com/", desc: "Sandbox gratuit — analyse comportementale fichiers", category: "scan-online" },
  { name: "ANY.RUN", url: "https://any.run/", desc: "Sandbox interactif — exécution malware en ligne", category: "scan-online" },
  { name: "ESET Online Scanner", url: "https://www.eset.com/int/home/online-scanner/", desc: "Scan en ligne ESET — sans installation", category: "scan-online" },
  { name: "Jotti Malware Scan", url: "https://virusscan.jotti.org/", desc: "Scan fichier avec plusieurs moteurs AV", category: "scan-online" },
  // Suppression
  { name: "Rkill", url: "https://www.bleepingcomputer.com/download/rkill/", desc: "Stoppe les processus malware avant désinfection", category: "removal" },
  { name: "Microsoft Defender Offline", url: "https://support.microsoft.com/windows/run-microsoft-defender-offline-9306d528-64bf-4668-5b80-ff533f183d6c", desc: "Scan hors-ligne officiel Windows — détecte rootkits", category: "removal" },
];

async function open(url: string) {
  try { await invoke("open_url", { url }); } catch { window.open(url, "_blank"); }
}
</script>

<template>
  <div class="ext-tools-cats">
    <div class="ext-cat">
      <div class="ext-cat-title">Scanner en local</div>
      <div class="ext-tools-list">
        <button v-for="t in tools.filter(x => x.category === 'scan-local')" :key="t.name"
          class="ext-tool-item" @click="open(t.url)">
          <div class="ext-tool-info">
            <span class="ext-tool-name">{{ t.name }}</span>
            <span class="ext-tool-desc">{{ t.desc }}</span>
          </div>
          <ExternalLink :size="14" style="color:var(--text-muted)" />
        </button>
      </div>
    </div>
    <div class="ext-cat">
      <div class="ext-cat-title" style="color:var(--accent-primary)">Scanner en ligne</div>
      <div class="ext-tools-list">
        <button v-for="t in tools.filter(x => x.category === 'scan-online')" :key="t.name"
          class="ext-tool-item ext-tool-online" @click="open(t.url)">
          <div class="ext-tool-info">
            <span class="ext-tool-name">{{ t.name }}</span>
            <span class="ext-tool-desc">{{ t.desc }}</span>
          </div>
          <ExternalLink :size="14" style="color:var(--accent-primary)" />
        </button>
      </div>
    </div>
    <div class="ext-cat">
      <div class="ext-cat-title" style="color:var(--warning)">Suppression / Désinfection</div>
      <div class="ext-tools-list">
        <button v-for="t in tools.filter(x => x.category === 'removal')" :key="t.name"
          class="ext-tool-item" @click="open(t.url)">
          <div class="ext-tool-info">
            <span class="ext-tool-name">{{ t.name }}</span>
            <span class="ext-tool-desc">{{ t.desc }}</span>
          </div>
          <ExternalLink :size="14" style="color:var(--text-muted)" />
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.ext-tools-cats { display: flex; flex-direction: column; gap: 20px; }
.ext-cat { display: flex; flex-direction: column; gap: 6px; }
.ext-cat-title {
  font-size: 11px; font-weight: 700; color: var(--success);
  text-transform: uppercase; letter-spacing: 0.8px;
  padding: 3px 8px;
  background: color-mix(in srgb, currentColor 10%, transparent);
  border-left: 3px solid currentColor;
  border-radius: 0 var(--radius-sm) var(--radius-sm) 0;
}
.ext-tools-list { display: flex; flex-direction: column; gap: 2px; }
.ext-tool-online { border-left: 2px solid var(--accent-primary); }
.ext-tool-item {
  display: flex; align-items: center; justify-content: space-between;
  gap: 12px; padding: 8px 10px; border: none; border-radius: var(--radius-md);
  background: transparent; cursor: pointer; font-family: inherit;
  text-align: left; width: 100%; transition: background var(--transition-fast);
}
.ext-tool-item:hover { background: var(--bg-tertiary); }
.ext-tool-info { display: flex; flex-direction: column; gap: 2px; }
.ext-tool-name { font-size: 13px; font-weight: 500; color: var(--text-primary); }
.ext-tool-desc { font-size: 11px; color: var(--text-muted); }
</style>
