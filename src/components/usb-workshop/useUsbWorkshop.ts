// Port de TokaISO 7.9 (MIT, Cédrick Roy) : état partagé de l'atelier USB (usb_workshop.js).
import { computed, ref } from "vue";
import { invoke } from "@/utils/invoke";

export type Method = "ventoy" | "rufus" | "etcher";

export interface UsbVolume { drive: string; label: string; format: string; size: number; free: number }
export interface UsbDisk {
  id: string; number: number; name: string; serial: string; size: number;
  hardwareSerial: string; diskPath: string; partitionStyle: string; logicalSector: number;
  accessPaths: string[]; volumes: UsbVolume[]; drive: string; format: string; free: number; ventoy: boolean;
  kind: string;
}
export interface UsbImage { path: string; name: string; size: number; methods: Method[] }

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

// État unique de la page (équivalent des variables globales usb79*).
const images = ref<UsbImage[]>([]);
const disks = ref<UsbDisk[]>([]);
const device = ref("");
const method = ref<Method>("ventoy");
const busy = ref(false);
const status = ref("");

const drive = computed(() => disks.value.find((row) => row.id === device.value));
const paths = computed(() => images.value.map((row) => row.path));

function notify(message: string) { status.value = message; }

/** usb79Run : une seule opération USB à la fois, statut partagé. */
async function run<T>(cmd: string, payload: Record<string, unknown> | null, timeout = 30000): Promise<T> {
  if (busy.value) throw new Error("Une opération USB est déjà en cours.");
  busy.value = true;
  notify("Opération USB en cours…");
  try {
    const result = await invoke<T>(cmd, payload ? { payload } : undefined, timeout);
    notify("");
    return result;
  } catch (error) {
    notify(errorText(error));
    throw error;
  } finally {
    busy.value = false;
  }
}

async function refreshDevices(preferred?: string) {
  const old = preferred || device.value;
  const result = await invoke<{ disks: UsbDisk[] }>("usb_ws_list", undefined, 21000);
  disks.value = result.disks;
  device.value = disks.value.some((row) => row.id === old) ? old : "";
}

async function setImages(selected: string[], add = false) {
  const list = [...new Set([...(add ? paths.value : []), ...selected])].slice(0, 24);
  const result = await invoke<{ images: UsbImage[] }>("usb_ws_images", { payload: { paths: list } }, 20000);
  images.value = result.images;
  if (images.value.length === 1 && !images.value[0].methods.includes(method.value)) {
    method.value = images.value[0].methods[0] || "ventoy";
  }
}

export function useUsbWorkshop() {
  return { images, disks, device, method, busy, status, drive, paths, notify, run, refreshDevices, setImages };
}
