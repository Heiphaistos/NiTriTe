// Port de TokaISO 7.9 (MIT, Cédrick Roy) : appels et types partagés du labo PXE + VM.
import { ref } from "vue";
import { invoke } from "@/utils/invoke";
import { useNotificationStore } from "@/stores/notifications";

export type Platform = "virtualbox" | "vmware";
export const platformNames: Record<string, string> = { virtualbox: "VirtualBox", vmware: "VMware Workstation", hyperv: "Hyper-V" };

export interface Machine {
  id: string; name: string; item: string; platform: Platform; folder: string; machine: string; file: string;
  state: string; date: string; ram: number; cpu: number; firmware: string; network: string; adapter: string;
  iso?: string; disk?: number; install_iso: boolean; iso_attached: boolean; disk_exceeds_free: boolean;
  test: boolean; boot: string[]; powered_at?: string; menu_at?: string; console_pending: boolean;
}
export interface ControlResult { machine?: Machine; launch?: string; confirmed?: boolean; removed: boolean }
export interface VmList {
  hypervisors: { virtualbox: boolean; vmware: boolean; hyperv: boolean };
  installers: { virtualbox: boolean; vmware: boolean };
  machines: Machine[]; busy: boolean; interfaces: string[]; seven_zip: boolean;
  links: { virtualbox: string; vmware: string; hyperv: string };
}

export const vmStates: Record<string, string> = {
  imported: "Importée",
  running: "VM allumée",
  guest: "Démarrage de l’invité confirmé",
  unknown: "État VMware à vérifier dans la console",
};

export function displayBytes(value: number | undefined | null): string {
  const size = Number(value) || 0;
  const units = ["octets", "Kio", "Mio", "Gio", "Tio"];
  let amount = size;
  let unit = 0;
  while (amount >= 1024 && unit < units.length - 1) { amount /= 1024; unit++; }
  return unit ? `${amount.toLocaleString("fr-FR", { maximumFractionDigits: 1 })} ${units[unit]}` : `${size} octets`;
}

export function errorText(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

export function dateText(value?: string): string {
  return value ? new Date(value).toLocaleString("fr-FR") : "";
}

export function fileName(path: string): string {
  return path.split(/[/\\]/).pop() || path;
}

/** Message affiché après l'ouverture d'une console VMware (vmLaunchMessage de TokaISO). */
export function launchMessage(result: ControlResult): string {
  if (result.launch && !result.confirmed) return "Vérifie le démarrage dans la console VMware.";
  if (result.launch === "console-fallback") return "VM démarrée dans la console VMware.";
  return "";
}

export async function pickFiles(name: string, extensions: string[], multiple = false): Promise<string[]> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ multiple, directory: false, filters: extensions.length ? [{ name, extensions }] : undefined });
  if (!picked) return [];
  return Array.isArray(picked) ? picked : [picked];
}

export async function pickFolder(): Promise<string> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ directory: true, multiple: false });
  return typeof picked === "string" ? picked : "";
}

export async function openLink(url: string) {
  try {
    await invoke("open_url", { url });
  } catch (error) {
    useNotificationStore().error("Lien", errorText(error));
  }
}

export async function openFolder(path: string) {
  try {
    await invoke("open_path", { path });
  } catch (error) {
    useNotificationStore().error("Dossier", errorText(error));
  }
}

const vmList = ref<VmList | null>(null);

/** Hyperviseurs et VM partagés entre les onglets. */
export function useVmList() {
  async function refresh() {
    vmList.value = await invoke<VmList>("boot_lab_vm_list", undefined, 20000);
    return vmList.value;
  }
  function replace(machine: Machine) {
    if (!vmList.value) return;
    const rows = vmList.value.machines.filter((row) => row.id !== machine.id);
    vmList.value = { ...vmList.value, machines: [machine, ...rows] };
  }
  function remove(id: string) {
    if (vmList.value) vmList.value = { ...vmList.value, machines: vmList.value.machines.filter((row) => row.id !== id) };
  }
  return { vmList, refresh, replace, remove };
}

/** Lance une commande et notifie l'erreur ; renvoie `null` en cas d'échec. */
export async function call<T>(cmd: string, args?: Record<string, unknown>, timeout = 30000): Promise<T | null> {
  try {
    return await invoke<T>(cmd, args, timeout);
  } catch (error) {
    useNotificationStore().error("Labo PXE & VM", errorText(error));
    return null;
  }
}

/** ISO, hyperviseur et démarrage repris d'un essai pour « Créer une VM avec cette ISO ». */
export interface InstallSource { iso?: string; platform: string; firmware: string }
export const installPrefill = ref<InstallSource | null>(null);
