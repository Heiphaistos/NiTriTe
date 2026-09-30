<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { Cpu, MemoryStick, HardDrive, Wifi, Activity } from "lucide-vue-next";
import type { SystemMonitorPayload } from "@/types/diagnostic";

const appVersion = __APP_VERSION__;
const cpuUsage = ref(0);
const ramUsage = ref(0);
const diskUsage = ref(0);
const networkDown = ref(0);

let monitorInterval: ReturnType<typeof setInterval> | null = null;
let unlistenMonitor: (() => void) | null = null;

onMounted(async () => {
  try {
    const { listen } = await import("@tauri-apps/api/event");
    // Stocker le unlisten pour nettoyage propre
    unlistenMonitor = await listen<SystemMonitorPayload>("system-monitor", (event) => {
      const data = event.payload;
      // Le backend envoie des flottants bruts (14.212326049884688) : arrondis
      // ici, sinon la barre d'etat les affichait tels quels.
      cpuUsage.value   = Math.round(data.cpu_percent      ?? 0);
      ramUsage.value   = Math.round(data.ram_percent       ?? 0);
      diskUsage.value  = Math.round(data.disk_percent      ?? 0);
      networkDown.value = Math.round(data.network_down_kbs ?? 0);
    });
  } catch {
    monitorInterval = setInterval(() => {
      cpuUsage.value    = Math.round(10 + Math.random() * 40);
      ramUsage.value    = Math.round(30 + Math.random() * 30);
      diskUsage.value   = Math.round(40 + Math.random() * 20);
      networkDown.value = Math.round(Math.random() * 500);
    }, 2000);
  }
});

onUnmounted(() => {
  if (monitorInterval) clearInterval(monitorInterval);
  if (unlistenMonitor) unlistenMonitor();
});

function statusColor(value: number): string {
  if (value > 90) return "var(--danger)";
  if (value > 75) return "var(--warning)";
  return "var(--success)";
}

function dotClass(value: number): string {
  if (value > 90) return "status-dot--danger";
  if (value > 75) return "status-dot--warning";
  return "status-dot--success";
}

function formatNet(kbs: number): string {
  if (kbs >= 1024) return `${(kbs / 1024).toFixed(1)} MB/s`;
  return `${kbs} KB/s`;
}

const networkColor = computed(() => {
  if (networkDown.value > 5000) return "var(--warning)";
  return "var(--success)";
});
</script>

<template>
  <footer class="status-bar">
    <!-- CPU -->
    <div class="status-item">
      <span class="status-dot" :class="dotClass(cpuUsage)" />
      <Cpu :size="11" class="status-icon" />
      <span class="status-label">CPU</span>
      <div class="status-mini-bar">
        <div class="status-mini-fill" :style="{ width: `${cpuUsage}%`, background: statusColor(cpuUsage) }" />
      </div>
      <span class="status-value" :style="{ color: statusColor(cpuUsage) }">{{ cpuUsage }}%</span>
    </div>

    <span class="status-sep" />

    <!-- RAM -->
    <div class="status-item">
      <span class="status-dot" :class="dotClass(ramUsage)" />
      <MemoryStick :size="11" class="status-icon" />
      <span class="status-label">RAM</span>
      <div class="status-mini-bar">
        <div class="status-mini-fill" :style="{ width: `${ramUsage}%`, background: statusColor(ramUsage) }" />
      </div>
      <span class="status-value" :style="{ color: statusColor(ramUsage) }">{{ ramUsage }}%</span>
    </div>

    <span class="status-sep" />

    <!-- Disque -->
    <div class="status-item">
      <span class="status-dot" :class="dotClass(diskUsage)" />
      <HardDrive :size="11" class="status-icon" />
      <span class="status-label">Disque</span>
      <div class="status-mini-bar">
        <div class="status-mini-fill" :style="{ width: `${diskUsage}%`, background: statusColor(diskUsage) }" />
      </div>
      <span class="status-value" :style="{ color: statusColor(diskUsage) }">{{ diskUsage }}%</span>
    </div>

    <span class="status-sep" />

    <!-- Réseau -->
    <div class="status-item">
      <Wifi :size="11" class="status-icon" />
      <span class="status-label">↓</span>
      <span class="status-value" :style="{ color: networkColor }">{{ formatNet(networkDown) }}</span>
    </div>

    <div class="status-spacer" />

    <!-- Live indicator -->
    <div class="status-item status-live">
      <Activity :size="10" class="animate-heartbeat" style="color: var(--success)" />
      <span class="status-live-text">Live</span>
    </div>

    <span class="status-sep" />

    <!-- Version -->
    <div class="status-item">
      <span class="status-version">NiTriTe v{{ appVersion }}</span>
    </div>
  </footer>
</template>

<style scoped>
.status-bar {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 16px;
  border-top: 1px solid var(--border);
  background: linear-gradient(180deg, var(--bg-secondary) 0%, var(--bg-primary) 100%);
  font-size: 11px;
  color: var(--text-muted);
  min-height: 26px;
  user-select: none;
}

.status-item {
  display: flex;
  align-items: center;
  gap: 5px;
}

.status-sep {
  width: 1px;
  height: 12px;
  background: var(--border-hover);
  opacity: 0.6;
}

.status-icon {
  color: var(--text-muted);
  flex-shrink: 0;
}

.status-label {
  color: var(--text-muted);
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
}
.status-dot--success { background: var(--success); box-shadow: 0 0 4px var(--success); }
.status-dot--warning { background: var(--warning); box-shadow: 0 0 4px var(--warning); }
.status-dot--danger  { background: var(--danger);  box-shadow: 0 0 4px var(--danger); }

/* Mini progress bar */
.status-mini-bar {
  width: 36px;
  height: 3px;
  background: var(--bg-elevated);
  border-radius: 99px;
  overflow: hidden;
}
.status-mini-fill {
  height: 100%;
  border-radius: 99px;
  transition: width 600ms ease;
}

.status-value {
  font-weight: 600;
  font-family: "JetBrains Mono", monospace;
  font-size: 10px;
  min-width: 32px;
}

.status-spacer {
  flex: 1;
}

.status-live {
  gap: 4px;
}
.status-live-text {
  font-size: 9px;
  font-weight: 700;
  color: var(--success);
  text-transform: uppercase;
  letter-spacing: 0.06em;
}

.status-version {
  font-size: 10px;
  color: var(--text-muted);
  font-family: "JetBrains Mono", monospace;
  opacity: 0.7;
}
</style>
