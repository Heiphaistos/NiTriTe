import { ref, onUnmounted, getCurrentInstance } from 'vue';
import { invoke } from "@/utils/invoke";
import type { SysInfo, SmartDiskInfo } from "@/types/diagnostic";

interface AlertThresholds {
  cpuTempCritical: number;   // °C
  gpuTempCritical: number;   // °C
  diskUsageWarn: number;     // %
  diskUsageCritical: number; // %
  ramUsageWarn: number;      // %
}

const DEFAULT_THRESHOLDS: AlertThresholds = {
  cpuTempCritical: 90,
  gpuTempCritical: 85,
  diskUsageWarn: 85,
  diskUsageCritical: 95,
  ramUsageWarn: 90,
};

export const activeAlerts = ref<Array<{
  id: string;
  type: 'temp' | 'disk' | 'ram' | 'smart';
  severity: 'warning' | 'critical';
  message: string;
  timestamp: Date;
  dismissed: boolean;
}>>([]);

let monitorInterval: ReturnType<typeof setInterval> | null = null;
let instanceCount = 0;

// Sondages consécutifs au-dessus du seuil, par capteur : une seule lecture haute
// (pic de charge au lancement de Nitrite) ne doit pas afficher « Surchauffe ».
const tempHits = new Map<string, number>();
const TEMP_CONFIRM_POLLS = 2;
// Sondages de températures ratés d'affilée : au-delà, la valeur affichée est
// périmée et l'alerte est retirée plutôt que laissée figée.
let tempFailures = 0;
const TEMP_STALE_FAILURES = 3;

function addAlert(id: string, type: 'temp' | 'disk' | 'ram' | 'smart', severity: 'warning' | 'critical', message: string) {
  const existing = activeAlerts.value.find(a => a.id === id && !a.dismissed);
  if (existing) {
    // Valeur en direct : le texte suivait sinon la toute première lecture (ex. 100 °C)
    existing.message = message;
    existing.severity = severity;
    return;
  }
  activeAlerts.value.push({ id, type, severity, message, timestamp: new Date(), dismissed: false });
  // Notification système (si permission)
  if ('Notification' in window && Notification.permission === 'granted') {
    new Notification(`Nitrite — ${severity === 'critical' ? '🔴' : '🟡'} ${message}`);
  }
}

// Retire les alertes d'un type dont l'id n'est plus dans currentIds : sans ça,
// une alerte restait affichée indéfiniment (bannière globale sur toute l'app,
// voir NAlertBanner.vue) même longtemps après le retour à la normale — un pic
// de température transitoire de quelques secondes laissait un faux "Surchauffe
// critique !" épinglé jusqu'au dismiss manuel, et empêchait toute nouvelle
// notification pour une récidive du même capteur tant que l'ancienne alerte
// n'était pas fermée. N'est appelé qu'après un sondage RÉUSSI (currentIds
// reflète l'état réel) — un échec de sondage ne doit jamais être interprété
// comme "condition résolue".
function clearResolvedAlerts(type: 'temp' | 'disk' | 'ram' | 'smart', currentIds: Set<string>) {
  activeAlerts.value = activeAlerts.value.filter(a => a.type !== type || currentIds.has(a.id));
}

export function dismissAlert(id: string) {
  const alert = activeAlerts.value.find(a => a.id === id);
  if (alert) alert.dismissed = true;
}

/** Remet à zéro le suivi des sondages (tests). */
export function resetAlertTracking() {
  tempHits.clear();
  tempFailures = 0;
}

export function dismissAll() {
  activeAlerts.value.forEach(a => { a.dismissed = true; });
}

export function useProactiveAlerts(thresholds: AlertThresholds = DEFAULT_THRESHOLDS) {
  async function checkOnce() {
    try {
      // Vérifier températures
      const temps = await invoke<Array<{ sensor_name: string; temp_celsius: number; source: string }>>('get_temperatures');
      tempFailures = 0;
      const currentTempIds = new Set<string>();
      const over = (id: string, hot: boolean) => {
        const hits = hot ? (tempHits.get(id) ?? 0) + 1 : 0;
        if (hits) tempHits.set(id, hits); else tempHits.delete(id);
        return hits >= TEMP_CONFIRM_POLLS;
      };
      for (const t of temps) {
        if (t.temp_celsius <= 0) continue;
        const name = t.sensor_name.toLowerCase();
        // « Distance to TjMax » = marge avant la limite (monte quand le CPU refroidit) ;
        // zones ACPI = valeur souvent figée par le firmware : jamais d'alerte critique dessus.
        if (name.includes('tjmax') || t.source.toLowerCase().includes('acpi')) continue;
        const value = Math.round(t.temp_celsius);
        if (name.includes('cpu') || t.source.toLowerCase().includes('cpu')) {
          const id = `cpu-temp-${t.sensor_name}`;
          if (over(id, t.temp_celsius >= thresholds.cpuTempCritical)) {
            addAlert(id, 'temp', 'critical', `CPU ${t.sensor_name} : ${value} °C — Surchauffe critique !`);
            currentTempIds.add(id);
          }
        }
        if (name.includes('gpu')) {
          const id = `gpu-temp-${t.sensor_name}`;
          if (over(id, t.temp_celsius >= thresholds.gpuTempCritical)) {
            addAlert(id, 'temp', 'critical', `GPU ${t.sensor_name} : ${value} °C — Surchauffe !`);
            currentTempIds.add(id);
          }
        }
      }
      clearResolvedAlerts('temp', currentTempIds);
    } catch {
      // Sondage échoué : état inconnu. On garde l'alerte un temps, puis on la retire
      // pour ne pas afficher indéfiniment une température périmée.
      tempFailures++;
      if (tempFailures >= TEMP_STALE_FAILURES) {
        tempHits.clear();
        clearResolvedAlerts('temp', new Set());
      }
    }

    try {
      // Vérifier utilisation disques/RAM
      const sysInfo = await invoke<SysInfo>('get_system_info').catch(() => null);
      if (sysInfo) {
        const currentRamIds = new Set<string>();
        if (sysInfo.ram?.usage_percent >= thresholds.ramUsageWarn) {
          const sev = sysInfo.ram.usage_percent >= 95 ? 'critical' : 'warning';
          addAlert('ram-usage', 'ram', sev, `RAM: ${sysInfo.ram.usage_percent.toFixed(0)}% utilisée`);
          currentRamIds.add('ram-usage');
        }
        clearResolvedAlerts('ram', currentRamIds);

        const currentDiskIds = new Set<string>();
        if (sysInfo.disks) {
          for (const d of sysInfo.disks) {
            for (const p of (d.partitions || [])) {
              const label = p.mount_point || d.model || 'Disque';
              const id = `disk-${label}`;
              if (p.usage_percent >= thresholds.diskUsageCritical) {
                addAlert(id, 'disk', 'critical', `Disque ${label}: ${p.usage_percent.toFixed(0)}% plein!`);
                currentDiskIds.add(id);
              } else if (p.usage_percent >= thresholds.diskUsageWarn) {
                addAlert(id, 'disk', 'warning', `Disque ${label}: ${p.usage_percent.toFixed(0)}% utilisé`);
                currentDiskIds.add(id);
              }
            }
          }
        }
        clearResolvedAlerts('disk', currentDiskIds);
      }
      // sysInfo null (get_system_info a échoué) : état inconnu, ne rien effacer
    } catch {}

    try {
      // Vérifier SMART
      const smart = await invoke<SmartDiskInfo[]>('get_smart_info');
      const currentSmartIds = new Set<string>();
      for (const s of smart) {
        if (s.reallocated_sectors > 0) {
          const id = `smart-realloc-${s.name}`;
          addAlert(id, 'smart', 'critical',
            `${s.name}: ${s.reallocated_sectors} secteur(s) réalloué(s) — Défaillance imminente!`);
          currentSmartIds.add(id);
        }
        const healthy = ['ok', 'good', 'passed', 'sain', 'healthy'];
        if (s.health_status && !healthy.some(h => s.health_status.toLowerCase().includes(h))) {
          const id = `smart-health-${s.name}`;
          addAlert(id, 'smart', 'critical', `${s.name}: État SMART dégradé (${s.health_status})`);
          currentSmartIds.add(id);
        }
      }
      clearResolvedAlerts('smart', currentSmartIds);
    } catch { /* sondage échoué : état inconnu, ne pas toucher aux alertes existantes */ }
  }

  function start(intervalMs = 60000) {
    instanceCount++;
    if (instanceCount > 1) return; // timer déjà actif — on incrémente juste le compteur
    checkOnce();
    monitorInterval = setInterval(checkOnce, intervalMs);
    if ('Notification' in window && Notification.permission === 'default') {
      Notification.requestPermission();
    }
  }

  function stop() {
    instanceCount = Math.max(0, instanceCount - 1);
    if (instanceCount === 0 && monitorInterval) {
      clearInterval(monitorInterval);
      monitorInterval = null;
    }
  }

  if (getCurrentInstance()) onUnmounted(() => stop());

  return { start, stop, activeAlerts, dismissAlert, dismissAll, checkOnce };
}
