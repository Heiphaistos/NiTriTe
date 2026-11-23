import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke as tauriInvoke } from "@tauri-apps/api/core";
const mockInvoke = vi.mocked(tauriInvoke);

// Reset module state between tests (global let vars)
vi.mock("@/composables/useProactiveAlerts", async (importOriginal) => {
  return await importOriginal();
});

import {
  useProactiveAlerts,
  activeAlerts,
  dismissAlert,
  dismissAll,
  resetAlertTracking,
} from "@/composables/useProactiveAlerts";

describe("useProactiveAlerts — ref-counting et alertes", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.useFakeTimers();
    mockInvoke.mockReset();
    activeAlerts.value = [];
    resetAlertTracking();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("dismissAlert marque l'alerte comme dismissed", () => {
    activeAlerts.value = [
      { id: "cpu-temp-1", type: "temp", severity: "critical", message: "CPU chaud", timestamp: new Date(), dismissed: false },
    ];
    dismissAlert("cpu-temp-1");
    expect(activeAlerts.value[0].dismissed).toBe(true);
  });

  it("dismissAll marque toutes les alertes comme dismissed", () => {
    activeAlerts.value = [
      { id: "a1", type: "temp", severity: "critical", message: "msg1", timestamp: new Date(), dismissed: false },
      { id: "a2", type: "disk", severity: "warning", message: "msg2", timestamp: new Date(), dismissed: false },
    ];
    dismissAll();
    expect(activeAlerts.value.every(a => a.dismissed)).toBe(true);
  });

  it("dismissAlert sur ID inexistant ne crashe pas", () => {
    activeAlerts.value = [];
    expect(() => dismissAlert("non-existant")).not.toThrow();
  });

  it("checkOnce ajoute alerte CPU si temp critique", async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_temperatures") return [{ sensor_name: "CPU Core #0", temp_celsius: 95, source: "cpu" }];
      if (cmd === "get_smart_info") return [];
      return null;
    });

    const { checkOnce } = useProactiveAlerts({ cpuTempCritical: 90, gpuTempCritical: 85, diskUsageWarn: 85, diskUsageCritical: 95, ramUsageWarn: 90 });
    await checkOnce();
    await checkOnce(); // confirmée sur deux sondages consécutifs

    const cpuAlert = activeAlerts.value.find(a => a.id.startsWith("cpu-temp"));
    expect(cpuAlert).toBeDefined();
    expect(cpuAlert!.severity).toBe("critical");
    expect(cpuAlert!.type).toBe("temp");
  });

  it("checkOnce ne double les alertes existantes", async () => {
    mockInvoke
      .mockResolvedValue([{ sensor_name: "CPU Core", temp_celsius: 95, source: "cpu" }]); // get_temperatures first call
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_temperatures") return [{ sensor_name: "CPU Core", temp_celsius: 95, source: "cpu" }];
      if (cmd === "get_system_info") return null;
      if (cmd === "get_smart_info") return [];
      return null;
    });

    const { checkOnce } = useProactiveAlerts({ cpuTempCritical: 90, gpuTempCritical: 85, diskUsageWarn: 85, diskUsageCritical: 95, ramUsageWarn: 90 });
    await checkOnce();
    await checkOnce(); // 2ème appel — même alerte, ne doit pas dupliquer

    const cpuAlerts = activeAlerts.value.filter(a => a.id.startsWith("cpu-temp") && !a.dismissed);
    expect(cpuAlerts.length).toBe(1);
  });

  it("checkOnce ajoute alerte RAM si usage élevé", async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_temperatures") return [];
      if (cmd === "get_system_info") return { ram: { usage_percent: 92 } };
      if (cmd === "get_smart_info") return [];
      return null;
    });

    const { checkOnce } = useProactiveAlerts({ cpuTempCritical: 90, gpuTempCritical: 85, diskUsageWarn: 85, diskUsageCritical: 95, ramUsageWarn: 90 });
    await checkOnce();

    const ramAlert = activeAlerts.value.find(a => a.id === "ram-usage");
    expect(ramAlert).toBeDefined();
    expect(ramAlert!.type).toBe("ram");
  });

  it("checkOnce ignore temp <= 0", async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_temperatures") return [{ sensor_name: "CPU Core", temp_celsius: 0, source: "cpu" }];
      if (cmd === "get_system_info") return null;
      if (cmd === "get_smart_info") return [];
      return null;
    });

    const { checkOnce } = useProactiveAlerts({ cpuTempCritical: 90, gpuTempCritical: 85, diskUsageWarn: 85, diskUsageCritical: 95, ramUsageWarn: 90 });
    await checkOnce();

    expect(activeAlerts.value.filter(a => a.type === "temp")).toHaveLength(0);
  });

  it("SMART: alerte si health_status n'est pas dans la liste saine", async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_temperatures") return [];
      if (cmd === "get_system_info") return null;
      if (cmd === "get_smart_info") return [{ name: "SSD 1", reallocated_sectors: 0, health_status: "Failed" }];
      return null;
    });

    const { checkOnce } = useProactiveAlerts({ cpuTempCritical: 90, gpuTempCritical: 85, diskUsageWarn: 85, diskUsageCritical: 95, ramUsageWarn: 90 });
    await checkOnce();

    const smartAlert = activeAlerts.value.find(a => a.id.startsWith("smart-health"));
    expect(smartAlert).toBeDefined();
    expect(smartAlert!.severity).toBe("critical");
  });

  it("SMART: pas d'alerte si health_status contient 'ok'", async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_temperatures") return [];
      if (cmd === "get_system_info") return null;
      if (cmd === "get_smart_info") return [{ name: "SSD 1", reallocated_sectors: 0, health_status: "OK" }];
      return null;
    });

    const { checkOnce } = useProactiveAlerts({ cpuTempCritical: 90, gpuTempCritical: 85, diskUsageWarn: 85, diskUsageCritical: 95, ramUsageWarn: 90 });
    await checkOnce();

    const smartAlerts = activeAlerts.value.filter(a => a.type === "smart");
    expect(smartAlerts).toHaveLength(0);
  });

  it("checkOnce efface une alerte CPU dont la température est redescendue sous le seuil", async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_temperatures") return [{ sensor_name: "CPU Core", temp_celsius: 95, source: "cpu" }];
      if (cmd === "get_system_info") return null;
      if (cmd === "get_smart_info") return [];
      return null;
    });
    const { checkOnce } = useProactiveAlerts({ cpuTempCritical: 90, gpuTempCritical: 85, diskUsageWarn: 85, diskUsageCritical: 95, ramUsageWarn: 90 });
    await checkOnce();
    await checkOnce();
    expect(activeAlerts.value.some(a => a.id.startsWith("cpu-temp") && !a.dismissed)).toBe(true);

    // La température redescend sous le seuil critique
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_temperatures") return [{ sensor_name: "CPU Core", temp_celsius: 55, source: "cpu" }];
      if (cmd === "get_system_info") return null;
      if (cmd === "get_smart_info") return [];
      return null;
    });
    await checkOnce();

    // L'alerte doit disparaître, pas rester affichée indéfiniment comme si le
    // CPU était toujours en surchauffe critique.
    expect(activeAlerts.value.some(a => a.id.startsWith("cpu-temp") && !a.dismissed)).toBe(false);
  });

  it("checkOnce ne touche pas aux alertes existantes si le fetch échoue (état inconnu)", async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_temperatures") return [{ sensor_name: "CPU Core", temp_celsius: 95, source: "cpu" }];
      if (cmd === "get_system_info") return null;
      if (cmd === "get_smart_info") return [];
      return null;
    });
    const { checkOnce } = useProactiveAlerts({ cpuTempCritical: 90, gpuTempCritical: 85, diskUsageWarn: 85, diskUsageCritical: 95, ramUsageWarn: 90 });
    await checkOnce();
    await checkOnce();
    expect(activeAlerts.value.some(a => a.id.startsWith("cpu-temp") && !a.dismissed)).toBe(true);

    // Le prochain sondage échoue (ex: capteurs temporairement indisponibles)
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_temperatures") throw new Error("capteurs indisponibles");
      if (cmd === "get_system_info") return null;
      if (cmd === "get_smart_info") return [];
      return null;
    });
    await checkOnce();

    // L'échec du sondage ne doit pas être interprété comme "condition résolue"
    // et effacer l'alerte à tort.
    expect(activeAlerts.value.some(a => a.id.startsWith("cpu-temp") && !a.dismissed)).toBe(true);
  });
  const T = { cpuTempCritical: 90, gpuTempCritical: 85, diskUsageWarn: 85, diskUsageCritical: 95, ramUsageWarn: 90 };
  const temps = (list: unknown) => mockInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === "get_temperatures") { if (list instanceof Error) throw list; return list; }
    if (cmd === "get_smart_info") return [];
    return null;
  });
  const cpuAlerts = () => activeAlerts.value.filter(a => a.id.startsWith("cpu-temp") && !a.dismissed);

  it("un pic isolé (un seul sondage) ne déclenche pas d'alerte", async () => {
    const { checkOnce } = useProactiveAlerts(T);
    temps([{ sensor_name: "CPU Package", temp_celsius: 97, source: "LibreHardwareMonitor" }]);
    await checkOnce();
    temps([{ sensor_name: "CPU Package", temp_celsius: 60, source: "LibreHardwareMonitor" }]);
    await checkOnce();
    expect(cpuAlerts()).toHaveLength(0);
  });

  it("« Distance to TjMax » et les zones ACPI ne déclenchent jamais d'alerte", async () => {
    const { checkOnce } = useProactiveAlerts(T);
    temps([
      { sensor_name: "CPU Core #1 Distance to TjMax", temp_celsius: 95, source: "LibreHardwareMonitor" },
      { sensor_name: "CPU Package (Zone 1)", temp_celsius: 99, source: "ACPI WMI" },
    ]);
    await checkOnce();
    await checkOnce();
    expect(cpuAlerts()).toHaveLength(0);
  });

  it("le texte de l'alerte suit la température en direct", async () => {
    const { checkOnce } = useProactiveAlerts(T);
    temps([{ sensor_name: "CPU Package", temp_celsius: 100, source: "LibreHardwareMonitor" }]);
    await checkOnce();
    await checkOnce();
    temps([{ sensor_name: "CPU Package", temp_celsius: 92, source: "LibreHardwareMonitor" }]);
    await checkOnce();
    expect(cpuAlerts()).toHaveLength(1);
    expect(cpuAlerts()[0].message).toContain("92 °C");
  });

  it("trois sondages ratés d'affilée retirent une alerte de température périmée", async () => {
    const { checkOnce } = useProactiveAlerts(T);
    temps([{ sensor_name: "CPU Package", temp_celsius: 98, source: "LibreHardwareMonitor" }]);
    await checkOnce();
    await checkOnce();
    temps(new Error("capteurs indisponibles"));
    await checkOnce();
    await checkOnce();
    expect(cpuAlerts()).toHaveLength(1);
    await checkOnce();
    expect(cpuAlerts()).toHaveLength(0);
  });
});
