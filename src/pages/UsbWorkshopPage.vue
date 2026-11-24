<script setup lang="ts">
// Port de TokaISO 7.9 (MIT, Cédrick Roy) : Atelier USB (index.html, usb_workshop.js, usb_workshop.css).
import { computed, onMounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import NCard from "@/components/ui/NCard.vue";
import NButton from "@/components/ui/NButton.vue";
import NBadge from "@/components/ui/NBadge.vue";
import { Usb, RefreshCw, FilePlus, X } from "lucide-vue-next";
import { useUsbWorkshop, displayBytes, errorText } from "@/components/usb-workshop/useUsbWorkshop";
import UsbWriteTab from "@/components/usb-workshop/UsbWriteTab.vue";
import UsbThemesTab from "@/components/usb-workshop/UsbThemesTab.vue";
import UsbHealthTab from "@/components/usb-workshop/UsbHealthTab.vue";
import UsbBootTab from "@/components/usb-workshop/UsbBootTab.vue";
import UsbTemplatesTab from "@/components/usb-workshop/UsbTemplatesTab.vue";
import UsbProfilesTab from "@/components/usb-workshop/UsbProfilesTab.vue";

const ws = useUsbWorkshop();
const { images, disks, device, busy, status, drive } = ws;

const tabs = [
  { key: "write", label: "Créer et vérifier" },
  { key: "themes", label: "Thèmes et menu" },
  { key: "health", label: "Santé USB" },
  { key: "boot", label: "Persistance et démarrage" },
  { key: "templates", label: "Modèles de clés" },
  { key: "profiles", label: "Profils Ventoy" },
] as const;
const view = ref<(typeof tabs)[number]["key"]>("write");

const labels: Record<string, string> = { ventoy: "Ventoy", rufus: "Rufus", etcher: "balenaEtcher" };

const deviceInfo = computed(() => {
  const d = drive.value;
  if (!d) return "Sélectionne le support USB.";
  return [d.drive, d.name, displayBytes(d.size), d.partitionStyle, d.ventoy && "Ventoy"].filter(Boolean).join(" · ");
});

async function handle(operation: () => Promise<unknown>) {
  try { await operation(); } catch (error) { ws.notify(errorText(error)); }
}

async function pickImages() {
  const chosen = await open({
    multiple: true,
    filters: [{ name: "Images de démarrage", extensions: ["iso", "img", "wim", "efi", "vhd", "vhdx", "xz", "gz", "bz2", "zip", "bin"] }],
  });
  const list = Array.isArray(chosen) ? chosen : chosen ? [chosen] : [];
  if (list.length) await ws.setImages(list, true);
}

function removeImage(path: string) {
  images.value = images.value.filter((row) => row.path !== path);
}

onMounted(() => { handle(() => ws.refreshDevices()); });
</script>

<template>
  <div class="usbws">
    <div class="page-header">
      <h1><Usb :size="22" /> Atelier USB</h1>
      <p class="page-subtitle">Ventoy, Rufus, balenaEtcher : préparer, vérifier et personnaliser une clé de démarrage</p>
    </div>

    <div class="usbws-selection">
      <NCard>
        <h3>Images à utiliser</h3>
        <NButton variant="secondary" size="sm" :disabled="busy" @click="handle(pickImages)"><FilePlus :size="14" /> Choisir des images</NButton>
        <div class="usbws-images">
          <p v-if="!images.length" class="usbws-muted">Aucune image sélectionnée.</p>
          <div v-for="row in images" :key="row.path" class="usbws-row">
            <div class="usbws-row-main">
              <strong>{{ row.name }}</strong>
              <span>{{ [displayBytes(row.size), ...row.methods.map((m) => labels[m])].join(" · ") }}</span>
            </div>
            <NButton variant="ghost" size="sm" :disabled="busy" @click="removeImage(row.path)"><X :size="14" /> Retirer</NButton>
          </div>
        </div>
      </NCard>
      <NCard>
        <h3>Support USB</h3>
        <div class="usbws-fields">
          <label>Clé USB, disque externe ou carte SD
            <select v-model="device" :disabled="busy">
              <option value="">Sélectionner un support</option>
              <option v-for="row in disks" :key="row.id" :value="row.id">
                {{ [row.kind, row.drive, row.name, displayBytes(row.size), row.ventoy && "Ventoy"].filter(Boolean).join(" · ") }}
              </option>
            </select>
          </label>
          <NButton variant="secondary" size="sm" :disabled="busy" @click="handle(() => ws.refreshDevices())"><RefreshCw :size="14" /> Actualiser</NButton>
        </div>
        <p class="usbws-muted">{{ deviceInfo }} <NBadge v-if="drive?.ventoy" variant="success">Ventoy</NBadge></p>
        <p class="usbws-muted">Clés USB, disques durs et SSD externes (USB) et cartes SD sont proposés ; jamais le disque système ni le disque de démarrage.</p>
      </NCard>
    </div>

    <nav class="usbws-tabs" aria-label="Outils USB">
      <button v-for="tab in tabs" :key="tab.key" :class="{ active: view === tab.key }" :aria-pressed="view === tab.key" @click="view = tab.key">
        {{ tab.label }}
      </button>
    </nav>

    <NCard>
      <UsbWriteTab v-if="view === 'write'" />
      <UsbThemesTab v-else-if="view === 'themes'" />
      <UsbHealthTab v-else-if="view === 'health'" />
      <UsbBootTab v-else-if="view === 'boot'" />
      <UsbTemplatesTab v-else-if="view === 'templates'" />
      <UsbProfilesTab v-else />
    </NCard>

    <p class="usbws-status" role="status" aria-live="polite">{{ status }}</p>
  </div>
</template>

<style scoped>
.page-header h1 { font-size: 24px; font-weight: 700; display: flex; align-items: center; gap: 8px; }
.page-subtitle { color: var(--text-muted); font-size: 13px; margin-top: 2px; }
.usbws-selection { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 14px; }
.usbws-selection h3 { font-size: 15px; margin: 0 0 8px; }
.usbws-images { max-height: 160px; overflow: auto; margin-top: 6px; }
.usbws-tabs { display: flex; flex-wrap: wrap; gap: 6px; }
.usbws-tabs button {
  background: var(--bg-secondary); border: 1px solid var(--border); border-radius: var(--radius-md);
  padding: 7px 12px; color: var(--text-primary); font-family: inherit; font-size: 13px; cursor: pointer;
}
.usbws-tabs button:hover { border-color: var(--accent-primary); }
.usbws-tabs button.active { background: var(--accent-muted); color: var(--accent-primary); border-color: var(--accent-primary); }
.usbws-status { min-height: 1.5em; font-size: 13px; color: var(--text-secondary); overflow-wrap: anywhere; }
@media (max-width: 720px) { .usbws-selection { grid-template-columns: 1fr; } }
</style>

<style>
/* Classes partagées par les onglets de l'atelier USB (préfixe .usbws). */
.usbws { display: flex; flex-direction: column; gap: 14px; }
.usbws h3 { font-size: 15px; font-weight: 600; margin: 10px 0 6px; }
.usbws-muted { font-size: 13px; color: var(--text-muted); line-height: 1.5; margin: 4px 0; }
.usbws-warning { font-size: 13px; color: var(--danger, #e5484d); font-weight: 600; margin: 6px 0; }
.usbws-fields { display: flex; flex-wrap: wrap; gap: 10px; align-items: flex-end; margin: 8px 0; }
.usbws-fields label { display: flex; flex-direction: column; gap: 3px; font-size: 12px; color: var(--text-secondary); min-width: 0; }
.usbws-fields label.usbws-check { flex-direction: row; align-items: center; gap: 6px; }
.usbws select, .usbws input[type="text"] {
  min-height: 32px; max-width: 100%; background: var(--bg-secondary); color: var(--text-primary);
  border: 1px solid var(--border); border-radius: var(--radius-sm, 6px); padding: 4px 8px; font-family: inherit; font-size: 13px;
}
.usbws-row { display: flex; align-items: center; justify-content: space-between; gap: 10px; padding: 6px 0; border-bottom: 1px solid var(--border); }
.usbws-row-main { display: flex; flex-direction: column; gap: 2px; min-width: 0; font-size: 13px; overflow-wrap: anywhere; }
.usbws-row-main span { color: var(--text-muted); font-size: 12px; }
.usbws-actions { display: flex; flex-wrap: wrap; gap: 6px; }
.usbws code { font-size: 11px; white-space: pre-wrap; overflow-wrap: anywhere; }
.usbws details { border: 1px solid var(--border); border-radius: var(--radius-md); padding: 8px 12px; margin: 8px 0; }
.usbws summary { cursor: pointer; font-weight: 600; font-size: 14px; }
.usbws progress { width: 100%; }
</style>
